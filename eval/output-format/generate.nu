# A small format experiment (run: OUT=<dir> nux < generate.nu; see docs/output-format-eval.md): real values from the nux log, questions with known answers,
# each value rendered as NUON, JSON, YAML and v0 (nux's readable form).

let out = ($env.OUT? | default (mktemp -d))  # where the renderings and answers.json go

# Values whose text is NUON data of a useful size, deduplicated.
let candidates = (
  nux-log --since 30day
  | where kind == eval and outcome == ok and text != null
  | where {|r| ($r.text | str length) > 150 and ($r.text | str length) < 2500 and ($r.text =~ '^[\[{]') }
  | get text
  | uniq
  | each {|text| try { {text: $text, value: ($text | from nuon)} } catch { null } }
  | compact
  | insert kind {|c| $c.value | describe | str replace -r '<.*' '' }
)

# Text of a scalar, as an answer.
def answer-of [v: any]: nothing -> string {
  if ($v | describe) == "string" { $v } else { $v | to nuon }
}

def plain [v: any]: nothing -> bool {
  ($v | describe) in [string int bool] and not (($v | describe) == "string" and (($v | str length) > 80 or ($v | str contains "\n") or ($v | str trim) == ""))
}

# Questions for a table.
def table-questions [rows: list<any>]: nothing -> list<any> {
  let columns = ($rows | columns)
  let key = ($columns | where {|c| ($rows | all {|r| ($r | get --optional $c | describe) == "string" }) and (($rows | get $c | uniq | length) == ($rows | length)) } | get 0?)
  let index = (($rows | length) // 2)
  let row = ($rows | get $index)
  let where_row = if $key != null { $"the row whose `($key)` is `($row | get $key)`" } else { $"row number ($index + 1) \(counting from 1\)" }
  let target = ($columns | where {|c| $c != $key and (plain ($row | get --optional $c)) } | get 0?)
  mut questions = []
  if $target != null {
    $questions ++= [{q: $"In ($where_row), what is the value of `($target)`?", a: (answer-of ($row | get $target)), kind: lookup}]
  }
  $questions ++= [{q: "How many rows does the table have?", a: ($rows | length | into string), kind: count}]
  let numeric = ($columns | where {|c| $c != $key and ($rows | all {|r| ($r | get --optional $c | describe) == "int" }) } | get 0?)
  if $numeric != null and $key != null {
    let best = ($rows | sort-by {|r| $r | get $numeric } --reverse | first)
    let tied = ($rows | where {|r| ($r | get $numeric) == ($best | get $numeric) } | length)
    if $tied == 1 {
      $questions ++= [{q: $"Which `($key)` has the largest `($numeric)`?", a: ($best | get $key), kind: extreme}]
    }
  } else {
    let some = ($columns | where {|c| ($rows | get --optional $c | all {|v| ($v | describe) in [string bool int] }) and (($rows | get $c | uniq | length) < ($rows | length)) } | get 0?)
    if $some != null {
      let v = ($rows | get $some | first)
      $questions ++= [{q: $"How many rows have `($some)` equal to `(answer-of $v)`?", a: ($rows | where {|r| ($r | get $some) == $v } | length | into string), kind: count}]
    }
  }
  $questions
}

def list-questions [items: list<any>]: nothing -> list<any> {
  let n = ($items | length)
  let k = ([2 ($n - 1)] | math min)
  let present = ($items | get ($n // 2))
  # A near miss for "no": one character changed.
  let absent = ($present | str replace -r '.$' '~')
  let ask_absent = ($absent not-in $items)
  [
    {q: $"What is item number ($k + 1) \(counting from 1\)? Give it exactly.", a: ($items | get $k), kind: copy}
    {q: "How many items are there?", a: ($n | into string), kind: count}
    (if $ask_absent { {q: $"Is `($absent)` one of the items? Answer yes or no.", a: "no", kind: exists} } else { {q: $"Is `($present)` one of the items? Answer yes or no.", a: "yes", kind: exists} })
  ]
}

def record-questions [record: record]: nothing -> list<any> {
  let fields = ($record | transpose key value)
  let simple = ($fields | where {|f| plain $f.value } | get 0?)
  mut questions = []
  if $simple != null {
    $questions ++= [{q: $"What is the value of `($simple.key)`?", a: (answer-of $simple.value), kind: lookup}]
  }
  let nested = ($fields | where {|f| ($f.value | describe) =~ '^(list|table)' and ($f.value | length) > 1 } | get 0?)
  if $nested != null {
    $questions ++= [{q: $"How many items does `($nested.key)` have?", a: ($nested.value | length | into string), kind: nested}]
    let item = ($nested.value | get 1)
    if (plain $item) {
      $questions ++= [{q: $"What is the second item of `($nested.key)`? Give it exactly.", a: (answer-of $item), kind: copy}]
    } else if ($item | describe) =~ '^record' {
      let field = ($item | transpose key value | where {|f| plain $f.value } | get 0?)
      if $field != null {
        $questions ++= [{q: $"In the second item of `($nested.key)`, what is `($field.key)`?", a: (answer-of $field.value), kind: nested}]
      }
    }
  }
  $questions
}

# A spread of shapes and sizes: sorted by size, evenly spaced picks per kind.
def pick [kind: string, n: int]: list<any> -> list<any> {
  let all = ($in | where kind == $kind | where {|c| if $kind == list { ($c.value | all {|v| ($v | describe) == "string" }) } else { true } } | sort-by {|c| $c.text | str length })
  let step = ((($all | length) / $n) | math floor)
  0..<$n | each {|i| $all | get ($i * $step + ($step // 2)) }
}

let chosen = ($candidates | pick table 6) ++ ($candidates | pick record 4) ++ ($candidates | pick list 4)

let items = ($chosen | enumerate | each {|c|
  let value = $c.item.value
  let questions = match $c.item.kind {
    "table" => (table-questions $value)
    "record" => (record-questions $value)
    _ => (list-questions $value)
  }
  {id: $c.index, kind: $c.item.kind, value: $value, questions: ($questions | enumerate | each {|q| $q.item | insert id $"($c.index).($q.index)" })}
} | where {|i| ($i.questions | length) >= 2 })

# Render in each format. v0: what nux prints, from the NUON file of the value.
let formats = {
  nuon: {|v| $v | to nuon --raw }
  json: {|v| $v | to json --raw }
  yaml: {|v| $v | to yaml | str trim }
  v0: {|v|
    let file = $"($out)/value.nuon"
    $v | to nuon | save --force $file
    with-env {NUX_FORMAT: "readable"} { $"open '($file)'" | ^nux }
  }
}

for format in ($formats | columns) {
  let render = ($formats | get $format)
  let body = ($items | each {|item|
    let shown = (do $render $item.value)
    let qs = ($item.questions | each {|q| $"- [($q.id)] ($q.q)" } | str join "\n")
    $"## Value ($item.id)\n\n~~~~\n($shown)\n~~~~\n\nQuestions:\n($qs)\n"
  } | str join "\n")
  $body | save --force $"($out)/($format).md"
}

let key = ($items | each {|i| $i.questions | each {|q| {id: $q.id, kind: $q.kind, shape: $i.kind, answer: $q.a} } } | flatten)
$key | to json | save --force $"($out)/answers.json"
{
  values: ($items | length),
  questions: ($key | length),
  by_kind: ($key | group-by kind --to-table | each {|g| {kind: $g.kind, n: ($g.items | length)} }),
  chars: ($formats | columns | each {|f| {format: $f, chars: (open --raw $"($out)/($f).md" | str length)} })
}
