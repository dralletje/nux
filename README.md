# nux

Nushell for agents, as one native binary. Code goes in on stdin; every call runs in a fresh
Nushell with nux's own commands (the prelude) loaded; the result comes back as NUON, and
external commands fail the call when they fail. `nux help` is the manual, written for the
agent using it.

This is the Rust successor of `packages/nux-cli` in studio-saam (TypeScript, a session
daemon around `nu --mcp`). It builds Nushell in as libraries instead of running it, and has
no sessions: nothing carries over between calls.

```
cargo build --release          # also while developing; see Cargo.toml
cargo test --release
target/release/nux <<< 'ls | length'
```

The Rust version is pinned in `rust-toolchain.toml`: a new compiler rebuilds every
dependency, all of Nushell. CI (`.github/workflows/build.yml`) builds and tests on Linux
x86_64 and arm64, each natively, and keeps the binaries as artifacts; a tag `v*` makes a
release with them. Its cache keeps the compiled dependencies, so only the first run (and
one after a change to `Cargo.lock` or the Rust version) builds everything.

## How it works

- `src/main.rs`: command line, signals, usage log, exit status.
- `src/engine.rs`: a Nushell engine set up like `nu -n` (without colors), the prelude
  loaded with `use prelude.nu *`, then the code parsed and run; the result turned into
  what is printed. Results over 10 KiB are saved under `$NUX_DIR/results` instead.
- `src/rewrite.rs`: adds `| complete` (and `| check-exit`) after top-level external
  commands, so their exit code and output are seen. The statements come from our own parse
  of the code; the TypeScript version asked Nushell with `ast --json`.
- `src/format.rs`: errors (message, code, help, line and column in the code) and the output
  of external commands.
- `src/assist/`: help for the agent beyond what Nushell says, behind two hooks the engine
  calls: hints after an error (`flags.rs`: an unknown flag lists the real ones; `shell.rs`:
  bash habits like `$(...)`, `>`, `export X=`), and a preview of a result too large to print
  (`preview.rs`). A new helper is a file there plus one line in `assist/mod.rs`.
- `src/commands/`: nux's own commands in Rust, added in `commands/mod.rs`. `kept` (values
  kept for a next call: results too large to print, variables of a failed call, or
  `--save`), and `try-nu` (code in a child nux, the outcome as a record; the child is this
  binary, told by `NUX_TRY_*` environment variables to write `{value, error}` to a file).
- `src/children.rs`: when nux is stopped (SIGTERM, SIGINT, SIGHUP), ends the processes the
  call started. A SIGKILL cannot be caught; then they keep running.
- `src/usage_log.rs`: every call, in the same JSONL format as the TypeScript nux (plus a
  `runner` field), so `nux-log` reads both.
- `prelude/`: copied from `packages/nux-cli/prelude`. Found at `$NUX_PRELUDE_DIR`, else in
  this source tree.
- `src/guide.md`: `nux guide`, how to write Nushell well. Adapted from the guide Nushell's
  MCP server gives agents (`nu-mcp` crate, MIT), with what is about that server taken out.

Dependency versions come from Nushell 0.117's own `Cargo.lock` (copied as the starting
point): cargo's own choice took a pre-release of `notify` that does not compile.

## Differences from the TypeScript nux

- No sessions: `-s`, `nux list` and `nux stop` explain that and fail.
- No time limit (`-t`): the caller's applies. No `--json` (never used in 634 logged calls):
  `| to json --raw`. Both options fail with a message saying so.
- No value prints nothing (not `[]`), and a list of one item stays a list.
- A large result is kept as `result-<n>` (`kept result-3`), not as `$history.N`, and
  the variables of a failed call are kept too.
- Nushell's plugins (polars, query, formats, gstat) are not loaded.
- The usage log's `text` has the reply, but not what the code printed along the way.

## Not done yet

- The prelude inside the binary (with the `py`/`js` runners written out on first use).
- Plugins, if they turn out to be needed (the usage log had one call using them in 629).
- Carrying variables over between calls, or a real session (`nux serve`), later.
