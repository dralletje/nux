//! An external command that failed where its output was going into a value or the next
//! command: inside `( )`, a `def`, a closure, or before a `|`. Nushell then gives only the exit
//! code; the output it had read is gone (the stderr went to nux's own stderr). nux shows an
//! external's output itself only for one that is a statement of its own.

use super::ErrorContext;

pub fn hint(context: &ErrorContext) -> Option<String> {
    if context.code.as_deref() != Some("nu::shell::non_zero_exit_code") {
        return None;
    }
    Some(
        "Its output went into a value or the next command, and is gone now that it failed. \
         To keep all of it: `^cmd ... | complete` gives {stdout, stderr, exit_code} and does not \
         fail; then `if $run.exit_code != 0 { ... }`. (An external that is a statement of its \
         own shows its output.)"
            .into(),
    )
}
