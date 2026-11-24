# `nux/npm` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `use nux/npm` gives `npm versions`, `npm info` and `npm files`: the npm registry as tables.

**Architecture:** One Nushell module, prelude/npm.nu, in the shape of prelude/pnpm.nu: each
command runs the npm CLI with `--json`, parses, and reshapes. Tests run nux against a
stand-in `npm` script on PATH that prints recorded output (no network).

**Tech Stack:** Nushell 0.117 (prelude modules), Rust integration tests (tests/cli.rs).

**Spec:** docs/npm-module.md

## Global Constraints

- All of nux goes through nux (`nux -r '...' <<'NU'`); cargo is at `~/.cargo/bin`.
- No `npm` command of its own: `use nux/npm` defines only `npm versions`, `npm info`,
  `npm files`; `npm install` etc. still run npm.
- Every column in every row (AGENTS.md), null where there is nothing.
- Examples and @search-terms on each command; `nux help` stays at most 12 KB.
- Errors carry npm's own `error.summary`.

## Review Focus

- A range or package with exactly one matching version: npm gives a record, not a list.
  Pinned in Task 1 (`solo`) and Task 2 (`info solo@1`).
- A version not in `time` (npm sometimes lacks it): `published` null, still sorted, not
  an error. Pinned in Task 1 (`2.0.0-beta.1` has no time in the fixture).
- A non-deprecated version: the deprecation query gives a record without `deprecated`:
  null, not a missing column. Pinned in Task 1.
- npm failing without JSON on stdout (no network, npm missing): an error with npm's
  stderr, not a parse error. Pinned in Task 2 (`broken`).
- A scoped name (`@scope/pkg@1.0.0`): `npm info` must split the version at the last `@`,
  not the first. Pinned in Task 2 (`@t/scoped`).

---

### Task 1: The module, and `npm versions`

**Files:**
- Create: `prelude/npm.nu`
- Test: `tests/cli.rs` (after `pnpm_by_package_takes_recursive_output_apart`)

**Interfaces:**
- Produces: `export def versions [package: string]: nothing -> table` with columns
  version (string), published (datetime or null), tags (list<string>), deprecated
  (string or null); a private `def npm-json [...arguments: string]: nothing -> any` that
  runs `^npm ...$arguments`, parses stdout as JSON and, on a non-zero exit, errors with
  `msg: $"npm ($arguments | str join ' ')"` and `help:` the JSON's `error.summary`, or
  npm's stderr when stdout is not JSON.
- Test helper produced: `fn fake_npm() -> (PathBuf /*dir*/, OsString /*PATH*/)` writing
  `bin/npm` (a sh `case "$*"`, as the pnpm test's stand-in) with every case below and the
  cases of Task 2; `*)` prints `fake npm: unexpected: $*` to stderr and exits 9.

Stand-in cases for this task (`$*` → stdout, exit 0 unless said):
- `view left-pad time dist-tags versions --json` →
  `{"time":{"created":"2014-03-01T00:00:00.000Z","modified":"2024-01-01T00:00:00.000Z","1.0.0":"2015-01-01T00:00:00.000Z","1.3.0":"2018-04-09T00:00:00.000Z"},"dist-tags":{"latest":"1.3.0","next":"2.0.0-beta.1"},"versions":["1.0.0","1.3.0","2.0.0-beta.1"]}`
- `view left-pad@>=0.0.0-0 version deprecated --json` →
  `[{"version":"1.0.0","deprecated":"use String.prototype.padStart()"},{"version":"1.3.0"},{"version":"2.0.0-beta.1"}]`
- `view solo time dist-tags versions --json` →
  `{"time":{"created":"2020-01-01T00:00:00.000Z","modified":"2020-01-01T00:00:00.000Z","1.0.0":"2020-01-01T00:00:00.000Z"},"dist-tags":{"latest":"1.0.0"},"versions":"1.0.0"}`
  (npm gives one version as a string)
- `view solo@>=0.0.0-0 version deprecated --json` → `{"version":"1.0.0"}`

- [ ] **Step 1: Write the failing test** `npm_versions_gives_a_row_per_version`

```rust
let code = "use nux/npm\nnpm versions left-pad | to nuon";
// rows by published, oldest first; the version without time last (null sorts last)
assert_eq!(out, "[[version, published, tags, deprecated]; [\"1.0.0\", 2015-01-01T00:00:00+00:00, [], \"use String.prototype.padStart()\"], [\"1.3.0\", 2018-04-09T00:00:00+00:00, [latest], null], [\"2.0.0-beta.1\", null, [next], null]]\n");
let code = "use nux/npm\nnpm versions solo | to nuon";
assert_eq!(out, "[[version, published, tags, deprecated]; [\"1.0.0\", 2020-01-01T00:00:00+00:00, [latest], null]]\n");
```
(Exact NUON of datetimes: take what nux prints for `2015-01-01T00:00:00.000Z | into datetime | to nuon` and use that.)

- [ ] **Step 2: Run it**: `cargo test --test cli npm_versions` → FAIL (module not found).
- [ ] **Step 3: Implement** `versions` and `npm-json` in `prelude/npm.nu`, with the file
  header in pnpm.nu's style (what `use nux/npm` gives, that `npm ...` stays npm). Two
  npm calls as in the spec; `versions` may be a string or a list, the deprecation result a
  record or a table.
- [ ] **Step 4: Run it**: `cargo test --test cli npm_versions` → PASS. Also check that
  `use nux/npm` loads without listing it anywhere (the prelude folder is embedded as a
  whole); if not, register it where `nux/pnpm` is.
- [ ] **Step 5: Commit**: "nux/npm: npm versions, the registry's versions as a table".

### Task 2: `npm info`, `npm files`, and errors

**Files:**
- Modify: `prelude/npm.nu`
- Test: `tests/cli.rs` (the stand-in of Task 1 gets these cases)

**Interfaces:**
- Consumes: `npm-json` (Task 1).
- Produces: `export def info [spec: string]: nothing -> record`;
  `export def files [spec: string]: nothing -> table` with columns path (string), size
  (filesize).

Stand-in cases:
- `view left-pad@1 --json` → `[{"name":"left-pad","version":"1.0.0","_id":"left-pad@1.0.0"},{"name":"left-pad","version":"1.3.0","_id":"left-pad@1.3.0","peerDependencies":{"x":"^1"}}]`
- `view left-pad@1.3.0 --json` → `{"name":"left-pad","version":"1.3.0","_id":"left-pad@1.3.0","_rev":"1","versions":["1.3.0"],"time":{},"users":{},"readme":"long","main":"index.js"}`
- `view solo@1 --json` → `{"name":"solo","version":"1.0.0"}`
- `view @t/scoped@2.0.0 --json` → `{"name":"@t/scoped","version":"2.0.0"}`
- `pack left-pad@1.3.0 --dry-run --json` → `[{"name":"left-pad","version":"1.3.0","files":[{"path":"index.js","size":1000,"mode":420},{"path":"package.json","size":600,"mode":420}]}]`
- `view nope --json` and `view nope time dist-tags versions --json` → stdout
  `{"error":{"code":"E404","summary":"Not Found - GET https://registry.npmjs.org/nope - Not found","detail":"..."}}`,
  stderr `npm error code E404`, exit 1
- `view left-pad@99 --json` → stdout `{"error":{"code":"E404","summary":"No match found for version 99"}}`, exit 1
- `view broken --json` → stdout empty, stderr `npm error network ECONNREFUSED`, exit 1

- [ ] **Step 1: Write the failing tests**
  `npm_info_gives_one_record` — `npm info left-pad@1 | get version` → `"1.3.0"` (a range:
  the highest, npm's last); `npm info left-pad@1 | get peerDependencies.x` → `"^1"`;
  `npm info left-pad@1.3.0 | columns` → `[name, version, main]` (no `_` fields, versions,
  time, users, readme); `npm info solo@1 | get version` → `"1.0.0"`;
  `npm info @t/scoped@2.0.0 | get name` → `"@t/scoped"`.
  `npm_files_lists_a_published_package` — `npm files left-pad@1.3.0 | to nuon` →
  `[[path, size]; [index.js, 1000b], [package.json, 600b]]` (exact NUON from nux), and the
  current directory has no `.tgz` after it.
  `npm_errors_say_what_npm_said` — `npm info nope` fails, stdout contains `Not Found - GET
  https://registry.npmjs.org/nope`; `npm versions nope` the same; `npm info left-pad@99`
  contains `No match found for version 99`; `npm info broken` contains `ECONNREFUSED`.
- [ ] **Step 2: Run them**: `cargo test --test cli npm_` → the three new ones FAIL.
- [ ] **Step 3: Implement** `info` (npm's list → its last item; reject columns starting
  with `_` and versions, time, users, readme) and `files` (pack's first item's files:
  path, size into filesize).
- [ ] **Step 4: Run them**: `cargo test --test cli npm_` → PASS.
- [ ] **Step 5: Commit**: "nux/npm: npm info and npm files".

### Task 3: In the help, the column test, and prelude.nu

**Files:**
- Modify: `src/help.txt` (the `With \`use\` first` list), `prelude/prelude.nu` (the
  comment on modules loaded with `use`), `tests/cli.rs`
  (`table_commands_give_every_column_in_every_row`, `help_is_compact_and_keeps_what_agents_use`)

- [ ] **Step 1: Tests** — add `"nux/npm"` to the parts of
  `help_is_compact_and_keeps_what_agents_use`; in a new test
  `npm_tables_give_every_column_in_every_row`, with the stand-in on PATH, the column check
  of `table_commands_give_every_column_in_every_row` over `npm versions left-pad` and
  `npm files left-pad@1.3.0` → `[]`.
- [ ] **Step 2: Run**: `cargo test --test cli help_is_compact` → FAIL (lacks nux/npm).
- [ ] **Step 3: Add** to the modules list in src/help.txt:
  `use nux/npm                           the registry: npm versions, npm info, npm files`
  (aligned with the others, at most 92 characters), and `nux/npm` to prelude.nu's comment.
- [ ] **Step 4: Run** the whole suite: `cargo test` → all pass.
- [ ] **Step 5: Check against the real registry** (not committed): with
  `target/debug/nux`, `use nux/npm; npm versions left-pad | last`, `npm info react-router@7
  | get version`, `npm files left-pad@1.3.0`, `npm info @types/node | get name` → each
  gives a value, no error.
- [ ] **Step 6: Commit**: "nux/npm in nux help".
