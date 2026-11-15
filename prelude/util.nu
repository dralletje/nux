# Helpers shared by the prelude's files. Not themselves available to the code nux runs.

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
