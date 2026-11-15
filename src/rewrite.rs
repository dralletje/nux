//! Seeing the exit code of external commands at the top level of the code.
//!
//! Nushell drops the output of an external command that is not the last of a block, and
//! fails without showing what it printed. `nux` promises more: the last external command of
//! a call shows its output and fails the call on a non-zero exit code, and an earlier one
//! that fails shows its output and ends the call. That is done by adding `| complete` after
//! each such command (and `| check-exit '<command>'`, from the prelude, after earlier ones).
//! The additions go right after each command, so line numbers in errors stay those of the
//! code as written.
//!
//! The TypeScript version asked Nushell's parser for the statements with `ast --json`; here
//! the parser is ours, so the statements come straight from the parsed block.

use nu_protocol::ast::{Block, Expr, PipelineRedirection, RedirectionSource, RedirectionTarget};

/// A top-level statement of the code: where it starts and ends (byte offsets in the code),
/// whether it ends in an external command, and whether that command's output is redirected
/// (`o> file`), in which case there is nothing to complete.
#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub start: usize,
    pub end: usize,
    pub external: bool,
    pub redirected: bool,
}

/// The statements of a parsed block. `base` is where the code starts in Nushell's global
/// span coordinates (`StateWorkingSet::next_span_start` before parsing).
pub fn statements(block: &Block, base: usize) -> Vec<Statement> {
    block
        .pipelines
        .iter()
        .filter_map(|pipeline| {
            let first = pipeline.elements.first()?;
            let last = pipeline.elements.last()?;
            // Only stderr redirected to a file (`e> /dev/null`, to silence a command) does not
            // count as redirected; the statement then ends after the redirection.
            let stderr_file = match &last.redirection {
                Some(PipelineRedirection::Single {
                    source: RedirectionSource::Stderr,
                    target: RedirectionTarget::File { expr, .. },
                }) => Some(expr.span.end),
                _ => None,
            };
            let end = stderr_file.unwrap_or(last.expr.span.end);
            Some(Statement {
                start: first.expr.span.start.checked_sub(base)?,
                end: end.checked_sub(base)?,
                external: matches!(last.expr.expr, Expr::ExternalCall(..)),
                redirected: last.redirection.is_some() && stderr_file.is_none(),
            })
        })
        .collect()
}

/// The name, in the prelude, of the command that prints a failed command's output and stops.
pub const CHECK_EXIT: &str = "check-exit";

#[derive(Debug, PartialEq)]
pub struct Checked {
    pub code: String,
    pub changed: bool,
    pub last_is_external: bool,
    /// The last command as shown in messages, when it is external.
    pub last_command: Option<String>,
    /// What was added: (position in the code as given, length), in order. To show errors at
    /// their place in the code as given (`format::SourceFile::local`).
    pub insertions: Vec<(usize, usize)>,
}

/// Add `| complete` to the code's top-level external commands: the last statement gets just
/// that (its record is the call's value), an earlier one also gets `| check-exit '<command>'`
/// (only when the prelude, which has `check-exit`, is loaded).
pub fn with_exit_checks(code: &str, statements: &[Statement], check_earlier: bool) -> Checked {
    let unchanged =
        || Checked { code: code.to_string(), changed: false, last_is_external: false, last_command: None, insertions: Vec::new() };
    let bytes = code.as_bytes();
    let mut previous_end = 0;
    for statement in statements {
        let in_order = statement.start >= previous_end
            && statement.end >= statement.start
            && statement.end <= bytes.len()
            && code.is_char_boundary(statement.start)
            && code.is_char_boundary(statement.end);
        if !in_order {
            return unchanged();
        }
        previous_end = statement.end;
    }

    let mut out = String::with_capacity(code.len() + 64);
    let mut position = 0;
    let mut changed = false;
    let mut last_is_external = false;
    let mut last_command = None;
    let mut insertions = Vec::new();
    for (index, statement) in statements.iter().enumerate() {
        if !statement.external || statement.redirected {
            continue;
        }
        let is_last = index == statements.len() - 1;
        if !is_last && !check_earlier {
            continue;
        }
        let shown = shorten(&code[statement.start..statement.end]);
        out.push_str(&code[position..statement.end]);
        let added = if is_last {
            last_is_external = true;
            last_command = Some(shown);
            " | complete".to_string()
        } else {
            format!(" | complete | {CHECK_EXIT} {}", nu_raw_string(&shown))
        };
        insertions.push((statement.end, added.len()));
        out.push_str(&added);
        position = statement.end;
        changed = true;
    }
    if !changed {
        return unchanged();
    }
    out.push_str(&code[position..]);
    Checked { code: out, changed, last_is_external, last_command, insertions }
}

/// A command as shown in messages: on one line, at most 60 characters.
fn shorten(text: &str) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 60 {
        one_line.chars().take(57).collect::<String>() + "..."
    } else {
        one_line
    }
}

/// Text as a Nushell raw string, r#'...'#, in which nothing is special. Such a string ends at
/// a quote followed by its number of #, so take more # than the text has after any quote.
/// The opening quote counts as one: Nushell (0.117) reads r#'#abc'# as an empty string
/// followed by loose text, so text that starts with # needs a longer fence too.
pub fn nu_raw_string(text: &str) -> String {
    let quoted = format!("'{text}");
    let mut hashes = 1;
    let mut run: Option<usize> = None;
    for c in quoted.chars() {
        match (c, run) {
            ('\'', _) => run = Some(0),
            ('#', Some(n)) => {
                run = Some(n + 1);
                hashes = hashes.max(n + 2);
            }
            _ => run = None,
        }
    }
    let fence = "#".repeat(hashes);
    format!("r{fence}'{text}'{fence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(start: usize, end: usize, external: bool, redirected: bool) -> Statement {
        Statement { start, end, external, redirected }
    }

    #[test]
    fn completes_the_last_external_command_and_checks_earlier_ones() {
        let code = "let a = 1\ngit add -A\ngit push # now\n";
        let statements = [st(0, 9, false, false), st(10, 20, true, false), st(21, 29, true, false)];
        assert_eq!(
            with_exit_checks(code, &statements, true),
            Checked {
                code: "let a = 1\ngit add -A | complete | check-exit r#'git add -A'#\ngit push | complete # now\n".into(),
                changed: true,
                last_is_external: true,
                last_command: Some("git push".into()),
                // " | complete | check-exit r#'git add -A'#" and " | complete"
                insertions: vec![(20, 40), (29, 11)],
            }
        );
        // Without the prelude there is no check-exit: only the last one is completed.
        assert_eq!(
            with_exit_checks(code, &statements, false).code,
            "let a = 1\ngit add -A\ngit push | complete # now\n"
        );
    }

    #[test]
    fn leaves_other_code_alone() {
        assert!(!with_exit_checks("ls | length", &[st(0, 11, false, false)], true).changed);
        assert!(!with_exit_checks("^echo x o> f", &[st(0, 7, true, true)], true).changed);
        assert!(!with_exit_checks("let x = ", &[], true).changed);
        assert!(!with_exit_checks("git push", &[st(0, 99, true, false)], true).changed);
    }

    #[test]
    fn counts_in_bytes() {
        // "é" is two bytes: the statement ends at byte 9, not character 8.
        let result = with_exit_checks("^echo é\n1", &[st(0, 8, true, false), st(9, 10, false, false)], true);
        assert_eq!(result.code, "^echo é | complete | check-exit r#'^echo é'#\n1");
        assert!(!result.last_is_external);
    }

    #[test]
    fn raw_strings_get_a_long_enough_fence() {
        assert_eq!(nu_raw_string(r#"plain "text" $x"#), r#"r#'plain "text" $x'#"#);
        assert_eq!(nu_raw_string("it's"), "r#'it's'#");
        assert_eq!(nu_raw_string("py r#'code'#"), "r##'py r#'code'#'##");
        assert_eq!(nu_raw_string("# a comment\nls"), "r##'# a comment\nls'##");
        assert_eq!(nu_raw_string("## two"), "r###'## two'###");
        assert_eq!(nu_raw_string("a '## b '# c"), "r###'a '## b '# c'###");
    }

    #[test]
    fn shortens_long_commands() {
        assert_eq!(shorten("git   push\n  origin"), "git push origin");
        assert_eq!(shorten(&"x".repeat(70)).chars().count(), 60);
    }
}
