# edit-file: replace exact text in a file, and fail loudly when the text is not there.

# Replace exact text in a file, and fail when it is not there exactly once.
#
# `old` and `new` are taken literally: no regular expressions, no `$1`, no escapes (with
# --regex, `old` is a regular expression and `new` may use its groups: `$1`, `${name}`). Each
# `old` has to occur exactly once in the file (any number of times with --all); not found,
# or found more than once, is an error that says which edit and how often. Several edits
# come in as a table with the columns `old` and `new`, applied in order, each to the result
# of the one before, all or nothing: when one fails, the file is not written. With a column
# `path` too, the edits are for several files, and still all or nothing: no file is written
# unless every edit of every file applies.
# Returns {path, edits, replacements}; for several files, a table of those.
@example "One edit" { edit-file app/root.tsx 'console.log(error)' 'console.error(error)' }
@example "Several edits at once" { [[old new]; ['let a = 1' 'let a = 2'] ['"2025"' '"2026"']] | edit-file config.ts }
@example "The same, with the table as an argument" { let edits = [[old new]; ['let a = 1' 'let a = 2']]; edit-file config.ts $edits }
@example "Edits in several files, all or nothing" {
  [[path old new]; [a.ts 'from "./x"' 'from "./y"'] [b.ts 'from "./x"' 'from "./y"']] | edit-file
}
@example "A regular expression, every match" { edit-file src/a.ts --regex --all 'console\.log\((.*)\)' 'logger.debug($1)' }
export def main [
  path?: path     # the file to change (leave out when the edits come in with a `path` column)
  old?: any       # text to find (leave out when edits come in as a table; or the table itself)
  new?: string    # text to put in its place
  --all (-a)      # replace every occurrence instead of requiring exactly one
  --regex (-r)    # `old` is a regular expression, `new` may use its groups ($1, ${name})
]: [nothing -> any, table -> any] {
  let piped = $in
  let kind = ($old | describe)
  # The edits as an argument (`edit-file f $edits`), as agents write it as often as piped.
  let edits = if ($kind =~ '^(table|list)') {
    if $new != null {
      error make {msg: "edit-file: a table of edits and a `new` as well", help: "With a table of old/new, leave out <new>: edit-file <path> $edits"}
    }
    $old
  } else if ($kind =~ '^record') {
    error make {msg: "edit-file: `old` is a record, not text", help: "edit-file <path> <old> <new>, or a table with the columns `old` and `new`: edit-file <path> $edits"}
  } else if $old != null {
    if $new == null {
      error make {msg: "edit-file: `old` given without `new`", help: "edit-file <path> <old> <new>   (use '' to delete text)"}
    }
    [{old: ($old | into string), new: $new}]
  } else if $piped != null {
    $piped
  } else {
    error make {msg: "edit-file: nothing to do", help: "Give <old> <new>, or pipe in a table with the columns `old` and `new` (and `path`, for several files)."}
  }
  let by_file = if "path" in ($edits | columns) {
    $edits | group-by path --to-table | each {|group| {path: $group.path, edits: ($group.items | select old new)} }
  } else if $path != null {
    [{path: $path, edits: $edits}]
  } else {
    error make {msg: "edit-file: which file?", help: "edit-file <path> <old> <new>, or edits in a table with a `path` column."}
  }
  # Every file's new text first; then write them all, or nothing when one edit failed.
  let changed = ($by_file | each {|file| apply $file.path $file.edits $all $regex })
  for file in $changed { $file.text | save --force --raw $file.path }
  let result = ($changed | each {|file| {path: $file.path, edits: $file.edits, replacements: $file.replacements} })
  if ($result | length) == 1 and "path" not-in ($edits | columns) { $result.0 } else { $result }
}

# A file's text after its edits: {path, text, edits, replacements}. Fails, writing nothing,
# when an edit does not apply.
def apply [path: string, edits: list<any>, all: bool, regex: bool]: nothing -> record {
  if not ($path | path exists) {
    error make {msg: $"edit-file: no such file: ($path); nothing was written"}
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
    # How often it matches: with a regular expression, by marking each match.
    let count = if $regex {
      let mark = "\u{0}nux-edit-file-match\u{0}"
      ($text | str replace --all --regex $old $mark | split row $mark | length) - 1
    } else {
      ($text | split row $old | length) - 1
    }
    if $count == 0 {
      error make {
        msg: $"edit-file: edit ($number) not found in ($path); nothing was written"
        help: (if $regex { $"Looked for the regular expression: ($shown)" } else { $"Looked for: ($shown)\n(nearest $text $old)" })
      }
    }
    if $count > 1 and not $all {
      error make {
        msg: $"edit-file: edit ($number) matches ($count) times in ($path); nothing was written"
        help: $"Looked for: ($shown)\nAdd surrounding text to make it unique, or pass --all to replace every occurrence."
      }
    }
    $text = if $regex {
      if $all { $text | str replace --all --regex $old $new } else { $text | str replace --regex $old $new }
    } else {
      $text | split row $old | str join $new
    }
    $replacements += $count
  }
  {path: $path, text: $text, edits: ($edits | length), replacements: $replacements}
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
