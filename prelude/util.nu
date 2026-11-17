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

# What a `from <tool>` parser is given, as {text, stderr, exit_code}: the record of `complete`
# (the usual way: a command that fails does not pipe its output into a custom command), or
# just the text, or its lines.
export def command-output []: any -> record {
  let input = $in
  let kind = ($input | describe)
  if $kind =~ '^record' and "stdout" in $input {
    {text: ($input.stdout | into string), stderr: ($input.stderr? | default "" | into string), exit_code: ($input.exit_code? | default null)}
  } else if $kind =~ '^list' {
    {text: ($input | str join "\n"), stderr: "", exit_code: null}
  } else {
    {text: ($input | into string), stderr: "", exit_code: null}
  }
}

# The input is not what this parser reads: fail, showing the start of it (a build error?).
export def not-the-format [parser: string, expected: string, hint: string]: record -> any {
  let source = $in
  let start = ($source.text | str trim | lines | first 15 | str join "\n")
  let errors = ($source.stderr | str trim | lines | last 15 | str join "\n")
  error make {
    msg: $"($parser): this is not ($expected)"
    help: ([
      $hint
      (if $source.exit_code != null { $"The command exited with code ($source.exit_code)." } else { "" })
      (if $start != "" { $"Its output starts with:\n($start)" } else { "It printed nothing." })
      (if $errors != "" { $"Its error output ends with:\n($errors)" } else { "" })
    ] | where $it != "" | str join "\n")
  }
}
