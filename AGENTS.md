# Notes for agents working on nux

## Branches and releases

- Work on `development`: build, test and commit there, and push it as often as you like.
  Pushing `development` starts nothing.
- `main` is for releases only. Every push to `main` builds and tests on GitHub, and makes a
  release when the version in `Cargo.toml` has none yet. So push to `main` only to release,
  and only with the version raised in `Cargo.toml` (and `Cargo.lock`, which `cargo build`
  updates). Merge `development` into it; do not commit on `main` itself.
- Not sure whether to release? Then do not push to `main`; ask.

## Building and testing

```
cargo build --release
cargo test --release
```

Release, also while developing: the dependencies (all of Nushell) are already built that
way, and a debug build would build them all again. Do not change `rust-toolchain.toml` or
`Cargo.lock` casually: either rebuilds every dependency, here and on GitHub.

Every change gets a test in `tests/cli.rs` (nux run as a process, as an agent uses it) or
next to the code. Check a test catches the bug: it should fail without the fix.

`nux help` (src/help.txt) is the manual agents read. When behaviour changes, change it too,
and run its examples: they must work as written.
