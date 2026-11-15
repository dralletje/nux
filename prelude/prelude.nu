# Commands that every nux call has. Bundled in the nux binary and loaded at the start of
# every call (src/prelude.rs, src/engine.rs).
# This file has py, js and search; the files next to it the rest.

export use git.nu
export use typescript.nu *
export use browser
export use servers.nu *
export use testing.nu *
export use ast.nu *
export use output.nu *
export use usage.nu *

const PRELUDE_DIR = (path self | path dirname)

# Run code in another language: the input goes to the runner as NUON, its output comes back
# as a value. A failure becomes an error that points at the code.
def run-code [
  language: string # for messages: "Python", "JavaScript"
  program: string  # the interpreter
  runner: string   # file name of the runner next to this file
  code: string
  code_span: record
]: any -> any {
  let input = $in
  # A polars frame cannot be written as NUON (and the error cannot be caught): take its rows.
  let input = if ($input | describe) =~ '^polars_' { $input | polars into-nu } else { $input }
  let run = ($input | to nuon --raw --serialize | run-external $program ($PRELUDE_DIR | path join $runner) $code | complete)
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
# Each call is a new Python process: nothing stays defined in Python between calls.
@example "An expression on the input" { [1 2 3] | py 'sum(data)' }
@example "Several lines go in a raw string" { py r#'
import datetime
datetime.date.today()
'# }
@example "A table in, a table out" { ls | select name size | py '[r for r in data if r["size"] > 1000]' }
export def py [
  code: string # Python code. Its last expression, or the variable `result`, is the value.
]: any -> any {
  $in | run-code Python ($env.NUX_PYTHON? | default "python3") py_runner.py $code (metadata $code).span
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
# Each call is a new Node process: nothing stays defined in JavaScript between calls.
@example "An expression on the input" { [1 2 3] | js 'data.reduce((a, b) => a + b, 0)' }
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
  code: string # JavaScript code. Its last expression (or `return`, with await) is the value.
]: any -> any {
  $in | run-code JavaScript ($env.NUX_NODE? | default "node") js_runner.mjs $code (metadata $code).span
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
  ...paths: string    # Files or directories to search (default: the current directory)
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
