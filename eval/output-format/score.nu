# Score the answers of a run of the format experiment.
#
#   OUT=<dir of generate.nu's output, with answers-<model>-<format>.json> nux < score.nu
#
# Strict: exactly the expected text. Lenient: also right with outer quotes, backticks and
# spaces removed (a copy with the format's quotes still on). `&gt;`, `&lt;`, `&amp;` are
# turned back first: subagents' answers came back HTML-escaped in an earlier run.

let out = $env.OUT
let key = (open $"($out)/answers.json")
let unescape = {|s| $s | into string | str replace --all "&gt;" ">" | str replace --all "&lt;" "<" | str replace --all "&amp;" "&" }
let loose = {|s| $s | str trim | str trim --char '"' | str trim --char '`' | str trim }

let runs = (glob $"($out)/answers-*.json" | each {|file|
  let name = ($file | path parse | get stem | str replace 'answers-' '')
  let model = ($name | split row '-' | first)
  let format = ($name | str replace $"($model)-" '')
  let given = (try { open $file } catch { {} })
  let checks = ($key | each {|k|
    let answer = (do $unescape ($given | get --optional $k.id | default ""))
    {
      id: $k.id, kind: $k.kind, shape: $k.shape,
      strict: ($answer == $k.answer),
      lenient: ((do $loose $answer) == (do $loose $k.answer)),
      given: $answer, expected: $k.answer
    }
  })
  {model: $model, format: $format, checks: $checks}
})

let total = ($key | length)
let summary = ($runs | each {|r|
  {
    model: $r.model
    format: $r.format
    strict: ($r.checks | where strict | length)
    lenient: ($r.checks | where lenient | length)
    of: $total
  }
} | sort-by model format)

let by_kind = ($runs | each {|r|
  $r.checks | group-by kind --to-table | each {|g| {model: $r.model, format: $r.format, kind: $g.kind, right: ($g.items | where lenient | length), of: ($g.items | length)} }
} | flatten)

let sizes = (glob $"($out)/*.md" | each {|f| {format: ($f | path parse | get stem), chars: (open --raw $f | str length)} } | sort-by chars)

let wrong = ($runs | each {|r| $r.checks | where not lenient | each {|c| $c | insert run $"($r.model)/($r.format)" } } | flatten | select run id kind shape given expected)

{summary: $summary, by_kind: $by_kind, sizes: $sizes, wrong: $wrong} | to json | save --force $"($out)/scores.json"
{summary: $summary, sizes: $sizes}
