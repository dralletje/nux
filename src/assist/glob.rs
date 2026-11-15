//! A glob pattern `glob` refuses. Its glob library (wax) is stricter than a shell's brace
//! expansion, and Nushell's message ("failed to parse glob expression") does not say what
//! it objects to. Two things it refuses that shells accept:
//!
//! - an empty alternative: `{,web/}x`, `{a,}` (wax: "one or more sub-globs", each with
//!   something in it). Bash expands those as text, so they are fine there.
//! - an alternative that starts at the root: `{web,/home/x}/y` (wax: "uncertain or
//!   overlapping roots"; one pattern walks one tree, from one starting point).
//!
//! Only a pattern written in the code can be looked at; `glob $pattern` gets no hint.

use super::ErrorContext;

pub fn hint(context: &ErrorContext) -> Option<String> {
    if context.message != "error with glob pattern" {
        return None;
    }
    let pattern = unquote(context.near.as_deref()?.trim())?;
    let (empty, rooted) = alternatives(&pattern);
    let mut hints = Vec::new();
    if empty {
        hints.push(
            "Nushell's glob has no empty alternatives (`{,web/}x`, `{a,}`): every alternative needs \
             something in it. Write them out (`{x,web/x}`), or use separate patterns.",
        );
    }
    if rooted {
        hints.push(
            "Nushell's glob walks from one starting point: an alternative may not start at `/` or `~` \
             (`{web,/home/x}/y`). Put the absolute part before the braces, or use separate patterns.",
        );
    }
    if hints.is_empty() {
        return None;
    }
    Some(format!(
        "{}\nSeparate patterns: [a b] | each {{|p| glob $p }} | flatten",
        hints.join("\n")
    ))
}

/// The pattern as written in the code: 'x', "x", `x`, r#'x'#, or bare. None for anything else
/// (a variable, a subexpression).
fn unquote(near: &str) -> Option<String> {
    if near.starts_with('$') || near.starts_with('(') {
        return None;
    }
    if let Some(raw) = near.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        let fence = "#".repeat(hashes);
        let inner = raw.strip_prefix(&format!("{fence}'"))?.strip_suffix(&format!("'{fence}"))?;
        return Some(inner.to_string());
    }
    for quote in ['\'', '"', '`'] {
        if let Some(inner) = near.strip_prefix(quote).and_then(|rest| rest.strip_suffix(quote)) {
            return Some(inner.to_string());
        }
    }
    Some(near.to_string())
}

/// Whether the pattern has an empty alternative, and whether an alternative starts at the
/// root. Looks at the text right after each `{` and `,` inside braces.
fn alternatives(pattern: &str) -> (bool, bool) {
    let chars: Vec<char> = pattern.chars().collect();
    let (mut depth, mut empty, mut rooted) = (0usize, false, false);
    for (index, c) in chars.iter().enumerate() {
        match c {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        let starts_alternative = *c == '{' || (*c == ',' && depth > 0);
        if !starts_alternative {
            continue;
        }
        match chars.get(index + 1) {
            Some(',') | Some('}') => empty = true,
            Some('/') | Some('~') => rooted = true,
            _ => {}
        }
    }
    (empty, rooted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_what_wax_refuses() {
        assert_eq!(alternatives("{,web/,../,/home/coder/}{,.}cloudflare.env"), (true, true));
        assert_eq!(alternatives("{,web/}x"), (true, false));
        assert_eq!(alternatives("{a,}"), (true, false));
        assert_eq!(alternatives("{web,/home/coder}/x"), (false, true));
        assert_eq!(alternatives("{web,~/x}/y"), (false, true));
        // Fine for wax: no hint.
        assert_eq!(alternatives("{web/,app/}x"), (false, false));
        assert_eq!(alternatives("{web,../x}/y"), (false, false));
        assert_eq!(alternatives("/home/coder/*.{rs,toml}"), (false, false));
        assert_eq!(alternatives("**/*.{go,rs}"), (false, false));
    }

    #[test]
    fn reads_the_pattern_as_written() {
        assert_eq!(unquote("'{,a}x'"), Some("{,a}x".into()));
        assert_eq!(unquote("\"{,a}x\""), Some("{,a}x".into()));
        assert_eq!(unquote("r#'{,a}x'#"), Some("{,a}x".into()));
        assert_eq!(unquote("$pattern"), None);
    }
}
