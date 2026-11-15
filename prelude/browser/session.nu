# Finding the browser tool, and a browser session for the length of a closure.

# By its name in the binary (src/prelude.rs): `../util.nu` is not found there, because Nushell
# does not resolve `..` in the names of virtual files.
use nux/util.nu find-up

# Where the browser tool is: $env.NUX_BROWSER, a `browser` on PATH, or the nearest
# .claude/bin/browser.
export def browser-tool []: nothing -> string {
  if ($env.NUX_BROWSER? | is-not-empty) { return $env.NUX_BROWSER }
  let on_path = (which --all browser | where type == external)
  if ($on_path | is-not-empty) { return $on_path.0.path }
  let found = (find-up ".claude/bin/browser" (pwd))
  if $found == null {
    error make {msg: "The browser tool was not found", help: "It is .claude/bin/browser in a repository that has it. Go there with cd, or set $env.NUX_BROWSER to its path."}
  }
  $found
}

# Start a browser session, run the closure with $env.BROWSER_SESSION set to its name (the
# closure also gets the name), and stop the session, whatever the closure did.
export def with-session [
  body: closure
  wait: duration # How long the browser may take to start
]: any -> any {
  let input = $in
  let tool = (browser-tool)
  let name = $"nux-(random chars --length 8 | str lowercase)"
  let log = (mktemp --tmpdir $"browser-($name)-XXXXXX.log")
  # The session is a process that keeps running: a job of this Nushell, stopped below.
  let job = (job spawn --description $"browser ($name)" { ^$tool serve $name o+e> $log })
  let stop = {||
    try { ^$tool $name stop | complete | ignore }
    try { job kill $job }
    rm --force $log
  }
  try {
    let deadline = (date now) + $wait
    loop {
      let said = (open --raw $log)
      if $said =~ $"\\[($name)\\] ready" { break }
      if $job not-in (job list | get id) {
        error make {msg: "The browser did not start", help: ($said | str trim)}
      }
      if (date now) > $deadline {
        error make {msg: $"The browser was not ready after ($wait)", help: ($said | str trim)}
      }
      sleep 100ms
    }
    with-env {BROWSER_SESSION: $name} { $input | do $body $name }
  } finally {
    do $stop
  }
}
