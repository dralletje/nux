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

## Formats

| | Format | Why it is in |
|---|---|---|
| A | NUON, as now (`to nuon --raw`, raw strings) | the baseline |
| B | JSON, compact | what models have seen most |
| C | TOON (Token-Oriented Object Notation) | made for this: YAML-like, tables as a header with field names, then one CSV-like row per item |
| D | our own, tables aligned with spaces | see below |
| E | our own, tables with `|` between cells | D with explicit delimiters: answers "are aligned tables easier?" with nothing else different |
| F | Markdown tables (`to md`) | what agents reach for themselves |

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
   values with durations, sizes and dates.
3. Sample ~25 per shape, ~200 values. Leave out secrets: the log holds what agents saw, so
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
- Size: pilot first, 40 values × 4 questions × 6 formats on Sonnet (960 requests), to check
  the questions and the scoring. Then all ~200 values on all three models (~14,400
  requests), cached system prompt. Run with the Batch API: not urgent, half the price.
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
  and less for us to maintain.

## Then, in real use

The winner behind a setting (`NUX_FORMAT=nuon|toon|agent`), the default for one week,
NUON for the week before. From the usage log, per week: share of calls with `str join
"\n"`, `to md`, `to json`, more than one `print`; calls per conversation; errors. The
review's counts are the baseline. Only then change the default and the manual.

## Open questions before starting

- The API key, and whether log contents may be sent to the API at all (they are agents'
  views of our repositories).
- Whether `--text` style output (`search --text`) should follow the same rules.
- A value that is printed in the new format can no longer be pasted back as Nushell code.
  Agents seldom do that (they use `kept`), but the log can confirm how often a reply's
  NUON shows up in a later call's code.
