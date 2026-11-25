//! A variable named like one of Nushell's own (`ans`, `in`, `nu`, `env`). Nushell says the
//! name is taken; agents then guess at another and sometimes take a second one that is
//! (`let ans` was the most common Nushell mistake in the eval). The hint names one that works.

use super::ErrorContext;

pub fn hint(context: &ErrorContext) -> Option<String> {
    if context.code.as_deref() != Some("nu::parser::name_is_builtin_var") {
        return None;
    }
    let name = context.near.as_deref()?.trim_start_matches('$');
    let other = match name {
        "ans" => "answer",
        "in" => "input",
        "nu" => "nu_info",
        "env" => "environment",
        _ => return None,
    };
    Some(format!("`${name}` is Nushell's own; name yours otherwise: `let {other} = ...`"))
}
