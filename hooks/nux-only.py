#!/usr/bin/env python3
"""Claude Code PreToolUse hook: Bash runs nux, and nothing else.

Allowed, as `nux help` teaches them:
    nux <<< 'code'                      one line, the code without single quotes
    nux -r 'reason' <<'NU'              a heredoc with a quoted delimiter, up to a line
    ...                                 that is the delimiter alone, which ends the command
    NU
    nux help [command]   nux find <word>
                                        also piped to head, tail, grep, less or more
                                        (`nux help | head -60`), to read the manual
Each may carry -r 'reason'. Anything else is blocked (exit 2), and the reason goes back to
the agent: a pipe after nux, `cd x && nux`, env prefixes, a second command after the heredoc.

One exception: a command with run_in_background passes. A server does not belong in a nux
call (nux shows a command's output when it ends), so the harness's background run is the
way to start one, and this hook lets it through as it is.
"""

import json
import re
import sys

REASON = r"(?: -r '[^'\n]*')?"
ONE_LINE = re.compile(rf"nux{REASON} <<< '[^'\n]*'")
# The manual may be read the way agents read any text: `nux help | head -60` was blocked
# in 4 of 8 runs of an eval (docs/nux-vs-bash-eval.md). Only viewers that print, with plain
# arguments: no `$`, backticks, `;`, `&` or redirections. sed only as `sed -n '1,80p'`
# (another script can write files).
ARGUMENT = r"(?:'[^'\n$`]*'|[\w.,:/^*+?=\[\]{}-]+)"
VIEWER = rf"(?:(?:head|tail|grep|less|more)(?: +{ARGUMENT})*|sed -n '\d+,\d+p')"
HELP = re.compile(rf"nux{REASON} (?:help(?: [\w -]+)?|find [\w -]+?)(?: *\| *{VIEWER})*")
HEREDOC_START = re.compile(rf"nux{REASON} <<'(\w+)'")


def allowed(command: str) -> bool:
    command = command.strip()
    if ONE_LINE.fullmatch(command) or HELP.fullmatch(command):
        return True
    lines = command.split("\n")
    start = HEREDOC_START.fullmatch(lines[0])
    if not start:
        return False
    delimiter = start.group(1)
    # The delimiter must be the last line and appear nowhere before it: whatever came
    # after an earlier one would be a shell command of its own.
    ends = [i for i, line in enumerate(lines) if line == delimiter]
    return ends == [len(lines) - 1]


def main() -> None:
    event = json.load(sys.stdin)
    if event.get("tool_name") != "Bash":
        return
    tool_input = event.get("tool_input") or {}
    if tool_input.get("run_in_background"):
        return
    if allowed(tool_input.get("command", "")):
        return
    print(
        "Blocked: Bash runs nux only, as one call and nothing around it:\n"
        "  nux -r 'why' <<< 'code'          (code without single quotes)\n"
        "  nux -r 'why' <<'NU'              (anything else; NU alone on the last line)\n"
        "  ...\n"
        "  NU\n"
        "No pipes, `&&`, `cd` or variables outside it: do that inside the Nushell code. "
        "Files: the Read, Write and Edit tools. Servers: run_in_background. "
        "How: nux help.",
        file=sys.stderr,
    )
    sys.exit(2)


if __name__ == "__main__":
    main()
