# TypeScript's diagnostics as a table. `tsc` has no structured output.
#
# There is no command here that runs tsc: how a project is type-checked (which tsc, which
# tsconfig, build mode, code generation first) is the project's own business, and guessing
# it goes wrong. Run what the project uses and read its output with `from tsc`.

use nux/util [command-output not-the-format]

# Turn what `tsc` prints into a table: file, line, column, category, code, message.
#
# Give it the record of `complete` from whatever runs tsc: a project's own script,
# `^npx tsc --noEmit`, a build. Both stdout and stderr are read (a script may send tsc's
# output to either), and a command that failed without printing any diagnostic is an error
# that shows what it printed instead: no rows then means tsc found nothing, never that it did
# not run. Plain text works too (output kept in a variable or a file), without that check.
#
# With `--pretty` too (colors, and the code excerpts under each error, are left out). A
# message over several lines is kept whole. Lines that are not diagnostics are skipped
# ("Found 3 errors"). A diagnostic about no file (a problem in tsconfig) has null for file,
# line and column, so filter with `where file != null and file =~ ...`. From `pnpm -r`, whose
# lines start with the package's folder and the script (`packages/a typecheck: src/b.ts(1,2):
# ...`), `file` is the path from where pnpm ran: `packages/a/src/b.ts`.
@search-terms typescript typecheck "type errors" diagnostics tsc
@example "A project's own check script" { ^pnpm typecheck | complete | from tsc }
@example "A project without one" { ^npx tsc --noEmit | complete | from tsc }
@example "Errors per file" {
  ^pnpm typecheck
  | complete
  | from tsc
  | group-by file --to-table
  | each {|group|
      {file: $group.file, errors: ($group.items | length)}
    }
}
export def "from tsc" []: any -> table {
  let source = ($in | command-output)
  # `file(line,col): error TS1: ...`, or with --pretty `file:line:col - error TS1: ...`.
  let in_file = '^(?<file>.+?)[(:](?<line>\d+)[,:](?<column>\d+)\)?:? (?:- )?(?<category>error|warning) (?<code>TS\d+): (?<message>.*)$'
  let global = '^(?<category>error|warning) (?<code>TS\d+): (?<message>.*)$'
  # What `pnpm -r` puts before each line of a package's output: its folder and the script.
  let prefixed = '^(?<prefix>\S+) (?<script>[\w:.-]+): (?<rest>.*)$'
  let diagnostic = {|line|
    let found = ($line | parse --regex $in_file)
    if ($found | is-not-empty) {
      let row = $found.0
      return {file: $row.file, line: ($row.line | into int), column: ($row.column | into int), category: $row.category, code: $row.code, message: $row.message}
    }
    let found = ($line | parse --regex $global)
    if ($found | is-not-empty) {
      let row = $found.0
      return {file: null, line: null, column: null, category: $row.category, code: $row.code, message: $row.message}
    }
    null
  }
  let lines = ($source.text | ansi strip | lines | append ($source.stderr | ansi strip | lines))
  let parsed = ($lines | reduce --fold {rows: [], open: false, prefix: null} {|line, state|
    # A line of pnpm -r: the diagnostic after the prefix, its file inside the package folder.
    # (Tried first only when the line as a whole is no diagnostic, or one whose "file" has
    # the prefix in it.)
    let plain = (do $diagnostic $line)
    let split = ($line | parse --regex $prefixed | get 0?)
    let inner = if $split != null and ($plain == null or ($plain.file? | default "" | str contains ": ")) {
      do $diagnostic $split.rest
    }
    if $inner != null {
      let row = if $inner.file != null { $inner | update file ($split.prefix | path join $inner.file) } else { $inner }
      return {rows: ($state.rows | append $row), open: true, prefix: $split.prefix}
    }
    if $plain != null {
      return {rows: ($state.rows | append $plain), open: true, prefix: null}
    }
    # An indented line right after a diagnostic continues its message (after the same pnpm
    # prefix, for one of pnpm -r). Anything else ends it: a blank line, and with --pretty the
    # code excerpt and the related locations after it.
    let body = if $state.prefix != null and $split != null and $split.prefix == $state.prefix { $split.rest } else { $line }
    if $state.open and ($body =~ '^\s+\S') {
      let last = (($state.rows | length) - 1)
      return {rows: ($state.rows | update $last {|row| $row | update message {|r| $"($r.message)\n($body | str trim)" } }), open: true, prefix: $state.prefix}
    }
    {rows: $state.rows, open: false, prefix: $state.prefix}
  })
  if ($parsed.rows | is-empty) and $source.exit_code not-in [null 0] {
    $source | not-the-format "from tsc" "what tsc prints (lines like `src/a.ts(1,7): error TS2322: ...`)" "The command failed, but printed no TypeScript error: did tsc run at all? (A script that does not exist, a step before tsc that failed.)"
  }
  $parsed.rows
}
