# `browser do { ... }`. It is the `main` of a module named `do`, which mod.nu takes in with
# `export use do.nu`: that way no command named `do` is defined anywhere, and Nushell's own
# `do` keeps working in these files. (`export def "main do"` in mod.nu would be named
# `browser main do`, and `export def do` there hides the built-in in the whole module.)

use session.nu with-session

# Run a closure with a browser of its own: one is started, `browser` inside the closure
# talks to it, and it is closed when the closure ends, also when it fails.
#
# The closure gets the session's name, which is rarely needed: `browser` already uses this
# session inside the closure (also in `each` and `par-each` there). The pipeline input goes
# to the closure, and what the closure returns comes back.
#
# Starting a browser takes a few seconds, so do everything you want from it in one closure.
# For a browser that stays open between calls, start a browser session yourself
# (`nux help browser`).
@search-terms playwright chromium page web
@example "Look at one page" { browser do { browser navigate http://localhost:8888/ | select url status title failed_requests } }
@example "Check a list of pages with one browser" {
  [/ /about /contact]
  | browser do {
      each {|path|
        browser navigate $"http://localhost:8888($path)"
        | select url status title
      }
    }
}
@example "Several steps" {
  browser do {
    browser navigate http://localhost:8888/login | ignore
    browser type 'input[name=email]' me@example.com | ignore
    browser click 'button[type=submit]' | get url
  }
}
export def main [
  body: closure            # What to do with the browser; its parameter is the session name
  --wait: duration = 90sec # How long the browser may take to start
]: any -> any {
  with-session $body $wait
}
