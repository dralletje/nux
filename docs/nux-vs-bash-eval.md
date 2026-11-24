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
- **nux's own mistakes.** Sonnet: 0.8 errors and 0.5 hook blocks a run (`nux help |
  head`, a `def` wrapping git without `--wrapped`); opus: 0.3. The bash runs: none.

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
