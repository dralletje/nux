//! How nux prints a value for the agent reading it ("v0", docs/output-format-eval.md).
//!
//! NUON is made for Nushell to read back; what nux prints is read by a model, and the exact
//! value stays one `kept` away (kept values are NUON). So this form is made for reading, with
//! its own forms only where agents were seen fighting the output, and NUON for the rest:
//!
//! - a list of texts: `N items`, then one item per line;
//! - a record: `key: value` per line;
//! - a table: a Markdown table when it is simple (header repeated every 20 rows), otherwise
//!   `## 0`, `## 1`, ... per row, each row as a record;
//! - text inside those: as it is when that cannot be misread, otherwise quoted as JSON (so
//!   an unquoted text is always literal, a `\n` in it is a backslash and an n);
//! - durations and file sizes as people write them, `~` in front when rounded;
//! - a nested value: NUON on the same line when short, else indented under its key;
//! - anything else: NUON.
//!
//! `NUX_FORMAT=nuon` prints NUON for everything, as before.

use nu_protocol::{Value, engine::EngineState};

/// A nested value up to this many characters (as NUON) stays on its key's line.
const INLINE: usize = 80;
/// A table is simple with at most this many columns ...
const SIMPLE_COLUMNS: usize = 8;
/// ... and cells of at most this many characters.
const SIMPLE_CELL: usize = 60;
/// A Markdown table repeats its header after this many rows.
const HEADER_EVERY: usize = 20;

/// Whether values are printed as NUON (`NUX_FORMAT=nuon`) rather than in the readable form.
pub fn nuon_wanted() -> bool {
    std::env::var("NUX_FORMAT").is_ok_and(|f| f.eq_ignore_ascii_case("nuon"))
}

/// A value as NUON on one line, the way nux printed everything before.
pub fn nuon(engine: &EngineState, value: &Value) -> String {
    let config = nuon::ToNuonConfig::default().style(nuon::ToStyle::Raw).raw_strings(true);
    nuon::to_nuon(engine, value, config).unwrap_or_else(|_| value.to_expanded_string(", ", &engine.get_config()))
}

/// A value as nux shows it: text as it is; anything else readable, or NUON with
/// NUX_FORMAT=nuon. (A call's value, `show`, and the sample of a large result.)
pub fn shown(engine: &EngineState, value: &Value) -> String {
    match value {
        Value::String { val, .. } => val.clone(),
        _ if nuon_wanted() => nuon(engine, value),
        _ => readable(engine, value),
    }
}

/// A value on one line, after a key: a scalar readable (text by the rules for a record's
/// values, durations and sizes for reading), a list or record as NUON.
pub fn inline(engine: &EngineState, value: &Value) -> String {
    match value {
        Value::List { .. } | Value::Record { .. } => nuon(engine, value),
        _ if nuon_wanted() => nuon(engine, value),
        _ => Render { engine }.scalar_or_nuon(value),
    }
}

/// The readable form of a value that is not text (text is printed as it is, by the caller).
pub fn readable(engine: &EngineState, value: &Value) -> String {
    let r = Render { engine };
    match value {
        // A list of texts: how many, then one per line, as it is wherever that is unambiguous.
        // (The count says it is a list, and how long: counting lines was the most common miss
        // in the format runs. Lines as they are is what agents made with `str join "\n"`.)
        Value::List { vals, .. } if !vals.is_empty() && vals.iter().all(|v| matches!(v, Value::String { .. })) => {
            let count = if vals.len() == 1 { "1 item".to_string() } else { format!("{} items", vals.len()) };
            std::iter::once(count)
                .chain(vals.iter().map(|v| r.text(v.as_str().unwrap_or_default(), Place::Line)))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Value::List { vals, .. } if !vals.is_empty() && vals.iter().all(|v| matches!(v, Value::Record { .. })) => r.table(vals),
        Value::Record { val, .. } if !val.is_empty() => r.record(val.iter(), 0).join("\n"),
        _ => r.scalar_or_nuon(value),
    }
}

/// Where a text stands: what it must not be mistaken for there.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    /// A line of its own, in a list of texts: spaces at its start are plain to see there
    /// (indented code stays as it is); only spaces at its end are not.
    Line,
    /// A value in a record (after `key: `) or an item of a nested list.
    Item,
    /// A cell of a Markdown table: a `|` would split it.
    Cell,
}

struct Render<'a> {
    engine: &'a EngineState,
}

impl Render<'_> {
    /// A text as it is when that cannot be misread, else quoted as JSON.
    fn text(&self, text: &str, place: Place) -> String {
        if needs_quotes(text, place) { serde_json::to_string(text).unwrap_or_default() } else { text.to_string() }
    }

    /// A value that is not a list or record: durations and sizes for reading, text by
    /// [`Self::text`], the rest as NUON.
    fn scalar_or_nuon(&self, value: &Value) -> String {
        match value {
            Value::Duration { val, .. } => duration(*val),
            Value::Filesize { val, .. } => filesize(val.get()),
            Value::String { val, .. } => self.text(val, Place::Item),
            _ => nuon(self.engine, value),
        }
    }

    /// A record's lines, `key: value`, indented by `indent` spaces.
    fn record<'v>(&self, fields: impl Iterator<Item = (&'v String, &'v Value)>, indent: usize) -> Vec<String> {
        let pad = " ".repeat(indent);
        let mut lines = Vec::new();
        for (key, value) in fields {
            let key = if needs_quotes(key, Place::Item) || key.contains(':') { serde_json::to_string(key).unwrap_or_default() } else { key.clone() };
            match value {
                // At the top, a text over several lines goes in a fenced block under its key;
                // deeper down, quoted (an indented block would change what it says).
                Value::String { val, .. } if val.contains('\n') && indent == 0 => {
                    let fence = "`".repeat(longest_backtick_run(val).max(2) + 1);
                    lines.push(format!("{key}:"));
                    lines.push(fence.clone());
                    lines.push(val.strip_suffix('\n').unwrap_or(val).to_string());
                    lines.push(fence);
                }
                Value::List { .. } | Value::Record { .. } => {
                    let inline = nuon(self.engine, value);
                    if inline.chars().count() <= INLINE && !inline.contains('\n') {
                        lines.push(format!("{pad}{key}: {inline}"));
                    } else {
                        lines.push(format!("{pad}{key}:"));
                        lines.extend(self.nested(value, indent + 2));
                    }
                }
                _ => lines.push(format!("{pad}{key}: {}", self.scalar_or_nuon(value))),
            }
        }
        lines
    }

    /// A list or record under a key: records as `key: value`, lists as `- item`, indented.
    fn nested(&self, value: &Value, indent: usize) -> Vec<String> {
        let pad = " ".repeat(indent);
        match value {
            Value::Record { val, .. } => self.record(val.iter(), indent),
            Value::List { vals, .. } => {
                let mut lines = Vec::new();
                for item in vals {
                    match item {
                        Value::List { .. } | Value::Record { .. } => {
                            let inline = nuon(self.engine, item);
                            if inline.chars().count() <= INLINE && !inline.contains('\n') {
                                lines.push(format!("{pad}- {inline}"));
                            } else {
                                lines.push(format!("{pad}-"));
                                lines.extend(self.nested(item, indent + 2));
                            }
                        }
                        _ => lines.push(format!("{pad}- {}", self.scalar_or_nuon(item))),
                    }
                }
                lines
            }
            _ => vec![format!("{pad}{}", self.scalar_or_nuon(value))],
        }
    }

    /// A list of records: a Markdown table when simple, else a record per row under `## n`.
    fn table(&self, rows: &[Value]) -> String {
        let mut columns: Vec<String> = Vec::new();
        for row in rows {
            if let Value::Record { val, .. } = row {
                for key in val.columns() {
                    if !columns.contains(key) {
                        columns.push(key.clone());
                    }
                }
            }
        }
        let cell = |row: &Value, column: &str| -> Option<String> {
            let Value::Record { val, .. } = row else { return None };
            match val.get(column) {
                None => Some(String::new()),
                Some(Value::List { .. } | Value::Record { .. }) => None,
                Some(Value::String { val, .. }) => Some(self.text(val, Place::Cell)),
                Some(other) => Some(self.scalar_or_nuon(other)),
            }
        };
        let cells: Option<Vec<Vec<String>>> =
            rows.iter().map(|row| columns.iter().map(|c| cell(row, c)).collect::<Option<Vec<_>>>()).collect();
        let simple = columns.len() <= SIMPLE_COLUMNS
            && cells.as_ref().is_some_and(|rows| rows.iter().flatten().all(|c| c.chars().count() <= SIMPLE_CELL && !c.contains('\n')));
        match cells {
            Some(cells) if simple && !columns.is_empty() => {
                let header = format!("| {} |\n|{}|", columns.iter().map(|c| self.text(c, Place::Cell)).collect::<Vec<_>>().join(" | "), vec!["---"; columns.len()].join("|"));
                let mut lines = Vec::new();
                for (index, row) in cells.iter().enumerate() {
                    if index % HEADER_EVERY == 0 {
                        lines.push(header.clone());
                    }
                    lines.push(format!("| {} |", row.join(" | ")));
                }
                lines.join("\n")
            }
            // The number of rows goes above `## 0`: without it, readers counted the rows by the
            // last heading, one short (docs/output-format-eval.md, both runs).
            _ => {
                let count = if rows.len() == 1 { "1 row".to_string() } else { format!("{} rows", rows.len()) };
                let blocks = rows
                    .iter()
                    .enumerate()
                    .map(|(index, row)| {
                        let fields = match row {
                            Value::Record { val, .. } => self.record(val.iter(), 0),
                            other => vec![nuon(self.engine, other)],
                        };
                        format!("## {index}\n{}", fields.join("\n"))
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                format!("{count}\n{blocks}")
            }
        }
    }
}

/// Whether a text has to be quoted to be read for what it is.
fn needs_quotes(text: &str, place: Place) -> bool {
    let ends = if place == Place::Line { text.trim_end() } else { text.trim() };
    text.is_empty()
        || text.trim().is_empty()
        || ends != text
        || text.contains('\n')
        || text.contains('\r')
        || text.starts_with('"')
        || text.starts_with('[')
        || text.starts_with('{')
        || matches!(text, "true" | "false" | "null")
        || text.parse::<f64>().is_ok()
        || (place == Place::Cell && text.contains('|'))
}

fn longest_backtick_run(text: &str) -> usize {
    text.split(|c| c != '`').map(str::len).max().unwrap_or(0)
}

/// A duration in its two largest units, `~` in front when smaller ones were left out:
/// `~1min 28sec`, `1sec 500ms`, `3day`.
pub fn duration(nanoseconds: i64) -> String {
    const UNITS: [(&str, i128); 7] = [
        ("day", 86_400_000_000_000),
        ("hr", 3_600_000_000_000),
        ("min", 60_000_000_000),
        ("sec", 1_000_000_000),
        ("ms", 1_000_000),
        ("µs", 1_000),
        ("ns", 1),
    ];
    let sign = if nanoseconds < 0 { "-" } else { "" };
    let rest = (nanoseconds as i128).abs();
    if rest == 0 {
        return "0sec".into();
    }
    let first = UNITS.iter().position(|(_, size)| rest >= *size).unwrap_or(UNITS.len() - 1);
    let (name, size) = UNITS[first];
    let mut text = format!("{}{name}", rest / size);
    let mut rest = rest % size;
    if let Some((name, size)) = UNITS.get(first + 1) {
        if rest / size > 0 {
            text.push_str(&format!(" {}{name}", rest / size));
        }
        rest %= size;
    }
    format!("{}{sign}{text}", if rest > 0 { "~" } else { "" })
}

/// A file size in the largest decimal unit it is at least 1 of, at most three decimals, `~` in
/// front when rounded: `2.311kB`, `~4.512MB`, `500b`.
pub fn filesize(bytes: i64) -> String {
    const UNITS: [(&str, i128); 6] = [
        ("EB", 1_000_000_000_000_000_000),
        ("PB", 1_000_000_000_000_000),
        ("TB", 1_000_000_000_000),
        ("GB", 1_000_000_000),
        ("MB", 1_000_000),
        ("kB", 1_000),
    ];
    let sign = if bytes < 0 { "-" } else { "" };
    let size = (bytes as i128).abs();
    let Some((name, unit)) = UNITS.iter().find(|(_, unit)| size >= *unit) else { return format!("{bytes}b") };
    let exact = (size * 1000) % unit == 0;
    let thousandths = (size * 1000 + unit / 2) / unit;
    let (whole, part) = (thousandths / 1000, thousandths % 1000);
    let decimals = format!("{part:03}");
    let decimals = decimals.trim_end_matches('0');
    let number = if decimals.is_empty() { whole.to_string() } else { format!("{whole}.{decimals}") };
    format!("{}{sign}{number}{name}", if exact { "" } else { "~" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_for_reading() {
        assert_eq!(duration(88_471_299_830), "~1min 28sec");
        assert_eq!(duration(1_500_000_000), "1sec 500ms");
        assert_eq!(duration(259_200_000_000_000), "3day");
        assert_eq!(duration(0), "0sec");
        assert_eq!(duration(-2_000_000), "-2ms");
        assert_eq!(duration(61_000_000_001), "~1min 1sec");
    }

    #[test]
    fn filesizes_for_reading() {
        assert_eq!(filesize(500), "500b");
        assert_eq!(filesize(2311), "2.311kB");
        assert_eq!(filesize(1500), "1.5kB");
        assert_eq!(filesize(4_500_000), "4.5MB");
        assert_eq!(filesize(4_512_345), "~4.512MB");
        assert_eq!(filesize(1_000_000_000), "1GB");
    }

    #[test]
    fn texts_are_quoted_only_when_they_could_be_misread() {
        assert!(!needs_quotes("src/a.ts", Place::Item));
        assert!(!needs_quotes(r#"let s = "a\nb";"#, Place::Item));
        assert!(needs_quotes("", Place::Item));
        assert!(needs_quotes("12", Place::Item));
        assert!(needs_quotes("true", Place::Item));
        assert!(needs_quotes(" padded", Place::Item));
        assert!(needs_quotes("two\nlines", Place::Item));
        assert!(needs_quotes("a | b", Place::Cell));
        assert!(!needs_quotes("a | b", Place::Item));
        // A line of its own: indentation is plain to see, spaces at the end are not.
        assert!(!needs_quotes("    return x;", Place::Line));
        assert!(needs_quotes("trailing ", Place::Line));
        assert!(needs_quotes("   ", Place::Line));
        assert!(needs_quotes("", Place::Line));
        assert!(needs_quotes("    return x;", Place::Item));
    }
}
