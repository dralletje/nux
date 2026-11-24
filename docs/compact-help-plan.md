# A more compact `nux help`: Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut `nux help` (src/help.txt) from 21.6 KB to about 10-12 KB without losing anything agents use.

**Architecture:** A text-only rewrite of src/help.txt, pinned by a test on its size and on
what it must still name. No code change: pointers go to command help that exists already.

**Tech Stack:** Rust (tests/cli.rs), Nushell prelude modules.

**Spec:** docs/compact-help.md

## Global Constraints

- All of nux goes through nux (`nux -r '...' <<'NU'`); cargo is at `~/.cargo/bin`.
- `nux help` at most 12 KB (12288 bytes of src/help.txt).
- Trim style: one example per idea; no reasons; one line per kind of value in "how it looks"
  lists; every command name and flag agents type stays.
- Git stays complete; FROM BASH stays (trimmed); every module is mentioned (cookie-jar,
  capped, pnpm, SQLite); the user's prelude, `use home/x` and `use project/x` are mentioned.
- Every example in the help runs as written (AGENTS.md).
- No topic pages and no change to src/main.rs.

## Review Focus

- The `{log_dir}` and `{kept_dir}` placeholders (replaced in src/main.rs `help_text`): both
  must survive the rewrite, or the paths read literally `{kept_dir}`. Pinned in Task 1.
- An example that worked in the old help and is reworded: it must still run. Task 2, step 4.
- Lines over 92 characters wrap badly in a terminal and in agents' views: keep the current
  width. Pinned in Task 1.
- A pitfall dropped from GOOD TO KNOW that agents do hit: every removal is listed for the
  user (Task 2, step 5), with its count in the log.
- `nux help capped` read by an agent must say what the command does and returns without
  "this file". Task 3.

---

### Task 1: Pin the budget and what must stay

**Files:**
- Test: `tests/cli.rs` (next to `help_is_the_one_manual`, line 53)

- [ ] **Step 1: Write the failing test** `help_is_compact_and_keeps_what_agents_use`

```rust
#[test]
fn help_is_compact_and_keeps_what_agents_use() {
    let source = include_str!("../src/help.txt");
    assert!(source.len() <= 12_288, "src/help.txt is {} bytes", source.len());
    assert!(source.contains("{kept_dir}") && source.contains("{log_dir}"));
    for line in source.lines() {
        assert!(line.chars().count() <= 92, "too wide: {line}");
    }
    let help = nux(&["help"], "").stdout;
    // The most used in the nux log (docs/compact-help.md), and every module.
    for part in [
        "search", "--glob", "--context", "to grep", "complete", "^git", "glob ", "git log",
        "git status", "git diff", "git hunks", "git info", "git add", "git show-file", "kept",
        "edit-file", "with-env", "from tsc", "from json", "to md", "group-by", "from tap",
        "from cargo-test", "from bun-test", "ports", "wait-for url", "timeout", "show",
        "check-exit", "try-nu", "nux help py", "nux help js", "cookie-jar", "capped",
        "nux/pnpm", "into sqlite", "prelude.nu", "use home/", "use project/", "NUX_FORMAT",
        "-r '", "nux find", "nux-log",
    ] {
        assert!(help.contains(part), "nux help lacks {part}");
    }
}
```

- [ ] **Step 2: Run it, and see it fail on the size only**

Run: `cargo test --test cli help_is_compact` → FAIL, "src/help.txt is 21662 bytes". If a
part is reported missing instead, it is missing today: tell the user before going on.

- [ ] **Step 3: Commit** the test only, `tests/cli.rs`: "A budget for nux help, and what it must keep".

### Task 2: Rewrite src/help.txt

**Files:**
- Modify: `src/help.txt` (all of it)

**Interfaces:** Consumes the test of Task 1.

- [ ] **Step 1: Record the size per section** (headings are lines matching `^[A-Z][A-Z ]+`)
  into the scratchpad, for the before/after table.

- [ ] **Step 2: Rewrite**, section by section, in this order and with these targets:
  - RUN SOMETHING (0.6 KB): keep; move `-r <reason>` here from the end of the next section.
  - WRITE IT ON MANY LINES + "What you get back" (4.5 KB → ~1.6 KB): as the approved sample,
    two sections `WRITE IT AS A SCRIPT` and `WHAT YOU GET BACK`.
  - NOTHING CARRIES OVER (0.65 KB → ~0.45): the four `kept` forms stay, `{kept_dir}` stays.
  - YOUR OWN COMMANDS + TRYING CODE (1.06 KB → ~0.4): prelude.nu, `use home/x`,
    `use project/x` in ~3 lines; `try-nu` in one line pointing to `nux help try-nu`.
  - COMMANDS THAT GIVE TABLES (6.6 KB → ~3.5): every git line stays; search, to grep,
    ast-find, from tsc and the test parsers in shorter descriptions; ports, wait-for url/
    port/file/pid and timeout; one line per module: cookie-jar, capped, pnpm, and
    `into sqlite` / `query db`.
  - OTHER TOOLS ALREADY SPEAK JSON (1.05 KB → ~0.6): the json forms, the http status line.
  - TRY NUSHELL BEFORE PY + PYTHON AND JAVASCRIPT (2 KB → ~0.35): prefer Nushell (`parse
    --regex`, `mut` + `for`); py/js for a library, `data`, `--with`; `nux help py`,
    `nux help js`.
  - MORE YOU CAN DO WITH A TABLE (0.64 KB → ~0.3): `to md`, `to csv | save`, `group-by`.
  - FROM BASH (1.4 KB): keep every row; shorten the right-hand notes only.
  - LOOK THINGS UP, GOOD TO KNOW, MORE (3 KB → ~1.6): keep the strings/`$"..."` rules,
    `-t application/json`, `job recv`, interactive and servers (`run_in_background`,
    `wait-for url` in a `timeout`); drop rare pitfalls already answered by an error hint.

- [ ] **Step 3: Run the help tests**

Run: `cargo test --test cli help_` → PASS (both `help_is_the_one_manual` and
`help_is_compact_and_keeps_what_agents_use`).

- [ ] **Step 4: Run every example in the new help** with `target/debug/nux`, in a scratch
  git repo with a package.json and a .ts file, each as written. Expected: each runs, or fails
  only for want of the thing it names (a server, a pnpm workspace, a commit), never with a
  parse error or an unknown flag.

- [ ] **Step 5: Before/after table and the list of removed lines**: per section, bytes
  before and after; every pitfall or example removed, with how often the log shows it
  (`nux-log --since 30day`). For the user, not committed.

- [ ] **Step 6: Run the whole suite**: `cargo test` → all pass.

- [ ] **Step 7: Commit** `src/help.txt`: "nux help at half its size: shorter, pointers to command help".

### Task 3: `nux help capped` without "this file"

**Files:**
- Modify: `prelude/capped.nu:21` (the doc comment of `main`)

- [ ] **Step 1: Rewrite the comment of `main`** so `nux help capped` says: what it does
  (an external command under a hard memory limit, kernel-enforced, and a time limit), that
  it returns a record (exit_code, killed_by, peak_memory, duration, stdout, stderr) instead of
  failing, and one `@example`: `capped --mem 1500MB --time 40sec pnpm exec tsc -b`. Say
  "coreutils' timeout" where the file comment means the external one, not nux's `timeout`.

- [ ] **Step 2: Check** `target/debug/nux help capped` after `cargo build`: no "this file",
  shows the record's fields and the example.

- [ ] **Step 3: Commit** `prelude/capped.nu`: "nux help capped: what it does and returns, not 'this file'".
