# Local servers: what is listening, and waiting until one answers.

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

# Wait until a server answers, then say how long that took.
#
# For a URL: until it gives any HTTP response (also a 404 or 500: the server is up), or with
# --status, until it gives one of those statuses (a health check that says 503 while
# starting). For a port number: until something on this machine listens on it. Gives a
# record: target, ready, status (of the HTTP response), waited. Not there in time is an error
# that says what it last answered. Use it after starting a server in the background (with your
# harness's way to run a command in the background), instead of sleeping and hoping.
@search-terms "wait for port" "wait for server" ready poll "health check"
@example "A dev server that was just started (a 404 on / counts: it is up)" { wait-for http://localhost:8888/ }
@example "Until the health check says 200" { wait-for http://localhost:3000/health --status [200] --timeout 2min }
@example "A port, for a server that does not speak HTTP" { wait-for 5432 }
export def wait-for [
  target: any                    # A URL, or a port number on this machine
  --timeout (-t): duration = 30sec # Give up after this long
  --interval (-i): duration = 250ms # Time between attempts
  --status (-s): list<int>       # For a URL: only these statuses count (default: any answer)
]: nothing -> record {
  let is_port = (($target | describe) == "int") or ($target | into string) =~ '^\d+$'
  if $is_port and (which ss | is-empty) {
    error make {msg: "wait-for a port needs `ss` (Linux), which is not installed", help: "Wait for a URL instead: wait-for http://localhost:<port>/"}
  }
  if $is_port and $status != null {
    error make {msg: "wait-for: --status is for a URL; a port has no status", label: {text: "a port", span: (metadata $target).span}}
  }
  let started = (date now)
  mut answer: any = null
  # What the server last answered (for the error when it never answered as wanted); -1: none.
  mut last = -1
  while $answer == null {
    let got = if $is_port {
      # Only whether something listens there: `ss` for that one port, not `ports`, which
      # looks up every process on the machine (four times a second, while waiting).
      let listening = (^ss -ltnH $"sport = :($target | into int)" | complete)
      if $listening.exit_code == 0 and ($listening.stdout | str trim | is-not-empty) { 0 } else { null }
    } else {
      try { http get --full --allow-errors --max-time 5sec ($target | into string) | get status } catch { null }
    }
    if $got != null { $last = $got }
    let wanted = $got != null and ($status == null or $got in $status)
    let waited = ((date now) - $started)
    if $wanted {
      $answer = {target: $target, ready: true, status: (if $is_port { null } else { $got }), waited: $waited}
    } else if $waited >= $timeout {
      let help = if $is_port {
        "Nothing listens on that port. See what does listen: ports"
      } else if $last != -1 {
        $"It answered, last with status ($last), not one of ($status | str join ', ')."
      } else {
        "No HTTP response. Is the server running, and on that port? See what listens: ports"
      }
      error make {
        msg: $"($target) did not answer as wanted within ($timeout)"
        help: $help
        label: {text: "waited for this", span: (metadata $target).span}
      }
    } else {
      sleep $interval
    }
  }
  $answer
}
