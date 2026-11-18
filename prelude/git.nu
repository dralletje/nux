# git, with tables as results. git itself has no structured output.
#
# In nux, `git` is this module: `git log`, `git status`, `git diff`, `git hunks`,
# `git info`, `git add` (and `git changes`, another name for `git diff`). Anything else
# (`git commit`, `git status --short`) does not fall through to git itself: `git` refuses it
# and says to use `^git`, so a call never gets git's text where it expected a table. The
# prelude takes it in with `export use git.nu` (no `*`), which names the commands `git ...`.

# Our git commands, one line each: name and the first line of its description.
def ours []: nothing -> string {
  help commands
  | where name =~ '^git \S'
  | each {|command| $"  ($command.name | fill --width 14)($command.description | lines | first)" }
  | str join "\n"
}

# What git's own subcommands are used for that one of ours gives as a table. Shown when
# `git <subcommand>` is refused, and in the list `git` prints. From what agents ran as
# `^git ...` in the nux log: mostly `diff --stat` and `rev-parse`.
const INSTEAD = {
  diff: "what changed per file: git diff (--staged, a file, main...HEAD); per line: git diff --lines; to read it: git diff | where path =~ x | git hunks | get text"
  show: "what a commit changed: git diff <commit>~1 <commit>; to read it: ... | git hunks | get text"
  ls-files: "changed and untracked files: git status"
  rev-parse: "the commit, branch, upstream and top directory: git info"
  branch: "the current branch, its upstream, ahead and behind: git info"
  remote: "the remotes and their urls: git info | get remotes"
}

# The INSTEAD lines as a list, for `git` without arguments.
def instead-list []: nothing -> string {
  $INSTEAD | transpose name use | each {|row| $"  ^git ($row.name | fill --width 10)→ ($row.use)" } | str join "\n"
}

# An argument as it would be typed: quoted when it has spaces, quotes and the like.
def as-typed [argument: string]: nothing -> string {
  if $argument =~ '^[\w./:=@%+,~^-]+$' { $argument } else { $argument | to nuon }
}

# nux's git commands, which give tables. For git itself: ^git ...
#
# Without arguments, the list of them. Anything that is not one of them is an error that
# says how to run it with git itself.
@example "The list of nux's git commands" { git }
@example "git itself, for what these do not cover" { ^git commit -m "Fix the thing" }
export def --wrapped main [...arguments] {
  if ($arguments | is-empty) {
    return ([
      "nux's git commands give tables. For git itself: ^git ..."
      ""
      (ours)
      ""
      "Instead of reading git's text for these, use the table:"
      (instead-list)
      ""
      "Details: help git <command>"
    ] | str join "\n")
  }
  let line = ($arguments | each {|argument| as-typed $argument } | str join ' ')
  # The first argument that is a git subcommand we know a table for (`-C dir diff` too).
  let instead = ($arguments | where $it in ($INSTEAD | columns) | first 1 | each {|name| $INSTEAD | get $name })
  let instead = if ($instead | is-empty) { "" } else { $"For ($instead.0)\n" }
  error make {
    msg: $"`git ($line)` is not one of nux's git commands"
    label: {text: "not one of nux's", span: (metadata $arguments).span}
    help: $"($instead)For git itself: ^git ($line)\nnux's git commands:\n(ours)"
  }
}

# Run git and give its output; a failure becomes an error with git's own message.
def run-git [arguments: list<string>]: nothing -> string {
  let run = (^git -c core.quotepath=off ...$arguments | complete)
  if $run.exit_code != 0 {
    error make {msg: $"git ($arguments | first) failed", help: ($run.stderr | str trim)}
  }
  $run.stdout
}

const STATES = {M: modified, A: added, D: deleted, R: renamed, C: copied, U: unmerged, T: "type changed"}

# The `a/` and `b/` in a diff's `--- a/path` and `+++ b/path`, which `parse-hunks` takes off.
# Given every time: git's config can change them (`diff.noprefix`, `diff.mnemonicPrefix`).
const PREFIXES = [--src-prefix=a/ --dst-prefix=b/]

# Paths in git's output are from the repository's root; ours are from the current directory,
# like every other path in nux (`open`, `ls`, `git add`). `--relative` would also
# leave out what is outside the current directory, so the conversion is done here.

# Where the current directory is in the repository: its path from the root, in parts ([] at
# the root). From git, so a symlink on the way does not matter.
def here []: nothing -> list<string> {
  run-git [rev-parse --show-prefix] | str trim | split row "/" | where $it != ""
}

# A path from the repository's root, from the current directory instead (`here`): `../` for
# what is above it. A directory keeps its trailing `/`; the current directory itself is `./`.
def from-here [here: list<string>]: string -> string {
  let path = $in
  let parts = ($path | split row "/" | where $it != "")
  let common = ($here | zip $parts | take while {|pair| $pair.0 == $pair.1 } | length)
  let up = ($here | length) - $common
  let relative = (if $up > 0 { 1..$up | each { ".." } } else { [] }) ++ ($parts | skip $common)
  let relative = ($relative | str join "/")
  if ($path | str ends-with "/") {
    if $relative == "" { "./" } else { $relative + "/" }
  } else {
    if $relative == "" { "." } else { $relative }
  }
}

# The commit history as a table: hash, short, date, author, email, refs, parents, subject.
#
# `date` is a datetime, so it can be compared (`where date > (date now) - 7day`). `refs` are
# the branches and tags that point at the commit. Newest first, like `^git log`.
@search-terms history commits
@example "What changed this week" { git log --since "1 week ago" | select short date author subject }
@example "Commits that touched a directory, with the files of each" { git log --files src | select short subject files }
@example "What a branch adds to main" { git log --range main..HEAD }
@example "Who commits most" {
  git log
  | group-by author --to-table
  | each {|group|
      {author: $group.author, commits: ($group.items | length)}
    }
  | sort-by commits --reverse
}
export def log [
  ...paths: path         # Only commits that touch these files or directories
  --max (-n): int        # At most this many commits (default: all)
  --since: string        # Only commits after this, as git reads it: "2 weeks ago", "2026-01-31"
  --author: string       # Only commits whose author matches this
  --range (-r): string   # A revision or range, e.g. main..HEAD (default: from HEAD back)
  --files (-f)           # Add `files`: per commit a table of path, added, deleted
  --body (-b)            # Add `body`: the rest of the commit message
]: nothing -> table {
  # A record starts with 1e; fields end with 1f. What follows the last field is the file list.
  let format = "%x1e%H%x1f%h%x1f%aI%x1f%an%x1f%ae%x1f%D%x1f%P%x1f%s%x1f%b%x1f"
  mut arguments = [log $"--pretty=format:($format)"]
  if $files { $arguments ++= [--numstat --no-renames] }
  if $max != null { $arguments ++= [--max-count ($max | into string)] }
  if $since != null { $arguments ++= [--since $since] }
  if $author != null { $arguments ++= [--author $author] }
  if $range != null { $arguments ++= [$range] }
  let arguments = $arguments ++ [--] ++ $paths
  # Only --files has paths to give from the current directory (another git process).
  let here = (if $files { here } else { [] })
  run-git $arguments | split row "\u{1e}" | skip 1 | each {|record|
    let fields = ($record | split row "\u{1f}")
    let commit = {
      hash: $fields.0
      short: $fields.1
      date: ($fields.2 | into datetime)
      author: $fields.3
      email: $fields.4
      refs: ($fields.5 | split row ", " | where $it != "")
      parents: ($fields.6 | split row " " | where $it != "")
      subject: $fields.7
    }
    let commit = if $body { $commit | insert body ($fields.8 | str trim) } else { $commit }
    if $files { $commit | insert files ($fields.9 | parse-numstat "\n" $here) } else { $commit }
  }
}

# `added<tab>deleted<tab>path` entries; a binary file has `-` for both numbers. The paths
# come out from the current directory (`here`).
def parse-numstat [separator: string, here: list<string>]: string -> table {
  $in | split row $separator | where ($it | str trim) != "" | each {|entry|
    let parts = ($entry | split row "\t")
    {
      path: ($parts | skip 2 | str join "\t" | from-here $here)
      added: (if $parts.0 == "-" { null } else { $parts.0 | into int })
      deleted: (if $parts.1 == "-" { null } else { $parts.1 | into int })
    }
  }
}

# The files that differ from the last commit, as a table: path, staged, unstaged, from.
#
# `staged` is what `git commit` would record for the file, `unstaged` what is changed on top
# of that: modified, added, deleted, renamed, untracked, ... or null. `from` is the old path
# of a renamed file. Paths are from the current directory: `../` for files above it, as
# `open` and `git add` read them. An untracked directory is one row, as in `^git status`.
# A clean working tree is an empty list.
#
# --short and --porcelain are taken and change nothing: what they are for, a short form a
# program can read, is this table.
@search-terms changed modified untracked
@example "Files with changes that are not staged yet" { git status | where unstaged != null | get path }
@example "Is anything staged?" { git status | where staged != null | is-not-empty }
export def status [
  --short (-s)  # Changes nothing: this table is the short form
  --porcelain   # Changes nothing: this table is the form for programs
]: nothing -> table {
  let entries = (run-git [status --porcelain=v1 -z] | split row (char nul) | where $it != "")
  let here = (here)
  mut rows = []
  mut index = 0
  while $index < ($entries | length) {
    let entry = ($entries | get $index)
    let staged = ($entry | str substring 0..<1)
    let unstaged = ($entry | str substring 1..<2)
    mut from: any = null
    # A rename or copy is followed by an entry of its own with the old path, staged or not
    # (an unstaged one: a file added with `git add -N`, then renamed, in some git versions).
    if $staged in [R C] or $unstaged in [R C] {
      $index += 1
      $from = ($entries | get $index | from-here $here)
    }
    let special = if $staged == "?" { "untracked" } else if $staged == "!" { "ignored" } else { null }
    $rows ++= [{
      path: ($entry | str substring 3.. | from-here $here)
      staged: (if $special != null { null } else { $STATES | get --optional $staged })
      unstaged: (if $special != null { $special } else { $STATES | get --optional $unstaged })
      from: $from
    }]
    $index += 1
  }
  $rows
}

# `versions` in the rows of `git diff`: what was compared. "--unstaged" (the working tree
# against what is staged), "--staged" (what is staged against the last commit), or the
# revisions as given ("main...HEAD", "HEAD~1 HEAD"), after "--staged" with --staged. It is
# how `git hunks` knows what to ask git for again. The two markers start with `-`, which no
# branch or tag name can, so a branch called `staged` is never read as one.
def versions-of [revisions: list<string>, staged: bool]: nothing -> string {
  let words = (if $staged { [--staged] } else { [] }) ++ $revisions
  if ($words | is-empty) { "--unstaged" } else { $words | str join " " }
}

# The arguments for git that a `versions` stands for.
def versions-arguments [versions: string]: nothing -> list<string> {
  let words = ($versions | split row " " | where $it not-in ["" --unstaged])
  (if "--staged" in $words { [--staged] } else { [] }) ++ ($words | where $it != --staged)
}

# git's unified diff as hunks: path, hunk (its number in the file, from 1), at (what git
# shows after `@@`, usually the function the hunk is in; null without), old, old_count,
# new, new_count (where the hunk is in each version), lines (kind, old, new, text) and
# text (the hunk's lines as git printed them, `@@` line first). Paths are from `here`.
def parse-hunks [output: string, here: list<string>]: nothing -> list<any> {
  # A header path: `a/path` or `b/path`, quoted by git when it has odd characters, and
  # followed by a tab when it has a space.
  let header_path = {|raw|
    let raw = ($raw | str trim --right --char "\t")
    let raw = if ($raw | str starts-with '"') { $raw | from json } else { $raw }
    if $raw == "/dev/null" { null } else { $raw | str substring 2.. }
  }
  mut hunks = []
  mut hunk: any = null
  mut path: any = null
  mut old_path: any = null
  mut number = 0
  mut old = 0
  mut new = 0
  for line in ($output | lines) {
    if ($line | str starts-with "diff --git ") {
      if $hunk != null { $hunks ++= [$hunk] }
      $hunk = null
      $path = null
      $old_path = null
      $number = 0
    } else if $path == null and ($line | str starts-with "--- ") {
      $old_path = (do $header_path ($line | str substring 4..))
    } else if $path == null and ($line | str starts-with "+++ ") {
      $path = (do $header_path ($line | str substring 4..) | default $old_path | from-here $here)
    } else if $path != null and ($line | str starts-with "@@ ") {
      if $hunk != null { $hunks ++= [$hunk] }
      let header = ($line | parse --regex '^@@ -(?<old>\d+)(?:,(?<old_count>\d+))? \+(?<new>\d+)(?:,(?<new_count>\d+))? @@ ?(?<at>.*)$' | first)
      $number += 1
      $old = ($header.old | into int)
      $new = ($header.new | into int)
      $hunk = {
        path: $path
        hunk: $number
        at: (if ($header.at | is-empty) { null } else { $header.at })
        old: $old
        old_count: (if ($header.old_count | is-empty) { 1 } else { $header.old_count | into int })
        new: $new
        new_count: (if ($header.new_count | is-empty) { 1 } else { $header.new_count | into int })
        lines: []
        text: [$line]
      }
    } else if $hunk != null {
      let mark = ($line | str substring 0..<1)
      let text = ($line | str substring 1..)
      if $mark == "+" {
        $hunk.lines ++= [{kind: added, old: null, new: $new, text: $text}]
        $new += 1
      } else if $mark == "-" {
        $hunk.lines ++= [{kind: deleted, old: $old, new: null, text: $text}]
        $old += 1
      } else if $mark == " " {
        $hunk.lines ++= [{kind: context, old: $old, new: $new, text: $text}]
        $old += 1
        $new += 1
      }
      # `\ No newline at end of file` is kept in the text, but is not a line of the file.
      $hunk.text ++= [$line]
    }
  }
  if $hunk != null { $hunks ++= [$hunk] }
  $hunks
}

# What changed, per file: path, added, deleted, versions. With --lines, per line.
#
# Which versions: without revisions, what is changed and not staged; with --staged, what is
# staged; with one revision, it against the working tree; with two, or a range, those
# against each other. The arguments are revisions and paths together, in any order: one
# that is a revision or a range is one, the rest are files or directories, from the
# current directory (a deleted file too). `versions` says what was compared (--unstaged,
# --staged, main...HEAD), and is how `git hunks` gets the same diff again.
#
# Per file: lines added and deleted; null for both in a binary file.
# --lines: one row per changed line: path, kind, old, new, text, hunk, at, versions. `kind`
# is added or deleted (or context, with --context). `old` is the line's number in the old
# version (null for an added line), `new` in the new one (null for a deleted line). `hunk`
# is which hunk of the file the line is in, `at` what git shows after `@@`: usually the
# function the hunk is in.
# To read a diff, pipe rows (per file or per line) into `git hunks`: the hunks as text,
# with context around them.
#
# A rename is a file deleted and one added. Untracked files are left out (`git status`).
@search-terms changes numstat stat patch lines hunks
@example "What is changed and not staged yet, per file" { git diff }
@example "Size of what a branch changes compared to main" { git diff main...HEAD | sort-by added --reverse }
@example "Read the changes in one file" { git diff | where path =~ cli | git hunks | get text }
@example "What a commit changed" { git diff 82223d8~1 82223d8 }
@example "Lines a branch adds that still say TODO, in their context" { git diff --lines main...HEAD | where kind == added and text =~ TODO | git hunks | get text }
@example "Which functions a change touches" { git diff --lines | get at | uniq }
export def diff [
  ...targets: string       # Revisions (HEAD~1, main...HEAD) and/or paths, in any order
  --staged (-s)            # What is staged, compared to the last commit
  --cached                 # The same as --staged
  --lines (-l)             # One row per changed line, instead of one per file
  --context (-c): int      # Unchanged lines around each change, as kind context (gives --lines)
  --unified (-U): int      # The same as --context
  --stat                   # One row per file, as without --lines (accepted as git takes it)
]: nothing -> table {
  # What git takes as a commit (or a range of commits) goes before `--`; everything else is a
  # path. A range (`a..b`, `a...b`) is one only when both its ends are commits, an empty end
  # being HEAD as for git: a path like `../README.md` (what `git status` gives in a
  # subdirectory) or `a..b.txt` contains `..` too. Each target is asked about once.
  let is_commit = {|name|
    (^git rev-parse --verify --quiet --end-of-options $"($name)^{commit}" | complete).exit_code == 0
  }
  let is_revision = {|target|
    let separator = if ($target | str contains "...") { "..." } else if ($target | str contains "..") { ".." } else { null }
    if $separator == null { return (do $is_commit $target) }
    let ends = ($target | split row $separator)
    # `..` alone is the parent directory: a range has at least one end of its own.
    if ($ends | all {|end| $end == "" }) { return false }
    (($ends | length) == 2) and ($ends | all {|end| do $is_commit (if $end == "" { "HEAD" } else { $end }) })
  }
  let classified = ($targets | each {|target| {target: $target, revision: (do $is_revision $target)} })
  let revisions = ($classified | where revision == true | each {|row| $row.target })
  let paths = ($classified | where revision == false | each {|row| $row.target })
  let versions = (versions-of $revisions ($staged or $cached))
  let context = ($context | default $unified)
  let here = (here)
  if not $lines and $context == null {
    return (
      run-git ([diff --numstat --no-renames -z] ++ (versions-arguments $versions) ++ [--] ++ $paths)
      | parse-numstat (char nul) $here
      | insert versions $versions
    )
  }
  let output = (run-git ([diff --no-color --no-ext-diff --no-renames ...$PREFIXES $"--unified=($context | default 0)"] ++ (versions-arguments $versions) ++ [--] ++ $paths))
  parse-hunks $output $here | each {|hunk|
    $hunk.lines | each {|line|
      {path: $hunk.path, kind: $line.kind, old: $line.old, new: $line.new, text: $line.text, hunk: $hunk.hunk, at: $hunk.at, versions: $versions}
    }
  } | flatten
}

# The hunks of a diff as text to read, with context: path, at, old, new, added, deleted, text.
#
# Takes rows from the pipeline. Rows of `git diff`: all hunks of those files. Rows of
# `git diff --lines`: only the hunks with one of those lines in them, each once. Rows with
# only a path (`git status`), or a list of paths: all hunks of those files, of what the
# arguments say (as for `git diff`; without, what is not staged).
# It asks git for the diff again, with 3 lines of context (--context), so the hunks are
# git's own: changes close together are one hunk. `at` is what git shows after `@@`, usually
# the function; `old` and `new` are where the hunk starts in each version. `text` is the
# hunk as git prints it: the `@@` line, then the lines with ` `, `+` or `-` in front.
# `get text` gives the texts alone, to read.
@search-terms diff patch read context unified
@example "Read what is changed in one file" { git diff | where path =~ route | git hunks | get text }
@example "Only the hunks inside one function" { git diff | git hunks | where at =~ save }
@example "Where a branch adds TODOs, in context" { git diff --lines main...HEAD | where text =~ TODO | git hunks | get text }
@example "What is staged, with more context" { git diff --staged | git hunks --context 10 | get text }
export def hunks [
  ...revisions: string     # For rows without `versions` (git status, paths): what to compare, as for git diff
  --staged (-s)            # For rows without `versions`: what is staged
  --cached                 # The same as --staged
  --context (-c): int = 3  # Unchanged lines around each change
  --unified (-U): int      # The same as --context
]: [table -> table, list<any> -> table, string -> table] {
  let input = $in
  if $input == null {
    error make {
      msg: "git hunks: no rows piped in"
      help: "Pipe in rows of git diff: git diff | where path =~ x | git hunks (or git diff --lines | where ... | git hunks)"
    }
  }
  let context = ($unified | default $context)
  let fallback = (versions-of $revisions ($staged or $cached))
  let rows = ($input | each {|item| if ($item | describe) =~ '^record' { $item } else { {path: $item} } })
  let here = (here)
  $rows
  | group-by {|row| $row.versions? | default $fallback }
  | transpose versions rows
  | each {|group|
    let wanted = $group.rows
    let paths = ($wanted | get path | uniq)
    let output = (run-git (
      [diff --no-color --no-ext-diff --no-renames ...$PREFIXES $"--unified=($context)"]
      ++ (versions-arguments $group.versions) ++ [--] ++ ($paths | each {|path| $":\(literal\)($path)" })
    ))
    # Line rows pick the hunks they are in; other rows (files, directories) all of theirs.
    let picked_lines = ($wanted | where {|row| $row.kind? != null })
    let whole = ($wanted | where {|row| $row.kind? == null } | get path)
    let in_whole = {|path|
      $whole | any {|entry| $entry in [. ./] or $path == $entry or ($path | str starts-with (($entry | str trim --right --char "/") + "/")) }
    }
    let has_line = {|hunk|
      $picked_lines | any {|row|
        $row.path == $hunk.path and (if $row.new? != null {
          $row.new >= $hunk.new and $row.new < $hunk.new + $hunk.new_count
        } else {
          $row.old? != null and $row.old >= $hunk.old and $row.old < $hunk.old + $hunk.old_count
        })
      }
    }
    parse-hunks $output $here
    | where {|hunk| (do $in_whole $hunk.path) or (do $has_line $hunk) }
    | each {|hunk|
      {
        path: $hunk.path
        at: $hunk.at
        old: $hunk.old
        new: $hunk.new
        added: ($hunk.lines | where kind == added | length)
        deleted: ($hunk.lines | where kind == deleted | length)
        text: ($hunk.text | str join "\n")
      }
    }
  }
  | flatten
}

# Another name for `git diff` (lines added and deleted per file), kept for older notes.
@search-terms diff numstat
@example "The same as git diff main...HEAD" { git changes main...HEAD }
export def changes [
  ...targets: string # Revisions and/or paths, as for git diff
  --staged (-s)      # What is staged, compared to the last commit
]: nothing -> table {
  diff ...$targets --staged=$staged
}

# Stage files, and get `git status` back to see the result.
#
# The files come as arguments, or from the pipeline: a table with a `path` column, like what
# `git status` and `git changes` give, or a list of paths (`git status | get path`). All
# paths are from the current directory, as those commands give them. Paths from the
# pipeline are taken exactly as written; as arguments, globs work as git reads them.
# A deleted file is staged as deleted. Rows of `git status` that are already staged
# entirely are skipped. An empty list or table stages nothing.
@search-terms stage index
@example "Stage everything that is modified, not what is new" { git status | where unstaged == modified | git add }
@example "Stage some files" { git add src/cli.ts src/help.ts }
@example "Stage everything, untracked files too (^git add -A)" { git add --all }
export def add [
  ...paths: path   # Files or directories, from the current directory
  --all (-A)       # Everything: changed, deleted and untracked files
]: [nothing -> table, list<string> -> table, table -> table] {
  let input = $in
  # Rows of our tables, or plain paths; as written, no globs (`:(literal)`). A row of
  # `git status` without unstaged changes has nothing to stage, and git would not find a
  # file whose deletion is already staged: skipped.
  let from_pipeline = if $input == null { [] } else {
    $input
    | where {|item|
        ($item | describe) !~ '^record' or ("unstaged" not-in ($item | columns)) or $item.unstaged != null
      }
    | each {|item| if ($item | describe) =~ '^record' { $item.path } else { $item } }
    | each {|path| $":\(literal\)($path)" }
  }
  let pathspecs = ($from_pipeline ++ $paths)
  if $all {
    run-git [add --all] | ignore
  } else if ($pathspecs | is-not-empty) {
    run-git ([add --] ++ $pathspecs) | ignore
  } else if $input == null {
    error make {
      msg: "git add: no files given"
      help: "Give paths, pipe in a list or a table with `path` (git status | where ... | git add), or use --all"
    }
  }
  status
}

# Where the repository stands, as a record: root, branch, head, short, upstream, ahead, behind, clean, remotes.
#
# `root` is the repository's top directory, as an absolute path (what the paths in git's
# own output are from). `branch` is null when HEAD is detached, `head` and `short` before
# the first commit.
# `upstream`, `ahead` and `behind` are null without an upstream branch; `ahead` is how many
# commits a push would send, `behind` how many a pull would bring in, as of the last fetch.
# `clean` is true when `git status` is empty (an untracked file counts as a change).
# `remotes` is a table: name, url.
@search-terms branch head upstream ahead behind push remote rev-parse toplevel root
@example "Is there anything to push?" { git info | get ahead }
@example "A file from the repository's top, wherever you are" { open (git info | get root | path join package.json) }
@example "The current commit, short" { git info | get short }
@example "Branch and upstream" { git info | select branch upstream ahead behind }
export def info []: nothing -> record {
  # One call gives the branch headers (`# branch.oid ...`) and, after them, the changed files.
  # The keys lose `branch.`: with the dot, `get` would read them as a path into the record.
  let entries = (run-git [status --porcelain=v2 --branch -z] | split row (char nul) | where $it != "")
  let headers = (
    $entries
    | where $it starts-with "# "
    | parse --regex '^# branch\.(?<key>\S+) (?<value>.*)$'
    | reduce --fold {} {|header, all| $all | insert $header.key $header.value }
  )
  let oid = ($headers | get --optional oid)
  let head = if $oid in [null "(initial)"] { null } else { $oid }
  let branch = ($headers | get --optional head)
  let ahead_behind = ($headers | get --optional ab | default "" | parse --regex '^\+(?<ahead>\d+) -(?<behind>\d+)$')
  let remotes = (
    run-git [remote -v]
    | lines
    | parse --regex '^(?<name>\S+)\t(?<url>\S+) \(fetch\)$'
  )
  # The root and the short hash in one call; before the first commit there is no hash.
  let parsed = (run-git ([rev-parse --show-toplevel] ++ (if $head == null { [] } else { [--short $head] })) | lines)
  {
    root: $parsed.0
    branch: (if $branch == "(detached)" { null } else { $branch })
    head: $head
    short: ($parsed | get --optional 1)
    upstream: ($headers | get --optional upstream)
    ahead: ($ahead_behind | get --optional 0.ahead | if $in == null { null } else { into int })
    behind: ($ahead_behind | get --optional 0.behind | if $in == null { null } else { into int })
    clean: ($entries | where not ($it starts-with "# ") | is-empty)
    remotes: $remotes
  }
}
