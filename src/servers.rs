//! A call that starts a dev server runs until something stops it: the server does not end on
//! its own. Agents did this (`bun run dev` in a call, review of 2026-11-17, item 4.8) and
//! waited until their harness killed the call, half an hour later. So when a call is still
//! running after a while and its code looks like it starts a server, nux says so on stderr,
//! which reaches the caller as it is written (`capture`), also when the call is killed later.

use std::time::Duration;

/// How long a call may run before the warning.
const AFTER: Duration = Duration::from_secs(30);

/// Watch this call: print the warning after [`AFTER`] if the code looks like a server.
pub fn watch(code: &str) {
    let Some(what) = looks_like_server(code) else { return };
    std::thread::spawn(move || {
        std::thread::sleep(AFTER);
        eprintln!(
            "nux: this call has run for {}s, and `{what}` looks like a server, which does not end on its own. \
             Start it with your harness's way to run a command in the background (Claude Code: run_in_background), \
             not in a nux call; then `wait-for http://localhost:<port>/` in a call.",
            AFTER.as_secs()
        );
    });
}

/// The words of the code that start a server, if any: `run dev`, `next dev`, `vite`, ...
pub fn looks_like_server(code: &str) -> Option<String> {
    let words: Vec<&str> =
        code.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_' || c == ':')).filter(|w| !w.is_empty()).collect();
    const SCRIPTS: &[&str] = &["dev", "start", "serve", "preview", "watch"];
    const RUNNERS: &[&str] = &["npm", "pnpm", "yarn", "bun", "deno", "npx", "bunx"];
    const ALONE: &[&str] = &["vite", "nodemon", "--watch", "http-server", "live-server"];
    for (index, word) in words.iter().enumerate() {
        if ALONE.contains(word) {
            return Some(word.to_string());
        }
        let next = words.get(index + 1).copied().unwrap_or_default();
        let after = words.get(index + 2).copied().unwrap_or_default();
        // `bun run dev`, `pnpm dev`, `npm start`, `pnpm --filter web dev`.
        if RUNNERS.contains(word) {
            if next == "run" && SCRIPTS.contains(&after) {
                return Some(format!("{word} run {after}"));
            }
            if SCRIPTS.contains(&next) {
                return Some(format!("{word} {next}"));
            }
            if let Some(script) = words[index + 1..].iter().take(4).find(|w| SCRIPTS.contains(w)) {
                return Some(format!("{word} ... {script}"));
            }
        }
        // `next dev`, `astro dev`, `wrangler dev`, `remix dev`.
        if next == "dev" && matches!(*word, "next" | "astro" | "wrangler" | "remix" | "nuxt" | "vite") {
            return Some(format!("{word} dev"));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn servers_and_not() {
        assert_eq!(looks_like_server("^bun run dev o+e> log"), Some("bun run dev".into()));
        assert_eq!(looks_like_server("pnpm --filter web dev"), Some("pnpm ... dev".into()));
        assert_eq!(looks_like_server("^npx next dev"), Some("npx ... dev".into()));
        assert_eq!(looks_like_server("vite --port 3000"), Some("vite".into()));
        assert_eq!(looks_like_server("^npm start"), Some("npm start".into()));
        assert_eq!(looks_like_server("^bun test"), None);
        assert_eq!(looks_like_server("^pnpm build; git status"), None);
        assert_eq!(looks_like_server("search 'dev' src"), None);
        assert_eq!(looks_like_server("^git log --since 'started'"), None);
    }
}
