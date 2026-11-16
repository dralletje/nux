# The usage log: what was asked of nux, by whom, and how it went (written by
# src/usage_log.rs; the TypeScript nux wrote the same format, without `runner`).

# The calls made to nux, oldest first: one row per call, by every agent on this machine.
#
# Columns: at, runner (which nux: "nux-rs <version>", or "nux-ts" for the TypeScript one),
# kind (eval, try-nu, help, find, invalid; nux had guide until 0.2, the TypeScript nux also
# had stop and list),
# outcome (ok, error, external_failed, usage_error, start_failed, caller_killed, or
# unfinished: still running, or killed outright; the TypeScript nux also had timeout and
# session_failed), duration, reason, code, error_code, text (what the caller got to see),
# conversation, cwd, argv, pid, id; and, filled only by the TypeScript nux (null for the
# Rust one): session, session_named, timeout_seconds, json.
@example "what failed today, and why it was run" { nux-log | where outcome not-in [ok unfinished] | select at outcome reason code text }
@example "which commands agents look up" {
  nux-log --since 7day
  | where kind in [help find]
  | get argv
  | each { str join " " }
  | uniq --count
  | sort-by count --reverse
}
@example "how often a reason is given" {
  nux-log --since 7day
  | where kind == eval
  | each { $in.reason != null }
  | uniq --count
}
@example "the two versions side by side" {
  nux-log --since 1day
  | where kind == eval
  | group-by runner --to-table
  | each {|group|
      {
        runner: $group.runner
        calls: ($group.items | length)
        failed: ($group.items | where outcome not-in [ok unfinished] | length)
      }
    }
}
export def nux-log [
  --since: duration = 1day # how far back to look
  --dir: path              # the log's directory (default: $env.NUX_LOG_DIR, else ~/.local/state/nux/log)
]: nothing -> table {
  let dir = ($dir | default ($env.NUX_LOG_DIR? | default ($nu.home-dir | path join .local state nux log)))
  let from = (date now) - $since
  # One file per UTC day, named after it.
  let first_day = ($from | date to-timezone UTC | format date "%Y-%m-%d")
  let files = (glob ($dir | path join "*.jsonl") | where {|file| ($file | path parse | get stem) >= $first_day } | sort)
  # One parse per file. A line that is not JSON (a write cut short) would make that fail:
  # then line by line, leaving such lines out.
  let lines = ($files | each {|file|
    let text = (open --raw $file)
    try { $text | from json --objects } catch {
      $text | lines | each {|line| try { $line | from json } catch { null } } | compact
    }
  } | flatten)
  let no_end = {duration_ms: null, ok: null, outcome: "unfinished", error_code: null, text: null}
  $lines
  | group-by id --to-table
  | each {|call|
      let start = ($call.items | where event == "start" | get 0?)
      if $start == null { return null }
      let end = ($call.items | where event == "end" | get 0? | default $no_end)
      {
        at: ($start.at | into datetime)
        runner: ($start.runner? | default "nux-ts")
        kind: $start.kind
        outcome: $end.outcome
        duration: (if $end.duration_ms == null { null } else { $end.duration_ms * 1ms })
        reason: $start.reason
        code: $start.code
        error_code: $end.error_code
        text: $end.text
        session: $start.session?
        session_named: $start.session_named?
        conversation: $start.conversation
        cwd: $start.cwd
        argv: $start.argv
        timeout_seconds: $start.timeout_seconds?
        json: $start.json?
        pid: $start.pid
        id: $start.id
      }
    }
  | compact
  | where at >= $from
  | sort-by at
}
