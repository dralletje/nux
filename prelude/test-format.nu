# Test results as a table: `nux/test-format`, the parsers of what test runners print, one
# per runner and format (`from tap`, `from node-26-test`, `from cargo-test`). Loaded in every
# call.
#
# There is no command here that runs tests. You run them the way the project does, and pipe
# what that prints into the parser for the test runner and version that printed it:
#
#     ^pnpm test | complete | from node-26-test
#
# (`complete`, because a test run with failures exits non-zero, and Nushell does not pipe the
# output of a failing command into a custom command: it raises an error before the parser
# runs. A parser also takes plain text, for output that is already in a variable or a file.)
#
# Every parser gives the same table (`test-results` below), so what you do with the rows does
# not depend on the runner. A parser does not guess: given output that is not in its format,
# it fails and says so.
#
# To support another runner or format (vitest, jest, pytest, ...): add
# `export def "from <runner>-test"` that reads what that runner prints (or its JSON or JUnit
# report) and ends in `| test-results`.

# What a test can end as. `skip`: not run on purpose. `todo`: marked as not finished yet,
# whatever it did.
const STATUSES = [pass fail skip todo]

# The result table every parser gives: one row per test, in the order they were reported.
#
#   name      string            the test's name, with the suites it is in: "suite > test"
#   status    string            pass, fail, skip or todo
#   duration  duration or null  how long it ran
#   file      string or null    the file it is in, relative to the current directory if inside
#   line      int or null       where in that file
#   error     string or null    the failure's message; null unless status is fail
#
# A parser builds rows with these columns and pipes them through this, which checks them and
# puts them in this shape. A row that does not fit is an error in the parser, reported here.
def test-results []: any -> table<name: string, status: string, duration: oneof<duration, nothing>, file: oneof<string, nothing>, line: oneof<int, nothing>, error: oneof<string, nothing>> {
  let here = (pwd)
  $in | each {|row|
    let status = ($row.status? | default "")
    if $status not-in $STATUSES {
      error make {msg: $"A test parser gave status '($status)' for test '($row.name?)'", help: $"It has to be one of: ($STATUSES | str join ', ')"}
    }
    if ($row.name? | describe) != "string" {
      error make {msg: "A test parser gave a test without a name", help: ($row | to nuon)}
    }
    {
      name: $row.name
      status: $status
      duration: ($row.duration? | default null | if ($in | describe) == "duration" { $in } else { null })
      file: (
        $row.file? | default null | if $in == null { null } else {
          let file = ($in | into string)
          try { $file | path relative-to $here } catch { $file }
        }
      )
      line: ($row.line? | default null | if $in == null { null } else { $in | into int })
      error: (if $status == "fail" { $row.error? | default null | if $in == null { null } else { $in | into string } } else { null })
    }
  }
}

# What a parser is given, and how it fails on input that is not its format: shared with
# `from tsc` (typescript.nu).
use nux/util [command-output not-the-format]

# "file:///a/b.test.js:4:1" or "/a/b.test.js:4:1" -> {file, line}
def parse-location []: any -> record {
  let found = ($in | default "" | into string | str replace --regex '^file://' '' | parse --regex '^(?<file>.+):(?<line>\d+):\d+$')
  if ($found | is-empty) { {file: null, line: null} } else { {file: $found.0.file, line: ($found.0.line | into int)} }
}

# Parse what Node 26's test runner prints into the test table.
#
# For the output of `node --test` (and of a package script that runs it) as Node 20 and newer
# print it when it is piped: lines like `✔ name (1.2ms)` and `✖ name (0.9ms)`, suites as
# `▶ name`, and the failures again at the end with their place and message. Checked with
# Node 26. The table: name, status, duration, file, line, error. This format only says where
# a test is (file, line) for the tests that failed.
#
# Output that has no such lines is an error that shows its start: the tests did not run.
# This is text meant for people, which Node may change between versions. Prefer `from tap`:
# it reads a standard format with the same information, which Node prints when asked
# (`--test-reporter=tap`; see `help from tap`).
#
# Put `complete` between the test command and this: `^pnpm test | complete | from node-26-test`.
# A test run with failures exits with an error code, and Nushell does not pass the output of
# such a command on to a command like this one; `complete` hands over output and code both.
@search-terms "test results" "failing tests" "node --test" "node:test" "pnpm test" "npm test"
@example "Which tests fail, and why" {
  ^pnpm test
  | complete
  | from node-26-test
  | where status == fail
  | select name file line error
}
@example "A summary" {
  ^npm test
  | complete
  | from node-26-test
  | group-by status --to-table
  | each {|group|
      {status: $group.status, tests: ($group.items | length)}
    }
}
@example "The slowest tests" {
  ^node --test
  | complete
  | from node-26-test
  | sort-by duration --reverse
  | first 5
  | select name duration
}
export def "from node-26-test" []: any -> table {
  let source = ($in | command-output)
  let text = $source.text
  let lines = ($text | lines)
  let result = '^(?<indent>\s*)(?<mark>[✔✖﹣⚠]) (?<name>.*) \((?<ms>[\d.]+)ms\)(?: # (?<note>.*))?$'

  # The failures are printed again after "failing tests:", each as
  #   test at <file>:<line>:<column>
  #   ✖ <name> (<duration>)
  #     <the error, indented>
  let failing_at = ($lines | enumerate | where item =~ '^✖ failing tests:' | get index | get 0?)
  let detail_lines = (if $failing_at == null { [] } else { $lines | skip ($failing_at + 1) })
  mut details = []
  mut place: any = {file: null, line: null}
  mut key: any = null
  mut message = []
  for line in ($detail_lines | append "test at end:0:0") {
    let at = ($line | parse --regex '^test at (?<where>.+)$')
    if ($at | is-not-empty) {
      if $key != null {
        # The message ends where the stack trace starts.
        let own = ($message | take until {|l| $l =~ '^\s+at ' } | str join "\n" | str trim | str replace --regex '^\w*Error(?: \[\w+\])?: ' '')
        $details = ($details | append {key: $key, file: $place.file, line: $place.line, error: $own})
      }
      $place = ($at.0.where | parse-location)
      $key = null
      $message = []
    } else {
      let found = ($line | parse --regex $result)
      if $key == null and ($found | is-not-empty) {
        $key = $"($found.0.name) \(($found.0.ms)ms\)"
      } else if $key != null {
        $message = ($message | append ($line | str replace --regex '^  ' ''))
      }
    }
  }
  let details = $details

  # The results themselves: everything before the summary (the lines that start with ℹ).
  let summary_at = ($lines | enumerate | where item =~ '^ℹ ' | get index | get 0?)
  let result_lines = (if $summary_at == null { $lines } else { $lines | first $summary_at })
  mut rows = []
  mut suites = []
  for line in $result_lines {
    let suite = ($line | parse --regex '^(?<indent>\s*)▶ (?<name>.*)$')
    if ($suite | is-not-empty) {
      let level = (($suite.0.indent | str length) // 2)
      $suites = ($suites | first $level | append $suite.0.name)
      continue
    }
    let found = ($line | parse --regex $result)
    if ($found | is-empty) { continue }
    let test = $found.0
    let level = (($test.indent | str length) // 2)
    # A suite ends with a result line of its own, at the level where it started.
    if ($suites | length) > $level and ($suites | get $level) == $test.name {
      $suites = ($suites | first $level)
      continue
    }
    let own_key = $"($test.name) \(($test.ms)ms\)"
    let detail = ($details | where key == $own_key | get 0? | default {file: null, line: null, error: null})
    let note = ($test.note? | default "")
    $rows = ($rows | append {
      name: ($suites | first $level | append $test.name | str join " > ")
      status: (if $test.mark == "﹣" { "skip" } else if $note =~ '^TODO' or $test.mark == "⚠" { "todo" } else if $test.mark == "✔" { "pass" } else { "fail" })
      duration: (($test.ms | into float) * 1ms)
      file: $detail.file
      line: $detail.line
      error: $detail.error
    })
  }
  if ($rows | is-empty) and $summary_at == null {
    let hint = if $text =~ '(?m)^TAP version' { "It is TAP: use `from tap`." } else { "Did the tests run at all? A build step before them may have failed." }
    $source | not-the-format "from node-26-test" "the output of Node 26's test runner (lines like `✔ name (1.2ms)`)" $hint
  }
  $rows | test-results
}

# Parse TAP, the Test Anything Protocol, into the test table.
#
# TAP is a plain-text format many test runners can print: lines like `ok 1 - name` and
# `not ok 2 - name`, each optionally followed by a block of details. It is a standard, so
# this is the parser to prefer where a runner offers TAP. The table: name, status, duration,
# file, line, error; a column is null where the TAP at hand does not say (the duration, and
# the place and message of a failure, are in the details block that Node writes; other
# runners may leave them out).
#
# Getting TAP from Node's test runner (`node --test`, `node:test`):
#   - Node 18 prints it by itself when piped:        ^node --test | complete | from tap
#   - Node 20 and newer need to be asked:           ^node --test --test-reporter=tap | complete | from tap
#   - through a package script (`pnpm test`), on any version, ask with NODE_OPTIONS:
#       with-env {NODE_OPTIONS: "--test-reporter=tap"} { ^pnpm test | complete } | from tap
# Checked with Node 18 and 26.
#
# Input that is not TAP is an error that shows its start: the tests did not run, or printed
# another format (Node 20+ without the reporter option: lines like `✔ name (1.2ms)`).
#
# Put `complete` between the test command and this, as above. A test run with failures exits
# with an error code, and Nushell does not pass the output of such a command on to a command
# like this one; `complete` hands over output and code both.
@search-terms "test results" "failing tests" "test anything protocol" "node --test" "node:test" "pnpm test" "npm test"
@example "Which tests fail, and why" {
  ^node --test --test-reporter=tap
  | complete
  | from tap
  | where status == fail
  | select name file line error
}
@example "A package script on Node 20 or newer" {
  with-env {NODE_OPTIONS: "--test-reporter=tap"} {
    ^pnpm test | complete
  }
  | from tap
  | where status != pass
}
@example "A summary" {
  ^node --test --test-reporter=tap
  | complete
  | from tap
  | group-by status --to-table
  | each {|group|
      {status: $group.status, tests: ($group.items | length)}
    }
}
export def "from tap" []: any -> table {
  let source = ($in | command-output)
  let text = $source.text
  if $text !~ '(?m)^\s*(TAP version \d+|1\.\.\d+|(not )?ok\b)' {
    let hint = if $text =~ '(?m)^\s*[✔✖] .* \([\d.]+ms\)' { "It is what Node 20 and newer print by default. Ask for TAP: --test-reporter=tap, or for a package script NODE_OPTIONS (see: help from tap). Or parse this format: from node-26-test." } else { "Did the tests run at all? A build step before them may have failed." }
    $source | not-the-format "from tap" "TAP (lines like `ok 1 - name`)" $hint
  }
  let lines = ($text | lines)
  let count = ($lines | length)
  mut rows = []
  mut suites = []
  # Per nesting level: whether a result was seen one level deeper since the last result here.
  mut has_children = []
  mut index = 0
  while $index < $count {
    let line = ($lines | get $index)
    $index += 1
    let subtest = ($line | parse --regex '^(?<indent>\s*)# Subtest: (?<name>.*)$')
    if ($subtest | is-not-empty) {
      let level = (($subtest.0.indent | str length) // 4)
      $suites = ($suites | first $level | append $subtest.0.name)
      continue
    }
    let found = ($line | parse --regex '^(?<indent>\s*)(?<not>not )?ok\b(?: +(?<number>\d+))?(?: +-)?(?: +(?<name>.*?))?(?: +# *(?<directive>(?i:skip|todo))\b.*)?$')
    if ($found | is-empty) { continue }
    let test = (
      $found.0
      | update name {|row| if ($row.name | is-empty) { $"test ($row.number | default '?')" } else { $row.name } }
      | update directive {|row| $row.directive | default "" | str uppercase }
    )
    let level = (($test.indent | str length) // 4)
    # A result that closes a group of deeper results is that group (a suite), not a test.
    let is_group = ($has_children | get --optional $level | default false)
    $has_children = ($has_children | first $level | append false)
    if $level > 0 { $has_children = ($has_children | update ($level - 1) true) }
    # The details: a YAML block between `---` and `...`, two spaces deeper than the result.
    let inner = $"($test.indent)  "
    mut block = []
    if $index < $count and ($lines | get $index) == $"($inner)---" {
      $index += 1
      while $index < $count and ($lines | get $index) != $"($inner)..." {
        $block = ($block | append ($lines | get $index | str substring ($inner | str length)..))
        $index += 1
      }
      $index += 1
    }
    let block_text = ($block | str join "\n")
    let info = (try { $block_text | from yaml } catch { {} } | default {})
    let info = (if ($info | describe) =~ '^record' { $info } else { {} })
    if $is_group or ($info.type? | default "test") == "suite" { continue }
    let place = ($info.location? | parse-location)
    $rows = ($rows | append {
      name: ($suites | first $level | append $test.name | str join " > ")
      status: (if $test.directive == "SKIP" { "skip" } else if $test.directive == "TODO" { "todo" } else if ($test.not | is-empty) { "pass" } else { "fail" })
      duration: (if ($info.duration_ms? | default null) == null { null } else { ($info.duration_ms | into float) * 1ms })
      file: $place.file
      line: $place.line
      error: ($info.error? | default ($info.message? | default null) | if $in == null { null } else { $in | into string | str trim })
    })
  }
  $rows | test-results
}

# Parse what `bun test` prints into the test table.
#
# Bun prints its report on stderr: a line per file (`test/math.test.ts:`), a line per test
# (`(pass) suite > name`, `(fail) ...`, `(skip)`, `(todo)`, a time in brackets for slow ones),
# and above each `(fail)` its error: the code around it, `error: ...`, and where (`at ...
# (file:line:col)`). A failed test gets that message, file and line. Checked with Bun 1.3.
#
# Bun in some environments (Claude Code sets CLAUDECODE=1) lists only failing tests; their
# number of passing ones is then only in its summary, and this says so on stderr, since they
# cannot be rows. An error outside any test (a syntax error, an import that failed) with no
# test to report is an error here, never an empty table.
#
# Put `complete` between `bun test` and this: it fails when a test fails, and hands over both
# streams.
@search-terms bun "bun test" "failing tests" "test results"
@example "Which tests fail, and why" {
  ^bun test
  | complete
  | from bun-test
  | where status == fail
  | select name file line error
}
@example "Through a package script" { ^bun run test | complete | from bun-test | where status != pass }
export def "from bun-test" []: any -> table {
  let source = ($in | command-output)
  let lines = ($source.stderr | ansi strip | lines | append ($source.text | ansi strip | lines))
  let summary = {|word|
    $lines | parse --regex ('^\s*(?<n>\d+) ' + $word + '$') | get 0?.n? | default "0" | into int
  }
  mut rows = []
  # The file the lines are about; "" before the first.
  mut file = ""
  mut block = []
  for line in $lines {
    let header = ($line | parse --regex '^(?<file>[^\s:()][^:]*\.[A-Za-z]+):$')
    if ($header | is-not-empty) {
      $file = $header.0.file
      $block = []
      continue
    }
    let result = ($line | parse --regex '^\((?<status>pass|fail|skip|todo)\) (?<name>.+?)(?: \[(?<time>[\d.]+)(?<unit>ms|s)\])?$')
    if ($result | is-empty) {
      $block ++= [$line]
      continue
    }
    let r = $result.0
    # The error above a failure: from its `error:` line, without the `at ...` lines.
    let error_lines = ($block | skip until {|l| $l =~ '^error:' })
    let place = ($error_lines | parse --regex 'at .*?\(?(?<file>/[^():]+|[\w./-]+\.\w+):(?<line>\d+):\d+\)?$' | get 0?)
    let message = ($error_lines | where $it !~ '^\s+at ' | str join "\n" | str replace --regex '^error: ' '' | str trim)
    $rows ++= [{
      name: $r.name
      status: $r.status
      duration: (if ($r.time | is-empty) { null } else if $r.unit == "s" { ($r.time | into float) * 1sec } else { ($r.time | into float) * 1ms })
      file: (if $place != null { $place.file } else if $file != "" { $file } else { null })
      line: (if $place != null { $place.line } else { null })
      error: (if $r.status == "fail" { if $message == "" { "failed" } else { $message } } else { null })
    }]
    $block = []
  }
  let errors = (do $summary "error")
  if ($rows | is-empty) and ($source.exit_code not-in [null 0] or $errors > 0) {
    $source | not-the-format "from bun-test" "a bun test report with test results (lines like `(pass) name`)" "No test ran to the end: an error outside any test (a syntax error, an import that failed)?"
  }
  let passes = (do $summary "pass")
  let listed = ($rows | where status == pass | length)
  if $passes > $listed {
    print -e $"from bun-test: ($passes - $listed) of ($passes) passing tests are not in the table: bun listed only failures \(it does with CLAUDECODE=1\); its summary has them."
  }
  if $errors > 0 {
    print -e $"from bun-test: bun also reported ($errors) error\(s\) outside any test; see its output."
  }
  $rows | test-results
}

# Parse what `swift test` prints into the test table: XCTest and swift-testing both (a
# package runs both, one after the other).
#
#   XCTest:         Test Case 'MathTests.testAdds' passed (0.004 seconds)
#                   /path/MathTests.swift:6: error: MathTests.testAddsWrong : XCTAssertEqual failed: ...
#   swift-testing:  ✔ Test joins() passed after 0.001 seconds.
#                   ✘ Test joinsWrong() recorded an issue at StringTests.swift:6:29: Expectation failed: ...
#                   ➜ Test later() skipped: "not yet"
#
# A failed test gets its issues as `error` (all of them, with swift-testing's `↳` details),
# and the file and line of the first. XCTest names come as "MathTests > testAdds";
# swift-testing ones as printed ("joins()"): it runs tests in parallel and does not say which
# suite a test is in. Checked with Swift 6.4 on Linux (macOS prints XCTest names as
# `-[MathTests testAdds]`, also read).
#
# The build's messages are on stderr: give it the record of `complete`. A build that failed
# (no test ran) is an error that shows the end of stderr, never an empty table.
@search-terms swift "swift test" xctest "swift-testing" "failing tests" "test results"
@example "Which tests fail, and why" {
  ^swift test
  | complete
  | from swift-test
  | where status == fail
  | select name file line error
}
export def "from swift-test" []: any -> table {
  let source = ($in | command-output)
  let lines = ($source.text | ansi strip | lines)
  mut rows = []
  # Issues per test, in order: XCTest errors before its result line, swift-testing ones as
  # `recorded an issue` lines with their continuation.
  mut issues = {}
  mut open = ""
  for line in $lines {
    # XCTest: a test's result.
    let xc = ($line | parse --regex "^Test Case '(?:-\\[(?<s1>\\S+) (?<t1>\\S+)\\]|(?<s2>[^.']+)\\.(?<t2>[^']+))' (?<result>passed|failed|skipped) \\((?<secs>[\\d.]+) seconds\\)")
    if ($xc | is-not-empty) {
      let x = $xc.0
      let name = if ($x.s1 | is-not-empty) { $"($x.s1) > ($x.t1)" } else { $"($x.s2) > ($x.t2)" }
      $rows ++= [{name: $name, status: (match $x.result { "passed" => "pass", "failed" => "fail", _ => "skip" }), duration: (($x.secs | into float) * 1sec)}]
      $open = ""
      continue
    }
    # XCTest: an error of a test, before its result.
    let xe = ($line | parse --regex '^(?<file>[^:]+):(?<line>\d+): error: (?:-\[(?<s1>\S+) (?<t1>\S+)\]|(?<s2>[^. ]+)\.(?<t2>\S+)) : (?<message>.*)$')
    if ($xe | is-not-empty) {
      let e = $xe.0
      let name = if ($e.s1 | is-not-empty) { $"($e.s1) > ($e.t1)" } else { $"($e.s2) > ($e.t2)" }
      $issues = ($issues | upsert $name (($issues | get --optional $name | default []) | append {file: $e.file, line: $e.line, message: $e.message}))
      $open = ""
      continue
    }
    # swift-testing: a line that starts with a symbol and "Test" (or "Suite").
    # (Not the run's own last line: `✘ Test run with 4 tests in 1 suite failed after ...`.)
    let st = ($line | parse --regex '^\S+ Test (?<name>.+?) (?:(?<result>passed|failed) after (?<secs>[\d.]+) seconds|(?<skipped>skipped)|recorded an (?:issue|known issue) at (?<file>[^:]+):(?<line>\d+):\d+: (?<message>.*))' | where name !~ '^run with \d+ tests?')
    if ($st | is-not-empty) {
      let s = $st.0
      if ($s.message | is-not-empty) or ($s.file | is-not-empty) {
        $issues = ($issues | upsert $s.name (($issues | get --optional $s.name | default []) | append {file: $s.file, line: $s.line, message: $s.message}))
        $open = $s.name
      } else {
        let status = if ($s.skipped | is-not-empty) { "skip" } else if $s.result == "passed" { "pass" } else { "fail" }
        $rows ++= [{name: $s.name, status: $status, duration: (if ($s.secs | is-empty) { null } else { ($s.secs | into float) * 1sec })}]
        $open = ""
      }
      continue
    }
    # The rest of an issue's message: lines that start no new event.
    if $open != "" and $line !~ '^(\S+ (Test|Suite) |Test (Case|Suite) |\s+Executed )' and $line != "" {
      let list = ($issues | get $open)
      let last = ($list | last)
      $issues = ($issues | upsert $open ($list | drop 1 | append ($last | update message $"($last.message)\n($line)")))
      continue
    }
    $open = ""
  }
  if ($rows | is-empty) {
    $source | not-the-format "from swift-test" "what swift test prints (lines like `Test Case '...' passed` or `✔ Test name() passed`)" "Did it build? The compiler's errors are on stderr: give from swift-test the whole record of `complete`."
  }
  let issues = $issues
  $rows | each {|row|
    let found = ($issues | get --optional $row.name | default [])
    let first = ($found | get 0?)
    {
      name: $row.name
      status: $row.status
      duration: $row.duration
      file: ($first.file? | default null)
      line: ($first.line? | default null)
      error: (if $row.status == "fail" { if ($found | is-empty) { "failed" } else { $found | get message | str join "\n\n" } } else { null })
    }
  } | test-results
}

# Parse what `cargo test` prints into the test table.
#
# For Rust's built-in test harness, as `cargo test` (or `cargo nextest`-less `cargo test
# --workspace`) prints it: `test name ... ok`, `... FAILED`, `... ignored`, then for each
# failure a `---- name stdout ----` block with its panic. A failed test gets its file and
# line from the panic (`panicked at src/lib.rs:12:9`) and the panic's message as `error`.
# The harness prints no time per test: `duration` is null. Checked with Rust 1.100.
#
# The test lines are on stdout and cargo's own messages (compiling, which binary runs) on
# stderr: give it the whole record of `complete`. Output without test lines, from a command
# that failed (it did not compile), is an error that shows the end of stderr.
@search-terms rust cargo "cargo test" "failing tests" "test results"
@example "Which tests fail, and why" {
  ^cargo test
  | complete
  | from cargo-test
  | where status == fail
  | select name file line error
}
@example "A summary" {
  ^cargo test --workspace
  | complete
  | from cargo-test
  | group-by status --to-table
  | each {|group|
      {status: $group.status, tests: ($group.items | length)}
    }
}
export def "from cargo-test" []: any -> table {
  let source = ($in | command-output)
  let lines = ($source.text | lines)
  # Doc tests have spaces in their names: `test src/lib.rs - add (line 3) ... ok`.
  let results = ($lines | parse --regex '^test (?<name>.+?) \.\.\. (?<result>ok|FAILED|ignored)(?:,.*)?$')
  let ran = ($lines | any {|line| $line =~ '^running \d+ tests?$' })
  if ($results | is-empty) and (not $ran or $source.exit_code not-in [null 0]) {
    $source | not-the-format "from cargo-test" "what cargo test prints (lines like `test name ... ok`)" "Did it compile? Compiler errors are on stderr: give from cargo-test the whole record of `complete`."
  }
  # The failures: `---- name stdout ----`, up to the next one, or up to the list of names
  # (`failures:`) that ends them.
  mut failures = {}
  # The test whose block this is; "" outside a block.
  mut current = ""
  mut block = []
  for line in $lines {
    let start = ($line | parse --regex '^---- (?<name>.+) stdout ----$')
    if ($start | is-not-empty) or ($current != "" and $line == "failures:") {
      if $current != "" {
        $failures = ($failures | upsert $current ($block | str join "\n" | str trim))
      }
      $current = if ($start | is-not-empty) { $start.0.name } else { "" }
      $block = []
      continue
    }
    if $current != "" { $block ++= [$line] }
  }
  let failures = $failures
  $results | each {|test|
    let output = ($failures | get --optional $test.name)
    let place = if $output == null { [] } else { $output | parse --regex "panicked at (?<file>[^\n]+?):(?<line>\\d+):\\d+:" }
    # The panic's message: what follows `panicked at ...:`, without the backtrace note.
    let message = if $output == null { null } else {
      $output
      | str replace --regex "(?s)^.*?panicked at [^\n]+:\n" ""
      | lines
      | where $it !~ '^note: run with `RUST_BACKTRACE'
      | str join "\n"
      | str trim
    }
    {
      name: $test.name
      status: (match $test.result { "ok" => "pass", "FAILED" => "fail", _ => "skip" })
      duration: null
      file: ($place.0?.file?)
      line: ($place.0?.line?)
      error: (if $test.result == "FAILED" { $message | default "failed" } else { null })
    }
  } | test-results
}
