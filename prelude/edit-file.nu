# edit-file: replace exact text in a file, and fail loudly when the text is not there.

# Replace exact text in a file, and fail when it is not there exactly once.
#
# `old` and `new` are taken literally: no regular expressions, no `$1`, no escapes. Each
# `old` has to occur exactly once in the file (any number of times with --all); not found,
# or found more than once, is an error that says which edit and how often. Several edits
# come in as a table with the columns `old` and `new`, applied in order, each to the result
# of the one before, all or nothing: when one fails, the file is not written.
# Returns {path, edits, replacements}.
@example "One edit" { edit-file app/root.tsx 'console.log(error)' 'console.error(error)' }
@example "Several edits at once" { [[old new]; ['let a = 1' 'let a = 2'] ['"2025"' '"2026"']] | edit-file config.ts }
export def main [
  path: path      # the file to change
  old?: string    # text to find (leave out when edits come in as a table)
  new?: string    # text to put in its place
  --all (-a)      # replace every occurrence instead of requiring exactly one
]: [nothing -> record, table -> record] {
  let piped = $in
  let edits = if $old != null {
    if $new == null {
      error make {msg: "edit-file: `old` given without `new`", help: "edit-file <path> <old> <new>   (use '' to delete text)"}
    }
    [{old: $old, new: $new}]
  } else if $piped != null {
    $piped
  } else {
    error make {msg: "edit-file: nothing to do", help: "Give <old> <new>, or pipe in a table with the columns `old` and `new`."}
  }
  if not ($path | path exists) {
    error make {msg: $"edit-file: no such file: ($path)"}
  }

  mut text = (open --raw $path | decode utf-8)
  mut replacements = 0
  for edit in ($edits | enumerate) {
    let old = $edit.item.old
    let new = $edit.item.new
    let shown = ($old | str substring 0..<70 | str replace --all "\n" "⏎")
    let number = $edit.index + 1
    if ($old | is-empty) {
      error make {msg: $"edit-file: edit ($number) has an empty `old`; nothing was written"}
    }
    let parts = ($text | split row $old)
    let count = ($parts | length) - 1
    if $count == 0 {
      error make {
        msg: $"edit-file: edit ($number) not found in ($path); nothing was written"
        help: $"Looked for: ($shown)\nThe text has to match exactly, including indentation and line endings."
      }
    }
    if $count > 1 and not $all {
      error make {
        msg: $"edit-file: edit ($number) matches ($count) times in ($path); nothing was written"
        help: $"Looked for: ($shown)\nAdd surrounding text to make it unique, or pass --all to replace every occurrence."
      }
    }
    $text = ($parts | str join $new)
    $replacements += $count
  }
  $text | save --force --raw $path
  {path: $path, edits: ($edits | length), replacements: $replacements}
}
