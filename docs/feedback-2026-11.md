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

## Open: a decision first

- **1.2 ast-grep drops the `;`.** It is ast-grep's behaviour, not nux's: a pattern that is a
  statement matches the statement with its `;`, and the replacement takes the place of all
  of it (`ast-grep run -p 'import { $$$A } from "./x"' -r '...'` does the same). A pattern
  that ends in `;` works. Other tools: comby (text templates, no syntax tree), GritQL,
  semgrep (heavy), jscodeshift/ts-morph (JavaScript only); none is clearly better at this.
  A fix that knows no language: when the match goes on past the end of the pattern's last
  literal text (`"./x"` then `;`), and the replacement does not end in that, keep it. That
  is the rule "only what the pattern names is replaced". Undecided.
- **1.3 ast-apply after ast-apply.** The hint ("apply all rows together: `$a | append $b |
  ast-apply`") is cheap. Together with 1.2.
- **1.4, 3.1 Kept results.** Not re-keeping a value that came from `kept` is a clear fix.
  The file path is for the harness's Read tool (paging with line numbers, without another
  call); that only helps when a string is written as plain text, not NUON. Undecided whether
  the path is worth it.
- **2.1, 2.7, 3.4, 3.5 `search`.** It is nux's own (prelude.nu, around ripgrep), so its flags
  are ours. Options: real flags where they add something (`-A`/`-B`, `-o` as a `match`
  column, `-c` as `--files`); `-n` and `-r` taken and ignored (what search does anyway);
  several `-e` patterns; a hint for `*.ts` as a path; a `--text` output in grep's form (~200
  calls format rows into that by hand); text on stdin.
- **2.2 Markdown.** `open x.md` gives a table of the file's elements (heading, paragraph,
  ...: `element`, `content`, `content_span`), from Nushell's `from md`. Done: the hint and the
  manual. Not done: making `open` give text (hiding `from md`), which goes against Nushell.
- **3.2 Values that are not shown.** Options: a footer naming the lines whose values were
  dropped; showing every top-level value, each under its line, unless muted (`| ignore`);
  pointing to `show` more. To decide.
- **3.3 A format for agents instead of NUON.** See the last section.

## Would do, later

- **1.7 A stopped call keeps nothing.** The log now has its output (1.1). Keeping its
  variables means reading them while the code still runs on another thread: harder.
  Streaming `show` as it happens is the first step.
- **2.5 `git status --short`.** The hint is right already; taking `--short`/`--porcelain` and
  giving the same table saves the round trip.
- **2.9 A raw string as a statement before `| ^git commit`.** A better parse error, or the
  manual's "bind it with `let` first".
- **2.10 Small hints**: `| do {|r| ...}`, `where index < 12` (a column in `where` needs
  `$it.` or a closure), `rm` of the folder you are in, `kept result-N | where` on a string.
- **4.2 `from bun-test`, `complete --merged`.** bun writes its report to stderr; a parser
  saves the regexes agents write per call. `from eslint` for `--format json`.
- **4.4 git tables**: `git show <rev>:<path>`, `git show <rev>`, `git log -5`/`--oneline`
  spellings, `short` in `git info`, `status`/`from` (renames) in `git diff`, untracked files
  expanded. In that order of use.
- **4.5 edit-file**: a table with a `path` column (several files, all or nothing); `--regex`.
- **4.6 `glob --files --relative`**, several patterns.
- **4.7 `wait-for --status [...]`** (404 counts as up for a dev server), `wait-for --gone`.
- **4.8 A server in the foreground**: a warning on stderr after ~30 s for `dev`/`serve`/
  `vite`. Not a `spawn` command: nux ends what a call started, on purpose.
- **Part 5, the manual**: each item is minutes. `-r` in the first example (6% of calls
  give a reason).

## Would not do

- **4.1 `view`.** The ~200 calls that read files through nux were made in Claude Code,
  which has a Read tool for that (and the workspace's instructions say to use it). A second
  way to read files is not what is missing.
- **4.5 `edit-lines`, `--show`, `with-edit`.** The same: the harness edits files.
- **4.3 `git commit` as a command.** `$message | ^git commit -F -` works; the failure seen was
  `-q` hiding git's message. Maybe later, as part of 4.4.
- **4.9 `nux -C`, a project env file.** `cd` works; a file that changes every call goes
  against "nothing carries over".
- **4.2 `from swift-test`**: one project's.
- **4.10** mostly: `count-by`, `with-tmp`, `save --json`, `str truncate` are each one line of
  Nushell. `from diff` (a .diff file as `git diff --lines` rows) is the one worth a look.

## A format for agents (3.3)

NUON is made to be read back by Nushell; what nux prints is read by a model, and only by a
model: when the data is needed again, `kept` gives it back with its types. So the printed
form can give up being parseable, and be made for reading:

- a list of strings one per line; a multi-line string as an indented block, no escapes;
- strings without quotes where nothing else could be meant (quotes only around what would
  read as a number, a bool, null, or has leading/trailing spaces);
- a table as a header and aligned rows (or `|`-separated), columns written once;
- durations, sizes and dates as people write them (`1min 28s`, `3.4 MB`, `2026-11-17 09:12`);
- nesting by indentation, records as `key: value` lines.

Something close exists: TOON ("Token-Oriented Object Notation", 2025), YAML-like with
tables as a header and CSV rows, made to save tokens for LLM input. To decide: take the
logged results of a day (`nux-log`, `text`), print them as NUON and as a candidate format,
compare length in tokens, and have agents answer questions about the same results in both
forms.
