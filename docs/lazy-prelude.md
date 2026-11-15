# Loading only the prelude modules a call uses (tried, reverted)

Every nux call starts a fresh Nushell and loads the whole prelude. This was tried in two
versions and reverted for the simplicity of loading everything. The code is in the history:
`aa3ddc1` (text scan) and `142f61d` (stubs) on `development`, reverted right after.

## Why

A call that only returns `1` took about 41 ms, measured on the Coder workspace (release build,
average of 10):

- about 15 ms without any prelude: the engine, Nushell's `std`, the plugin registry
- about 26 ms for the prelude (about 1,700 lines), per module:
  - `git`: 8.6 ms
  - `testing`: 4.1 ms
  - `ast`: 3.1 ms
  - `usage`: 1.8 ms
  - `edit-file`: 1.5 ms
  - `output`: 0.9 ms
  - `servers`: 0.7 ms
  - `typescript`: 0.6 ms
  - `prelude.nu` itself (`py`, `js`, `search`): the rest

Parsing at build time is not possible: Nushell has no form of parsed modules that can be
saved and loaded again (a parsed command is a Rust trait object holding blocks with spans
into the engine's files). Nushell parses its own `std` again on every start too.

## Version 1: a text scan (`aa3ddc1`)

Before the prelude is loaded, the code is split into words (letters, digits, `_`, `-`; so
`--show` is not `show` and `^git` is `git`). `prelude.nu` is loaded with its
`export use nux/<module>` lines left out, except for the modules the words name:

- a module used without `*` (`git`, `edit-file`) by its own name;
- a module used with `*` by the names of its commands, read from its text
  (`export def`/`export alias`, after flags like `--env`; `main` is the module's name;
  `from tsc` is two words in a row);
- `nux/output` always: nux adds its `check-exit` after external commands itself
  (src/rewrite.rs), so it is never in the code as given.

Everything was loaded when the code `use`s or `source`s a file of its own (its code is not
seen; `use nux/...` and `use std...` excepted), mentions `help`, `scope`, `which` or `view`,
and for `nux help` and `nux find`.

Result: a call without modules 41.7 → 17.7 ms; `git status | length` 48.6 → 31.6 ms;
`search ...` 47.3 → 20.9 ms; with `use .nu/d1.nu *` unchanged (everything loads).

Drawbacks:
- a name in a string or comment loads its module (a few ms, harmless);
- code that uses a file of its own loads everything;
- a typo of a command in a module not loaded (`wait_for`) lost Nushell's "Did you mean
  `wait-for`?", which comes from the commands known to the engine.

## Version 2: stubs (`142f61d`)

Every command of a module not loaded gets a stub: `export def --wrapped "<name>" [...arguments]
{ error make {...} }`, one per command for a `*` module, one under the module's name otherwise.
Then:

1. `Nu::start` loads `prelude.nu` without those lines, and the stubs as an overlay of their
   own (`module nux-stubs { ... }`, `overlay use nux-stubs`).
2. Before running code, `Nu::load_what_code_uses` parses it once in a throwaway working set
   and collects every `FlatShape::InternalCall` (via `nu_parser::flatten_block`) from the
   block parse returns and every block parsing added: closures, and the functions of files
   the code `use`s or `source`s.
3. The modules whose stubs are called are loaded: `overlay hide nux-stubs`, the `use` lines,
   then stubs for what is still left out in a second overlay (`nux-stubs-left`). Calling
   `help`, `scope`, `which` or `view` loads all. A `use nux/<module>` in the code loads that
   module first (a plain text check), since it must not be parsed while its stub is there.
4. The code is parsed again for real and run.

Gains over version 1: a word in a string or comment loads nothing; a file of the code's own
loads only what it calls; "Did you mean `wait-for`?" works, also after other modules loaded.
Speed about the same as version 1 (`1`: 18 ms, `git status`: 36 ms).

Traps found on the way, each a bug at first:

- The block `nu_parser::parse` returns is not among the blocks it adds to the working set
  (those are closures and functions): it has to be flattened too, or top-level calls are
  missed.
- Calls inside the prelude's own modules must not count (`git.nu` calls `help commands`,
  which would load everything): blocks whose span is in a file named `nux/...` are skipped
  (`StateWorkingSet::find_file_by_span`).
- A module's `@example` blocks are parsed when it loads. An example that passes a block to
  one of the module's own commands (`<module> do { ... }`) found the module's stub, and a
  `--wrapped` stub takes no block ("Parse mismatch: expected non-block value:
  external-argument"). So the stubs must be gone before loading. A plain
  `hide "<stub>"` in the same source did not reach into modules parsed after it;
  `overlay hide` does. Hence the overlays.
- The check-exit rule from version 1 stays: `nux/output` is always loaded.

Cost in code: about 70 lines more than version 1, nearly all in `src/engine.rs`
(`load_what_code_uses`), plus `Later`, `with_stubs` and `stubs_source` in `src/prelude.rs`;
every call that uses a module is parsed twice; two overlays.

## Why reverted

The gain is about 20 ms per call. For now the simplicity of loading everything is worth more
than that, and version 2 needs its traps known by whoever changes it.

If it comes back, a middle way: version 1 plus only the typo hint, done by nux itself: after a
"command not found", compare the name with the commands of the modules not loaded and add
"Did you mean ...".

## Idea, not tried: signatures from build time

Nushell parses a `use`d module completely while parsing the code that uses it: its `def`s,
their bodies, its `@example` blocks, and the modules it uses in turn. It has to, because a call
cannot be parsed without the command's signature (flags, whether `{...}` is a closure or a
record, positional arguments). Before parsing a block it also collects the `def`s in it (a
pre-pass that reads only names and signatures), which is why a command can be called above its
definition. That pre-pass is cheap; the bodies and examples are what take the 26 ms. Doing
only the pre-pass at build time would therefore save little.

What build time can give is what the stubs lacked: every command's full signature (flags,
types, description, examples). `nu_protocol::Signature` can be serialized, so `build.rs` could
parse the prelude once and store the signatures in the binary. Then:

1. At startup nux registers every prelude command as a small Rust `Command` with its real
   signature: nothing is parsed, so a call costs about the 15 ms without prelude.
2. The code is parsed once, exactly: with the real signatures, flags, closures and
   `@example` blocks parse as they should, and `help <command>`, `nux find` and "Did you mean"
   work. None of the stub traps (no second parse, no overlays, no block-into-stub errors).
3. When such a command runs, it parses its module and hands the call to the real command.

Step 3 is the hard part. `Command::run` gets `&EngineState`, so it cannot add the module's
commands to the engine. It would parse the module into a clone of the engine (`EngineState`
is mostly `Arc`s, so a clone is cheap), find the real declaration there, and run it with the
same call, pipeline input and stack. Questions to settle first:

- whether the call's arguments (spans and blocks in the original engine) evaluate correctly
  against the clone, also for closures passed in;
- environment changes made by a command (`def --env`): the stack is shared, so probably fine;
- caching: the second call of a module's commands in the same process should not parse it
  again (a `OnceLock` in the command, holding the parsed clone, but the engine may have
  changed in between: new `def`s in the code);
- errors: spans in the module must still point into the right file for `in nux/git, line 3`;
- build time: `build.rs` would need nu-parser and the other Nushell crates as
  build-dependencies, and the stored signatures must match the commands exactly
  (a test that compares them with a full parse at runtime).

Expected complexity: at least that of version 2. A cheap first step before building it:
measure how much of the 26 ms is the `def` pre-pass and how much the bodies and examples, to
know what there is to gain.
