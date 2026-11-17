# ast-find --rewrite: only what the pattern names is replaced

Tests and history for one rule of `ast-find --rewrite` / `ast-apply` (prelude/ast.nu). The
cases are in `cases.json`; `tests/ast_rewrite.rs` runs each one through nux, as an agent
would: `ast-find ... --rewrite ... | ast-apply`, then compares the file.

## Why

A review of a day of agent use (2026-11-17, nux 0.2.0) found imports that had lost their
`;` after a mass rewrite, next to imports that still had it. Nobody noticed until much
later. The call:

```nu
ast-find --lang ts 'import { $$$A } from "./x"' . --rewrite 'import { $$$A } from "./z"' | ast-apply
```

turned `import { a } from "./x";` into `import { a } from "./z"`.

It is ast-grep's behaviour, not something nux added: `ast-grep run -p '...' -r '...'` does
the same. A pattern is parsed as code; `import { $$$A } from "./x"` is a statement, and in
the tree-sitter grammars a statement's node includes its `;`. So the pattern matches the
statement with its `;`, although the pattern has none, and the replacement takes the place
of the whole match. Patterns that are expressions (`foo($A)`) are not affected: an
expression's node has no `;`. Seen with ast-grep 0.51.3 in TypeScript, JavaScript and TSX
(import, return, const, export, throw), Rust (let, use), C and Java (return).

## The rule

Only the code the pattern names is replaced. What the match has after the end of the
pattern stays in the file.

Where the pattern ends in a match: after the pattern's last hole (where it matched is in
ast-grep's JSON, `metaVariables`), the pattern's remaining text is followed through the
match, character by character, whitespace aside. What is left of the match after that is
the trailing part: `;`, or ` /* why */;`. Nothing in this knows a language: there is no
list of terminators, only "the pattern did not say this".

`ast-find --rewrite` then gives each row:

- `text`, `end`: the match up to the end of the pattern (so `ast-apply` replaces only that);
- `trailing`: the part after it, which stays (null when there is none).

Because the range is narrowed in `ast-find` (not added back in `ast-apply`), a replacement
that is changed or computed afterwards (`update replacement {...}`) keeps the `;` as well.

Two exceptions, both cases in `cases.json`:

- a rewrite that ends in the trailing text itself (`--rewrite 'import ... "./z";'` for a
  pattern without `;`): the whole match is replaced, so the `;` is not doubled;
- a rewrite to '' (deleting): the whole match is deleted, `;` included. A lone `;` left
  behind is never what is wanted.

### Holes ast-grep does not report

Anonymous holes (`$_`, `$_X`, `$$$`) are not in ast-grep's `metaVariables`, so where they
matched is not known. With --rewrite, nux gives each a name of its own before searching
(`$NUXANON0`, `$$$NUXANON1`): distinct names, so they still match independently, as
anonymous holes do. They are left out of `vars`. A `$$$` hole that matched nothing has no
place in the match; the pattern is followed as if it were not there.

### When the end cannot be found

When the pattern's text after its last hole is not found in the match (ast-grep matched
something written differently than whitespace can explain, a comment inside the pattern's
own text), the match is replaced whole, as before. No case for this is known; should one
turn up, add it to `cases.json`.

## Not covered by this rule

- Deleting an expression (`console.log($A)` with `--rewrite ''`) leaves its `;`: the match
  never had it. Put the `;` in the pattern to delete the statement. A case documents both.
- ast-grep lays out what a `$$$` hole matched in its own way when it spans lines:
  `f(\n  1,\n  2\n)` rewritten with `g($$$A)` becomes `g(1,\n2)`. Seen while writing these
  cases; not addressed.
- Without --rewrite, `ast-find` is unchanged: `text` is the whole match, and there is no
  `trailing` column.

## Other tools

Looked at before choosing to keep ast-grep and add this rule: comby (matches text templates,
no syntax tree, so it has no such node but also no structure), GritQL, semgrep (heavy), and
jscodeshift / ts-morph (JavaScript only). None was clearly better at this; each changes what
agents already know how to write.

## History

- 2026-11-17: found in the review of nux 0.2.0 (`nu-feedback.md`, item 1.2). Reproduced in
  ast-grep itself.
- 2026-11-17 (later the same day): the rule above, with these 30 cases. With 0.2.0's ast.nu, 19 of them fail
  (every one with something after the pattern); the 11 others pass both ways. CI now
  installs ast-grep 0.51.3, so these run there too (they are skipped without ast-grep).

## Adding a case

An object in `cases.json`: `name`, `why` (one line: what it shows), `lang` (as for --lang),
`ext`, `source`, `pattern`, `rewrite`, `expected` (the whole file after `ast-apply`), and
`trailing` (the first row's, or null). Run `cargo test --release --test ast_rewrite`.
