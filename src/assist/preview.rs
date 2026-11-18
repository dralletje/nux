//! A result too large to print: its shape and a sample; all of it is kept under a name for
//! the next call (`kept <name>`, see `commands/kept.rs`). Often the shape and the first rows
//! are all the agent needs.

use nu_protocol::{Record, Span, Value, engine::EngineState};

/// Rows (or items) shown of a list.
const ROWS: usize = 10;
/// Fields shown of a record.
const FIELDS: usize = 30;
/// Characters kept of a string inside a sample.
const CELL: usize = 80;
/// Items kept of a list inside a sample row.
const INNER_ITEMS: usize = 3;
/// Characters a sample may take; fewer rows are shown to stay under it.
const BUDGET: usize = 3000;
/// Lines shown from the start and from the end of text.
const TEXT_LINES: usize = 10;
/// Characters kept of a line of text.
const LINE: usize = 200;

pub fn size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{} KiB", bytes.div_ceil(1024))
    }
}

/// For data. `name` is what the whole value is kept as, `bytes` its size as NUON.
pub fn large_value(engine: &EngineState, value: &Value, name: &str, bytes: usize) -> String {
    let heading = format!("The result is too large to print ({}); all of it is kept as `{name}`.", size(bytes));
    let body = match value {
        Value::List { vals, .. } => {
            let what = if vals.iter().all(|v| matches!(v, Value::Record { .. })) { "rows" } else { "items" };
            let mut shown = ROWS.min(vals.len());
            let mut sample = shown_as(engine, &Value::list(vals[..shown].iter().map(shorten).collect(), Span::unknown()));
            while sample.len() > BUDGET && shown > 1 {
                shown /= 2;
                sample = shown_as(engine, &Value::list(vals[..shown].iter().map(shorten).collect(), Span::unknown()));
            }
            format!(
                "{}, {} {what}. The first {shown} (long text cut short):\n{sample}\nMore, in a next call: kept {name} | where ... | first 20",
                type_of(value),
                vals.len()
            )
        }
        Value::Record { val, .. } => {
            let fields = val
                .iter()
                .take(FIELDS)
                .map(|(key, field)| {
                    let shown = crate::readable::inline(engine, &shorten(field));
                    if shown.len() <= CELL { format!("  {key}: {shown}") } else { format!("  {key}: {} ({})", type_of(field), size_of(field)) }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let more = if val.len() > FIELDS { format!("\n  ... and {} more fields", val.len() - FIELDS) } else { String::new() };
            format!("A record with {} fields:\n{fields}{more}\nMore, in a next call: kept {name} | get <field>", val.len())
        }
        other => format!("{}. Look at it in a next call: kept {name}", type_of(other)),
    };
    format!("{heading}\n{body}")
}

/// For text (also an external command's output). `name` is what all of it is kept as.
pub fn large_text(text: &str, name: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let cut = |line: &&str| {
        if line.chars().count() > LINE { format!("{}…", line.chars().take(LINE).collect::<String>()) } else { line.to_string() }
    };
    let shown = if lines.len() <= 2 * TEXT_LINES {
        lines.iter().map(cut).collect::<Vec<_>>().join("\n")
    } else {
        let head = lines[..TEXT_LINES].iter().map(cut).collect::<Vec<_>>().join("\n");
        let tail = lines[lines.len() - TEXT_LINES..].iter().map(cut).collect::<Vec<_>>().join("\n");
        format!("{head}\n... ({} lines left out) ...\n{tail}", lines.len() - 2 * TEXT_LINES)
    };
    format!(
        "Too large to print ({}, {} lines); all of it is kept as `{name}`. The first and last lines:\n{shown}\nMore, in a next call: kept {name} | lines | find <word>",
        size(text.len()),
        lines.len()
    )
}

/// A sample as nux shows values (readable, or NUON with NUX_FORMAT=nuon).
fn shown_as(engine: &EngineState, value: &Value) -> String {
    crate::readable::shown(engine, value)
}

/// Nushell's name for the type, e.g. `table<name: string, size: filesize>`, kept short.
fn type_of(value: &Value) -> String {
    let name = value.get_type().to_string();
    if name.chars().count() > 300 { format!("{}…", name.chars().take(300).collect::<String>()) } else { name }
}

fn size_of(value: &Value) -> String {
    match value {
        Value::List { vals, .. } => format!("{} items", vals.len()),
        Value::Record { val, .. } => format!("{} fields", val.len()),
        Value::String { val, .. } => format!("{} characters", val.chars().count()),
        Value::Binary { val, .. } => format!("{} bytes", val.len()),
        _ => "large".into(),
    }
}

/// A value for a sample: long strings cut, long inner lists cut (with a note of how many more).
fn shorten(value: &Value) -> Value {
    match value {
        Value::String { val, .. } if val.chars().count() > CELL => {
            Value::string(format!("{}…", val.chars().take(CELL).collect::<String>()), Span::unknown())
        }
        Value::List { vals, .. } if vals.len() > INNER_ITEMS => {
            let mut kept: Vec<Value> = vals[..INNER_ITEMS].iter().map(shorten).collect();
            kept.push(Value::string(format!("… {} more", vals.len() - INNER_ITEMS), Span::unknown()));
            Value::list(kept, Span::unknown())
        }
        Value::List { vals, .. } => Value::list(vals.iter().map(shorten).collect(), Span::unknown()),
        Value::Record { val, .. } => {
            let record: Record = val.iter().map(|(k, v)| (k.clone(), shorten(v))).collect();
            Value::record(record, Span::unknown())
        }
        Value::Binary { val, .. } if val.len() > CELL => Value::string(format!("<{} bytes>", val.len()), Span::unknown()),
        other => other.clone(),
    }
}
