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

Through nux, like every other command (`~/projects/AGENTS.md`): the installed
`~/.local/bin/nux` is a copy, so building does not touch the nux you run.

```
nux <<< '^cargo build --release'
nux <<< '^cargo test --release | complete | get stdout | lines | where $it =~ "test result|FAILED"'
install -m 755 target/release/nux ~/.local/bin/nux     # from nux too: ^install ...
```

Release, also while developing: the dependencies (all of Nushell) are already built that
way, and a debug build would build them all again. Do not change `rust-toolchain.toml` or
`Cargo.lock` casually: either rebuilds every dependency, here and on GitHub.

Every change gets a test in `tests/cli.rs` (nux run as a process, as an agent uses it) or
next to the code. Check a test catches the bug: it should fail without the fix.

## Writing prelude commands

A command that gives a table gives every column in every row, with null where a row has
nothing (`git status`: `staged: null`). Agents filter with `where staged != null`, and a
column missing from some rows is an error there, unless they remember `staged?`, which they
do not. The same for records that come in lists. `tests/cli.rs` checks this for the table
commands (`table_commands_give_every_column_in_every_row`); add a new one there.

`nux help` (src/help.txt) is the manual agents read. When behaviour changes, change it too,
and run its examples: they must work as written.
