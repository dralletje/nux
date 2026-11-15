//! The command line.

/// How code is given: for the messages that have to say so.
pub const CODE_ON_STDIN: &str = "Code is read from stdin; there is no -c option. With a quoted heredoc nothing needs escaping:
    nux <<'NU'
    git status
    NU
For one line without single quotes of its own, a here-string does it:
    nux <<< 'git status'";

pub const NO_SESSIONS: &str = "nux has no sessions: every call runs in a fresh Nushell, and nothing carries over to \
the next call. Do in one call what belongs together, or save a value to a file \
(`$rows | to nuon | save rows.nuon`, later `open rows.nuon`).";

/// The TypeScript nux had -t; agents gave it in a quarter of their calls.
const NO_TIME_LIMIT: &str = "nux has no time limit of its own (no -t): a call runs until it is done, or until \
whatever runs nux stops it. For a long build or install, give your own tool call more time.";

/// The TypeScript nux had --json; agents never used it.
const NO_JSON: &str = "nux has no --json option: end the code with `| to json` (one line: `| to json --raw`).";

#[derive(Debug, PartialEq)]
pub struct UsageError(pub String);

#[derive(Debug, Default, PartialEq)]
pub struct Arguments {
    /// -r / --reason: why the caller runs this; kept in the usage log
    pub reason: Option<String>,
    pub help: bool,
    /// Subcommand and its words, e.g. ["help", "str", "replace"]. Empty when running code.
    pub words: Vec<String>,
}

/// Options may come before or after the subcommand. Everything after `--` is a plain word.
pub fn parse(argv: &[String]) -> Result<Arguments, UsageError> {
    let mut result = Arguments::default();
    let mut index = 0;
    while index < argv.len() {
        let word = argv[index].as_str();
        match word {
            "--" => {
                result.words.extend(argv[index + 1..].iter().cloned());
                break;
            }
            "-r" | "--reason" => {
                index += 1;
                let reason = argv.get(index).ok_or_else(|| UsageError(format!("{word} needs a value.")))?;
                result.reason = Some(reason.clone());
            }
            // There was such an option until 2026-11-14. Code has one way in, the one where
            // nothing has to be escaped for the shell.
            "-c" | "--code" => return Err(UsageError(CODE_ON_STDIN.into())),
            "-s" | "--session" => return Err(UsageError(NO_SESSIONS.into())),
            "-t" | "--timeout" => return Err(UsageError(NO_TIME_LIMIT.into())),
            "--json" => return Err(UsageError(NO_JSON.into())),
            "-h" | "--help" => result.help = true,
            _ if word.starts_with('-') && word.len() > 1 && result.words.is_empty() => {
                return Err(UsageError(format!("Unknown option {word}.")));
            }
            _ => result.words.push(word.to_string()),
        }
        index += 1;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|w| w.to_string()).collect()
    }

    #[test]
    fn options_around_a_subcommand() {
        assert_eq!(
            parse(&words(&["help", "str", "replace", "-r", "why"])).unwrap(),
            Arguments { reason: Some("why".into()), words: words(&["help", "str", "replace"]), ..Default::default() }
        );
        assert_eq!(parse(&words(&["-h"])).unwrap(), Arguments { help: true, ..Default::default() });
    }

    #[test]
    fn rejects_unknown_options_and_missing_values() {
        assert!(parse(&words(&["--nope"])).is_err());
        assert!(parse(&words(&["-r"])).is_err());
    }

    #[test]
    fn options_of_the_typescript_nux_say_what_to_do_instead() {
        let message = |argv: &[&str]| parse(&words(argv)).unwrap_err().0;
        assert!(message(&["-c", "git status"]).contains("<<'NU'"));
        assert!(message(&["-s", "x1"]).contains("no sessions"));
        assert!(message(&["-t", "300"]).contains("no time limit"));
        assert!(message(&["--json"]).contains("| to json"));
    }
}
