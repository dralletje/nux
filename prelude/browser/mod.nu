# The `browser` tool (packages/browser-cli: a real browser that stays open between commands),
# with data as result instead of text.
#
# A module: `use browser` gives `browser <operation>` (main, below) and `browser do { ... }`
# (do.nu), and nothing else. The prelude does that for every call.

use session.nu browser-tool
export use do.nu

# Do one thing in a browser session and get the outcome as data.
#
# The operations and their arguments are those of the browser tool (`^browser help` in a
# shell, or `browser tools`): navigate, click, type, status, evaluate, network_requests, ...
# What comes back depends on the operation:
#   - one that reports on the page (navigate, click, type, status, ...): a record with url,
#     changed_from, title, status, headings, text, counts, failed_requests, console,
#     elements (ref, role, name, ...), broken_images, snapshot_file
#   - network_requests, console_messages, tabs list: a table
#   - evaluate: the value the function returned
#   - anything else: its text
# A failed operation is an error.
#
# A session has to be running. For one piece of work, `browser do { ... }` starts one and
# closes it afterwards. For a browser that stays open between calls (a login, a page you
# come back to), start it once as a BACKGROUND shell command (not in nux),
# `browser serve <name>`, and tell this session its name: `$env.BROWSER_SESSION = "<name>"`.
# When you just want to look at a page yourself, the tool's own text report is easier to
# read: use it from the shell. This command is for filtering and for many pages at once.
@search-terms playwright chromium page web
@example "Open a page and look at what failed" { browser navigate http://localhost:8888/ | select url status failed_requests console }
@example "Check a list of pages" { [/ /about /contact] | each {|path| browser navigate $"http://localhost:8888($path)" | select url status title failed_requests } }
@example "The links on the page" { browser status | get elements | where role == link | select ref name url }
@example "Requests that did not succeed" { browser network_requests | where status >= 400 }
@example "A value from the page" { browser evaluate '() => document.querySelectorAll("img").length' }
export def main [
  operation: string       # navigate, click, type, status, evaluate, network_requests, ...
  ...arguments: any       # Its arguments, as the browser tool takes them: values, key=value
  --session (-s): string  # Session name (default: $env.BROWSER_SESSION)
  --quiet (-q)            # Skip the page report after an action
  --text (-t)             # Give the tool's text output instead of data
]: nothing -> any {
  let tool = (browser-tool)
  let name = ($session | default $env.BROWSER_SESSION?)
  if ($name | is-empty) {
    error make {
      msg: "No browser session named"
      help: $"For one piece of work: browser do { browser ($operation) ... }\nFor a browser that stays open, start one as a background shell command \(not in nux\): ($tool) serve <name>\nThen: $env.BROWSER_SESSION = \"<name>\"   \(or pass --session <name>\)"
    }
  }
  let flags = [--json] ++ (if $quiet { [--quiet] } else { [] })
  let run = (run-external $tool $name $operation ...($arguments | each { into string }) ...$flags | complete)
  let reply = (try { $run.stdout | from json } catch { null })
  if $reply == null {
    error make {msg: $"browser ($operation) gave no usable answer", help: ($"($run.stdout)($run.stderr)" | str trim)}
  }
  if not $reply.ok {
    let lines = ($reply.text | lines)
    error make {
      msg: ($lines | first | default $"browser ($operation) failed")
      help: ($lines | skip 1 | str join "\n")
      label: {text: "this operation", span: (metadata $operation).span}
    }
  }
  if $text { return $reply.text }
  if "page" in $reply {
    if "result" in $reply { $reply.page | insert result $reply.result } else { $reply.page }
  } else if "result" in $reply {
    $reply.result
  } else {
    $reply.text
  }
}
