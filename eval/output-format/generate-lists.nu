# The format experiment for lists of texts and their friends (multi-line items, blank items,
# indented code, diff lines, paths). See docs/output-format-eval.md.
#
#   OUT=<dir> REPO=<a git repo to take text from> NUX_BIN=<the nux whose v0 to test> \
#   nux < eval/output-format/generate-lists.nu
#
# Lists come from the repo's files and history (and a few from the usage log); questions are
# generated with known answers. Writes <format>.md per format and answers.json to OUT.

let out = ($env.OUT? | default (mktemp -d))
let repo = ($env.REPO? | default (pwd))
let nux_bin = ($env.NUX_BIN? | default "nux")
mkdir $out
cd $repo

# --- The lists -------------------------------------------------------------------------

let code_windows = [
  [src/readable.rs 100 160]
  [src/engine.rs 180 240]
  [src/rewrite.rs 40 110]
  [src/assist/flags.rs 1 70]
  [prelude/ast.nu 50 120]
  [prelude/prelude.nu 120 200]
  [src/help.txt 30 90]
]
let code = ($code_windows | each {|w| {name: $"lines ($w.1)..($w.2) of ($w.0)", items: (open --raw $w.0 | lines | slice ($w.1 - 1)..($w.2 - 1))} })

let commits = (^git log -n 40 --format=%H | lines)
let diffs = ($commits | first 3 | each {|c| {name: $"lines of git show ($c | str substring 0..<7)", items: (^git show $c | lines | first 70)} })

let paths = [{name: "files of the repo", items: (^git ls-files | lines | first 70)}]
let subjects = [{name: "commit subjects", items: (^git log -n 45 --format=%s | lines)}]
# Whole commit messages: items over several lines.
let messages = [{name: "commit messages", items: (^git log -n 12 --format=%B%x00 | split row (char nul) | each { str trim } | where $it != "")}]

# Lists of texts agents really got back (from the usage log), when there are any.
let logged = (
  nux-log --since 30day
  | where kind == eval and outcome == ok and text != null
  | where {|r| ($r.text | str length) > 800 and ($r.text | str length) < 9000 and ($r.text | str starts-with '[') }
  | get text
  | uniq
  | where {|t| not ($t | str contains (ansi escape)) }
  | each {|t| try { $t | from nuon } catch { null } }
  | compact
  | where {|v| ($v | describe) == "list<string>" }
  | sort-by {|v| $v | to nuon | str length }
)
let logged = if ($logged | length) <= 3 { $logged } else { [($logged | get 0) ($logged | get (($logged | length) // 2)) ($logged | last)] }
let logged = ($logged | each {|v| {name: "a list from the usage log", items: $v} })

let lists = ($code ++ $diffs ++ $paths ++ $subjects ++ $messages ++ $logged)

# --- Questions -------------------------------------------------------------------------

def near-miss [text: string]: nothing -> string {
  if ($text | str length) > 1 { $text | str replace -r '.$' '~' } else { $"($text)~" }
}

def one-line [t: string]: nothing -> bool { not ($t | str contains "\n") and ($t | str trim) != "" }

def list-questions [items: list<string>]: nothing -> list<any> {
  let n = ($items | length)
  let numbered = ($items | enumerate)
  mut q = [{q: "How many items are there?", a: ($n | into string), kind: count}]
  # Exact copies: a late item, and the first that is hard to copy (spaces at its start,
  # quotes, backslashes), when they are one line.
  let late = ($numbered | where {|i| one-line $i.item } | last 3 | first)
  if $late != null {
    $q ++= [{q: $"What is item number ($late.index + 1)? Give it exactly.", a: $late.item, kind: copy}]
  }
  let late_index = ($late.index? | default (-1))
  let hard_ones = ($numbered | where {|i| (one-line $i.item) and ($i.item =~ '^\s|["\\]') and $i.index != $late_index })
  let hard = ($hard_ones | get --optional (($hard_ones | length) // 2))
  if $hard != null {
    $q ++= [{q: $"What is item number ($hard.index + 1)? Give it exactly.", a: $hard.item, kind: copy}]
  }
  # Which number: an item that occurs once.
  let counts = ($items | uniq --count)
  let unique = ($numbered | where {|i| (one-line $i.item) and (($counts | where value == $i.item | get 0.count) == 1) and ($i.item | str length) > 8 })
  let pick = ($unique | get --optional (($unique | length) // 2))
  if $pick != null {
    $q ++= [{q: $"Which item number is exactly `($pick.item | str trim)`, spaces at its start aside?", a: ($pick.index + 1 | into string), kind: index}]
  }
  # Is it there: a near miss (no) and an item that is there (yes).
  let base = ($unique | get --optional (($unique | length) // 3))
  if $base != null {
    let absent = (near-miss $base.item)
    if $absent not-in $items {
      $q ++= [{q: $"Is `($absent | str trim)` one of the items \(spaces at its start aside\)? Answer yes or no.", a: "no", kind: exists}]
    }
    $q ++= [{q: $"Is `($base.item | str trim)` one of the items \(spaces at its start aside\)? Answer yes or no.", a: "yes", kind: exists}]
  }
  # Empty items, when there are.
  let empty = ($items | where $it == "" | length)
  if $empty > 0 {
    $q ++= [{q: "How many items are empty (no characters at all)?", a: ($empty | into string), kind: count}]
  }
  # Items over several lines.
  let multi = ($numbered | where {|i| ($i.item | lines | length) >= 3 })
  if ($multi | is-not-empty) {
    $q ++= [{q: "How many items have more than one line?", a: ($numbered | where {|i| $i.item | str contains "\n" } | length | into string), kind: count}]
    let m = ($multi | get (($multi | length) // 2))
    let line = ($m.item | lines | get 2)
    $q ++= [{q: $"How many lines does item number ($m.index + 1) have?", a: ($m.item | lines | length | into string), kind: multiline}]
    if ($line | str trim) != "" {
      $q ++= [{q: $"What is line 3 of item number ($m.index + 1)? Give it exactly.", a: $line, kind: multiline}]
    }
  }
  $q
}

let items = ($lists | enumerate | each {|l|
  {id: $l.index, name: $l.item.name, value: $l.item.items, questions: (list-questions $l.item.items | enumerate | each {|q| $q.item | insert id $"($l.index).($q.index)" })}
})

# --- Formats ---------------------------------------------------------------------------

def v0 [value: any]: nothing -> string {
  let file = $"($out)/value.nuon"
  $value | to nuon | save --force $file
  with-env {NUX_FORMAT: "readable"} { $"open '($file)'" | ^$nux_bin }
}

def markdown-kv [value: any]: nothing -> string {
  $value | each {|v| $"- ($v)" } | str join "\n"
}

let formats = {
  nuon: {|v| $v | to nuon --raw }
  json: {|v| $v | to json --raw }
  yaml: {|v| $v | to yaml | str trim }
  v0: {|v| v0 $v }
  markdown-kv: {|v| markdown-kv $v }
}

for format in ($formats | columns) {
  let render = ($formats | get $format)
  let body = ($items | each {|item|
    let shown = (do $render $item.value)
    let qs = ($item.questions | each {|q| $"- [($q.id)] ($q.q)" } | str join "\n")
    $"## Value ($item.id): ($item.name)\n\n~~~~\n($shown)\n~~~~\n\nQuestions:\n($qs)\n"
  } | str join "\n")
  $body | save --force $"($out)/($format).md"
}

let key = ($items | each {|i| $i.questions | each {|q| {id: $q.id, kind: $q.kind, shape: $i.name, answer: $q.a} } } | flatten)
$key | to json | save --force $"($out)/answers.json"
{
  out: $out,
  lists: ($items | length),
  questions: ($key | length),
  by_kind: ($key | group-by kind --to-table | each {|g| {kind: $g.kind, n: ($g.items | length)} }),
  chars: ($formats | columns | each {|f| {format: $f, chars: (open --raw $"($out)/($f).md" | str length)} })
}
