# The npm registry, with tables back. Load it with `use nux/npm`, which names its commands
# `npm ...`:
#
#   npm versions react            every version: version, published, tags, deprecated
#   npm info react-router@7       one version's manifest, as a record (a range: the highest)
#   npm files @dral/tsconfig@1    what the published package contains: path, size
#
# It asks the npm CLI, so ~/.npmrc applies: private packages and scoped registries work as
# they do for npm. It is not npm itself: there is no `npm` command of its own, so
# `npm install` and every other `npm ...` still run npm, as in any shell.
# Details: nux help npm <command>

# Run npm with these arguments and give its JSON. On a failure, an error with npm's own
# summary (npm puts it in the JSON), or what npm wrote to stderr when there is no JSON.
def --wrapped npm-json [...arguments: string]: nothing -> any {
  let run = (^npm ...$arguments | complete)
  let found = (try { $run.stdout | from json } catch { null })
  if $run.exit_code != 0 {
    let summary = ($found | get -o error.summary | default null)
    let said = if $summary != null { $summary } else { $run.stderr | str trim }
    error make {msg: $"npm ($arguments | str join ' ') failed", help: $said}
  }
  if $found == null {
    error make {msg: $"npm ($arguments | str join ' ') gave no JSON", help: ([($run.stderr | str trim) ($run.stdout | str trim | str substring 0..500)] | where $it != "" | str join "\n")}
  }
  $found
}

# Every published version of a package: version, published (when), tags (the dist-tags that
# point at it: latest, next, ...), deprecated (npm's message, or null).
#
# Oldest first, so `| last` is the newest; a version npm has no date for comes last.
@search-terms "npm view versions" "dist-tags" "latest version" "published" "npm view time" deprecated
@example "The newest 18.x" { npm versions react | where version =~ '^18\.\d+\.\d+$' | last }
@example "What the tags point at" { npm versions typescript | where tags != [] | select version tags }
@example "Released in the last month" { npm versions vite | where published > ((date now) - 30day) }
export def versions [
  package: string   # The package's name: react, @types/node
]: nothing -> table {
  let found = (npm-json view $package time dist-tags versions --json)
  # One version comes as a string; several as a list.
  let names = ($found.versions? | default [] | append [] | flatten)
  let times = ($found.time? | default {})
  let tags = ($found | get -o dist-tags | default {} | transpose tag version)
  # The plain `deprecated` field is only the latest version's: per version, a range.
  let deprecations = (npm-json view $"($package)@>=0.0.0-0" version deprecated --json | append [] | flatten)
  $names | each {|version|
    {
      version: $version
      published: ($times | get -o $version | default null | if $in != null { $in | into datetime } else { null })
      tags: ($tags | where version == $version | get tag)
      deprecated: ($deprecations | where version == $version | get -o 0.deprecated | default null)
    }
  } | sort-by published
}

# One version's manifest (package.json as published), as a record: name, version,
# dependencies, peerDependencies, exports, files, ...
#
# A range (react@18, react@^18.2) is the highest version in it, where npm itself gives a
# list. Leaves out npm's own `_` fields and versions, time, users and readme: for every
# version, `npm versions`.
@search-terms "npm view" "package.json" peerDependencies exports manifest "npm info"
@example "What a version needs next to it" { npm info react-router@7 | get peerDependencies }
@example "The newest in a range" { npm info typescript@5 | get version }
@example "How a package is laid out" { npm info @dral/tsconfig | select version exports files }
export def info [
  spec: string   # A package, with a version, range or tag: react, react@18, react@next
]: nothing -> record {
  let found = (npm-json view $spec --json)
  # A range that matches several versions comes as a list, lowest first.
  let manifest = if ($found | describe) =~ '^(list|table)' { $found | last } else { $found }
  let leave_out = ($manifest | columns | where {|name| $name starts-with '_' or $name in [versions time users readme] })
  $manifest | reject ...$leave_out
}

# What a published package contains, without downloading it here: one row per file, with
# path and size. (`npm pack --dry-run`: nothing is written.)
@search-terms "npm pack" "package contents" tarball "published files"
@example "Is the build in it" { npm files @dral/tsconfig@1.8.0 | where path =~ '^dist/' }
@example "The largest files" { npm files typescript | sort-by size --reverse | first 5 }
export def files [
  spec: string   # A package, with a version, range or tag: left-pad@1.3.0
]: nothing -> table {
  npm-json pack $spec --dry-run --json
  | get 0.files
  | each {|file|
      {path: $file.path, size: ($file.size | into filesize)}
    }
}
