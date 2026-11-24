"""Tests of nux-only.py: python3 hooks/test_nux_only.py (no output and exit 0: all pass)."""

import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location("nux_only", Path(__file__).parent / "nux-only.py")
nux_only = importlib.util.module_from_spec(spec)
spec.loader.exec_module(nux_only)

ALLOWED = [
    "nux help",
    "nux help search",
    "nux find str",
    "nux -r 'read the manual' help",
    "nux <<< 'git status'",
    "nux -r 'why' <<'NU'\nls\nNU",
    # The manual read the way agents read text (it was blocked: 4 of 8 sonnet runs in the
    # eval started so, docs/nux-vs-bash-eval.md).
    "nux help | head -60",
    "nux help | head -n 60",
    "nux help search | tail -20",
    "nux find str | grep replace",
    "nux help | grep -i 'wait-for'",
    "nux help | sed -n '1,80p'",
    "nux help | less",
    "nux help | head -80 | tail -20",
]

BLOCKED = [
    "git status",
    "nux help | bash",
    "nux help | sh -c 'rm -rf x'",
    "nux help; rm -rf x",
    "nux help | head -5 && rm x",
    "nux help | head -5; rm x",
    "nux help | grep $(rm x)",
    "nux help | grep `rm x`",
    "nux help > out.txt",
    "nux help | tee out.txt",
    "cat notes | nux help",
    "nux <<< 'git status' | head",
    "nux -r 'why' <<'NU'\nls\nNU\n| head",
]

failures = [f"should pass: {c!r}" for c in ALLOWED if not nux_only.allowed(c)]
failures += [f"should be blocked: {c!r}" for c in BLOCKED if nux_only.allowed(c)]
if failures:
    raise SystemExit("\n".join(failures))
