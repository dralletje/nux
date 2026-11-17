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
    if $old == $new {
      print -e $"edit-file: edit ($number) changes nothing: its old and new are the same \(($shown)\)"
    }
    let parts = ($text | split row $old)
    let count = ($parts | length) - 1
    if $count == 0 {
      error make {
        msg: $"edit-file: edit ($number) not found in ($path); nothing was written"
        help: $"Looked for: ($shown)\n(nearest $text $old)"
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

# Why `old` is not in `text`: where the file stops matching it. In characters (graphemes).
def nearest [text: string, old: string]: nothing -> string {
  let shown = {|s| $s | str substring --grapheme-clusters 0..<50 | str replace --all "\n" "⏎" }
  # Bash's way to put a ' in '...' is literal text in Nushell.
  let quoting = if ($old | str contains `'"'"'`) {
    "\n`'\"'\"'` is bash quoting: in Nushell it is those five characters. For a ' in the text use r#'...'# or \"...\"."
  } else { "" }
  let squeezed = {|s| $s | str replace --all --regex '\s+' ' ' | str trim }
  if ((do $squeezed $text) | str contains (do $squeezed $old)) {
    return $"The text is there, but with other whitespace: indentation, line endings or spaces differ.($quoting)"
  }
  # The longest start of `old` that is in the file (a start that is there has every shorter
  # start there too, so: halve).
  mut low = 0
  mut high = ($old | str length --grapheme-clusters)
  while $high - $low > 1 {
    let middle = (($low + $high) // 2)
    if ($text | str contains ($old | str substring --grapheme-clusters 0..<$middle)) {
      $low = $middle
    } else {
      $high = $middle
    }
  }
  if $low < 5 {
    return $"Not even its start is in the file: is it the right file?($quoting)"
  }
  let start = ($old | str substring --grapheme-clusters 0..<$low)
  let line = ($text | split row $start | first | split row "\n" | length)
  let after = ($text | split row $start | skip 1 | str join $start)
  let wanted = (do $shown ($old | str substring --grapheme-clusters $low..))
  let there = (do $shown $after)
  $"The first ($low) characters match, at line ($line). Then the file has:\n  ($there)\nwhere edit-file was given:\n  ($wanted)($quoting)"
}
