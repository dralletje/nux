# TypeScript's diagnostics as a table. `tsc` has no structured output.

use util.nu find-up

# Turn the text `tsc` prints into a table: file, line, column, category, code, message.
#
# For output you already have, e.g. of a project's own check script:
# `^pnpm typecheck | complete | get stdout | from tsc`. Lines that are not diagnostics are
# skipped. A diagnostic that is about no file (a problem in tsconfig) has null for file,
# line and column.
@example "Diagnostics from the output of a script" { ^pnpm typecheck | complete | get stdout | from tsc }
export def "from tsc" []: string -> table {
  let in_file = '^(?<file>.+?)[(:](?<line>\d+)[,:](?<column>\d+)\)?:? (?:- )?(?<category>error|warning) (?<code>TS\d+): (?<message>.*)$'
  let global = '^(?<category>error|warning) (?<code>TS\d+): (?<message>.*)$'
  $in | lines | reduce --fold [] {|line, rows|
    let found = ($line | parse --regex $in_file)
    if ($found | is-not-empty) {
      let row = $found.0
      $rows | append {file: $row.file, line: ($row.line | into int), column: ($row.column | into int), category: $row.category, code: $row.code, message: $row.message}
    } else {
      let found = ($line | parse --regex $global)
      if ($found | is-not-empty) {
        let row = $found.0
        $rows | append {file: null, line: null, column: null, category: $row.category, code: $row.code, message: $row.message}
      } else if ($line =~ '^\s+\S') and ($rows | is-not-empty) {
        # An indented line continues the message of the diagnostic before it.
        $rows | update (($rows | length) - 1) {|row| $row | update message {|r| $"($r.message)\n($line | str trim)" } }
      } else {
        $rows
      }
    }
  }
}

# Type-check a TypeScript project and get the diagnostics as a table.
#
# Runs the project's own `tsc` (the nearest node_modules/.bin/tsc) with --noEmit. One row per
# diagnostic: file, line, column, category, code, message; file paths are relative to the
# current directory. No rows means the project type-checks. A project whose check is a script
# that does more than `tsc` (code generation first): run that script and use `from tsc`.
#
# A "solution" tsconfig (no files of its own, only `references`) is checked in build mode
# (`tsc -b --force`), which follows the references: with --project it would check nothing.
# A tsconfig that matches no files at all is an error, not an empty table.
@search-terms tsc typescript diagnostics "type errors"
@example "The project in the current directory" { typecheck }
@example "Another project, counted per file" { typecheck packages/web | group-by file --to-table | each {|g| {file: $g.file, errors: ($g.items | length)} } }
@example "Which kinds of errors there are" { typecheck | group-by code --to-table | each {|g| {code: $g.code, count: ($g.items | length), example: $g.items.0.message} } }
export def typecheck [
  project?: path # Directory with a tsconfig.json, or a tsconfig file (default: current directory)
]: nothing -> table {
  let target = ($project | default "." | path expand)
  let directory = if ($target | path type) == "file" { $target | path dirname } else { $target }
  let tsc = (find-up "node_modules/.bin/tsc" $directory)
  if $tsc == null {
    error make {msg: $"No node_modules/.bin/tsc in ($directory) or above it", help: "Install the project's dependencies first, or check its output with `from tsc`."}
  }
  # What the config resolves to. `--showConfig` prints plain JSON (no comments or trailing
  # commas, `extends` applied), with `files` the files it would check.
  let shown = (do { cd $directory; ^$tsc --showConfig --project $target } | complete)
  let config = (try { $shown.stdout | from json } catch { null })
  if $shown.exit_code != 0 or $config == null {
    error make {msg: "tsc could not read the tsconfig", help: ($"($shown.stdout)($shown.stderr)" | str trim)}
  }
  let has_files = ($config.files? | default [] | is-not-empty)
  let is_solution = (not $has_files) and ($config.references? | default [] | is-not-empty)
  if not $has_files and not $is_solution {
    error make {
      msg: "This tsconfig checks no files"
      help: $"No file matches its `files`/`include`, and it has no `references`: ($target)"
    }
  }
  # A solution config checks nothing with --project. Build mode follows its references in
  # order; --force ignores saved build info, so a stale *.tsbuildinfo cannot skip a check.
  let arguments = if $is_solution {
    [-b --force --noEmit --pretty false $target]
  } else {
    [--noEmit --pretty false --project $target]
  }
  let run = (do { cd $directory; ^$tsc ...$arguments } | complete)
  let rows = ($run.stdout | from tsc)
  # tsc exits with 1 or 2 when there are diagnostics; anything else without diagnostics is a crash.
  if ($rows | is-empty) and $run.exit_code != 0 {
    error make {msg: "tsc failed without diagnostics", help: ($"($run.stdout)($run.stderr)" | str trim)}
  }
  let here = (pwd)
  $rows | update file {|row|
    if $row.file == null { null } else {
      let absolute = ($directory | path join $row.file)
      try { $absolute | path relative-to $here } catch { $absolute }
    }
  }
}
