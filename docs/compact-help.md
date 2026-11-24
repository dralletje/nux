# A more compact `nux help` (2026-11-24)

## Why

Every agent session reads all of `nux help` (src/help.txt): 332 lines, 21.6 KB, roughly
5-6k tokens, every session. The goal is to pay less for that without agents losing anything
they actually use.

## What the log showed (30 days, ~7,400 calls)

- Agents read the whole manual and almost never look further: of 269 help calls, 249 were
  plain `nux help` and about 20 asked for a command. Whatever leaves the main help is, in
  practice, gone, unless an error hint points to it.
- Use, share of calls: search 28%, complete 21%, ^git 17%, glob 13%, to grep 11%, git
  log/status/add/diff/info 2-7% each, kept 5.5%, edit-file 5%, with-env, from tsc, from json,
  to md and group-by 2-4% each, the test parsers ~2% each. Rare: try-nu 42 calls, py + js
  12, cookie-jar 11, capped 7, show 6, use home/project 2, into sqlite 1.
- Errors: ~9% of calls. The largest cause is unknown flags, mostly grep's on `search`, which
  error hints already answer when they happen.

## Goals

- `nux help` at about 10-12 KB, roughly half.
- Everything agents use often stays in it, git complete.
- Every module stays mentioned (cookie-jar, capped, pnpm, SQLite), so agents know they
  exist; the user's own prelude and `use home/x`, `use project/x` too.

## Non-goals and anti-goals

- No topic pages, no change to `main.rs`: the pointers go to help that exists already
  (`nux help py`, `js`, `try-nu`, `kept`, `cookie-jar`, `capped`, `pnpm`).
- Not a help that is short but leaves out a command, flag or form agents type, or an example
  that no longer runs as written (AGENTS.md).

## Approaches considered

1. Edit only: trim the text, point to the help of commands. Chosen.
2. Topic pages (`nux help python`, ...) next to a smaller core, with a lookup in `main.rs`.
   Dropped: with the modules and the user's prelude mentioned in the core, only the Python
   topic was left, and `nux help py` already is that page.
3. Using the log to decide what stays: not an approach on its own; it is how the cuts in 1
   are decided.

## How the text is trimmed (approved on a sample)

- One example per idea; no reasons ("a call is read again later, by people").
- "How it looks" lists: one line per kind of value.
- Every command name and flag that agents type stays.
- TRY NUSHELL BEFORE PY and PYTHON AND JAVASCRIPT: about three lines, prefer Nushell, py/js
  for a library, `--with`, `nux help py` / `nux help js`.
- YOUR OWN COMMANDS: about three lines, with `use home/x` and `use project/x`; TRYING CODE:
  one line for `try-nu`.
- Modules: one line each.
- A pitfall in GOOD TO KNOW that is rare in the log and has an error hint may go; every one
  removed is listed for the user to veto.

## Also

`nux help capped` says "see the top of this file", which means nothing to an agent: fix it.

## Checks

- Every example in the new help runs as written.
- Size per section, before and after.
- The log's most-used features are each still in the help.

## Left to the builder

Exact wording, the order of sections, which rare pitfalls go (listed for the user).
