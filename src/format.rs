//! Turning results and errors into what is printed.

use miette::Diagnostic;

pub fn without_final_newline(text: &str) -> &str {
    text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).unwrap_or(text)
}

/// Parts of output, in order, without the empty ones.
pub fn join(parts: &[&str]) -> String {
    parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join("\n")
}

/// What a command wrote to stderr, marked as such and with the command it came from, so that
/// it is not taken for a value or for the output of something else. One line stays one line;
/// more are indented under a heading. (`stderr-block` in prelude/output.nu makes the same,
/// for commands that are not the last of a call.)
pub fn format_stderr(command: Option<&str>, text: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return String::new();
    }
    let heading = match command {
        Some(command) => format!("stderr ({command}):"),
        None => "stderr:".to_string(),
    };
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() == 1 {
        return format!("{heading} {}", lines[0]);
    }
    std::iter::once(heading).chain(lines.iter().map(|line| format!("  {line}"))).collect::<Vec<_>>().join("\n")
}

/// The end of a text, when it is longer than `limit` bytes, with a line saying so.
pub fn last_part(text: &str, limit: usize, what: &str) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let mut cut = text.len() - limit;
    while !text.is_char_boundary(cut) {
        cut += 1;
    }
    format!("... (the first {cut} bytes of {what} are left out)\n{}", &text[cut..])
}

/// How a call that ended in an external command turned out: its exit code, and the text to
/// show for it (output, marked stderr, and a failure line).
pub fn format_completed(
    stdout: &str,
    stderr: &str,
    exit_code: i64,
    command: Option<&str>,
) -> (bool, String) {
    let failure = match (exit_code, command) {
        (0, _) => String::new(),
        (code, Some(command)) => format!("Error: `{command}` exited with code {code}"),
        (code, None) => format!("Error: the command exited with code {code}"),
    };
    let stderr = format_stderr(command, &last_part(stderr.trim(), 4000, "its stderr"));
    (exit_code == 0, join(&[without_final_newline(stdout), &stderr, &failure]))
}

/// Where code came from, to say where in it something went wrong.
pub struct SourceFile<'a> {
    pub name: &'a str,
    pub content: &'a [u8],
    /// Where the file starts in Nushell's global span coordinates.
    pub start: usize,
    pub end: usize,
}

/// The file a span is in, and its contents. Implemented for an engine and a working set.
pub trait Sources {
    fn file_of(&self, offset: usize) -> Option<SourceFile<'_>>;
}

impl Sources for nu_protocol::engine::EngineState {
    fn file_of(&self, offset: usize) -> Option<SourceFile<'_>> {
        self.files().find(|f| f.covered_span.start <= offset && offset < f.covered_span.end.max(f.covered_span.start + 1)).map(|f| SourceFile {
            name: &f.name,
            content: &f.content,
            start: f.covered_span.start,
            end: f.covered_span.end,
        })
    }
}

impl Sources for nu_protocol::engine::StateWorkingSet<'_> {
    fn file_of(&self, offset: usize) -> Option<SourceFile<'_>> {
        self.files().find(|f| f.covered_span.start <= offset && offset < f.covered_span.end.max(f.covered_span.start + 1)).map(|f| SourceFile {
            name: &f.name,
            content: &f.content,
            start: f.covered_span.start,
            end: f.covered_span.end,
        })
    }
}

/// The name nux gives to the code it was given, in Nushell's list of files.
pub const CODE_FILE: &str = "nux-code";

/// An error as nux prints it:
///
/// ```text
/// Error: File not found  (nu::shell::io::file_not_found)
///   '/nope.json' does not exist
///   line 1, column 6   near `/nope.json`
/// ```
pub fn format_error(error: &dyn Diagnostic, sources: &dyn Sources) -> String {
    let mut lines = Vec::new();
    describe(error, sources, "", &mut lines);
    lines.join("\n")
}

/// Nushell's error code, for the usage log.
pub fn error_code(error: &dyn Diagnostic) -> Option<String> {
    error.code().map(|code| code.to_string())
}

fn describe(error: &dyn Diagnostic, sources: &dyn Sources, indent: &str, lines: &mut Vec<String>) {
    let message = error.to_string();
    let code = error.code().map(|code| format!("  ({code})")).unwrap_or_default();
    if indent.is_empty() {
        lines.push(format!("Error: {message}{code}"));
    } else {
        lines.push(format!("{indent}caused by: {message}{code}"));
    }
    let inner = format!("{indent}  ");
    if let Some(help) = error.help() {
        for line in help.to_string().lines() {
            lines.push(format!("{inner}{line}"));
        }
    }
    for label in error.labels().into_iter().flatten() {
        let offset = label.offset();
        let what = label.label().filter(|text| *text != message).map(|text| format!(": {text}")).unwrap_or_default();
        let Some(file) = sources.file_of(offset) else {
            if !what.is_empty() {
                lines.push(format!("{inner}in the code{what}"));
            }
            continue;
        };
        let at = offset - file.start;
        let (line, column) = line_and_column(&file, offset);
        let end = (offset + label.len()).min(file.end).saturating_sub(file.start).min(file.content.len());
        let span_text = String::from_utf8_lossy(&file.content[at.min(end)..end]).to_string();
        let near = if span_text.is_empty() {
            String::new()
        } else if span_text.chars().count() > 80 {
            format!("   near `{}...`", span_text.chars().take(77).collect::<String>())
        } else {
            format!("   near `{span_text}`")
        };
        let place = if file.name == CODE_FILE {
            format!("line {line}, column {column}")
        } else {
            format!("in {}, line {line}", short_name(file.name))
        };
        lines.push(format!("{inner}{place}{what}{near}"));
    }
    for related in error.related().into_iter().flatten() {
        describe(related, sources, &inner, lines);
    }
}

/// Line and column (from 1) of a global offset in a file.
fn line_and_column(file: &SourceFile, offset: usize) -> (usize, usize) {
    let at = offset.saturating_sub(file.start).min(file.content.len());
    let before = &file.content[..at];
    let line = before.iter().filter(|b| **b == b'\n').count() + 1;
    let line_start = before.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
    (line, String::from_utf8_lossy(&before[line_start..]).chars().count() + 1)
}

/// An error as data, for `try-nu`: its message, code, help, and where in the agent's code
/// (the first label that points there).
pub fn error_record(error: &dyn Diagnostic, sources: &dyn Sources, text: &str) -> nu_protocol::Value {
    use nu_protocol::{Span, Value};
    let place = error.labels().into_iter().flatten().find_map(|label| {
        let file = sources.file_of(label.offset())?;
        (file.name == CODE_FILE).then(|| line_and_column(&file, label.offset()))
    });
    let optional = |s: Option<String>| s.map_or(Value::nothing(Span::unknown()), |s| Value::string(s, Span::unknown()));
    let int = |n: Option<usize>| n.map_or(Value::nothing(Span::unknown()), |n| Value::int(n as i64, Span::unknown()));
    Value::record(
        nu_protocol::record! {
            "msg" => Value::string(error.to_string(), Span::unknown()),
            "code" => optional(error_code(error)),
            "help" => optional(error.help().map(|h| h.to_string())),
            "line" => int(place.map(|p| p.0)),
            "column" => int(place.map(|p| p.1)),
            "text" => Value::string(text, Span::unknown()),
        },
        Span::unknown(),
    )
}

/// A file of the prelude by its own name, not its whole path.
fn short_name(name: &str) -> &str {
    name.rsplit('/').next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_shows_output_like_a_shell() {
        assert_eq!(format_completed("out\n", "", 0, None), (true, "out".into()));
        assert_eq!(
            format_completed("out\n", "warn\n", 3, None),
            (false, "out\nstderr: warn\nError: the command exited with code 3".into())
        );
        assert_eq!(format_completed("", "", 1, None), (false, "Error: the command exited with code 1".into()));
    }

    #[test]
    fn completed_marks_stderr_and_names_the_command() {
        assert_eq!(
            format_completed("pushed\n", "To github.com:x/y\n", 0, Some("git push")),
            (true, "pushed\nstderr (git push): To github.com:x/y".into())
        );
        assert_eq!(
            format_completed("", "warn one\nwarn two\n", 2, Some("pnpm build")),
            (false, "stderr (pnpm build):\n  warn one\n  warn two\nError: `pnpm build` exited with code 2".into())
        );
    }

    #[test]
    fn last_part_keeps_the_end() {
        assert_eq!(last_part("abc", 5, "x"), "abc");
        assert_eq!(last_part("abcdef", 2, "it"), "... (the first 4 bytes of it are left out)\nef");
    }
}
