# TypeScript's diagnostics as a table. `tsc` has no structured output.
#
# There is no command here that runs tsc: how a project is type-checked (which tsc, which
# tsconfig, build mode, code generation first) is the project's own business, and guessing
# it goes wrong. Run what the project uses and read its output with `from tsc`.

# Turn the text `tsc` prints into a table: file, line, column, category, code, message.
#
# Give it the output of whatever runs tsc: a project's own script, `^npx tsc --noEmit`, a
# build. With `--pretty` too (colors, and the code excerpts under each error, are left out).
# A message over several lines is kept whole. Lines that are not diagnostics are skipped
# ("Found 3 errors"). A diagnostic about no file (a problem in tsconfig) has null for file,
# line and column, so filter with `where file != null and file =~ ...`.
@search-terms typescript typecheck "type errors" diagnostics tsc
@example "A project's own check script" { ^pnpm typecheck | complete | get stdout | from tsc }
@example "A project without one" { ^npx tsc --noEmit | complete | get stdout | from tsc }
@example "Errors per file" { ^pnpm typecheck | complete | get stdout | from tsc | group-by file --to-table | each {|g| {file: $g.file, errors: ($g.items | length)} } }
export def "from tsc" []: string -> table {
  # `file(line,col): error TS1: ...`, or with --pretty `file:line:col - error TS1: ...`.
  let in_file = '^(?<file>.+?)[(:](?<line>\d+)[,:](?<column>\d+)\)?:? (?:- )?(?<category>error|warning) (?<code>TS\d+): (?<message>.*)$'
  let global = '^(?<category>error|warning) (?<code>TS\d+): (?<message>.*)$'
  let parsed = ($in | ansi strip | lines | reduce --fold {rows: [], open: false} {|line, state|
    let found = ($line | parse --regex $in_file)
    if ($found | is-not-empty) {
      let row = $found.0
      return {
        rows: ($state.rows | append {file: $row.file, line: ($row.line | into int), column: ($row.column | into int), category: $row.category, code: $row.code, message: $row.message})
        open: true
      }
    }
    let found = ($line | parse --regex $global)
    if ($found | is-not-empty) {
      let row = $found.0
      return {
        rows: ($state.rows | append {file: null, line: null, column: null, category: $row.category, code: $row.code, message: $row.message})
        open: true
      }
    }
    # An indented line right after a diagnostic continues its message. Anything else ends
    # it: a blank line, and with --pretty the code excerpt and the related locations after it.
    if $state.open and ($line =~ '^\s+\S') {
      let last = (($state.rows | length) - 1)
      return {rows: ($state.rows | update $last {|row| $row | update message {|r| $"($r.message)\n($line | str trim)" } }), open: true}
    }
    {rows: $state.rows, open: false}
  })
  $parsed.rows
}
