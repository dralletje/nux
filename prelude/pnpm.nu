# pnpm, with tables back. Load it with `use nux/pnpm`, which names its commands `pnpm ...`:
#
#   pnpm run-recursive test     a script in every package: one row per package, its output
#   pnpm packages               the workspace's packages: name, version, path, scripts
#   pnpm outdated               dependencies with a newer version, and who uses them
#   pnpm why react              which packages need a dependency, and through what
#   ... | pnpm by-package       what `^pnpm -r <script>` printed, per package
#
# It is not pnpm itself: there is no `pnpm` command of its own, so `pnpm build`, `pnpm -r
# test` and every other `pnpm ...` still run pnpm, as in any shell. Details: nux help pnpm <command>

use nux/util [command-output]

# Run a package script in every package of the workspace that has it (`pnpm -r run
# <script>`), and give what each printed: one row per package, as `pnpm by-package` gives
# it: package, script, command, status, output.
#
# Options for pnpm come after the script (`--filter ./packages/core`, `--no-bail`); what
# comes after `--` goes to the script itself. pnpm stops at the first package that fails
# (status failed; the ones it stopped have status null), unless --no-bail. A run that fails
# is not an error here: see `status`. pnpm's own error, when there is one, is on stderr.
#
# For test results, read each package's output with the parser for its runner.
@search-terms "pnpm -r" "pnpm recursive" "run in every package" workspace monorepo
@example "Which packages fail their tests" { pnpm run-recursive test | where status == failed | get package }
@example "Test results of every package, from TAP" {
  with-env {NODE_OPTIONS: "--test-reporter=tap"} {
    pnpm run-recursive test
  }
  | each {|package|
      $package.output | from tap | insert package $package.package
    }
  | flatten
  | where status == fail
}
@example "Some packages only, all of them even after a failure" { pnpm run-recursive build --filter './packages/*' --no-bail | select package status }
export def --wrapped run-recursive [
  script: string   # The package script, as in package.json: test, build, typecheck
  ...options       # For pnpm (--filter x, --no-bail); after `--`, for the script
]: nothing -> any {
  let split = ($options | enumerate | where item == "--" | get index | get 0?)
  let for_pnpm = if $split == null { $options } else { $options | first $split }
  let for_script = if $split == null { [] } else { $options | skip ($split + 1) }
  # --stream: every line with its package in front, also when only one package runs (pnpm
  # leaves the prefix out then, and nobody could tell which package it was).
  let run = (^pnpm --recursive --stream ...$for_pnpm run $script ...$for_script | complete)
  let packages = (try { $run | by-package } catch { null })
  if $packages != null {
    # pnpm's own words (which package failed) go with the rows, on stderr.
    if $run.stderr != "" { print -e ($run.stderr | str trim) }
    return $packages
  }
  # No package ran it, and pnpm did not mind (with --if-present): nothing to give, which is
  # an empty table. Otherwise pnpm itself failed: "None of the selected packages has a ...
  # script", a bad --filter, no workspace.
  if $run.exit_code == 0 {
    return []
  }
  error make {
    msg: $"pnpm run-recursive ($script): pnpm failed before any package ran it"
    help: ([($run.stdout | str trim) ($run.stderr | str trim)] | where $it != "" | str join "\n")
  }
}

# The packages of the workspace, as a table: name, version, path, private, scripts.
#
# `path` is from the current directory ("." for the one you are in); `scripts` are the names
# of each package's scripts. The workspace's root package is a row too.
@search-terms workspace monorepo "list packages" "package.json" scripts
@example "Which packages have a test script" { pnpm packages | where "test" in $it.scripts | get name }
@example "Where a package is" { pnpm packages | where name == "@acme/core" | get 0.path }
export def packages []: nothing -> table {
  let run = (^pnpm ls --recursive --depth -1 --json | complete)
  if $run.exit_code != 0 {
    error make {msg: "pnpm packages: pnpm ls failed", help: ($run.stderr | str trim)}
  }
  let here = (pwd)
  $run.stdout | from json | each {|package|
    let manifest = (try { open ($package.path | path join package.json) } catch { {} })
    let path = (try { $package.path | path relative-to $here } catch { $package.path })
    {
      name: ($package.name? | default null)
      version: ($package.version? | default null)
      path: (if $path == "" { "." } else { $path })
      private: ($package.private? | default false)
      scripts: ($manifest.scripts? | default {} | columns)
    }
  }
}

# Dependencies that have a newer version, in every package of the workspace, as a table:
# name, current, wanted, latest, type, deprecated, used_by.
#
# `wanted` is the newest that the version range in package.json allows, `latest` the newest
# there is. `used_by` are the workspace packages that depend on it. Nothing outdated is an
# empty table (pnpm itself exits with 1 when something is outdated; that is not an error).
@search-terms "outdated dependencies" upgrade update "new version" npm
@example "What is behind, and who uses it" { pnpm outdated | select name current latest used_by }
@example "What `pnpm update` would change (within the ranges)" { pnpm outdated | where current != wanted }
export def outdated []: nothing -> table {
  let run = (^pnpm outdated --recursive --format json | complete)
  let found = (try { $run.stdout | from json } catch { null })
  if $run.exit_code > 1 or $found == null {
    error make {msg: "pnpm outdated failed", help: ([($run.stderr | str trim) ($run.stdout | str trim | str substring 0..500)] | where $it != "" | str join "\n")}
  }
  $found | transpose name info | each {|row|
    {
      name: $row.name
      current: ($row.info.current? | default null)
      wanted: ($row.info.wanted? | default null)
      latest: ($row.info.latest? | default null)
      type: ($row.info.dependencyType? | default null)
      deprecated: ($row.info.isDeprecated? | default false)
      used_by: ($row.info.dependentPackages? | default [] | get name)
    }
  }
}

# Why a dependency is installed: one row per way a workspace package comes to need it.
# Columns: dependency, version, package, field, through.
#
# `package` is the workspace package, `field` where it lists the first step
# (dependencies, devDependencies, ...), and `through` the packages in between, from the
# package down to the dependency: [] when the package depends on it itself.
@search-terms "why installed" "who depends" dependents "dependency tree" "pnpm why"
@example "Who needs two versions of react" { pnpm why react | group-by version --to-table | each {|group| {version: $group.version, packages: ($group.items.package | uniq)} } }
@example "The packages that use it directly" { pnpm why lodash | where through == [] | get package | uniq }
export def why [
  dependency: string   # The dependency's name: react, @types/node
]: nothing -> table {
  let run = (^pnpm why $dependency --recursive --json | complete)
  let found = (try { $run.stdout | from json } catch { null })
  if $run.exit_code != 0 or $found == null {
    error make {msg: $"pnpm why ($dependency) failed", help: ([($run.stderr | str trim) ($run.stdout | str trim | str substring 0..500)] | where $it != "" | str join "\n")}
  }
  # pnpm writes a package's dependents out once; where it comes again it is `deduped`, with
  # none. Their dependents, by name@version, to follow those too.
  mut full = {}
  mut stack = $found
  while ($stack | is-not-empty) {
    let node = ($stack | first)
    $stack = ($stack | skip 1)
    let dependents = ($node.dependents? | default [])
    if not ($node.deduped? | default false) and ($dependents | is-not-empty) {
      $full = ($full | upsert (key-of $node) $dependents)
    }
    $stack = ($stack | append $dependents)
  }
  let full = $full
  $found | each {|root|
    $root.dependents? | default [] | each {|dependent| ways $dependent [] $full } | flatten | each {|way|
      {dependency: $root.name, version: ($root.version? | default null), package: $way.package, field: $way.field, through: $way.through}
    }
  } | flatten | uniq
}

def key-of [node: record]: nothing -> string {
  $"($node.name)@($node.version? | default '')"
}

# The ways up from one dependent in `pnpm why`'s tree to the workspace packages: per way
# the package, the field and what is in between (`below`: the steps under this one, from
# the top down). A `deduped` node's dependents come from `full`.
def ways [node: record, below: list<string>, full: record]: nothing -> list<record> {
  # A package that depends on itself through others: that way goes nowhere new.
  if $node.name in $below { return [] }
  let dependents = if ($node.deduped? | default false) {
    $full | get --optional (key-of $node) | default []
  } else {
    $node.dependents? | default []
  }
  if ($dependents | is-empty) {
    [{package: $node.name, field: ($node.depField? | default null), through: ($below | reverse)}]
  } else {
    $dependents | each {|up| ways $up ($below | append $node.name) $full } | flatten
  }
}

# What `pnpm -r <script>` (or --filter, or several packages) printed, one row per package:
# package, script, command, status, output.
#
# pnpm puts `<package dir> <script>: ` in front of every line a package's script prints,
# with the packages' lines mixed as they ran at the same time, and `<dir> <script>$ <cmd>`
# for the command it runs. This takes those apart: `output` is what that package printed,
# in order, without the prefix, as if it ran alone. So a parser can read it:
# `| each {|package| $package.output | from tap | insert package $package.package } | flatten`.
#
# `status` is done or failed when pnpm said so, else null (pnpm stops the other packages
# when one fails: those have no end). Lines without a package prefix (pnpm's own: `Scope:`,
# its error at the end) are left out; with `complete` they are still in its stdout and stderr.
#
# Take `complete` of the run: a failing script fails the whole command, and then only
# `complete` hands its output on. To run a script and get this at once: pnpm run-recursive.
@search-terms "pnpm -r" "pnpm recursive" workspace monorepo packages "per package"
@example "Which packages failed" { ^pnpm -r test | complete | pnpm by-package | where status == failed | get package }
@example "One package's output, to read" { ^pnpm -r build | complete | pnpm by-package | where package =~ diff | get 0.output }
export def by-package []: any -> table {
  let source = ($in | command-output)
  let lines = ($source.text | lines) ++ ($source.stderr | lines)
  # `packages/diff test$ node --test` and `packages/diff test: ok 1 - x` (two spaces and
  # more after the colon are the line's own indentation: only one is pnpm's).
  let started = ($lines | parse --regex '^(?<package>\S+) (?<script>[\w:.-]+)\$ (?<command>.*)$')
  let printed = ($lines | parse --regex '^(?<package>\S+) (?<script>[\w:.-]+): ?(?<text>.*)$')
  # The packages pnpm said it runs; without those lines, prefixes that look like a folder.
  # (Not any `word word:` line: pnpm's own `Scope: 4 of 5` or a tool's `warning x: y`.)
  let packages = if ($started | is-not-empty) {
    $started | select package script | uniq
  } else {
    $printed | where package =~ '/' | select package script | uniq
  }
  let printed = ($printed | where {|line| $packages | any {|key| $key.package == $line.package and $key.script == $line.script } })
  if ($packages | is-empty) {
    let start = ($source.text | str trim | lines | first 10 | str join "\n")
    error make {
      msg: "pnpm by-package: no lines of a package in this"
      help: ([
        "It reads what `pnpm -r <script>` prints: lines like `packages/diff test: ...`. pnpm leaves the prefix out with --reporter-hide-prefix, and for one package alone."
        (if $start != "" { $"The output starts with:\n($start)" } else { "It printed nothing." })
      ] | str join "\n")
    }
  }
  $packages | each {|key|
    let own = ($printed | where package == $key.package and script == $key.script | get text)
    # pnpm's own last line for a package: `Done` or `Failed`.
    let last = ($own | last 1 | get 0? | default "")
    let status = if $last == "Done" { "done" } else if $last =~ '^Failed\b' { "failed" } else { null }
    let output = if $status != null { $own | drop 1 } else { $own }
    {
      package: $key.package
      script: $key.script
      command: ($started | where package == $key.package and script == $key.script | get command | get 0? | default null)
      status: $status
      output: ($output | str join "\n")
    }
  }
}
