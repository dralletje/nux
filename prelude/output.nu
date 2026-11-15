# Showing values on the way through a pipeline.

# More than this many characters of one `show` are cut off: unlike a call's final value,
# what is printed along the way is not kept in the session to be looked at later.
const SHOW_LIMIT = 2000

# Of a failed command's output, this many characters are shown, from the end (where the
# error usually is).
const FAILED_OUTPUT_LIMIT = 20000

# Of what a command that succeeded wrote to stderr, this many characters are shown, from
# the end.
const STDERR_LIMIT = 4000

# The end of a text, when it is longer than `limit` characters, with a line saying so.
def last-part [limit: int, what: string]: string -> string {
  let text = $in
  let length = ($text | str length)
  if $length <= $limit { $text } else {
    $"... \(the first ($length - $limit) characters of ($what) are left out\)\n($text | str substring ($length - $limit)..)"
  }
}

# What a command wrote to stderr, marked as such and with the command it came from, so that
# it is not taken for a value or for the output of something else. One line stays one line;
# more are indented under a heading. (`format_stderr` in src/format.ts makes the same, for
# the last command of a call.)
def stderr-block [command: string]: string -> string {
  let lines = ($in | str trim | lines)
  if ($lines | is-empty) { return "" }
  if ($lines | length) == 1 { return $"stderr \(($command)\): ($lines.0)" }
  $lines | each {|line| $"  ($line)" } | prepend $"stderr \(($command)\):" | str join "\n"
}

# Stop when a command failed, after printing what it wrote. Takes the record of `complete`.
#
# nux adds this itself after every external command that stands on its own at the top level
# of the code you give it (`pnpm build` on a line, before other lines), so that a failure
# there shows the command's output and ends the call. You only need it by hand for a command
# inside a loop, a closure or a `def`, where nux does not add it:
#     for file in $files { ^prettier --check $file | complete | check-exit $"prettier ($file)" }
# For a command that succeeded, what it wrote to stderr is printed (warnings; `git push`
# reports there), its normal output is not, and nothing is passed on. What came from stderr
# is always marked: `stderr (the command): ...`.
@search-terms "exit code" "fail on error" "set -e"
@example "Inside a loop" { for file in (glob *.ts) { ^prettier --check $file | complete | check-exit $"prettier ($file)" } }
export def check-exit [
  command: string # What ran, for the messages
]: record -> any {
  let run = $in
  # (No stderr in the record when the command's stderr was redirected: `e> /dev/null`.)
  let stderr = ($run.stderr? | default "" | into string)
  if $run.exit_code == 0 {
    # What a command writes to stderr is meant to be seen whatever happens to its output:
    # warnings, and for some tools the result itself (`git push` reports there).
    let said = ($stderr | str trim | last-part $STDERR_LIMIT "its stderr" | stderr-block $command)
    if $said != "" { print $said }
    return
  }
  let output = ($run.stdout | into string | str trim --right | last-part $FAILED_OUTPUT_LIMIT "the output")
  let said = ($stderr | str trim | last-part $FAILED_OUTPUT_LIMIT "its stderr" | stderr-block $command)
  let shown = ([$output $said] | where $it != "" | str join "\n")
  if $shown != "" { print $shown }
  error make --unspanned {msg: $"`($command)` exited with code ($run.exit_code)"}
}

# Print a value in the middle of a pipeline, and pass it on unchanged.
#
# A call only prints the value of its last expression; this is for seeing something before
# that. It prints the way the last value is printed: data as NUON, text as it is.
#
# With a closure, what is printed is what the closure makes of the value (a count, the first
# rows, some columns), while the whole value still goes on to the next command:
#     ls **/* | show { length } | where size > 1mb | show { select name size } | get name
# The closure gets the value as its input, like the closure of `each` gets a row.
#
# Not after an external command that may fail: Nushell does not pass a failing command's
# output on to a command like this. Put `complete` in between: `^pnpm build | complete | show`.
@search-terms print log debug tee inspect "intermediate value" peek
@example "See a value and go on with it" { ls | show | length }
@example "See a summary, go on with everything" { ls | show { length } | sort-by size }
@example "Several, with labels to tell them apart" { git log | show --label commits { length } | where author =~ 'Dral' | show --label mine { length } | first 3 }
@example "As a statement, for a variable" { let rows = (ls); $rows | show { first 2 }; $rows | length }
export def show [
  view?: closure         # What to print of the value (default: the value itself)
  --label (-l): string   # Printed in front, to tell several apart
]: any -> any {
  let value = $in
  let seen = (if $view == null { $value } else { $value | do $view })
  # A polars frame cannot be written as NUON: take its rows.
  let seen = (if ($seen | describe) =~ '^polars_' { $seen | polars into-nu } else { $seen })
  let text = (if ($seen | describe) == "string" { $seen } else { $seen | to nuon --raw --serialize })
  let length = ($text | str length)
  let text = if $length <= $SHOW_LIMIT { $text } else {
    $"($text | str substring 0..<$SHOW_LIMIT)\n... \(($length - $SHOW_LIMIT) more characters not shown. Narrow it down: show { first 20 }, show { length }, show { select <columns> }\)"
  }
  print (if $label == null { $text } else { $"($label): ($text)" })
  $value
}
