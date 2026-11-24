# `nux/npm`: the npm registry, as tables (2026-11-24)

## Why

Agents ask the npm registry about packages with `^npm view ... --json | from json` and
reshape what comes back by hand: the newest version of a major, dist-tags, when versions
were published, peerDependencies, a manifest's files and exports, and what a published
package contains (`npm pack`, which downloads into the current directory). In the 30 days
of the nux log before this: 44 calls ran npm, 9 failed; inside them `npm view` 58 times,
`npm pack` 5. A range (`react-router@7`) gives a list, one version a string or record, and
`time` is a record with `created` and `modified` among the versions.

`nux help` lost its `npm view` line when it was made compact (docs/compact-help.md); a
module gives tables and takes one line there.

## Goals

- Registry questions as tables and records, in one command each.
- Private packages (@limbic/*) and scoped registries work as with npm itself.

## Non-goals

- Installing, running scripts, a project's dependencies: pnpm's (`nux/pnpm`), and later
  perhaps here.

## Approaches considered

1. The npm CLI (`npm view`, `npm pack --dry-run`). Chosen: it reads ~/.npmrc, so auth and
   scoped registries need nothing; every query is one or two calls that give JSON.
2. The registry over HTTP. Faster and no npm needed, but .npmrc (scoped registries, auth
   tokens) would have to be read by us, or @limbic/* gives 404; listing files would mean
   downloading and unpacking the tarball.

## Design

Loaded with `use nux/npm`, which names its commands `npm ...`. There is no `npm` command of
its own: `npm install` and every other `npm ...` still run npm (as `nux/pnpm`).

- `npm versions <package>`: one row per version: version, published (datetime), tags
  (list of dist-tags pointing at it, [] for none), deprecated (text or null). Sorted by
  published, oldest first, so `| last` is the newest. From
  `npm view <package> time dist-tags versions --json` (without time's `created` and
  `modified`) and `npm view '<package>@>=0.0.0-0' version deprecated --json` for the
  deprecation per version (the plain `deprecated` field is only the latest's).
- `npm info <spec>`: a record, from `npm view <spec> --json`. A range gives npm a list:
  the highest version that matches. Leaves out the `_`-fields and `versions`, `time`,
  `users`, `readme` (versions: `npm versions`).
- `npm files <spec>`: one row per file in the published package: path, size (filesize).
  From `npm pack <spec> --dry-run --json`: nothing is written.
- A package or version that is not there: an error with npm's own summary
  ("Not Found - GET ...", "No match found for version 99").

Every column in every row (AGENTS.md). Examples and @search-terms per command. The main
help names it in its list of modules; prelude.nu's note on modules loaded with `use` too.
Tests with a stand-in `npm` that prints recorded output, no network (as the pnpm tests).

## Left to the builder

Wording of help and errors.
