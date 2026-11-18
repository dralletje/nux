# Feedback from a day of agent use (2026-11-16/17): what was done, what was not, and why

The review of ~1,100 calls on the Coder workspace `dral-flow` (all nux-rs 0.2.0) is in
`nu-feedback.md` next to the repos there; the numbers below (1.1, 2.3, ...) are its
sections. This file keeps what became of each item, so that it can be picked up later.

## Done on `development`

- **1.1 The log has what the caller saw.** `src/capture.rs`: stdout and stderr are pipes
  that pass everything on and keep a copy; the end line has `stdout` and `stderr` next to
  `text` (the reply). A very long stream keeps its first and last 64 KiB. A stopped call
  logs what it printed until then. nux exits once nothing writes to the caller any more, as
  before (the harness waited for that already).
- **1.5, 1.6 `from tsc`.** Takes the record of `complete`, reads stdout and stderr, and fails
  when the command failed without a diagnostic. pnpm -r's `<folder> <script>: ` before a
  line is joined into `file`. The helpers for that moved from `testing.nu` to `util.nu`
  (`command-output`, `not-the-format`), shared with `from tap` and `from node-26-test`.
- **1.8 Unknown flags.** The hint looks commands up as the code sees them (the parse's
  working set: a `use` in the same code counts), and says nothing when the command it found
  has the flag (it is then the wrong command). `nux help cookie-jar http` loads the module.
- **Error hints**: `\.` in "..." (2.3), `\'` in '...' (2.8), several patterns to `glob`
  (2.6), Markdown opened as a table (2.2), and `range` works as `slice` with a note (2.4).
- **edit-file 4.5 #6, #7.** Not found: where the file stops matching (the longest start of
  `old` that is there, its line, what follows in the file and in `old`), "only whitespace
  differs", and bash's `'"'"'` named. old == new is a warning.
- **1.2 ast-grep drops the `;`.** ast-grep's own behaviour (a statement pattern matches the
  statement with its `;`). Fixed in nux with a rule that knows no language: only what the
  pattern names is replaced; the rest of the match stays, in the column `trailing`. Why,
  how, 30 cases and their history: `tests/ast-rewrite/README.md`. CI installs ast-grep
  0.51.3 so they run there.
- **1.3 ast-apply after ast-apply.** The "changed since the search" error says to apply both
  searches' rows at once, or search again after the first apply.
- **2.1, 2.7, 3.4 `search`.** New flags where nux had no way: --before-context (-B),
  --after-context (-A), --only-matching (-o). To read rows as grep prints them, `to grep`
  (search and ast-find rows; after filtering, which a flag on search would not allow).
  For grep flags that do have a way here (-n, -r, -c, -e, -w, -E, -v, -h), the unknown-flag
  hint says what it is. A path with `*` is a glob. The manual asks for long flag names.
- **3.2 Every value is shown.** Each top-level statement's value, under `── line N:
  <statement>` when there is more than one; declarations, assignments, loops, `print`,
  `| ignore` and output sent to a file give none. Earlier external commands' output too
  (check-exit passes it on). The usage log has all of it in `stdout`.

## Upstream (Nushell), to fix later

- **`to json` writes control characters unescaped.** A string with an ESC (terminal colors in
  logged test output) comes out of `to json` with the ESC as is, which is not valid JSON:
  Node's `JSON.parse` refuses it ("Bad control character in string literal"). Found
  2026-11-18 while rendering logged values for the format runs (they now leave such values
  out). To report to Nushell (0.117); nux could escape them in the meantime.

## Done later (2026-11-18)

- **1.4 Kept results**: a result too large to print that is already kept says that name
  instead of keeping a copy. (3.1, the file path, not done: see below.)
- **2.5** `git status --short` and `--porcelain` give the table.
- **2.10** Hints for `| do {|r| ...}`, a bare column after `or (` in `where`, `rm` of the
  folder the call is in, and a table command on text.
- **3.5** `search` on text piped in (`file` is "(input)").
- **4.2** `from bun-test` and `from swift-test` in `nux/test-format` (with `from tap`,
  `from node-26-test`, `from cargo-test`).
- **4.5** `edit-file` with a `path` column (several files, all or nothing) and `--regex`.
- **4.6** In the manual: `glob --no-dir` for files only, `path relative-to (pwd)`, braces for
  several patterns. Not a `glob` of nux's own: it would change Nushell's command.
- **4.7** `wait-for --status [200]`. Any answer, a 404 too, counted as up already; the
  manual says so now.
- **4.8** After 30 s, a call whose code looks like it starts a dev server says so on stderr,
  and to use the harness's background mode. Not a `spawn` (nor `job spawn`): starting things
  in the background is the harness's work.
- **Part 5** in the manual: `-r` in the first example, `ast-find --rewrite` for changes
  across files, the test parsers.
- Also: `show` and the sample of a large result print values the way a call does
  (`to readable`).

## Open: a decision first

- **3.1 The file path of a kept result**, for the harness's Read tool. Only of use when text
  is written as plain text, not NUON. Undecided.
- **2.2 Markdown.** `open x.md` gives a table of the file's elements (heading, paragraph,
  ...: `element`, `content`, `content_span`), from Nushell's `from md`. Done: the hint and the
  manual. Not done: making `open` give text (hiding `from md`), which goes against Nushell.
- **3.3 A format for agents instead of NUON.** The plan to test it: `docs/output-format-eval.md`.

## Would do, later

- **1.7 A stopped call keeps nothing.** The log now has its output (1.1). Keeping its
  variables means reading them while the code still runs on another thread: harder.
  Streaming `show` as it happens is the first step.
- **2.9 A raw string as a statement before `| ^git commit`.** A better parse error, or the
  manual's "bind it with `let` first".
- **4.2 `complete --merged`** (stdout and stderr in order), `from eslint` for `--format json`.
- **4.4 git tables**: `git show <rev>:<path>`, `git show <rev>`, `git log -5`/`--oneline`
  spellings, `short` in `git info`, `status`/`from` (renames) in `git diff`, untracked files
  expanded. In that order of use.
- **4.7 `wait-for --gone`** (a process to end).
- **ast-grep** lays out a multi-line `$$$` hole its own way (tests/ast-rewrite/README.md).

## Would not do

- **4.1 `view`.** The ~200 calls that read files through nux were made in Claude Code,
  which has a Read tool for that (and the workspace's instructions say to use it). A second
  way to read files is not what is missing.
- **4.5 `edit-lines`, `--show`, `with-edit`.** The same: the harness edits files.
- **4.3 `git commit` as a command.** `$message | ^git commit -F -` works; the failure seen was
  `-q` hiding git's message, and the follow-up call to see the commit is gone now that every
  statement's output is shown (git prints `[branch abc1234] subject` and the files changed).
- **4.9 `nux -C`, a project env file.** `cd` works; a file that changes every call goes
  against "nothing carries over".
- **4.10** mostly: `count-by`, `with-tmp`, `save --json`, `str truncate` are each one line of
  Nushell. `from diff` (a .diff file as `git diff --lines` rows) is the one worth a look.

## A format for agents (3.3)

Done as "v0", nux's default since 2026-11-17: what it is, why, and the runs that tested
it are in `docs/output-format-eval.md` (src/readable.rs; `NUX_FORMAT=nuon` for NUON).
