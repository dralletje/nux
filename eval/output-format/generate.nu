# The format experiment's material: real values from the nux usage log, questions with known
# answers, and each value rendered in every format. See docs/output-format-eval.md.
#
#   OUT=<dir> MIN=2500 MAX=10240 TABLES=12 RECORDS=8 LISTS=5 \
#   TOON_DEPS=<dir with @toon-format/toon installed> nux < eval/output-format/generate.nu
#
# Writes <format>.md (values and questions, one file per format) and answers.json to OUT.
# The values stay out of the repo: they come from agents' views of private repositories.

let out = ($env.OUT? | default (mktemp -d))
let min = ($env.MIN? | default "2500" | into int)
let max = ($env.MAX? | default "10240" | into int)
let toon_deps = ($env.TOON_DEPS? | default null)
let here = ($env.FILE_PWD? | default (pwd))

# Values whose reply was NUON data in the size range, without repeats.
let candidates = (
  nux-log --since 30day
  | where kind == eval and outcome == ok and text != null
  | where {|r| ($r.text | str length) >= $min and ($r.text | str length) <= $max and ($r.text =~ '^[\[{]') }
  | get text
  | uniq
  # Terminal colors (from logged test output) are not what this measures; Nushell's
  # `to json` also writes their ESC unescaped, which makes the JSON invalid.
  | where {|text| not ($text | str contains (ansi escape)) }
  | each {|text| try { {text: $text, value: ($text | from nuon)} } catch { null } }
  | compact
  | insert kind {|c| $c.value | describe | str replace -r '<.*' '' }
)

def answer-of [v: any]: nothing -> string {
  if ($v | describe) == "string" { $v } else { $v | to nuon }
}

# A value a question can ask for: text on one line (not too long, not blank), a number, a bool.
def plain [v: any]: nothing -> bool {
  let type = ($v | describe)
  $type in [int bool] or ($type == "string" and ($v | str length) <= 120 and not ($v | str contains "\n") and ($v | str trim) != "")
}

# Text that is hard to copy: quotes, backslashes, spaces at an end, a `|`.
def tricky [v: any]: nothing -> bool {
  ($v | describe) == "string" and ($v =~ '["\\|`]' or ($v | str trim) != $v)
}

def near-miss [text: string]: nothing -> string {
  if ($text | str length) > 1 { $text | str replace -r '.$' '~' } else { $"($text)~" }
}

def table-questions [rows: list<any>]: nothing -> list<any> {
  let columns = ($rows | columns)
  let n = ($rows | length)
  # A column that names rows: text, a different value in every row.
  let key = ($columns | where {|c| ($rows | all {|r| (plain ($r | get --optional $c)) and (($r | get --optional $c | describe) == "string") }) and (($rows | get $c | uniq | length) == $n) } | get 0?)
  let name_row = {|i|
    if $key != null { $"the row whose `($key)` is `(($rows | get $i) | get $key)`" } else { $"row number ($i + 1) \(counting from 1\)" }
  }
  let values = {|i| $columns | where {|c| $c != $key and (plain (($rows | get $i) | get --optional $c)) } }
  mut questions = []
  # Lookups: one far down the table (far from its header), one in the middle.
  for i in ([($n - 2) ($n // 2)] | where $it >= 0 | uniq) {
    let column = (do $values $i | last)
    if $column != null {
      $questions ++= [{q: $"In (do $name_row $i), what is the value of `($column)`?", a: (answer-of (($rows | get $i) | get $column)), kind: lookup}]
    }
  }
  $questions ++= [{q: "How many rows does the table have?", a: ($n | into string), kind: count}]
  let repeated = ($columns | where {|c| $c != $key and ($rows | all {|r| plain ($r | get --optional $c) }) and (($rows | get $c | uniq | length) < $n) } | get 0?)
  if $repeated != null {
    let v = ($rows | get $repeated | uniq --count | sort-by count | get 0.value)
    $questions ++= [{q: $"How many rows have `($repeated)` equal to `(answer-of $v)`?", a: ($rows | where {|r| ($r | get $repeated) == $v } | length | into string), kind: count}]
  }
  let numeric = ($columns | where {|c| $c != $key and ($rows | all {|r| ($r | get --optional $c | describe) == "int" }) } | get 0?)
  if $numeric != null and $key != null {
    let best = ($rows | sort-by {|r| $r | get $numeric } --reverse | first)
    if ($rows | where {|r| ($r | get $numeric) == ($best | get $numeric) } | length) == 1 {
      $questions ++= [{q: $"Which `($key)` has the largest `($numeric)`?", a: ($best | get $key), kind: extreme}]
    }
  }
  if $key != null {
    let present = (($rows | get ($n // 3)) | get $key)
    let absent = (near-miss (($rows | get (($n * 2) // 3)) | get $key))
    if $absent not-in ($rows | get $key) {
      $questions ++= [{q: $"Is there a row whose `($key)` is `($absent)`? Answer yes or no.", a: "no", kind: exists}]
    }
    $questions ++= [{q: $"Is there a row whose `($key)` is `($present)`? Answer yes or no.", a: "yes", kind: exists}]
  }
  # Exact copy: a cell that is hard to copy when there is one, else a late one.
  let cells = ($rows | enumerate | each {|r| $columns | where {|c| $c != $key and (plain ($r.item | get --optional $c)) and (($r.item | get --optional $c | describe) == "string") } | each {|c| {i: $r.index, c: $c, v: ($r.item | get $c)} } } | flatten)
  let hard = ($cells | where {|cell| tricky $cell.v } | get 0?)
  let pick = if $hard != null { $hard } else { $cells | last }
  if $pick != null {
    $questions ++= [{q: $"In (do $name_row $pick.i), what is `($pick.c)`? Give it exactly.", a: $pick.v, kind: copy}]
  }
  $questions
}

def list-questions [items: list<any>]: nothing -> list<any> {
  let n = ($items | length)
  let hard = ($items | enumerate | where {|i| tricky $i.item } | get 0?)
  let late = ($n - 2)
  let absent = (near-miss ($items | get ($n // 2)))
  [
    {q: $"What is item number ($late + 1) \(counting from 1\)? Give it exactly.", a: ($items | get $late), kind: copy}
    (if $hard != null { {q: $"What is item number ($hard.index + 1) \(counting from 1\)? Give it exactly.", a: $hard.item, kind: copy} })
    {q: "How many items are there?", a: ($n | into string), kind: count}
    (if $absent not-in $items { {q: $"Is `($absent)` one of the items? Answer yes or no.", a: "no", kind: exists} })
    {q: $"Is `($items | get ($n // 3))` one of the items? Answer yes or no.", a: "yes", kind: exists}
  ] | compact
}

def record-questions [record: record]: nothing -> list<any> {
  let fields = ($record | transpose key value)
  mut questions = []
  for field in ($fields | where {|f| plain $f.value } | last 2) {
    $questions ++= [{q: $"What is the value of `($field.key)`?", a: (answer-of $field.value), kind: lookup}]
  }
  for field in ($fields | where {|f| ($f.value | describe) =~ '^(list|table)' and ($f.value | length) > 2 } | first 2) {
    let n = ($field.value | length)
    $questions ++= [{q: $"How many items does `($field.key)` have?", a: ($n | into string), kind: count}]
    let item = ($field.value | get ($n - 2))
    if (plain $item) {
      $questions ++= [{q: $"What is item number ($n - 1) of `($field.key)` \(counting from 1\)? Give it exactly.", a: (answer-of $item), kind: nested}]
    } else if ($item | describe) =~ '^record' {
      let inner = ($item | transpose key value | where {|f| plain $f.value } | get 0?)
      if $inner != null {
        $questions ++= [{q: $"In item number ($n - 1) of `($field.key)` \(counting from 1\), what is `($inner.key)`?", a: (answer-of $inner.value), kind: nested}]
      }
    }
  }
  $questions
}

def pick [kind: string, n: int]: list<any> -> list<any> {
  let all = ($in | where kind == $kind | where {|c| if $kind == list { ($c.value | all {|v| ($v | describe) == "string" }) } else { true } } | sort-by {|c| $c.text | str length })
  if ($all | length) <= $n { return $all }
  let step = (($all | length) / $n)
  0..<$n | each {|i| $all | get (($i * $step + $step / 2) | math floor) }
}

let chosen = (
  ($candidates | pick table ($env.TABLES? | default "12" | into int))
  ++ ($candidates | pick record ($env.RECORDS? | default "8" | into int))
  ++ ($candidates | pick list ($env.LISTS? | default "5" | into int))
)

let items = ($chosen | enumerate | each {|c|
  let value = $c.item.value
  let questions = match $c.item.kind {
    "table" => (table-questions $value)
    "record" => (record-questions $value)
    _ => (list-questions $value)
  }
  {id: $c.index, kind: $c.item.kind, size: ($c.item.text | str length), value: $value, questions: ($questions | enumerate | each {|q| $q.item | insert id $"($c.index).($q.index)" })}
} | where {|i| ($i.questions | length) >= 2 })

# v0, as nux prints it.
def v0 [value: any]: nothing -> string {
  let file = $"($out)/value.nuon"
  $value | to nuon | save --force $file
  with-env {NUX_FORMAT: "readable"} { $"open '($file)'" | ^nux }
}

# Markdown-KV as in the Improving Agents post: a heading per record, `key: value` lines.
def markdown-kv [value: any]: nothing -> string {
  let line = {|k v| $"($k): (if ($v | describe) == 'string' { $v } else { $v | to nuon --raw })" }
  let type = ($value | describe)
  if $type =~ '^table' or ($type =~ '^list' and ($value | all {|v| ($v | describe) =~ '^record' })) {
    $value | enumerate | each {|r|
      let fields = ($r.item | transpose k v | each {|f| do $line $f.k $f.v } | str join "\n")
      $"## Record ($r.index + 1)\n\n($fields)"
    } | str join "\n\n"
  } else if $type =~ '^record' {
    $value | transpose k v | each {|f| do $line $f.k $f.v } | str join "\n"
  } else if $type =~ '^list' {
    $value | each {|v| $"- (if ($v | describe) == 'string' { $v } else { $v | to nuon --raw })" } | str join "\n"
  } else {
    $value | to nuon --raw
  }
}

# The v0 variants for the off-by-one: the number of rows above `## 0`, or headings from 1.
def v0-count [value: any]: nothing -> string {
  let text = (v0 $value)
  if ($text | str starts-with "## 0\n") { $"($value | length) rows\n($text)" } else { $text }
}
def v0-from-1 [value: any]: nothing -> string {
  let text = (v0 $value)
  if not ($text | str starts-with "## 0\n") { return $text }
  $text | lines | each {|l| if $l =~ '^## \d+$' { $"## (($l | str substring 3.. | into int) + 1)" } else { $l } } | str join "\n"
}

mut formats = {
  nuon: {|v| $v | to nuon --raw }
  json: {|v| $v | to json --raw }
  yaml: {|v| $v | to yaml | str trim }
  v0: {|v| v0 $v }
  v0-count: {|v| v0-count $v }
  v0-from-1: {|v| v0-from-1 $v }
  markdown-kv: {|v| markdown-kv $v }
  markdown-table: {|v| $v | to md }
}
if $toon_deps != null {
  $formats = ($formats | merge {toon: {|v| $v | to json | ^node ($here | path join to-toon.mjs) $toon_deps }})
}
let formats = $formats

for format in ($formats | columns) {
  let render = ($formats | get $format)
  let body = ($items | each {|item|
    let shown = (do $render $item.value)
    let qs = ($item.questions | each {|q| $"- [($q.id)] ($q.q)" } | str join "\n")
    $"## Value ($item.id)\n\n~~~~\n($shown)\n~~~~\n\nQuestions:\n($qs)\n"
  } | str join "\n")
  $body | save --force $"($out)/($format).md"
}

let key = ($items | each {|i| $i.questions | each {|q| {id: $q.id, kind: $q.kind, shape: $i.kind, size: $i.size, answer: $q.a} } } | flatten)
$key | to json | save --force $"($out)/answers.json"
{
  out: $out,
  values: ($items | length),
  questions: ($key | length),
  by_kind: ($key | group-by kind --to-table | each {|g| {kind: $g.kind, n: ($g.items | length)} }),
  by_shape: ($key | group-by shape --to-table | each {|g| {shape: $g.shape, n: ($g.items | length)} }),
  chars: ($formats | columns | each {|f| {format: $f, chars: (open --raw $"($out)/($f).md" | str length)} })
}
