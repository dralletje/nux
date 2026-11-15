"""Runs Python code for the `py` command of nux (see prelude.nu).

    python3 py_runner.py <code>     with the input as NUON on stdin

The input is available to the code as `data`. The value of the last expression (or of the
variable `result`, when the code ends in a statement) is written to stdout as NUON. NUON is
Nushell's data notation (see nuon.py): in both directions dates, durations, file sizes and
bytes travel as those types and not as text.
Anything the code prints goes to stderr, to keep stdout for the value.
"""

import ast
import contextlib
import linecache
import os
import sys
import traceback

import nuon

CODE_NAME = "<py>"


def run(code, data):
    """The value of the code: its last expression, else its variable `result`."""
    linecache.cache[CODE_NAME] = (len(code), None, code.splitlines(True), CODE_NAME)
    tree = ast.parse(code, CODE_NAME)
    last = tree.body[-1] if tree.body else None
    scope = {"__name__": "__main__", "data": data, "Filesize": nuon.Filesize}
    if isinstance(last, ast.Expr):
        tree.body.pop()
        exec(compile(tree, CODE_NAME, "exec"), scope)
        return eval(compile(ast.Expression(last.value), CODE_NAME, "eval"), scope)
    exec(compile(tree, CODE_NAME, "exec"), scope)
    return scope.get("result")


def main():
    code = sys.argv[1]
    raw = sys.stdin.read()
    data = nuon.loads(raw) if raw.strip() else None
    # Like `python -c`: modules next to where the code is run can be imported.
    sys.path.insert(0, os.getcwd())
    try:
        with contextlib.redirect_stdout(sys.stderr):
            text = nuon.dumps(run(code, data))
    except BaseException as error:
        if isinstance(error, SystemExit) and not error.code:
            sys.exit(0)
        # Show the code's own frames, not this runner's.
        frames = [f for f in traceback.extract_tb(error.__traceback__) if f.filename != __file__]
        if isinstance(error, SyntaxError):
            frames = []
        sys.stderr.write("".join(traceback.format_list(frames)))
        sys.stderr.write("".join(traceback.format_exception_only(type(error), error)))
        sys.exit(1)
    sys.stdout.write(text)


if __name__ == "__main__":
    main()
