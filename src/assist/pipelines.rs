//! Mistakes with Nushell's own pipelines, seen in the review of 2026-11-17 (item 2.10), where
//! Nushell's message says what failed but not what to write:
//!
//! - `[1 2] | do {|r| ... }`: `do` does not hand its input to a parameter;
//! - `where index < 12 or (index > 15)`: a bare column only works at the start of `where`;
//! - `rm -r $dir` from inside `$dir`;
//! - `<text> | where ...`: text is not a list (a kept result that was text, `to md` output).

use super::ErrorContext;

pub fn hint(context: &ErrorContext) -> Option<String> {
    let code = context.code.as_deref().unwrap_or_default();
    let near = context.near.as_deref().unwrap_or_default().trim();
    let line = context.line.as_deref().unwrap_or_default();

    if code == "nu::shell::missing_parameter" && near.starts_with("{|") && line.contains("do {|") {
        return Some(
            "`do` does not give its input to the closure's parameter. Use `$in` in the closure \
             (`| do { $in + 1 }`), `each {|r| ... }` for each item, or `let r = ...` first."
                .into(),
        );
    }
    if context.message.starts_with("External command failed") && is_column_in_where(line, near) {
        return Some(format!(
            "In `where`, a bare column only works at the start: `where {near} < 12`. After `and` or `or`, \
             in parentheses, write `$it.{near}`, or use a closure: `where {{|row| $row.{near} < 12 or $row.{near} > 15 }}`."
        ));
    }
    if context.message.starts_with("Cannot remove any parent directory") {
        return Some("The call is in that folder (or below it): `cd` somewhere else first, then remove it.".into());
    }
    if code == "nu::parser::input_type_mismatch"
        && context.message.contains("string input")
        && ["where", "each", "get", "select", "sort-by", "uniq", "length", "group-by"].contains(&near)
    {
        return Some(
            "That is text, not a list or a table. Its lines: `| lines | where ...`; JSON text: `| from json`. \
             (Text from `to md` or `to json` is no table any more: keep the table, and print it at the end.)"
                .into(),
        );
    }
    None
}

/// `near` is a word used like a column inside `where ... (word ...)`.
fn is_column_in_where(line: &str, near: &str) -> bool {
    if near.is_empty() || !near.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return false;
    }
    let Some(at) = line.find(&format!("({near}")) else { return false };
    line[..at].contains("where ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_column_after_or_in_where() {
        assert!(is_column_in_where("[[index]; [1]] | where index < 12 or (index > 15)", "index"));
        assert!(!is_column_in_where("ls | each { (index) }", "index"));
        assert!(!is_column_in_where("where x < 1 or (foo bar)", "$x"));
    }
}
