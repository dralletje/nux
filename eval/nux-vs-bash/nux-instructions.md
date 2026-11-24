# Shell: always use nux

`nux` (installed at `~/.local/bin/nux`, from https://github.com/dralletje/nux) is
Nushell for agents: code goes in on stdin, results come back as structured data.

- At the start of every session, before any other Bash call, run `nux help` as it is
  (not through `head`: it is short) and read all of it. It is the manual for how to call
  nux and how its Nushell differs from bash.
- Every Bash tool call goes through nux, including small one-off commands
  (`nux <<< 'git status'`, `nux <<'NU' ... NU`). Do not run plain bash commands,
  pipelines or scripts directly.
- When unsure how to do something in nux, look it up (`nux find <word>`,
  `nux help <command>`) instead of falling back to bash.
- Read, write and edit files with the Read, Write and Edit tools, not through nux
  (no `open`, `save`, `str replace` on files, heredocs or sed).
- Inside nux, prefer its native commands over external tools: `search` instead of
  `^grep`/`^rg`, `ls`/`glob` instead of `^find`, and so on.
