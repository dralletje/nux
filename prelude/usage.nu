# The usage log: what was asked of nux, by whom, and how it went (written by src/usage_log.ts).

# The calls made to nux, oldest first: one row per call, from every session on this machine.
#
# Columns: at, kind (eval, help, find, guide, stop, list, invalid), outcome (ok, error,
# external_failed, timeout, usage_error, session_failed, caller_killed, or unfinished: still
# running, or killed outright), duration, reason, code, error_code, text (what the caller
# got to see), session, session_named, conversation, cwd, argv, timeout_seconds,
# json, pid, id.
@example "what failed today, and why it was run" { nux-log | where outcome not-in [ok unfinished] | select at outcome reason code text }
@example "which commands agents look up" { nux-log --since 7day | where kind in [help find] | get argv | each { str join " " } | uniq --count | sort-by count -r }
@example "how often a reason is given" { nux-log --since 7day | where kind == eval | each { $in.reason != null } | uniq --count }
export def nux-log [
  --since: duration = 1day # how far back to look
  --dir: path              # the log's directory (default: $env.NUX_LOG_DIR, else ~/.local/state/nux/log)
]: nothing -> table {
  let dir = ($dir | default ($env.NUX_LOG_DIR? | default ($nu.home-dir | path join .local state nux log)))
  let from = (date now) - $since
  # One file per UTC day, named after it.
  let first_day = ($from | date to-timezone UTC | format date "%Y-%m-%d")
  let files = (glob ($dir | path join "*.jsonl") | where {|file| ($file | path parse | get stem) >= $first_day } | sort)
  let lines = ($files | each {|file| open --raw $file | lines | each { from json } } | flatten)
  let no_end = {duration_ms: null, ok: null, outcome: "unfinished", error_code: null, text: null}
  $lines
  | group-by id --to-table
  | each {|call|
      let start = ($call.items | where event == "start" | get 0?)
      if $start == null { return null }
      let end = ($call.items | where event == "end" | get 0? | default $no_end)
      {
        at: ($start.at | into datetime)
        kind: $start.kind
        outcome: $end.outcome
        duration: (if $end.duration_ms == null { null } else { $end.duration_ms * 1ms })
        reason: $start.reason
        code: $start.code
        error_code: $end.error_code
        text: $end.text
        session: $start.session
        session_named: $start.session_named
        conversation: $start.conversation
        cwd: $start.cwd
        argv: $start.argv
        timeout_seconds: $start.timeout_seconds
        json: $start.json
        pid: $start.pid
        id: $start.id
      }
    }
  | compact
  | where at >= $from
  | sort-by at
}
