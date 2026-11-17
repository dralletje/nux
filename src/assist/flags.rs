//! An unknown flag: list the flags the command does have, so that the agent does not need
//! another call for `nux help <command>`. (15 of 93 errors in the TypeScript nux's log were
//! unknown flags, mostly flags of other tools: `search -A`, `git status --short`.)

use nu_protocol::{ParseError, did_you_mean};

use super::{ErrorContext, Raw};

pub fn hint(context: &ErrorContext) -> Option<String> {
    let Raw::Parse(ParseError::UnknownFlag(command, flag, _, _)) = context.raw else { return None };
    let (command, signature) = full_name(context, command)
        .into_iter()
        .chain(std::iter::once(command.clone()))
        .find_map(|name| context.sources.signature_of(&name).map(|signature| (name, signature)))?;
    let command = &command;
    let flags: Vec<_> = signature.named.iter().filter(|f| f.long != "help").collect();
    // The command found has the flag that was refused, so it is not the command that refused
    // it (`cookie-jar http --full` once found Nushell's `http`, and said "Did you mean
    // --full?"). No hint is better than a wrong one.
    let given = flag.trim_start_matches('-');
    if flags.iter().any(|f| f.long == given || (given.chars().count() == 1 && f.short.is_some_and(|s| given.starts_with(s)))) {
        return None;
    }
    // nux's `git status` is not git's: the flag may be meant for the program itself.
    let program = command.split(' ').next().filter(|first| *first != command && on_path(first));
    let instead = program
        .map(|p| format!("\nFor the program `{p}` itself: `^{} {}`.", command, flag_as_given(context, flag)))
        .unwrap_or_default();
    if flags.is_empty() {
        return Some(format!("`{command}` has no flags.{instead}"));
    }

    // Only for a long flag: a single letter is usually another tool's (`-A` of grep is not
    // `-a` here), and a guess from it would mislead.
    let names: Vec<String> = flags.iter().map(|f| f.long.clone()).collect();
    let suggestion = if given.chars().count() > 1 { did_you_mean(&names, given) } else { None };

    let list = flags
        .iter()
        .map(|f| {
            let short = f.short.map(|s| format!(" (-{s})")).unwrap_or_default();
            let arg = f.arg.as_ref().map(|shape| format!(" <{shape}>")).unwrap_or_default();
            let desc = f.desc.lines().next().unwrap_or_default();
            format!("--{}{short}{arg}   {desc}", f.long)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mean = suggestion.map(|s| format!("Did you mean --{s}? ")).unwrap_or_default();
    Some(format!("{mean}The flags of `{command}`:\n{list}{instead}"))
}

/// The flag as the agent wrote it (`--short`, `-s`), from the code when it is there.
fn flag_as_given(context: &ErrorContext, flag: &str) -> String {
    match &context.near {
        Some(near) if near.starts_with('-') => near.clone(),
        _ if flag.chars().count() == 1 => format!("-{flag}"),
        _ => format!("--{flag}"),
    }
}

/// Whether a program of this name is on PATH.
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

/// Nushell names a subcommand by its last word in this error (`status` for `git status`).
/// The longest name made of the words before it on that line, e.g. `git status`.
fn full_name(context: &ErrorContext, command: &str) -> Vec<String> {
    let (Some(line), Some(near)) = (&context.line, &context.near) else { return Vec::new() };
    let before = line.split(near.as_str()).next().unwrap_or_default();
    let words: Vec<&str> = before.split_whitespace().collect();
    let Some(end) = words.iter().rposition(|w| *w == command) else { return Vec::new() };
    (0..end).map(|start| words[start..=end].join(" ")).collect()
}
