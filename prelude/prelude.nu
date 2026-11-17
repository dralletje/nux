# Commands that every nux call has. Bundled in the nux binary and loaded from it at the start
# of every call, as the virtual file nux/prelude.nu (src/prelude.rs, src/engine.rs).
# This file has py, js and search; the files next to it the rest.

# Modules by the name they are loaded with from the binary, `nux/<file name without .nu>`,
# also from each other: a relative path (`git.nu`, `../other.nu`) is not found there.
export use nux/git
export use nux/typescript *
export use nux/servers *
export use nux/testing *
export use nux/ast *
export use nux/output *
export use nux/usage *
export use nux/edit-file
# Not loaded here, only with `use` (listed in `nux help`): nux/capped, nux/cookie-jar.

# The folder the runners of py and js are written to (src/prelude.rs).
use nux/runners RUNNERS

# Run code in another language: the input goes to the runner as NUON, its output comes back
# as a value. A failure becomes an error that points at the code.
def run-code [
  language: string # for messages: "Python", "JavaScript"
  program: string  # the interpreter
  runner: string   # file name of the runner next to this file
  code: string
  code_span: record
  with: record     # more values, each defined in the code under its key
]: any -> any {
  let input = $in
  # A polars frame cannot be written as NUON (and the error cannot be caught): take its rows.
  let as_rows = {|value|
    if ($value | describe) =~ '^polars_' {
      $value | polars into-nu
    } else {
      $value
    }
  }
  let input = (do $as_rows $input)
  let with = ($with | items {|name, value| {$name: (do $as_rows $value)} } | reduce --fold {} {|it, all| $all | merge $it })
  # The input and the values of --with go to the runner together, as one record.
  let sent = ({data: $input, with: $with} | to nuon --raw --serialize)
  let run = ($sent | run-external $program ($RUNNERS | path join $runner) $code | complete)
  if $run.exit_code != 0 {
    let lines = ($run.stderr | str trim | lines)
    error make {
      msg: $"($language): ($lines | last | default 'failed without a message')"
      help: ($lines | drop 1 | str join "\n")
      label: {text: $"in this ($language) code", span: $code_span}
    }
  }
  if ($run.stderr | is-not-empty) { print -e ($run.stderr | str trim --right) }
  $run.stdout | from nuon
}

# Run Python code on the pipeline input and get a Nushell value back.
#
# The input is available in Python as `data`. The value of the last expression is the
# result; when the code ends in a statement, the variable `result` is. Values keep their type
# in both directions: datetime <-> datetime, duration <-> timedelta, binary <-> bytes,
# record <-> dict, table <-> list of dicts, filesize <-> Filesize (an int, in bytes). Python
# keeps microseconds, so nanoseconds are cut off. Also returnable: numpy arrays, pandas
# DataFrames. A polars dataframe as input is converted to rows first.
# What the code prints is shown, and is not part of the result.
#
# With --with {name: value}, each key is also defined in the code under that name, with the
# value's type, so a value need not be put into the code's text.
#
# Each call is a new Python process: nothing stays defined in Python between calls.
@example "An expression on the input" { [1 2 3] | py 'sum(data)' }
@example "Values by name" { let limit = 1kb; ls | py --with {limit: $limit} '[f["name"] for f in data if f["size"] > limit]' }
@example "Several lines go in a raw string" { py r#'
import datetime
datetime.date.today()
'# }
@example "A table in, a table out" { ls | select name size | py '[r for r in data if r["size"] > 1000]' }
export def py [
  code: string         # Python code. Its last expression, or the variable `result`, is the value.
  --with: record = {}  # Values to define in the code, each under its key: {limit: $limit}
]: any -> any {
  $in | run-code Python ($env.NUX_PYTHON? | default "python3") py_runner.py $code (metadata $code).span $with
}

# Run JavaScript code (Node) on the pipeline input and get a Nushell value back.
#
# The input is available as `data`. The value of the last expression is the result; a
# promise is awaited. Code that uses `await` at the top level runs as the body of an async
# function instead, and gives its result with `return`. `require(...)` and `import(...)`
# find packages from the working directory, so a project's own node_modules work.
# Values keep their type in both directions: binary <-> Buffer, record <-> object,
# table <-> array of objects, datetime <-> Temporal.ZonedDateTime, duration <->
# Temporal.Duration, filesize <-> Nu.FileSize. Dates and durations are exact (nanoseconds,
# and a datetime keeps its UTC offset), but Temporal values do not work with `<`, `-` or
# `+`: use `.epochMilliseconds`, `Temporal.ZonedDateTime.compare(a, b)`, `a.since(b)`,
# `duration.total("seconds")`. A Date can be returned too. A file size has `.bytes`, `.kb`,
# `.mb`, `.gb` (and `.kib`, `.mib`, `.gib`); make one with `Nu.FileSize(1500)`, in bytes.
# Sizes compare and sort (`a.size > b.size`), but arithmetic on one gives a plain number,
# and like any object a size of 0 is truthy: test `.bytes`. A whole number comes back as an
# int, also when it was a float going in. Needs Node 26 or newer.
# What the code logs with console.log is shown, and is not part of the result.
#
# With --with {name: value}, each key is also defined in the code under that name, with the
# value's type, so a value need not be put into the code's text.
#
# Each call is a new Node process: nothing stays defined in JavaScript between calls.
@example "An expression on the input" { [1 2 3] | js 'data.reduce((a, b) => a + b, 0)' }
@example "Values by name" { let limit = 1kb; ls | js --with {limit: $limit} 'data.filter((f) => f.size.bytes > limit.bytes).map((f) => f.name)' }
@example "A package of the project, several lines in a raw string" { open config.yaml --raw | js r#'
const { parse } = require("yaml");
parse(data)
'# }
@example "File sizes and durations" { ls | js 'data.filter((file) => file.size.kb > 10).map((file) => ({name: file.name, mb: file.size.mb}))' }
@example "With await, the result is returned" { js r#'
const response = await fetch("https://example.com");
return response.status
'# }
export def js [
  code: string         # JavaScript code. Its last expression (or `return`, with await) is the value.
  --with: record = {}  # Values to define in the code, each under its key: {limit: $limit}
]: any -> any {
  $in | run-code JavaScript ($env.NUX_NODE? | default "node") js_runner.mjs $code (metadata $code).span $with
}

# Search file contents with ripgrep and get the matches as a table.
#
# One row per matching line: file, line, text. Files listed in .gitignore, hidden files and
# binary files are skipped (--all includes the first two). Lines longer than 300 characters
# are cut. No matches is an empty list, not an error.
@search-terms grep rg ripgrep "find in files"
@example "A regular expression, everywhere below the current directory" { search 'export const \w+' }
@example "Plain text in some files only" { search --literal 'foo(' src --glob '*.ts' }
@example "Which files mention it, and how often" { search TODO --files | sort-by matches --reverse }
@example "With two lines around each match (the column `hit` is true for the matching lines)" { search panic --context 2 }
export def search [
  pattern: string     # Regular expression (Rust syntax); plain text with --literal
  ...paths: path      # Files or directories to search (default: the current directory)
  --literal (-F)      # Take the pattern as plain text
  --ignore-case (-i)  # Do not tell upper and lower case apart
  --glob (-g): string # Only files matching this glob, e.g. '*.ts' or '!*.test.ts'
  --context (-C): int # Also give this many lines before and after each match
  --files (-l)        # One row per file instead: file, matches
  --all (-a)          # Also search hidden files and files that .gitignore excludes
]: nothing -> table {
  if (which rg | is-empty) {
    error make {msg: "search needs ripgrep (`rg`), which is not installed"}
  }
  mut options = [--json]
  if $literal { $options ++= [--fixed-strings] }
  if $ignore_case { $options ++= [--ignore-case] }
  if $glob != null { $options ++= [--glob $glob] }
  if $context != null { $options ++= [--context ($context | into string)] }
  if $all { $options ++= [--hidden --no-ignore] }
  # Without a path ripgrep would read stdin here, so name the directory.
  let places = if ($paths | is-empty) { ["."] } else { $paths }
  let run = (^rg ...$options --regexp $pattern -- ...$places | complete)
  # ripgrep exits with 1 when nothing matched, with 2 when something went wrong.
  if $run.exit_code > 1 {
    error make {
      msg: "search failed"
      help: ($run.stderr | str trim)
      label: {text: "searching for this", span: (metadata $pattern).span}
    }
  }
  let rows = (
    $run.stdout | from json --objects | where type in ["match" "context"] | each {|event|
      let text = ($event.data.lines.text? | default "(not text)" | str trim --right --char "\n" | str trim --right --char "\r")
      {
        file: ($event.data.path.text? | default "(unreadable name)" | str replace --regex '^\./' '')
        line: $event.data.line_number
        text: (if ($text | str length) > 300 { ($text | str substring 0..<300) + "…" } else { $text })
        hit: ($event.type == "match")
      }
    }
  )
  if $files {
    $rows | where hit | group-by file --to-table | each {|group| {file: $group.file, matches: ($group.items | length)} }
  } else if $context == null {
    $rows | reject hit
  } else {
    $rows
  }
}

# The old name of `slice`, which agents still write: it works, and says so on stderr.
@search-terms slice
@example "Rows 2 to 4" { [a b c d e] | range 1..3 }
export def range [
  rows: range # the rows to give, by index from 0: 1..3, 5.., ..<10
]: any -> any {
  print -e "range: this is `slice` in Nushell now; write `slice`"
  $in | slice $rows
}
