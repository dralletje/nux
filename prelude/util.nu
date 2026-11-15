# Helpers for the prelude's files. Not loaded into calls; `use nux/util <name>` loads one.

# The nearest `name` in `start` or a directory above it, or null.
export def find-up [name: string, start: string]: nothing -> any {
  mut directory = ($start | path expand)
  loop {
    let candidate = ($directory | path join $name)
    if ($candidate | path exists) { return $candidate }
    let parent = ($directory | path dirname)
    if $parent == $directory { return null }
    $directory = $parent
  }
}
