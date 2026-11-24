# Is nux better for agents than plain bash? Experiment log

A running log: what was asked, how it is measured, every run, and what it showed. Newest
entries at the bottom.

## The question (2026-11-24)

From the transcripts (Nov 12-24, ~10,800 Bash calls), nux did not look clearly better:

| | plain bash before Nov 16 | plain bash from Nov 16 | nux |
|---|---|---|---|
| calls | 704 (6 sessions) | 2,640 (103) | 4,766 (268) |
| calls that errored | 1.8% | 5.4% | 9.6% |
| median output | 1,051 chars | 618 | 769 |
| next call fine after an error | 83% | 83% | 85% |

nux's own errors: Nushell syntax 2.3% of calls, types and missing columns 2.1%. On the
other side, 24% of plain bash calls parse text with grep/sed/awk/jq/python -c, 6% hide a
failure (`|| true`, `2>/dev/null`), a third cut output with head or tail. But those are
different work at different times, and transcripts do not say whether a task got done. So:
the same tasks, given to agents with nux and to agents with plain bash.

## Design

`eval/nux-vs-bash/`:

- `setup.py <task> <folder>`: a deterministic fixture per task.
- `tasks/<task>.md`: the prompt, the same for both tools.
- `check.py <task> <folder>`: passed or not, per check; truths computed from the fixture.
- `reference.py`: a known-right solution per task; check.py must pass it (it does) and
  fail an untouched fixture (it does).
- `runner.py`: one headless `claude -p` per run, `--model sonnet|opus`.
- `results/<run>/<task>.<tool>.<model>.<rep>.json`: per run.

The tasks, after what the nux log shows agents do most (search, git, tests, edits):

| task | what | checks |
|---|---|---|
| git-history | 40 commits, 3 authors: who added parseConfig, which commit deleted a file, the most changed file, lines added by one author | 4 answers, against git |
| fix-tests | Node project, 21 tests, 4 fail from 3 bugs: name them, fix the source | tests untouched, all pass, the 4 names |
| log-analysis | 21,000 JSON-line requests: top 5xx paths, a p95, distinct users with a 4xx, busiest hour | 4 answers, recomputed |
| refactor | rename getUser to fetchUser across 7 files (aliased import, namespace import, re-export, a comment), but an RPC name `getUser` must stay | a hidden test, own tests, data file unchanged, no old name left in code |

### Both tools start from the same place

Subagents of an agent session would carry that session's nux-only hook and its CLAUDE.md,
so a "bash" subagent could not use bash. Each run is therefore its own `claude -p`:

- `--setting-sources ""`: no user, project or local settings, so no hook; checked: a plain
  bash pipeline ran.
- No CLAUDE.md: the working folder is under /tmp, outside any project, and user settings
  are off; checked: the transcript of a probe run has no trace of the user's CLAUDE.md.
- Tools: Bash, Read, Write, Edit (`--allowedTools`), for both.
- A nux run adds what agents in this workspace get: the hook (`~/.claude/hooks/nux-only.py`,
  via `--settings`), the nux part of the user's CLAUDE.md (`nux-instructions.md`, via
  `--append-system-prompt`), and nux 0.3.0 (built from 14ed40b+) first on PATH.

So "nux" here means nux as this workspace uses it: the manual read at the start, the hook,
the instructions. Its cost is in the numbers.

### Measured per run

passed (all checks), turns, cost (USD, as Claude Code reports it), duration, tokens (input,
output, cache read, cache creation), and from the transcript: Bash calls, Bash errors,
calls the hook blocked, `nux help` calls, characters of Bash output.

## Pilot (2026-11-24)

git-history, sonnet, one run per tool, to check the harness.

| | passed | turns | cost | duration | Bash calls | errors |
|---|---|---|---|---|---|---|
| bash | yes | 4 | $0.047 | 7.6 s | 3 | 0 |
| nux | yes | 8 | $0.091 | 15.5 s | 6 | 4 |

The nux run's four errors were the agent's, not the harness's: `nux help | head -60`
(blocked by the hook: nothing around nux), `uniq-by-count` (no such command), and twice a
`def g [...a] { ^git ...$a }` wrapper that then rejected `--format` and `--name-only` as
its own flags (it needs `def --wrapped`). Worth a hint in nux: an unknown flag on a `def`
whose body runs an external could say `def --wrapped`.

## Run 1 (2026-11-24): 4 tasks × 2 tools × 2 models × 2 repetitions

32 runs, 4 at a time; models as reported: claude-sonnet-5-5, claude-opus-5-5. Results in
`eval/nux-vs-bash/results/run1/`.

**A checker fix first.** All four opus runs of refactor failed one check, nux and bash
alike: they wrote the handler key as `'getUser':` (quoted), which is right, and check.py
only allowed `getUser:`. check.py now allows both; those four runs were checked again on
their files (their records say `rechecked`), and the reference solution still passes.

### Every run did its task

32 of 32 passed. On these tasks the tool made no difference to whether the work got done,
so what is left to compare is what it cost.

### What it cost, per run (averages over 8 runs)

| model | tool | turns | cost | duration | Bash calls | Bash errors | blocked | Bash output |
|---|---|---|---|---|---|---|---|---|
| sonnet | bash | 4.3 | $0.053 | 9.5 s | 3.3 | 0 | 0 | 2.0k chars |
| sonnet | nux | 6.6 | $0.079 | 12.2 s | 4.1 | 0.8 | 0.5 | 13.4k chars |
| opus | bash | 4.0 | $0.125 | 15.2 s | 3.0 | 0 | 0 | 4.1k chars |
| opus | nux | 10.3 | $0.200 | 23.7 s | 3.6 | 0.3 | 0 | 13.7k chars |

nux cost 52% more with sonnet and 60% more with opus, and took 28% and 56% longer. Per
task the picture is the same: nux more turns and cost on every task, most on refactor
(12.3 turns against 4.0) and fix-tests (11.0 against 5.0).

### Where the extra turns come from

| model | tool | Bash | Read | Write | Edit | `nux help` | cache written | cache read |
|---|---|---|---|---|---|---|---|---|
| sonnet | bash | 3.3 | 0 | 0 | 0 | 0 | 8.8k tokens | 78k |
| sonnet | nux | 4.1 | 0.3 | 0.5 | 0.8 | 1.4 | 13.7k | 127k |
| opus | bash | 3.0 | 0 | 0 | 0 | 0 | 10.3k | 74k |
| opus | nux | 3.6 | 2.3 | 0.5 | 2.9 | 1.0 | 15.7k | 136k |

- **The file-tools rule, most of all.** The bash agents did everything in Bash: answers
  written with a heredoc, fixes and the rename with `sed -i` or a script, in 3 to 4 calls.
  The nux agents follow the instruction that comes with nux, "Read, write and edit files
  with the Read, Write and Edit tools": opus averaged 2.3 Read and 2.9 Edit calls a run,
  each a turn that re-reads the whole context. That is a rule of the setup, not of nux.
- **The manual.** One `nux help` call, ~11 KB of output: the ~5k more cache written per
  run, and most of the 13k characters of Bash output (the bash runs: 2-4k).
- **Errors.** Sonnet: 0.8 errors a run, opus: 0.3; the bash runs: none. All 8:
  - `nux help | head -50`, three times (sonnet): blocked by the hook, nothing may be
    around the nux call; the next call was a plain `nux help`.
  - a heredoc without its closing `NU` line (sonnet): blocked by the hook.
  - `open --raw src/text.js`, twice (sonnet): a file name guessed (it is strings.js);
    could happen with any shell.
  - `let ans = ...`, twice (opus): `ans` is a Nushell builtin variable, a parse error.
  So 4 were the hook's rules, 2 ordinary, 2 Nushell's. (The `def` wrapping git without
  `--wrapped` was in the pilot only.)

### What this does and does not show

- On small, clear tasks both get it right, and plain bash is cheaper: about a third less
  cost and time. nux's strengths (tables instead of parsing text, failures that cannot be
  hidden, kept results instead of `head`) did not get a chance to matter: the outputs were
  small, nothing failed silently, and no answer needed parsing that grep or python could
  not do in one line.
- Most of nux's extra cost is the setup around it, not the shell: the file-tools rule and
  reading the manual each session.
- 2 runs per cell; turn counts vary (refactor nux opus: 11 and 14). The direction is the
  same in every cell, the sizes are rough.

### Next, to tell nux from its setup and to give its strengths a chance

1. A third condition, **nux without the file-tools rule** (nux for shell, files any way):
   how much of the gap is that rule.
2. Harder tasks where bash's habits cost: large outputs (head/tail loses the answer),
   failures hidden in a pipeline, a format to parse (a test runner's text, `git log` with
   odd messages), several steps on one data set (kept values against re-running).
3. nux fixes from the runs: a hint for an unknown flag on a `def` that wraps an external
   (`def --wrapped`); whether `nux help | head` should be allowed by the hook.

## Run 2 (2026-11-24): three tweaks to `nux help`, nux runs only

The question: do three lines in the manual change what agents do?

1. An external's output is a value to pipe on as it is (`^git log --format=%h | lines`);
   `| complete` only for stderr, the exit code or to go on after a failure, "never
   `| complete | get stdout`". In the 30-day nux log, 1,846 of 8,079 calls use complete,
   337 of them for a test or tsc parser, 310 exactly `| complete | get stdout`.
2. "in, nu, env and ans are taken: not names for a variable (`let ans = ...` fails)."
3. "A def that passes flags on to an external needs --wrapped".

nux help went from 11,156 to 11,659 bytes. Same 16 nux runs as run 1 (4 tasks × sonnet,
opus × 2), nux built with the new help; results in `results/run2/`.

| | runs | passed | turns | cost/run | duration | Bash calls | errors | blocked | `complete \| get stdout` | `let ans` |
|---|---|---|---|---|---|---|---|---|---|---|
| run 1 opus | 8 | 8 | 10.3 | $0.200 | 23.7 s | 3.6 | 2 | 0 | 0 | 2 |
| run 2 opus | 8 | 8 | 8.9 | $0.190 | 20.7 s | 4.0 | 3 | 1 | 0 | 2 |
| run 1 sonnet | 8 | 8 | 6.6 | $0.079 | 12.2 s | 4.1 | 6 | 4 | 1 | 0 |
| run 2 sonnet | 8 | 8 | 7.0 | $0.083 | 12.8 s | 4.4 | 7 | 4 | 1 | 0 |

(errors and blocked: totals over the 8 runs.)

**No measurable effect.**

- `| complete | get stdout` was rare in both runs (1 each): these tasks do not provoke it,
  so the eval cannot tell. The 30-day log can: count it again in a week.
- `let ans`: both opus log-analysis runs read the manual, with the new line in it, and
  still wrote `let ans = ...` when building the answer record. A line in the manual does
  not beat the name the task suggests. What would: a hint on that error (nux has hints for
  errors, src/assist/), or nothing, since the next call fixes it.
- `nux help | head -60`: 4 of 8 sonnet runs again, as in run 1. It is the first call, made
  before the manual is read, so no manual text can prevent it. The places that can: the
  instruction that tells agents to run `nux help` (say: as it is, no pipe), or the hook
  (let `nux help` through with a pager or head).
- Turns and cost moved a little both ways (opus -1.4 turns, sonnet +0.4): noise at 8 runs.

The lever for nux's own errors is where the agent is when it makes them: before reading
the manual (the instruction, the hook), or at the error (a hint), not the manual.
