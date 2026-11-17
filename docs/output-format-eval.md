# Plan: which format should nux print for agents? (NUON vs TOON vs our own)

Status: plan, nothing run yet. Follows from item 3.3 of `docs/feedback-2026-11.md`.

## The question

nux prints a call's values as NUON. NUON is made so that Nushell can read it back, but what
nux prints is only ever read by a model: when the data itself is needed again, `kept` gives
it back with its types. So the printed form may give up being parseable, and be chosen only
for how well a model reads it, and at what cost in tokens.

The review of 2026-11-17 suggests NUON costs us: 40% of calls contain `str join "\n"`,
mostly to turn a list into lines; ~30 end in `| to md` to read a table; 322 use several
`print (...)`. Those are agents working around the output format.

Three things to find out:

1. **Reading**: with which format does a model answer questions about a result correctly?
2. **Cost**: how many tokens does each format take for the same value?
3. **Behaviour**: with the winner as the default, do agents stop reformatting their output
   (fewer `str join`, `to md`, `print` calls, fewer calls per task)?

1 and 2 are an offline experiment; 3 needs the format in real use.

## What others found

Improving Agents (improvingagents.com) compared formats in three posts. What they found,
and why it does not settle our question:

- **Flat tables** ("Which Table Format Do LLMs Understand Best?",
  /blog/best-input-data-format-for-llms): 1,000 records of 8 flat fields, 1,000 lookups of
  one field of one record, GPT-4.1-nano. Markdown-KV (each record under a `## Record N`
  heading, then `key: value` lines) was best: 60.7% (CI 57.6–63.7), 52,104 tokens. Markdown
  table: 51.9%, 25,140 tokens. JSON: 52.3%, 66,396 tokens. CSV: 44.3%, 19,524 tokens.
  Pipe-delimited was last: 41.1%.
- **TOON** (/blog/is-toon-good-for-table-data), same setup: 47.5% (CI 44.4–50.6), 21,518
  tokens. Below Markdown table, about level with CSV.
- **Nested data** (/blog/best-nested-data-format): Terraform-like configuration 6–7 levels
  deep, three small models. YAML was best for GPT-5 Nano (62.1%, Markdown 54.3, JSON 50.3)
  and for Gemini 2.5 Flash Lite (51.9%, Markdown 48.2, JSON 43.1). For Llama 3.2 3B no format
  was clearly better. Markdown (nesting as headings, leaves as `key: value`) used the fewest
  tokens, about a third fewer than JSON.

Their own caveats: one or three small models, no Claude; data made so large that accuracy
falls to 40–60% on purpose (with small data it is "close to 100%"); only lookup questions;
the table study had its header 1,000 rows away from most rows. nux prints at most 10 KiB
(more is kept), and agents run on Opus and Sonnet. So the differences may be much smaller
for us, or gone, and only a test on our values and models tells. What they do give us:
candidates (Markdown-KV and YAML), and a hint that the best format depends on the shape:
key-per-value for big tables, YAML for deep nesting, a plain table when tokens matter.

## Formats

| | Format | Why it is in |
|---|---|---|
| A | NUON, as now (`to nuon --raw`, raw strings) | the baseline |
| B | JSON, compact | what models have seen most |
| C | TOON (Token-Oriented Object Notation) | made for this: YAML-like, tables as a header with field names, then one CSV-like row per item |
| D | our own, tables aligned with spaces | see below |
| E | our own, tables with `|` between cells | D with explicit delimiters: answers "are aligned tables easier?" with nothing else different |
| F | Markdown tables (`to md`) | what agents reach for themselves |
| G | Markdown-KV: `## <n>` per row, then `key: value` lines; nesting as deeper headings | best on large flat tables above |
| H | YAML | best on deep nesting above |
| X | **by shape**: per value, the format that wins for its shape (see "Choosing per value") | the question below |

### Our own format (D and E), a first spec to test

Made for reading, not parsing:

- text: as it is; a multi-line string inside a record or list is an indented block, no escapes;
- strings unquoted, except where they would read as something else: a number (`"12"`),
  `true`/`false`/`null`, empty, leading or trailing spaces. Those get `"..."`;
- a list of scalars: one item per line, `- ` before each;
- a record: `key: value` lines; nesting by two spaces of indentation;
- a table: a header line, then one line per row. D pads columns to line up; E writes
  `a | b | c`. A cell with a newline or the delimiter falls back to the record form for that
  table;
- durations, file sizes and dates as people write them: `1min 28s`, `3.4 MB`,
  `2026-11-17 09:12:03 +00:00`;
- above the value, its type in one short line when it is not plain to see: `table, 120 rows`
  (the same idea as the large-result preview).

Examples go in the eval's README together with the renderer, so the spec is the code.

## Data

Real results, not invented ones: the usage log has every call's reply (`text`; NUON for
data) since 2026-11-16, and kept results hold the large ones whole.

1. Take the results of a week of `nux-log`, parse the NUON replies back to values (`from
   nuon`), drop text-only replies (text is printed as it is in every format).
2. Classify by shape: scalar, list of strings, small table (≤ 10 rows), long table
   (> 100 rows), wide table (> 8 columns), nested (records in cells: `ast-find` vars,
   `git diff` versions), records with multi-line strings (commit messages, errors), and
   values with durations, sizes and dates. Record for each value the measures the choice
   per value would use (below): rows, columns, depth, size in bytes.
3. Sample ~25 per shape, ~200 values. Also a stress set: the same shapes grown to the
   10 KiB limit (more rows, deeper), since that is where formats should differ most for us,
   as in the studies above. Leave out secrets: the log holds what agents saw, so
   filter on the usual patterns (tokens, keys, `.env` contents) and look over the sample
   before it leaves the machine.

## Questions, with answers known from the data

Generated from each value by a script, so no hand labelling and exact checking:

| Kind | Example | What it tests |
|---|---|---|
| lookup | "What is `status` of the row whose `path` is `src/a.ts`?" | cell to column to row |
| count | "How many rows have `staged` set?" / "How many items?" | scanning a whole table |
| extreme | "Which file has the most `added` lines?" | numbers across rows |
| exists | "Is there a row for `README.md`?" (half of them no) | not inventing rows |
| nested | "What did hole `A` match in the match on line 12?" | nesting |
| multi-line | "What is the second line of the message of commit `9caf6b7`?" | blocks and escapes |
| exact copy | "Give the exact path of the third row." (paths with spaces, quotes, unicode) | what an agent pastes into its next call |
| type | "Is `version` a number or text?", "Is it larger than 1 MB?" | `"12"` vs `12`, sizes, null vs "null" |

Four questions per value, with the kinds that fit its shape. Answers are checked exactly
(after trimming), numbers numerically.

## Running it

- Models: the ones agents run on: Opus 5.5, Sonnet 5.5, Haiku 5.5. Format sensitivity
  shows most in the smaller ones.
- Prompt: a short system prompt as an agent would have ("This is the output of a nux call:
  ..."), one paragraph that describes the format (what `nux help` would say), the value,
  the question, "answer with the value only". Same wording for every format; questions in
  random order.
- Tokens: counted with the API's token counting, per format per value.
- Size: pilot first, 40 values × 4 questions × 8 formats (A–H) on Sonnet (1,280 requests),
  to check the questions and the scoring. Then all ~200 values and the stress set on all
  three models (~25,000 requests), cached system prompt. Run with the Batch API: not
  urgent, half the price. X needs no requests of its own at first: its answers are those
  of the format it picks (see below).
- Where: a folder `eval/output-format/` in this repo, the renderers as nux commands (`to
  agent`, and TOON through its reference implementation with `js`), the runner as a script.
  Needs an Anthropic API key, which this workspace does not have.

## Reading the results

Per format: accuracy overall and per question kind and per shape, tokens per value, and
accuracy per 1,000 tokens. Then:

- a format that loses accuracy on any shape against JSON is out, whatever it saves;
- of the rest, the fewest tokens wins;
- D against E answers the alignment question;
- if our own and TOON are close, TOON: a published spec, which models may come to know,
  and less for us to maintain;
- per shape as well as overall: if the winner differs by shape, the rule of "Choosing per
  value" (X) is tested against the best single format.

## Choosing per value

Possible in nux, and cheap: every value nux prints goes through one function
(`value_text` in src/engine.rs), which has the value itself, with its types, before it
becomes text. It can measure it (rows, columns, nesting depth, size of the text) and pick a
renderer, in microseconds. The renderers would be Rust functions there, or Nushell commands
in the prelude (`to agent`) that it calls.

A rule of the kind the studies suggest, to be replaced by what our results say:

| Shape | Format |
|---|---|
| scalar, text | as now |
| list of scalars | one per line |
| table, flat, few rows | a table (D, E or F, whichever wins) |
| table, flat, many rows or wide | Markdown-KV (G), or the table with its header repeated every N rows |
| nested deeper than 2 | YAML (H) or Markdown headings |

How the experiment decides it:

1. From the per-shape results, the best format per shape: highest accuracy, fewest tokens
   among those within its confidence interval.
2. Find the thresholds (how many rows, how deep) where the winner changes, from the
   measures recorded per value. Few, round numbers; a rule an agent can be told in one line.
3. Check the rule on a held-out third of the values: X must be at least as accurate as the
   best single format on every shape. If it is not, one format for all, and no rule.

What speaks against a rule, also to weigh:

- An agent sees different formats for similar values, and a value that grows by a few rows
  changes form between two calls. Each value would say which format it is in, on the line
  above it (`── line 2: git status — table, 120 rows, as key: value`), and `nux help`
  would describe the forms once.
- Code an agent writes from the output (a column name, a path) must not depend on the
  form; the questions of kind "exact copy" check that per format.

## Then, in real use

The winner behind a setting (`NUX_FORMAT=nuon|toon|agent`), the default for one week,
NUON for the week before. From the usage log, per week: share of calls with `str join
"\n"`, `to md`, `to json`, more than one `print`; calls per conversation; errors. The
review's counts are the baseline. Only then change the default and the manual.

## Open questions before starting

- The API key, and whether log contents may be sent to the API at all (they are agents'
  views of our repositories).
- Whether `to grep` (rows of search and ast-find as grep prints them) should be what nux
  prints for such rows by default, without being asked.
- A value that is printed in the new format can no longer be pasted back as Nushell code.
  Agents seldom do that (they use `kept`), but the log can confirm how often a reply's
  NUON shows up in a later call's code.
