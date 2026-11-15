Nushell for agents: how to write it well. (`nux help` is the manual of nux itself. This
guide is adapted from the one Nushell's MCP server gives agents, MIT licensed, with what
is about that server taken out.)

## Don't cap the result, slice it

Run a command uncapped: no `head`, `tail`, `first N` or `-n 5` in the pipe just to keep
the output small. A result too large to print shows its type, size and first rows, and
all of it is kept: `kept result-3` gets it back in the next call, to slice as you like.
Capping the command itself only throws away rows you may need.

```nu
# BAD: cut down while it runs
ls **/*.rs | first 20
cargo build o+e>| tail -50

# GOOD: run once, then look at what you need
cargo build | complete | get stderr | lines | where $it =~ '^error'
kept result-3 | where size > 1mb | first 20     # a large result of an earlier call
```

`complete` gives `stdout`, `stderr` and `exit_code` as separate fields. Reach for
`o+e>| complete` only when you want the two streams interleaved (a build log where
warnings must stay next to the lines they belong to).

Cap inside the command only when making the output costs something real: a remote API
that bills per byte, a paid model call.

## Structured output: prefer native commands

Native Nushell commands give structured values (records, tables, lists). Do not pipe them
to `| to json`; they already are data. `nux find <word>` finds commands, `nux help <command>`
shows a command's usage, flags, input and output types, and examples.

```nu
ps | columns                    # see what columns are available
ls | where size > 1mb | get name
sys cpu | length                # a record has a length like anything else
```

External commands give a `string`. Parse it with `from json`, `from yaml`, `from csv`,
`from dotenv` (a .env file), `lines`, `split column`, `detect columns` as
needed.

## String literals

| Form          | Example             | Escapes              | Use case                       |
| ------------- | ------------------- | -------------------- | ------------------------------ |
| Single-quoted | `'hello'`           | none                 | literal, Windows paths, SQL    |
| Double-quoted | `"a\nb"`            | `\n \t \" \\ \u{1b}` | strings needing escapes        |
| Raw           | `r#'he said "hi"'#` | none                 | mixed quotes, multi-line       |
| Bare word     | `hello`             | none                 | command args (word chars only) |
| Backtick      | `` `my file.txt` `` | none                 | paths/globs with spaces        |
| Interpolated  | `$"x=($var)"`       | per-quote            | embedding variables            |

Interpolation needs `$"..."` (or `$'...'`) **and** parentheses around the expression:

```nu
let name = "world"
"hello $name"          # literal: "hello $name"   <- WRONG
$"hello ($name)"       # "hello world"            <- RIGHT
```

Flags with a variable in them: the **whole** flag must be one interpolated string, or use
`--key=value`:

```nu
# BAD
mysql -p $env.PASSWORD db       # becomes two separate arguments
# GOOD
mysql $"-p($env.PASSWORD)" db
mysql $"--password=($env.PASSWORD)" db
```

For control characters use a braced escape such as `"\u{1b}"` (bare `\u001b` is a parse
error) or `char escape` / `char newline` / `char tab`. Strip ANSI color codes with
`ansi strip`, not a regex.

## Redirection: no `2>&1`

| Bash                      | Nushell                            |
| ------------------------- | ---------------------------------- |
| `cmd > file`              | `cmd o> file`, or `... \| save file` for a value |
| `cmd >> file`             | `cmd o>> file`                     |
| `cmd > /dev/null`         | `cmd \| ignore`                    |
| `cmd 2>&1`                | `cmd o+e>\| next_cmd`              |
| `cmd > /dev/null 2>&1`    | `cmd o+e>\| ignore`                |
| `cmd \| tee log \| other` | `cmd \| tee { save log } \| other` |

## Bash to Nushell, quickly

| Bash                          | Nushell                                           |
| ----------------------------- | ------------------------------------------------- |
| `mkdir -p path`               | `mkdir path`                                      |
| `rm -rf path`                 | `rm -r path`                                      |
| `cat file`                    | `open --raw file` (`open file` parses json, yaml, toml, csv, ...) |
| `grep pat`                    | `where $it =~ pat` / `find pat`; in files: `search pat` |
| `sed 's/a/b/'`                | `str replace a b`                                 |
| `head -5` / `tail -5`         | `first 5` / `last 5`, after you have the whole result |
| `for f in *.md; do ...; done` | `glob *.md \| each {\|f\| ... }`, or `for f in (glob *.md) { ... }` |
| `$(cmd)`                      | `(cmd)` in expressions, `...(cmd)` to spread a list |
| `echo $PATH`                  | `$env.PATH`                                       |
| `echo $?`                     | `$env.LAST_EXIT_CODE`                             |
| `export FOO=bar`              | `$env.FOO = "bar"`                                |
| `FOO=bar ./bin`               | `FOO=bar ./bin`, or `with-env {FOO: bar} { ./bin }` |
| `type foo`                    | `which foo`                                       |
| `cmd1 && cmd2`                | `cmd1; cmd2` (a failing command ends the call)   |
| `cmd1 \|\| cmd2`              | `try { cmd1 } catch { cmd2 }`                     |
| line continuation `\`         | wrap in `( ... )`                                 |

## HTTP: already parsed

`http get|post|put|...` parses JSON responses by their `Content-Type`. Do NOT pipe to
`from json`.

```nu
http get https://api.example.com/users | get 0.name    # just works
http get -H {Authorization: $"Bearer ($token)"} $url
http post -t application/json $url {key: "value"}
http get --full --allow-errors $url | get status       # the status, also for 404 and 500
http get --raw $url | from json                        # opt out of parsing
```

`-t json` does **not** work: pass the full MIME type (`application/json`).

## Parallelism

`par-each` runs closures on several threads; use it when order does not matter and the
work is not trivial.

```nu
ls **/*.rs
| par-each {|file|
    open $file.name | lines | length
  }

glob **/*.log
| par-each --threads 8 {|file|
    ...
  }
```

Use plain `each` when order matters or side effects must happen one after another.

## Globs and finding files

`ls **/*` skips hidden files and directories unless you pass `--all`. `glob **/*`
includes them, so it walks `.git`; trim with `--exclude` or `--depth`, e.g.
`glob **/*.rs --exclude [**/target/** **/.git/**]`. Nushell's `find` filters its input; it
does not search the file system. For file contents use `search`.

## Polars (when the plugin is registered)

For parquet, jsonl, ndjson, csv and avro, `polars` is much faster than native Nushell or
external tools. nux loads the plugins registered for `nu` (`plugin list`).

```nu
polars open data.parquet | polars select name status | polars save out.parquet
ps | polars into-df | polars collect
polars open x.parquet | polars into-nu            # back to a Nushell table
```

## Background jobs

`job spawn` runs a closure in the background, but only for the length of the call: when
the call ends, its jobs end. A server that must keep running is started from your shell,
not from nux; then `wait-for` it.

```nu
# Two slow things at once, within one call
let a = (job spawn { ^slow-thing-a | job send 0 })
let b = (job spawn { ^slow-thing-b | job send 0 })
[(job recv) (job recv)]
```

There is no `job ls` (use `job list`). `job recv` reads only the current job's mailbox and
takes no id. `job send` always takes a target id; the main thread is `0`.

## Other quick tips

- `detect columns` structures columnar CLI output (`df | detect columns`) better than a
  hand-written `parse` pattern.
- Nothing carries over between calls: every call is a fresh Nushell. What a call keeps
  (large results, the variables of a call that failed, `kept --save`) comes back with
  `kept <name>`.
- External processes do **not** see `let` variables, only environment variables.
