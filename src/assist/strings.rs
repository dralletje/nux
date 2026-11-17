//! Backslashes in strings. Agents write regular expressions in "..." (where `\.` is an
//! escape Nushell does not know) and `\'` in '...' (where nothing is an escape, so the `'`
//! ends the string). Nushell's messages say what is wrong, not what to write.

use super::ErrorContext;

pub fn hint(context: &ErrorContext) -> Option<String> {
    let near = context.near.as_deref().unwrap_or_default();
    let line = context.line.as_deref().unwrap_or_default();

    // "Invalid literal: unrecognized escape sequence '\.' in string", near `\.b"`.
    if context.message == "Invalid literal" && near.starts_with('\\') {
        let escape: String = near.chars().take(2).collect();
        return Some(format!(
            "In \"...\" a backslash starts an escape (\\n, \\t, \\\", \\\\), and `{escape}` is none. For a regular \
             expression (or a Windows path) write '...' or r#'...'#, in which a backslash is just a \
             backslash: search 'a\\.b'."
        ));
    }
    if context.code.as_deref() == Some("nu::parser::unclosed_delimiter") && line.contains("\\'") {
        return Some(
            "'...' has no escapes: in it, `\\'` is a backslash, and the ' ends the string. For text \
             with a ' in it: \"it's\", or r#'it's'#."
                .into(),
        );
    }
    None
}
