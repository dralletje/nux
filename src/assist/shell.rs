//! Habits from bash and other shells, where Nushell's own message does not say what to write
//! instead. (`&&`, `||`, `2>&1` and `$HOME` are left alone: Nushell explains those itself.)

use super::ErrorContext;

pub fn hint(context: &ErrorContext) -> Option<String> {
    let code = context.code.as_deref().unwrap_or_default();
    let near = context.near.as_deref().unwrap_or_default().trim();
    let line = context.line.as_deref().unwrap_or_default().trim();

    if near.starts_with("$(") {
        return Some(
            "`$(...)` is bash. In Nushell a command's output is a subexpression: `(pwd)`, \
             or in a string `$\"in (pwd)\"`."
                .into(),
        );
    }
    if code == "nu::parser::unexpected_keyword" && near == "export" && starts_with_assignment(line, "export ") {
        return Some("To set an environment variable: `$env.NAME = \"value\"` (for one command: `with-env {NAME: value} { ... }`).".into());
    }
    if (near == ">" || near == ">>") && code.starts_with("nu::shell::io") {
        return Some(
            "`>` is not a redirection in Nushell. To write a value to a file: `... | save out.txt` \
             (`save --append`, `save --force`); for an external command's output: `^cmd o> out.txt`."
                .into(),
        );
    }
    if line.starts_with("if [") || line.starts_with("if test ") || (line.starts_with("if ") && line.contains("; then")) {
        return Some(
            "Nushell's if: `if <condition> { ... } else { ... }`. For files: `if ('x' | path exists) { ... }`, \
             `('x' | path type) == dir`."
                .into(),
        );
    }
    if line.starts_with("for ") && (line.contains("; do") || line.ends_with(" do")) {
        return Some(
            "Nushell's for: `for f in (glob *.txt) { ... }`; or, for a value: `glob *.txt | each {|f| ... }`.".into(),
        );
    }
    None
}

/// `export NAME=value`
fn starts_with_assignment(line: &str, prefix: &str) -> bool {
    line.strip_prefix(prefix).and_then(|rest| rest.split_once('=')).is_some_and(|(name, _)| {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}
