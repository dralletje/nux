//! Text in parentheses inside `$"..."`. In Nushell everything in `( )` in such a string is
//! code that runs, also when it was meant as text: `$"Fixed the footer (see root.tsx)."`
//! runs a command `see`. What the agent gets is far from that cause:
//!
//! - "External command failed ... Command `see` not found" (Nushell may even suggest some
//!   unrelated command), when the parentheses are balanced;
//! - "Unclosed delimiter", when the parser cannot find where the code ends, typically an
//!   apostrophe: in `(it's)` the `'` starts a string. It points somewhere else entirely.
//!
//! Both come up most with longer text: commit messages, PR bodies, reports.

use super::ErrorContext;

pub fn hint(context: &ErrorContext) -> Option<String> {
    let code = context.code.as_deref()?;
    if code != "nu::shell::external_command" && code != "nu::parser::unclosed_delimiter" {
        return None;
    }
    let source = context.source.as_deref()?;
    // The first label: where the command is, or where the unclosed `(` opens.
    let place = *context.places.first()?;
    let inside = interpolated_strings(source).into_iter().any(|(start, end)| start < place && place < end);
    if !inside {
        return None;
    }
    let cause = if code == "nu::parser::unclosed_delimiter" {
        "Inside $\"...\", text in ( ) is code that runs, and here the parser lost track of where \
         that code ends (an apostrophe in it, like `it's`, starts a string)."
    } else {
        "Inside $\"...\", text in ( ) is code that runs: this ran the words in the parentheses as a \
         command. Only ( ) around a value is meant to run: $\"Hello ($name)\"."
    };
    Some(format!(
        "{cause}\n\
         For ( ) as text write \\( and \\), or put the text in a raw string, where nothing runs:\n\
         \x20   r#'Fixed the footer (see root.tsx). It's done.'#\n\
         With values in it: [r#'Thanks to '# $name] | str join\n\
         A commit message: r#'...'# | ^git commit -F -   (git reads the message from stdin)"
    ))
}

/// Where the `$"..."` and `$'...'` strings are in the code: (start, end) byte positions, end
/// being the code's end for one that is not closed. Other strings, raw strings and comments
/// are skipped, so that a `$"` inside them does not count.
fn interpolated_strings(code: &str) -> Vec<(usize, usize)> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    // The end of a string that starts at `from` with quote `quote` (after its closing quote).
    let close = |from: usize, quote: u8, escapes: bool| -> usize {
        let mut j = from;
        while j < bytes.len() {
            if escapes && bytes[j] == b'\\' {
                j += 2;
                continue;
            }
            if bytes[j] == quote {
                return j + 1;
            }
            j += 1;
        }
        bytes.len()
    };
    while i < bytes.len() {
        let at_word_start = i == 0 || bytes[i - 1].is_ascii_whitespace() || b"([{|;=:,".contains(&bytes[i - 1]);
        match bytes[i] {
            b'$' if matches!(bytes.get(i + 1), Some(b'"')) => {
                let end = close(i + 2, b'"', true);
                found.push((i, end));
                i = end;
            }
            b'$' if matches!(bytes.get(i + 1), Some(b'\'')) => {
                let end = close(i + 2, b'\'', false);
                found.push((i, end));
                i = end;
            }
            b'r' if at_word_start && matches!(bytes.get(i + 1), Some(b'#')) => {
                // r#'...'#, with as many # as it opened with.
                let hashes = bytes[i + 1..].iter().take_while(|b| **b == b'#').count();
                if bytes.get(i + 1 + hashes) != Some(&b'\'') {
                    i += 1;
                    continue;
                }
                let fence: Vec<u8> = std::iter::once(b'\'').chain(std::iter::repeat_n(b'#', hashes)).collect();
                let start = i + 2 + hashes;
                i = bytes[start..].windows(fence.len()).position(|w| w == fence.as_slice()).map_or(bytes.len(), |p| start + p + fence.len());
            }
            b'"' => i = close(i + 1, b'"', true),
            b'\'' if at_word_start => i = close(i + 1, b'\'', false),
            b'`' => i = close(i + 1, b'`', false),
            b'#' if at_word_start => {
                i = bytes[i..].iter().position(|b| *b == b'\n').map_or(bytes.len(), |p| i + p);
            }
            _ => i += 1,
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inside(code: &str, needle: &str) -> bool {
        let place = code.find(needle).unwrap();
        interpolated_strings(code).iter().any(|(start, end)| *start < place && place < *end)
    }

    #[test]
    fn finds_interpolated_strings() {
        assert!(inside(r#"$"Logout (a root loader).($t)""#, "a root"));
        assert!(inside("let m = $\"line one\n(it's broken) on two\"", "it's"));
        assert!(inside(r#"$'single (quoted)'"#, "quoted"));
        // Not closed: up to the end.
        assert!(inside(r#"$"see (the notes"#, "the notes"));
    }

    #[test]
    fn skips_other_strings_and_comments() {
        assert!(!inside(r#""plain (text)" | str length"#, "text"));
        assert!(!inside(r#"r#'raw $"not (this)" either'#"#, "this"));
        assert!(!inside("# a comment $\"(not this)\"\nls", "not this"));
        assert!(!inside(r#"'single $"(nor this)"'"#, "nor this"));
        // After a closed one.
        assert!(!inside(r#"$"a ($x)" | (b c)"#, "b c"));
    }
}
