# nux

Nushell for agents, as one native binary. Code goes in on stdin; every call runs in a fresh
Nushell with nux's own commands (the prelude) loaded; the value of every statement comes
back in a form made for reading (tables as Markdown, records as `key: value` lines; NUON
with `NUX_FORMAT=nuon`), and external commands fail the call when they fail. `nux help` is
the manual, written for the agent using it.

A call, in this repository:

```
nux <<'NU'
open Cargo.toml | get package.version
ls src | where size > 10kb | select name size
NU
```

```
── line 1: open Cargo.toml | get package.version
0.3.0
── line 2: ls src | where size > 10kb | select name size
| name | size |
|---|---|
| src/engine.rs | 24.17kB |
| src/format.rs | 11.893kB |
| src/help.txt | 11.248kB |
| src/readable.rs | 15.499kB |
| src/rewrite.rs | 12.499kB |
```

It builds Nushell 0.117 in as libraries, with its prelude and plugin support, so it needs
no `nu` on the machine. It has no sessions: nothing carries over between calls, except what
is kept (`kept`). It replaced a TypeScript version (a session daemon around `nu --mcp`),
which is in the history of studio-saam (`packages/nux-cli`).

## Install

The latest release, for Linux on x86_64 or arm64 (glibc 2.34 or newer):

```
arch=$(uname -m | sed 's/aarch64/arm64/')
curl -fsSL "https://github.com/dralletje/nux/releases/latest/download/nux-linux-$arch.tar.gz" \
  | tar -xz -C ~/.local/bin nux
```

Tools some commands use, when they are on PATH: `rg` (`search`), `ast-grep` (`ast-find`,
from https://github.com/ast-grep/ast-grep/releases), `python3` (`py`), Node 26 or newer
(`js`, or `NUX_NODE` set to one), `ss` (`ports`, `wait-for port`). Plugins are the ones
registered for `nu`, or with `plugin add` in nux; they must be of Nushell 0.117.

### Let an agent install it

Paste this to the agent, on the machine nux goes on:

```
Install nux (https://github.com/dralletje/nux), the tools it uses and Nushell's plugins.
Linux, x86_64 or arm64. No sudo: everything goes in ~/.local, nothing through apt.
Skip any step whose tool is already there and new enough.

1. nux: the release tarball for this architecture (arm64 for aarch64), into ~/.local/bin:
   https://github.com/dralletje/nux/releases/latest/download/nux-linux-<x86_64|arm64>.tar.gz
   Make sure ~/.local/bin is on PATH. Then run `nux help` as it is (not through `head`)
   and read all of it: from here
   on, run every command through nux as it describes.
2. ripgrep (`rg`, for `search`): the latest release of BurntSushi/ripgrep, the
   x86_64-unknown-linux-musl or aarch64-unknown-linux-gnu tarball; `rg` into ~/.local/bin.
3. ast-grep (for `ast-find`): the latest release of ast-grep/ast-grep,
   app-<x86_64|aarch64>-unknown-linux-gnu.zip; `ast-grep` into ~/.local/bin. Not `sg`:
   on many systems that name is a different program (shadow-utils).
4. Node 26 or newer (for `js`): if `node --version` is older, the latest Node 26 linux
   tarball from nodejs.org, unpacked to ~/.local/opt/node26, without replacing the
   system node. Then set NUX_NODE=~/.local/opt/node26/bin/node (absolute path) where the
   agent's environment comes from; for Claude Code that is "env" in ~/.claude/settings.json.
5. python3 (for `py`) and ss (for `ports`): say so if either is missing; do not install.
6. Plugins: the Nushell 0.117.0 release tarball (exactly that version; plugins must match),
   https://github.com/nushell/nushell/releases/tag/0.117.0,
   nu-0.117.0-<x86_64|aarch64>-unknown-linux-gnu.tar.gz. Copy every nu_plugin_* binary in
   it to ~/.local/bin, and register each one from nux: plugin add ~/.local/bin/nu_plugin_x
7. Check, all in one nux call: `plugin list` shows each plugin at 0.117.0;
   `search 'nux' ~/.local/bin --files` and `ast-find --lang js 'console.log($A)' .`
   run without a missing-tool error; `[1 2] | js 'data.length'` and `[1 2] | py 'len(data)'`
   give 2. Report what is installed, with versions, and anything that failed.
```

## Configuring

Nothing needs configuring. What can be:

- `~/.config/nux/prelude.nu`: runs at the start of every call, so its `def`s and `$env` are
  there in each one.
- Modules of your own, loaded in a call with `use`: `use home/tools` is
  `~/.config/nux/tools.nu` (or `tools/mod.nu`), and `use project/checks` is `.nu/checks.nu`
  in the project (the nearest folder upwards with a `.git`).
- `XDG_CONFIG_HOME`: when it is set, `$XDG_CONFIG_HOME/nux` instead of `~/.config/nux`.
- `NUX_FORMAT=nuon`: every result as NUON, not in the form made for reading.
- `NUX_NODE`: the `node` that `js` runs, when the one on PATH is older than Node 26.

nux keeps its own files in two places: `/tmp/nux-<uid>` (values kept for a next call, and the
runners of `py` and `js`) and `~/.local/state/nux/log` (every call, a file per day;
`nux-log` reads it; `NUX_LOG_DIR` puts it elsewhere).

## Telling agents to use it

In `AGENTS.md`, `CLAUDE.md` or wherever the agent reads its standing instructions:

```markdown
# Shell: always use nux

`nux` (https://github.com/dralletje/nux) is Nushell for agents: code goes in on stdin,
results come back as structured data.

- At the start of every session, before any other shell command, run `nux help` as it is
  (not through `head`: it is short) and read all of it. It is the manual for how to call
  nux and how its Nushell differs from bash.
- Every shell command goes through nux, including small one-off commands
  (`nux <<< 'git status'`, `nux <<'NU' ... NU`). Do not run plain bash commands,
  pipelines or scripts directly.
- When unsure how to do something in nux, look it up (`nux find <word>`,
  `nux help <command>`) instead of falling back to bash.
- Read, write and edit files with your file tools, not through nux (no `open`, `save`,
  `str replace` on files, heredocs or sed).
- Inside nux, prefer its native commands over external tools: `search` instead of
  `^grep`/`^rg`, `ls`/`glob` instead of `^find`, and so on.
```

The file-tools rule is for agents that have them (Claude Code: Read, Write, Edit); leave it
out for one whose only tool is the shell.

## Claude Code: nux and nothing else (optional)

Instructions are followed most of the time. To make it every time, `hooks/nux-only.py` is a
PreToolUse hook that blocks a Bash call unless it is one nux call, as `nux help` writes
them: `nux <<< '...'`, `nux <<'NU'` up to the last line `NU`, `nux help`, `nux find`, each
with or without `-r '...'`; `nux help` and `nux find` also piped to head, tail, grep, less
or more, to read the manual. Any other pipe after it, `cd x &&` before it, a second
command after the heredoc: blocked, and the agent is told how to do it instead. It holds in every
permission mode, `bypassPermissions` too, which permission rules alone do not (a deny rule
cannot say "everything except nux", and allow rules are skipped when bypassing).

One way around it is left open on purpose: a call with `run_in_background` runs as it is,
because that is how to start a server (`nux help`: a server does not belong in a nux call).

Install it (needs `python3`):

```
mkdir -p ~/.claude/hooks
curl -fsSL https://raw.githubusercontent.com/dralletje/nux/main/hooks/nux-only.py \
  -o ~/.claude/hooks/nux-only.py
```

and in `~/.claude/settings.json` (or a project's `.claude/settings.json`, for that project
only), next to what is there:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          { "type": "command", "command": "python3 ~/.claude/hooks/nux-only.py" }
        ]
      }
    ]
  }
}
```

The hook reads only the command, so it is no sandbox: inside nux, `^rm -rf` runs as it
would in bash. It makes agents use nux, not behave.

## Working on it

Work on `development`; `main` is for releases (see AGENTS.md).

```
cargo build --release          # also while developing; see Cargo.toml
cargo test --release
target/release/nux <<< 'ls | length'
```

The Rust version is pinned in `rust-toolchain.toml`: a new compiler rebuilds every
dependency, all of Nushell. CI (`.github/workflows/build.yml`) runs on every push to `main`:
it builds and tests on Linux x86_64 and arm64, each natively. When the version in
`Cargo.toml` has no release yet, it then makes one (`v<version>`) with both binaries. Its
cache keeps the compiled dependencies, so only the first run (and one after a change to
`Cargo.lock` or the Rust version, or a week without runs) builds everything.

Dependency versions come from Nushell 0.117's own `Cargo.lock` (copied as the starting
point): cargo's own choice took a pre-release of `notify` that does not compile.

## How it works

- `src/main.rs`: command line, signals, usage log, exit status.
- `src/engine.rs`: a Nushell engine set up like `nu -n` (without colors, with the plugins
  registered for `nu`), the prelude loaded with `use prelude.nu *`, then the code parsed and
  run; the result turned into what is printed.
- `src/rewrite.rs`: adds `| complete` (and `| check-exit`) after top-level external
  commands, so their exit code and output are seen. Errors are shown at their place in the
  code as written, not as rewritten.
- `src/format.rs`: errors (message, code, help, line and column in the code) and the output
  of external commands.
- `src/assist/`: help for the agent beyond what Nushell says, behind two hooks the engine
  calls: hints after an error (unknown flags, bash habits, glob patterns, text in `( )`
  inside `$"..."`), and a preview of a result too large to print. A new helper is a file
  there plus one line in `assist/mod.rs`.
- `src/commands/`: nux's own commands in Rust, added in `commands/mod.rs`. `kept` (values
  kept for a next call: results too large to print, variables of a failed call, or
  `--save`), and `try-nu` (code in a child nux, the outcome as a record; the child is this
  binary, told by `NUX_TRY_*` environment variables to write `{value, error}` to a file).
- `src/children.rs`: when nux is stopped (SIGTERM, SIGINT, SIGHUP), ends the processes the
  call started. A SIGKILL cannot be caught; then they keep running.
- `src/usage_log.rs`: every call, as JSONL in `~/.local/state/nux/log` (`nux-log` reads it).
- `src/paths.rs`: nux's folder, `/tmp/nux-<uid>` (owner-only; refused when it is not).
- `prelude/`: nux's commands written in Nushell, bundled into the binary by `build.rs`
  (`src/prelude.rs`). The `.nu` files are loaded from the binary as virtual modules under
  `nux/`, the way Nushell loads `std`: `git.nu` is `nux/git`, a folder `x/` with a `mod.nu`
  (and `x/y.nu`) is `nux/x` (and `nux/x/y`), and a call can `use` them too. A module in
  `prelude/` refers to another one by that name (`use nux/git`), not by a relative path, which
  is not found. Only the
  runners of `py` and `js` are written out, on first use, to `/tmp/nux-<uid>/runners/`.
  `NUX_PRELUDE_DIR` points nux at this folder instead, to work on it without building.
  Every call loads all of it (about 26 of a call's 41 ms); loading only what a call uses was
  tried and reverted for simplicity: `docs/lazy-prelude.md`.
- `src/help.txt`: `nux help`, the manual agents read. Its FROM BASH table and several of
  its GOOD TO KNOW notes are adapted from the guide Nushell's MCP server gives agents
  (`nu-mcp` crate, MIT).
