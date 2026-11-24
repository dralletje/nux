# Local servers: what is listening; and waiting, until a server answers, a file is there or
# a process has ended.

# What is listening on TCP ports of this machine, as a table: port, address, pid, name,
# command, cwd.
#
# `command` and `cwd` say which program it is and where it was started (which project's dev
# server). They are null for a process of another user. A server listening on both IPv4 and
# IPv6 has a row for each address.
@search-terms listening sockets lsof netstat "who uses port"
@example "Everything that listens" { ports | select port name cwd }
@example "What is on port 8888" { ports 8888 }
@example "Free a port: stop whatever listens on it" {
  ports 8888
  | get pid
  | uniq
  | each {|pid|
      kill $pid
    }
}
export def ports [
  port?: int # Only this port
]: nothing -> table {
  if (which ss | is-empty) {
    error make {msg: "ports needs `ss` (Linux), which is not installed", help: "Elsewhere: ^lsof -iTCP -sTCP:LISTEN -nP"}
  }
  let processes = (ps --long | select pid command cwd)
  let rows = (
    ^ss -ltnpH | lines | each {|line|
      let local = ($line | split row --regex '\s+' | get 3)
      let place = ($local | parse --regex '^(?<address>.*):(?<port>\d+)$' | first)
      let owner = ($line | parse --regex 'users:\(\("(?<name>[^"]*)",pid=(?<pid>\d+)' | get 0?)
      let pid = ($owner.pid? | default null | if $in == null { null } else { $in | into int })
      let process = (if $pid == null { null } else { $processes | where pid == $pid | get 0? })
      {
        port: ($place.port | into int)
        address: ($place.address | str trim --char "[" | str trim --char "]")
        pid: $pid
        name: ($owner.name? | default null)
        command: ($process.command? | default null)
        cwd: ($process.cwd? | default null)
      }
    } | sort-by port
  )
  if $port == null { $rows } else { $rows | where port == $port }
}

# Time between two looks, for every wait-for.
const INTERVAL = 250ms

# Try `check` until it says ready. `check` gets what it gave the time before (null the first
# time), so it can carry state (a file's size), and gives a record with `ready` and whatever
# else to report. Gives that record, with target, ready and waited first.
#
# No time limit here: a wait goes on until it is ready. A limit goes around it, as around any
# code: timeout 2min { wait-for ... }. When it is stopped (by `timeout`, or an interrupt), it
# says on stderr what it last saw, which `describe` makes of the last record, and is stopped.
def poll [kind: string, target: any, check: closure, describe: closure]: nothing -> any {
  let started = (date now)
  let stopped = {|last, error|
    let waited = $"((((date now) - $started) / 1sec) | math round --precision 1)sec"
    let saw = if $last == null { "it had not looked yet" } else { do $describe $last }
    print -e $"wait-for ($kind) ($target): stopped after ($waited); ($saw)"
    error make $error.raw
  }
  mut last: any = null
  mut result: any = null
  while $result == null {
    # A copy that a closure can see (it cannot see a `mut`).
    let before = $last
    let got = (try { do $check $before } catch {|error| do $stopped $before $error })
    if $got.ready {
      $result = ({target: $target, ready: true, waited: ((date now) - $started)} | merge ($got | reject ready))
    } else {
      $last = $got
      try { sleep $INTERVAL } catch {|error| do $stopped $got $error }
    }
  }
  $result
}

# Wait for something, then say how long that took. Say what it is: a URL, a port, a file or a
# process.
#
#   wait-for url <url>      until a server answers (any HTTP status; --status for some)
#   wait-for port <port>    until something on this machine listens on it
#   wait-for file <path>    until it is there (--contains text, --stable for done writing)
#   wait-for pid <pid>      until that process has ended
#
# Each gives a record: target, ready, waited (and more per kind). Use it instead of `sleep`
# and looking again, after starting something in the background (with your harness's way to
# run a command in the background).
#
# A wait has no time limit of its own: it goes on until what it waits for happens. To give
# up after a while, put it in `timeout`: timeout 2min { wait-for url http://localhost:3000/ }
@search-terms "wait for port" "wait for server" "wait for file" ready poll "health check" sleep
@example "A dev server that was just started, for at most a minute" { timeout 1min { wait-for url http://localhost:8888/ } }
@example "A database's port" { timeout 30sec { wait-for port 5432 } }
@example "A background job's output, until it says it is done" { timeout 10min { wait-for file /tmp/build.log --contains "Done in" } }
export def --wrapped wait-for [...arguments] {
  let first = ($arguments | get 0? | default "" | into string)
  let rest = ($arguments | each {|argument| $argument | into string } | str join " ")
  let kind = if $first =~ '^https?://' { "url" } else if $first =~ '^\d+$' { "port" } else if $first != "" { "file" } else { null }
  let guess = if $kind == null { "" } else { $"\nFor this: wait-for ($kind) ($rest)" }
  error make {
    msg: "wait-for: say what to wait for: url, port, file or pid"
    help: $"wait-for url <url>     wait-for port <port>     wait-for file <path>     wait-for pid <pid>($guess)"
    label: {text: "what is this?", span: (metadata $arguments).span}
  }
}

# Wait until a server answers at this URL: any HTTP response, also a 404 or 500 (the server
# is up); with --status, only one of those (a health check that says 503 while starting).
#
# Gives a record: target, ready, waited, status. No time limit of its own: put it in
# `timeout`. Not curl in a loop.
@search-terms "wait for server" "health check" ready poll curl
@example "A dev server that was just started (a 404 on / counts: it is up)" { timeout 1min { wait-for url http://localhost:8888/ } }
@example "Until the health check says 200" { timeout 2min { wait-for url http://localhost:3000/health --status [200] } }
export def "wait-for url" [
  url: string              # The URL to ask, e.g. http://localhost:3000/
  --status (-s): list<int> # Only these statuses count (default: any answer)
]: nothing -> record {
  let check = {|before|
    let got = (try { http get --full --allow-errors --max-time 5sec $url | get status } catch { null })
    # The last status there was: an attempt without an answer keeps it, to say when stopped.
    {ready: ($got != null and ($status == null or $got in $status)), status: ($got | default ($before.status? | default null))}
  }
  let describe = {|last|
    if $last.status == null {
      "no HTTP response yet: is the server running, and on that port? (ports)"
    } else {
      $"it answered, last with status ($last.status), not one of ($status | str join ', ')"
    }
  }
  poll url $url $check $describe
}

# Wait until something on this machine listens on this TCP port: for a server that does not
# speak HTTP (a database). Gives a record: target, ready, waited. No time limit of its own:
# put it in `timeout`.
@search-terms "wait for port" listening ready poll
@example "Postgres, after starting its container" { timeout 1min { wait-for port 5432 } }
export def "wait-for port" [
  port: int # The port number
]: nothing -> record {
  if (which ss | is-empty) {
    error make {msg: "wait-for port needs `ss` (Linux), which is not installed", help: "Wait for a URL instead: wait-for url http://localhost:<port>/"}
  }
  # Only whether something listens there: `ss` for that one port, not `ports`, which looks up
  # every process on the machine (four times a second, while waiting).
  let check = {|before|
    let listening = (^ss -ltnH $"sport = :($port)" | complete)
    {ready: ($listening.exit_code == 0 and ($listening.stdout | str trim | is-not-empty))}
  }
  poll port $port $check {|last| "nothing listened on it yet (what does listen: ports)" }
}

# Wait until a file is there; with --contains, until its text has that in it; with --stable,
# until its size has not changed for that long (whoever writes it is done).
#
# For the output file of a command running in the background: instead of sleeping and then
# looking at its size. Gives a record: target, ready, waited, size. No time limit of its own:
# put it in `timeout`.
@search-terms "wait for file" "wait for output" sleep tail poll
@example "Until a log says the server is ready" { timeout 1min { wait-for file /tmp/dev.log --contains "ready in" } }
@example "Until a background job's output stops growing" { timeout 10min { wait-for file /tmp/tests.out --stable 5sec } }
export def "wait-for file" [
  path: path              # The file
  --contains (-c): string # Until its text has this in it (plain text, not a pattern)
  --stable (-s): duration # Until its size has not changed for this long
]: nothing -> record {
  let check = {|before|
    if not ($path | path exists) {
      return {ready: false, size: null, since: null, found: false}
    }
    let size = (ls --long $path | get 0.size)
    # Since when the size is what it is now: when it changed, now.
    let since = if ($before.size? | default null) == $size { $before.since } else { date now }
    let found = if $contains == null { true } else {
      try { open --raw $path | decode utf-8 | str contains $contains } catch { false }
    }
    let settled = ($stable == null or ((date now) - $since) >= $stable)
    {ready: ($found and $settled), size: $size, since: $since, found: $found}
  }
  let describe = {|last|
    if $last.size == null {
      "the file was not there yet"
    } else if not $last.found {
      $"the file was there \(($last.size)\), without: ($contains)"
    } else {
      $"the file was still changing \(($last.size), last at ($last.since | format date '%H:%M:%S')\)"
    }
  }
  poll file $path $check $describe | reject since found
}

# Wait until a process has ended (by its pid). Gives a record: target, ready, waited. No time
# limit of its own: put it in `timeout`.
#
# Only that it ended: not how. For a command whose exit code matters, run it in your
# harness's background and let that say when it is done.
@search-terms "wait for process" "wait for pid" ended exit sleep
@example "Until a build started earlier is done" { timeout 5min { wait-for pid 12345 } }
export def "wait-for pid" [
  pid: int # The process id
]: nothing -> record {
  # A zombie has ended: it only waits for its parent to read its exit code.
  let check = {|before| {ready: (ps | where pid == $pid and status != Zombie | is-empty)} }
  poll pid $pid $check {|last| $"it was still running \(ps | where pid == ($pid)\)" }
}
