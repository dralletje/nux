# cookie-jar: keep cookies between `http` calls, the way a browser does.
#
#     use nux/cookie-jar      # without `*`: its `update`, `http` and `list` would replace Nushell's
#     let base = "http://localhost:8888"
#
#     let admin_jar = (cookie-jar new)
#     cookie-jar http $admin_jar post $"($base)/login?action=login" {password: $pw} -t multipart/form-data
#     cookie-jar http $admin_jar get $"($base)/tag/interieur" | get status
#
#     let visitor_jar = (cookie-jar new)                # a second, separate browser
#     let second_admin_jar = (cookie-jar clone $admin_jar)  # same cookies now, separate from here on
#
# A jar is a variable that you pass along. Nushell cannot change a variable handed to a
# command, so the variable holds a handle: the path of a private file with the cookies in
# it, which the commands update. Nothing is shared by name: only code that has the variable
# can use the jar. To share one on purpose (with another session, or to keep it), choose the
# path yourself: `cookie-jar new ~/keep/admin.nuon`.
#
# `cookie-jar http` is `http` with a jar: it sends the jar's cookies, stores the ones that
# come back, and follows redirects itself so that a cookie set on the way (a login) is kept.
#
# With plain `http`, the two halves are available on their own:
#     http post --full --allow-errors --redirect-mode manual $url $data | cookie-jar update $admin_jar
#     http get --headers (cookie-jar headers $admin_jar $url) $url
# Use `--redirect-mode manual` on a request that sets a cookie: when `http` follows a
# redirect itself, the response you get is the last one, without the cookie.
#
# Which cookies go with a url follows the browser rules: host, path, Secure, expiry. Ports
# are ignored, as browsers do.
#
# Not possible with Nushell's `http` (0.117): a multipart file part gets no content type of
# its own, so a server that checks an upload's type (image/...) rejects it. Use curl there.

const COOKIE_JAR_HELP = "Make one with `let admin_jar = (cookie-jar new)` and pass that variable."

def cookie-jar-read [jar: path]: nothing -> list<any> {
  if not ($jar | path exists) {
    error make {msg: $"cookie-jar: ($jar) is not a cookie jar", help: $COOKIE_JAR_HELP}
  }
  open --raw $jar | from nuon
}

# Reading and saving the same file in one pipeline is refused by Nushell ("pipeline input and
# output are the same file"), so callers read first and hand the finished list to this.
def cookie-jar-write [jar: path, cookies: list<any>]: nothing -> nothing {
  $cookies | to nuon | save --force --raw $jar
}

# A new, empty cookie jar. Returns its handle: keep it in a variable and pass it along.
#
# Without an argument the jar is a private temporary file. Give a path to put it somewhere
# you choose (to keep it, or to share it with another session); an existing file is kept.
export def new [
  path?: path  # where to keep the jar; a temporary file when left out
]: nothing -> string {
  let jar = if $path == null { mktemp --tmpdir cookie-jar.XXXXXX.nuon } else { $path | path expand }
  if $path == null or not ($jar | path exists) {
    cookie-jar-write $jar []
  }
  $jar
}

# A copy of a jar: the same cookies now, separate from then on. Returns the new handle.
export def clone [
  jar: path    # the jar to copy
  path?: path  # where to keep the copy; a temporary file when left out
]: nothing -> string {
  let cookies = (cookie-jar-read $jar)
  let copy = if $path == null { mktemp --tmpdir cookie-jar.XXXXXX.nuon } else { $path | path expand }
  cookie-jar-write $copy $cookies
  $copy
}

# One Set-Cookie header value, for a response from `url`, as a cookie record.
def cookie-jar-parse [set_cookie: string, url: string]: nothing -> record {
  let request = ($url | url parse)
  let parts = ($set_cookie | split row ";" | each {|part| $part | str trim })
  let pair = ($parts | first | split row --number 2 "=")
  let attributes = ($parts | skip 1 | where $it != "" | each {|part|
    let attribute = ($part | split row --number 2 "=")
    {name: ($attribute.0 | str trim | str lowercase), value: ($attribute | get --optional 1 | default "" | str trim)}
  })
  let attribute = {|name| $attributes | where name == $name | get --optional 0.value }

  let domain = (do $attribute "domain")
  let path = (do $attribute "path")
  let max_age = (do $attribute "max-age")
  let expires_text = (do $attribute "expires")
  # Without a Path attribute a cookie belongs to the directory of the page that set it.
  let default_path = ($request.path | str replace --regex '/[^/]*$' '' | if $in == "" { "/" } else { $in })
  let expires = if $max_age != null {
    (date now) + (($max_age | into int) * 1sec)
  } else if $expires_text != null {
    try { $expires_text | into datetime } catch { null }
  } else {
    null  # a session cookie: kept until the jar is cleared
  }
  {
    name: ($pair.0 | str trim)
    value: ($pair | get --optional 1 | default "")
    domain: (if ($domain | is-empty) { $request.host } else { $domain | str trim --left --char "." | str lowercase })
    host_only: ($domain | is-empty)
    path: (if ($path | is-empty) or not ($path | str starts-with "/") { $default_path } else { $path })
    secure: ("secure" in $attributes.name)
    expires: $expires
  }
}

# Put the cookies a response sets into a jar. Returns its input, so a pipeline can go on.
#
# The input is what `http <method> --full` returns. A table of response headers (columns
# `name` and `value`) works too, with the url they came from given as --url.
export def update [
  jar: path      # the jar, from `cookie-jar new`
  --url: string  # the url the response came from (taken from a --full response)
]: any -> any {
  let response = $in
  let is_full = (($response | describe) =~ '^record') and ("headers" in ($response | columns))
  let headers = if $is_full { $response.headers.response } else { $response }
  let from = if $url != null {
    $url
  } else if $is_full and ($response.urls? | is-not-empty) {
    $response.urls | last
  } else {
    error make {msg: "cookie-jar update: which url did these headers come from?", help: "Pipe in a `http ... --full` response, or pass --url."}
  }

  let set = ($headers | where {|header| ($header.name | str lowercase) == "set-cookie" } | get value | each {|value| cookie-jar-parse $value $from })
  if ($set | is-empty) { return $response }

  let now = (date now)
  # A new cookie replaces one with the same name, domain and path; an expired one removes it.
  let kept = (cookie-jar-read $jar | where {|old| not ($set | any {|new| $new.name == $old.name and $new.domain == $old.domain and $new.path == $old.path }) })
  let fresh = ($set | where {|cookie| $cookie.expires == null or $cookie.expires > $now })
  cookie-jar-write $jar ($kept | append $fresh)
  $response
}

# The jar's cookies that a browser would send to `url`, as rows.
export def for-url [
  jar: path    # the jar, from `cookie-jar new`
  url: string  # the url the request goes to
]: nothing -> list<any> {
  let request = ($url | url parse)
  let host = ($request.host | str lowercase)
  let path = (if ($request.path | is-empty) { "/" } else { $request.path })
  let is_local = ($host in ["localhost" "127.0.0.1" "::1"])
  let now = (date now)
  cookie-jar-read $jar | where {|cookie|
    let host_matches = if $cookie.host_only { $host == $cookie.domain } else { $host == $cookie.domain or ($host | str ends-with $".($cookie.domain)") }
    let path_matches = ($path == $cookie.path) or (($path | str starts-with $cookie.path) and (($cookie.path | str ends-with "/") or (($path | str substring ($cookie.path | str length)..) | str starts-with "/")))
    let secure_ok = (not $cookie.secure) or $request.scheme == "https" or $is_local
    let alive = $cookie.expires == null or $cookie.expires > $now
    $host_matches and $path_matches and $secure_ok and $alive
  }
}

# The Cookie header for a request to `url`, ready for `http ... --headers`. Empty record when
# the jar has nothing for that url.
export def headers [
  jar: path    # the jar, from `cookie-jar new`
  url: string  # the url the request goes to
]: nothing -> record {
  let cookies = (for-url $jar $url)
  if ($cookies | is-empty) { {} } else { {Cookie: ($cookies | each {|cookie| $"($cookie.name)=($cookie.value)" } | str join "; ")} }
}

# One request with the jar's cookies, without following redirects.
def cookie-jar-request [jar: path, method: string, url: string, data: any, content_type: any, headers: record]: nothing -> record {
  let all_headers = ($headers | merge (headers $jar $url))
  # Nushell cannot call `http $method`, so every method is spelled out.
  let response = match ($method | str lowercase) {
    "get" => (http get --full --allow-errors --redirect-mode manual --headers $all_headers $url)
    "head" => (http head --redirect-mode manual --headers $all_headers $url | {urls: [$url], headers: {response: $in}, body: null, status: null})
    "delete" => (http delete --full --allow-errors --redirect-mode manual --headers $all_headers $url)
    "post" | "put" | "patch" => {
      let body = ($data | default "")
      # Without --content-type Nushell picks one from the value (a record becomes JSON).
      match [($method | str lowercase), ($content_type != null)] {
        ["post", true] => (http post --full --allow-errors --redirect-mode manual --content-type $content_type --headers $all_headers $url $body)
        ["post", false] => (http post --full --allow-errors --redirect-mode manual --headers $all_headers $url $body)
        ["put", true] => (http put --full --allow-errors --redirect-mode manual --content-type $content_type --headers $all_headers $url $body)
        ["put", false] => (http put --full --allow-errors --redirect-mode manual --headers $all_headers $url $body)
        ["patch", true] => (http patch --full --allow-errors --redirect-mode manual --content-type $content_type --headers $all_headers $url $body)
        ["patch", false] => (http patch --full --allow-errors --redirect-mode manual --headers $all_headers $url $body)
      }
    }
    _ => (error make {msg: $"cookie-jar http: unknown method ($method)", help: "One of: get, head, post, put, patch, delete."})
  }
  $response | update $jar --url $url
}

# `http` with a cookie jar: sends the jar's cookies, stores the ones that come back, and
# follows redirects itself so that a cookie set on the way (a login) is not lost.
#
#     cookie-jar http $admin_jar post $"($base)/login?action=login" {password: $pw} -t multipart/form-data
#     cookie-jar http $admin_jar get $"($base)/tag/interieur" | get status
#
# Returns the full response of the last request ({urls, headers, body, status}, as
# `http ... --full` does), with `urls` listing every url that was visited. An error status
# is a result, not a failure. A redirect after a POST is followed with GET, like a browser
# (except 307 and 308, which repeat the request).
export def http [
  jar: path                    # the jar, from `cookie-jar new`
  method: string               # get, head, post, put, patch or delete
  url: string
  data?: any                   # the body, for post, put and patch
  --content-type (-t): string  # as for `http post`, e.g. multipart/form-data
  --headers (-H): record = {}  # extra request headers
  --no-follow                  # return a redirect instead of following it
  --max-redirects: int = 10
]: nothing -> any {  # a record; `any` because Nushell cannot see that the loop always returns
  mut method = $method
  mut url = $url
  mut data = $data
  mut content_type = $content_type
  mut visited = []
  loop {
    let response = (cookie-jar-request $jar $method $url $data $content_type $headers)
    $visited = ($visited | append $url)
    let location = ($response.headers.response | where {|header| ($header.name | str lowercase) == "location" } | get --optional 0.value)
    let is_redirect = ($response.status in [301 302 303 307 308]) and $location != null
    if $no_follow or not $is_redirect {
      return ($response | upsert urls $visited)
    }
    if ($visited | length) > $max_redirects {
      error make {msg: $"cookie-jar http: more than ($max_redirects) redirects", help: ($visited | str join "\n")}
    }
    # The Location header may be relative to the url it came from.
    let from = ($url | url parse)
    let origin = $"($from.scheme)://($from.host)(if ($from.port | is-empty) { '' } else { $':($from.port)' })"
    $url = if $location =~ '^[a-zA-Z][a-zA-Z0-9+.-]*://' {
      $location
    } else if ($location | str starts-with "/") {
      $"($origin)($location)"
    } else {
      $"($origin)($from.path | str replace --regex '[^/]*$' '')($location)"
    }
    if $response.status not-in [307 308] {
      $method = "get"
      $data = null
      $content_type = null
    }
  }
}

# Everything in a jar (values included: do not paste this output where it does not belong).
export def list [
  jar: path  # the jar, from `cookie-jar new`
]: nothing -> list<any> {
  cookie-jar-read $jar
}

# Forget a jar's cookies (log out). The jar itself stays usable.
export def clear [
  jar: path  # the jar, from `cookie-jar new`
]: nothing -> nothing {
  cookie-jar-read $jar | ignore
  cookie-jar-write $jar []
}
