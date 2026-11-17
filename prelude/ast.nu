# Find and rewrite code by its structure, not its text (ast-grep).
#
# Two steps, so that what gets changed can be looked at and narrowed down in between:
#
#     let found = (ast-find --lang ts 'console.log($$$ARGS)' src --rewrite 'logger.debug($$$ARGS)')
#     $found | select file line text replacement          # look
#     $found | where file !~ 'test' | ast-apply           # change only what is left

# Run ast-grep and give its matches; no match is an empty list.
def run-ast-grep [arguments: list<string>]: nothing -> list<any> {
  if (which ast-grep | is-empty) {
    error make {msg: "ast-find needs `ast-grep`, which is not installed", help: "It is the `ast-grep` binary from https://github.com/ast-grep/ast-grep/releases, on PATH."}
  }
  let run = (^ast-grep run ...$arguments --json=stream | complete)
  # ast-grep exits with 1 when nothing matched, and with more for a real problem (an unknown
  # language, a path that does not exist), which it describes on stderr.
  if $run.exit_code > 1 {
    error make {msg: "ast-grep failed", help: ($run.stderr | str trim)}
  }
  $run.stdout | from json --objects
}

# What is wrong with a pattern as code of a language, or null.
#
# ast-grep searches with a pattern that does not parse anyway, finds nothing (or something
# unexpected) and exits normally, so a broken pattern would look like "no matches". Two
# signs, both from ast-grep itself, on empty input so that it is only about the pattern:
#   - its warning that the pattern "contains an ERROR node": the parser could not make
#     sense of part of it (`foo(`, JSX in plain ts). Only when the search found NOTHING
#     (`found_nothing`): a hole where the grammar wants a literal, `import $X from $SRC`
#     (a string after `from`), also gives that warning, yet finds exactly what it should.
#     Broken patterns (`foo(`, `console.log($A`) find nothing, so they are still caught;
#   - a closing bracket or quote that the parser had to make up to go on (`def f(:`,
#     `if ($A) {`), which it shows as MISSING in `--debug-query=cst`. Only those count: a
#     made-up `;` at the end is how a short pattern such as `$A.unwrap()` is read in Rust,
#     Java or C, and is fine.
# What cannot be told apart from a valid pattern: other things a parser quietly repairs.
def pattern-problem [pattern: string, lang: string, found_nothing: bool]: nothing -> any {
  if $found_nothing and ("" | ^ast-grep run --pattern $pattern --lang $lang --stdin | complete | get stderr | str contains "Pattern contains an ERROR node") {
    return $"it does not parse as ($lang)"
  }
  let made_up = (
    "" | ^ast-grep run --pattern $pattern --lang $lang --debug-query=cst --stdin | complete | get stderr | lines
    | parse --regex '^\s*MISSING (?<token>\S+) \(' | get token | where $it in [")" "]" "}" '"' "'" "`" ">"] | uniq
  )
  if ($made_up | is-not-empty) { return $"it is not complete: no closing ($made_up | str join ' ') was found" }
  null
}

# The holes of a pattern (`$A`, `$$$ARGS`, `$_`, `$$$`), in order, with the text before each:
# {before, hole, name}. Names as ast-grep reads them: `$` then capitals, digits and `_`.
def holes-of [pattern: string]: nothing -> table {
  $pattern | parse --regex '(?s)(?<before>.*?)(?<hole>\$\$\$[A-Z0-9_]*|\$\$?[A-Z_][A-Z0-9_]*)'
  | insert name {|piece| $piece.hole | str trim --left --char '$' }
}

# Prefix of the names nux gives anonymous holes (`$_`, `$_X`, `$$$`), so that ast-grep reports
# where they matched too. Left out of `vars`.
const ANONYMOUS = "NUXANON"

# The pattern with every anonymous hole named: `$_` -> `$NUXANON1`. Each gets a name of its
# own, so they still match independently, as anonymous holes do.
def name-anonymous-holes [pattern: string]: nothing -> string {
  let pieces = (holes-of $pattern)
  if ($pieces | is-empty) { return $pattern }
  let named = ($pieces | enumerate | each {|piece|
    let p = $piece.item
    let anonymous = ($p.name == "" or ($p.name | str starts-with "_"))
    let hole = if $anonymous {
      let dollars = ($p.hole | str replace --regex '[^$].*$' '')
      $"($dollars)($ANONYMOUS)($piece.index)"
    } else { $p.hole }
    $"($p.before)($hole)"
  } | str join)
  let used = ($pieces | each {|p| $p.before + $p.hole } | str join | str length)
  $named + ($pattern | str substring $used..)
}

# Where, in a match, the code the pattern itself names ends: {end, trailing}, `end` a byte
# offset in the file, `trailing` the rest of the match after it ("" when there is none).
# Null when that cannot be told (the pattern's text after its last hole is not found there).
#
# Why: a pattern that is a statement, such as `import { $$$A } from "./x"` or `return $X`,
# matches the whole statement, `;` included, although the pattern has no `;`; ast-grep's
# replacement then takes the place of all of it, and the `;` is gone. See
# tests/ast-rewrite/README.md.
def pattern-end [pattern: string, match: record]: nothing -> any {
  let start = $match.range.byteOffset.start
  let text = ($match.text | into binary)
  let found = ($match.metaVariables? | default {})
  # The hole the pattern ends with, and where it ended in the match. A hole for many that
  # matched nothing adds nothing: then the one before it, and so on.
  mut rest_of_pattern = $pattern
  mut pieces = (holes-of $pattern)
  mut anchor = $start
  mut tail = $pattern
  while ($pieces | is-not-empty) {
    let last = ($pieces | last)
    let single = ($found.single? | default {} | get --optional $last.name)
    let multi = ($found.multi? | default {} | get --optional $last.name | default [])
    # Bytes of the pattern up to the end of this hole.
    let used = ($pieces | each {|p| $p.before + $p.hole } | str join | str length)
    if $single != null {
      $anchor = $single.range.byteOffset.end
      $tail = ($rest_of_pattern | str substring $used..)
      break
    }
    if ($multi | is-not-empty) {
      $anchor = ($multi | last | get range.byteOffset.end)
      $tail = ($rest_of_pattern | str substring $used..)
      break
    }
    if ($last.hole | str starts-with '$$$') {
      # Matched nothing: as if the pattern did not have it.
      $rest_of_pattern = ($rest_of_pattern | str substring 0..<($used - ($last.hole | str length))) + ($rest_of_pattern | str substring $used..)
      $pieces = ($pieces | drop 1)
      continue
    }
    return null
  }
  # Without holes left, the tail is the whole pattern, from the start of the match.
  if ($pieces | is-empty) {
    $anchor = $start
    $tail = $rest_of_pattern
  }
  # Walk the tail's characters through the match, whitespace aside.
  mut rest = ($text | bytes at ($anchor - $start).. | decode utf-8)
  for char in ($tail | split chars --grapheme-clusters | where $it !~ '^\s$') {
    $rest = ($rest | str trim --left)
    if not ($rest | str starts-with $char) { return null }
    $rest = ($rest | str substring --grapheme-clusters 1..)
  }
  {end: ($match.range.byteOffset.end - ($rest | into binary | bytes length)), trailing: $rest}
}

# Find code that has a certain shape, as a table: file, line, column, text, vars, language,
# start, end (and replacement and trailing, with --rewrite).
#
# The pattern is code in the language you search, with holes: `$NAME` stands for any one
# expression, statement or name; `$$$NAME` for any number of them (arguments, statements);
# `$_` for one you do not care about. `console.log($MSG)` finds calls with one argument,
# whatever it is and however it is laid out; `console.log($$$ARGS)` with any number. Text
# inside strings and comments does not match, unlike a text search.
#
# `vars` has what each hole matched: `$found | where vars.MSG =~ 'password'`. `start` and
# `end` are positions in the file, for `ast-apply`.
#
# With --rewrite, each row also gets `replacement`: what the match would become. Nothing is
# changed yet. Look, filter (`where`), adjust (`update replacement {...}`), and then pipe the
# rows you want into `ast-apply`.
#
# Only what the pattern names is replaced. A pattern that is a statement without its `;`
# (`return $X`, `import { $$$A } from "./x"`, `let $X = $Y` in Rust) matches the statement
# with its `;`; that `;` (and a comment before it) is then not part of `text`, stays in the
# file, and is in the column `trailing` (null when there is nothing after the pattern). It
# is replaced after all when the rewrite ends in it itself (`--rewrite 'return $X;'`), and a
# rewrite to '' deletes the whole statement, `;` included.
#
# --lang is required: a pattern is code in one language, and only files of that language
# are searched (`ts` does not cover .tsx files: search those with `--lang tsx`). Files in
# .gitignore are skipped. A pattern that is not valid code in that language (`foo(` instead
# of `foo($A)`, JSX with `--lang ts`) is an error, not "no matches".
@search-terms ast-grep sg "structural search" codemod refactor "find code" "find calls"
@example "Calls of a function, with what is passed" { ast-find --lang ts 'fetch($URL, $$$REST)' src | select file line vars }
@example "Narrow down by what a hole matched" { ast-find -l ts 'console.log($MSG)' | where vars.MSG =~ 'token' | select file line text }
@example "Preview a rewrite" {
  ast-find -l js 'assert.equal($A, $B)' test --rewrite 'assert.strictEqual($A, $B)'
  | select file line text replacement
}
@example "JSX, in some files only" { ast-find -l tsx '<Button $$$PROPS />' --glob '!*.test.tsx' | get file | uniq }
@example "Python" { ast-find -l python 'print($$$ARGS)' scripts }
export def ast-find [
  pattern: string        # Code with holes: $NAME (one thing), $$$NAME (any number), $_
  ...paths: path         # Files or directories to search (default: the current directory)
  --lang (-l): string    # Required. The pattern's language: ts, tsx, js, python, rust, go, css, html, json, yaml, java, ...
  --rewrite (-r): string # What a match should become; may use the pattern's holes
  --glob (-g): string    # Only files matching this glob, e.g. '*.tsx' or '!*.test.ts'
]: nothing -> table {
  if $lang == null {
    error make {
      msg: "ast-find needs --lang: the language the pattern is written in"
      help: "For example: ast-find --lang ts 'console.log($A)' src. Common ones: ts, tsx (for JSX), js, python, rust, go, css, html, json, yaml, java, kotlin, ruby, php, c, cpp, csharp, swift, bash."
      label: {text: "which language is this?", span: (metadata $pattern).span}
    }
  }
  # With --rewrite, anonymous holes get names, so that where the pattern ends can be told.
  let searched = if $rewrite != null { name-anonymous-holes $pattern } else { $pattern }
  mut arguments = [--pattern $searched --lang $lang]
  if $rewrite != null { $arguments ++= [--rewrite $rewrite] }
  if $glob != null { $arguments ++= [--globs $glob] }
  let places = if ($paths | is-empty) { ["."] } else { $paths }
  let missing = ($places | where not ($it | path exists))
  if ($missing | is-not-empty) {
    error make {msg: $"No such file or directory: ($missing | str join ', ')", help: $"Looked from (pwd)."}
  }
  let with_rewrite = ($rewrite != null)
  let matches = (run-ast-grep ($arguments ++ $places))
  let problem = (pattern-problem $pattern $lang ($matches | is-empty))
  if $problem != null {
    error make {
      msg: $"The pattern cannot be used: ($problem)"
      help: $"A pattern has to be a complete expression or statement of the language, with holes: `foo\($A\)`, not `foo\(`. For JSX use --lang tsx. To see how ast-grep reads it: ^ast-grep run --pattern '...' --lang ($lang) --debug-query=cst ."
      label: {text: "this pattern", span: (metadata $pattern).span}
    }
  }
  $matches | each {|match|
    # A hole for one thing gives its text; one for many, the texts (without the commas between).
    # A pattern without holes has no metaVariables at all (ast-grep 0.51 leaves the key out).
    let holes = ($match.metaVariables? | default {})
    let single = ($holes.single? | default {} | transpose name node | each {|var| {name: $var.name, value: $var.node.text} })
    let multi = ($holes.multi? | default {} | transpose name nodes | each {|var| {name: $var.name, value: ($var.nodes | get text | where $it !~ '^[,;]$')} })
    let vars = (
      $single | append $multi
      | where name !~ $'^($ANONYMOUS)'
      | reduce --fold {} {|var, all| $all | insert $var.name $var.value }
    )
    let start = $match.range.byteOffset.start
    # Only what the pattern names is replaced: what the match has after that (the `;` of a
    # statement the pattern wrote without one) stays, as `trailing`. Not when the replacement
    # deletes the match (a deleted statement goes with its `;`), nor when it ends in that
    # text itself (the rewrite wrote the `;`: it is replaced along).
    let cut = if $with_rewrite { pattern-end $searched $match }
    let keep = (
      $cut != null
      and ($cut.trailing | str trim) != ""
      and ($match.replacement | str trim) != ""
      and not ($match.replacement | str trim --right | str ends-with ($cut.trailing | str trim))
    )
    let text = if $keep {
      $match.text | into binary | bytes at 0..<($cut.end - $start) | decode utf-8
    } else { $match.text }
    let row = {
      file: ($match.file | str replace --regex '^\./' '')
      line: ($match.range.start.line + 1)
      column: ($match.range.start.column + 1)
      text: $text
      vars: $vars
    }
    let row = if $with_rewrite {
      $row | insert replacement $match.replacement | insert trailing (if $keep { $cut.trailing })
    } else { $row }
    $row | merge {language: $match.language, start: $start, end: (if $keep { $cut.end } else { $match.range.byteOffset.end })}
  }
}

# Change the code that `ast-find --rewrite` found: each row's `text` becomes its `replacement`.
#
# Give it the rows you want changed, from `ast-find --rewrite`, after any filtering. The
# `replacement` column may have been changed in between (`update replacement {...}`), so a
# replacement can be computed. Gives one row per changed file: file, replacements.
#
# Nothing is written unless everything can be: it is an error, and no file is touched, when
# a file no longer has a row's `text` at that place (it changed since the search: search
# again), or when two rows overlap (one match inside another: filter one out).
@search-terms ast-grep "structural replace" codemod refactor rewrite "apply rewrite"
@example "Find, look, then change" {
  let found = (ast-find -l ts 'console.log($$$A)' src --rewrite 'logger.debug($$$A)')
  $found | select file line text replacement
  $found | ast-apply
}
@example "Change only some of the matches" { ast-find -l ts 'foo($A)' --rewrite 'bar($A)' | where file !~ '\.test\.' | ast-apply }
@example "A replacement computed in Nushell" {
  ast-find -l ts 't($KEY)' --rewrite 't($KEY)'
  | update replacement {|row|
      $"t\(($row.vars.KEY | str lowercase)\)"
    }
  | ast-apply
}
export def ast-apply []: table -> table {
  let rows = $in
  if ($rows | is-empty) { return [] }
  let missing = ([file start end text replacement] | where $it not-in ($rows | columns))
  if ($missing | is-not-empty) {
    error make {msg: $"ast-apply needs rows from `ast-find --rewrite`; missing: ($missing | str join ', ')", help: "Keep the columns file, start, end, text and replacement when you filter (use `where`, not `select`)."}
  }
  # First work out every file's new content, then write: all files or none.
  let changed = (
    $rows | group-by file --to-table | each {|group|
      let original = (open --raw $group.file | into binary)
      let edits = ($group.items | sort-by start)
      mut pieces = []
      mut position = 0
      for edit in $edits {
        if $edit.start < $position {
          error make {msg: $"Two matches overlap in ($group.file), at line ($edit.line)", help: "One match lies inside another. Filter one of them out, apply, and search again for the rest."}
        }
        let current = ($original | bytes at $edit.start..<$edit.end | decode utf-8)
        if $current != $edit.text {
          error make {msg: $"($group.file) changed since the search, at line ($edit.line)", help: $"Expected:\n($edit.text)\nFound:\n($current)\nSearch again. Nothing was written.\nAfter an earlier ast-apply on the same files: apply the rows of both searches at once \(`$a | append $b | ast-apply`\), or run the second search after the first apply."}
        }
        $pieces = ($pieces | append [($original | bytes at $position..<$edit.start) ($edit.replacement | into binary)])
        $position = $edit.end
      }
      let content = ($pieces | append ($original | bytes at $position..) | bytes collect)
      {file: $group.file, replacements: ($edits | length), content: $content}
    }
  )
  for file in $changed { $file.content | save --force --raw $file.file }
  $changed | select file replacements
}
