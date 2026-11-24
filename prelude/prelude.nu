# Commands that every nux call has. Bundled in the nux binary and loaded from it at the start
# of every call, as the virtual file nux/prelude.nu (src/prelude.rs, src/engine.rs).
# This file has py, js and search; the files next to it the rest.

# Modules by the name they are loaded with from the binary, `nux/<file name without .nu>`,
# also from each other: a relative path (`git.nu`, `../other.nu`) is not found there.
export use nux/git
export use nux/typescript *
export use nux/servers *
export use nux/timeout *
export use nux/test-format *
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
# One row per matching line: file, line, text. Line numbers are always there, and folders
# are always searched through. Files listed in .gitignore, hidden files and binary files are
# skipped (--all includes the first two). Lines longer than 300 characters are cut. No
# matches is an empty list, not an error.
#
# With lines around the matches (--context, --before-context, --after-context), the rows
# also have `hit`: true for a matching line, false for one around it. To read the rows as
# grep prints them, after any filtering: `| to grep`. A path with `*` in it is a glob
# (`search foo src/*.ts`).
#
# grep's -n and -r are taken and change nothing: line numbers and folders are always there.
# For grep's other flags there is a way here already: -c and --files-with-matches are
# --files; --include and --exclude are --glob ('*.ts', '!*.test.ts'); -e a b is one pattern
# 'a|b'; -w is '\bword\b'; -v is `where` on the lines (open --raw f | lines | where $it !~ x).
@search-terms grep rg ripgrep "find in files"
@example "A regular expression, everywhere below the current directory" { search 'export const \w+' }
@example "Plain text in some files only" { search --literal 'foo(' src --glob '*.ts' }
@example "Which files mention it, and how often" { search TODO --files | sort-by matches --reverse }
@example "With two lines around each match (the column `hit` is true for the matching lines)" { search panic --context 2 }
@example "To read, as grep prints it" { search 'fn main' src --after-context 3 | where file !~ test | to grep }
@example "Only what matched, not the whole line" { search 'TS\d{4}' build.log --only-matching | get text | uniq --count }
@example "In text you have, not a file (`file` is \"(input)\")" { ^git show HEAD~3:src/router.ts | search 'createAccount' --context 5 | to grep }
export def search [
  pattern: string              # Regular expression (Rust syntax); plain text with --literal
  ...paths: path               # Files or directories to search (default: the current directory); `*` globs
  --literal (-F)               # Take the pattern as plain text
  --ignore-case (-i)           # Do not tell upper and lower case apart
  --glob (-g): string          # Only files matching this glob, e.g. '*.ts' or '!*.test.ts'
  --context (-C): int          # Also give this many lines before and after each match
  --before-context (-B): int   # Also give this many lines before each match
  --after-context (-A): int    # Also give this many lines after each match
  --only-matching (-o)         # One row per match, `text` only what matched (not with lines around)
  --files (-l)                 # One row per file instead: file, matches (grep's -c and -l)
  --all (-a)                   # Also search hidden files and files that .gitignore excludes
  --line-number (-n)           # Changes nothing: `line` is always there (grep's -n)
  --recursive (-r)             # Changes nothing: folders are always searched through (grep's -r)
]: [nothing -> table, string -> table] {
  # Text piped in is searched instead of files (`file` is "(input)").
  let input = $in
  if (which rg | is-empty) {
    error make {msg: "search needs ripgrep (`rg`), which is not installed"}
  }
  let around = ($context != null or $before_context != null or $after_context != null)
  if $only_matching and $around {
    error make {msg: "search: --only-matching gives what matched, without lines around it", help: "Leave out --context, --before-context and --after-context, or --only-matching."}
  }
  mut options = [--json]
  if $literal { $options ++= [--fixed-strings] }
  if $ignore_case { $options ++= [--ignore-case] }
  if $glob != null { $options ++= [--glob $glob] }
  if $context != null { $options ++= [--context ($context | into string)] }
  if $before_context != null { $options ++= [--before-context ($before_context | into string)] }
  if $after_context != null { $options ++= [--after-context ($after_context | into string)] }
  if $all { $options ++= [--hidden --no-ignore] }
  # Without a path ripgrep would read stdin here, so name the directory. A path that does
  # not exist but has glob characters is a glob (`src/*.ts`): ripgrep takes no globs there.
  let places = if ($paths | is-empty) { ["."] } else {
    $paths | each {|place|
      if ($place | path exists) or ($place !~ '[*?\[{]') {
        [$place]
      } else {
        let found = (glob $place)
        if ($found | is-empty) {
          error make {msg: $"search: nothing matches ($place)", help: $"Looked from (pwd). For a kind of file everywhere below a folder: --glob '*.ts'."}
        }
        # Relative as given, so that `file` is too.
        if not ($place | str starts-with '/') {
          $found | path relative-to (pwd)
        } else {
          $found
        }
      }
    } | flatten
  }
  let from_input = ($input != null and ($paths | is-empty))
  let run = if $from_input {
    $input | ^rg ...$options --regexp $pattern -- - | complete
  } else {
    ^rg ...$options --regexp $pattern -- ...$places | complete
  }
  # ripgrep exits with 1 when nothing matched, with 2 when something went wrong.
  if $run.exit_code > 1 {
    error make {
      msg: "search failed"
      help: ($run.stderr | str trim)
      label: {text: "searching for this", span: (metadata $pattern).span}
    }
  }
  let cut = {|text| if ($text | str length --grapheme-clusters) > 300 { ($text | str substring --grapheme-clusters 0..<300) + "…" } else { $text } }
  let rows = (
    $run.stdout | from json --objects | where type in ["match" "context"] | each {|event|
      let file = if $from_input { "(input)" } else { $event.data.path.text? | default "(unreadable name)" | str replace --regex '^\./' '' }
      if $only_matching {
        # One row per match on the line.
        $event.data.submatches | each {|sub|
          {file: $file, line: $event.data.line_number, text: (do $cut ($sub.match.text? | default "(not text)")), hit: true}
        }
      } else {
        let text = ($event.data.lines.text? | default "(not text)" | str trim --right --char "\n" | str trim --right --char "\r")
        [{file: $file, line: $event.data.line_number, text: (do $cut $text), hit: ($event.type == "match")}]
      }
    } | flatten
  )
  if $files {
    $rows | where hit | group-by file --to-table | each {|group| {file: $group.file, matches: ($group.items | length)} }
  } else if not $around {
    $rows | reject hit
  } else {
    $rows
  }
}

# Places in files as grep prints them: one text to read, from rows with file, line and text
# (what `search` and `ast-find` give).
#
# `file:line:text` for each line. With a column `hit` (from search with --context and the
# like), a line around a match gets `-` instead: `file-line-text`, and `--` stands between
# places that do not follow each other. A text over several lines (an ast-find match) gives
# one line each, numbered on from its first.
#
# For git's changes, read the diff itself: `git diff | git hunks`.
@search-terms grep "read results" "search results as text" lines
@example "Matches with a line around them, after filtering" { search 'TODO' src --context 1 | where file !~ test | to grep }
@example "Where a pattern is used" { ast-find --lang ts 'useEffect($$$A)' src | to grep }
export def "to grep" []: table -> string {
  let rows = $in
  if ($rows | is-empty) { return "" }
  let missing = ([file line text] | where $it not-in ($rows | columns))
  if ($missing | is-not-empty) {
    error make {
      msg: $"to grep needs the columns file, line and text; missing: ($missing | str join ', ')"
      help: "Rows from search or ast-find have them. For git's changes, the diff itself: git diff | git hunks."
    }
  }
  let around = ("hit" in ($rows | columns))
  mut lines = []
  mut last_file = ""
  mut last_line = -1
  for row in $rows {
    let hit = if $around { $row.hit } else { true }
    let mark = if $hit { ":" } else { "-" }
    if ($lines | is-not-empty) and $around and ($last_file != $row.file or $last_line + 1 != $row.line) {
      $lines ++= ["--"]
    }
    mut number = $row.line
    for text in ($row.text | into string | lines) {
      $lines ++= [$"($row.file)($mark)($number)($mark)($text)"]
      $number += 1
    }
    $last_file = $row.file
    $last_line = $number - 1
  }
  $lines | str join "\n"
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
