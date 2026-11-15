# nux

Nushell for agents, as one native binary. Code goes in on stdin; every call runs in a fresh
Nushell with nux's own commands (the prelude) loaded; the result comes back as NUON, and
external commands fail the call when they fail. `nux help` is the manual, written for the
agent using it.

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
(`js`), `ss` (`ports`, `wait-for` a port). Plugins are the ones registered for `nu`, or
with `plugin add` in nux; they must be of Nushell 0.117.

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
- `prelude/`: nux's commands written in Nushell, bundled into the binary by `build.rs` and
  written out on first use (`src/prelude.rs`). `NUX_PRELUDE_DIR` points nux at this folder
  instead, to work on it without building.
- `src/guide.md`: `nux guide`, how to write Nushell well. Adapted from the guide Nushell's
  MCP server gives agents (`nu-mcp` crate, MIT), with what is about that server taken out.
