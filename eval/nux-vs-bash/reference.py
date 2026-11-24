"""Do a task the known-right way, to check check.py: python3 reference.py <task> <folder>.

Not shown to the agents. After it, check.py must say passed.
"""

import json
import math
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path


def git(folder: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=folder, check=True, capture_output=True, text=True).stdout


def git_history(folder: Path) -> None:
    touched = Counter(line for line in git(folder, "log", "--format=", "--name-only").splitlines() if line)
    carol = sum(int(line.split("\t")[0]) for line in git(folder, "log", "--author=Carol", "--format=", "--numstat").splitlines() if line.split("\t")[0].isdigit())
    answer = {
        "parse_config_added_in": git(folder, "log", "--format=%h", "--reverse", "-S", "function parseConfig").split()[0],
        "legacy_deleted_in": git(folder, "log", "--format=%h", "--diff-filter=D", "--", "src/legacy.js").split()[0],
        "most_changed_file": touched.most_common(1)[0][0],
        "lines_added_by_carol": carol,
    }
    (folder / "answer.json").write_text(json.dumps(answer))


def fix_tests(folder: Path) -> None:
    math_js = (folder / "src/math.js").read_text()
    math_js = math_js.replace("return sum(numbers) / numbers.length", "return numbers.length ? sum(numbers) / numbers.length : 0")
    math_js = math_js.replace("Math.min(low, Math.max(high, value))", "Math.min(high, Math.max(low, value))")
    (folder / "src/math.js").write_text(math_js)
    strings = (folder / "src/strings.js").read_text().replace("text.slice(0, length) + '…'", "text.slice(0, length - 1) + '…'")
    (folder / "src/strings.js").write_text(strings)
    failed = ["average of nothing is 0", "clamp keeps a value inside", "clamp lowers a high value", "truncate long text"]
    (folder / "answer.json").write_text(json.dumps({"failed_before": failed}))


def log_analysis(folder: Path) -> None:
    entries = [json.loads(line) for path in sorted((folder / "logs").glob("*.jsonl")) for line in path.read_text().splitlines()]
    errors = Counter(e["path"] for e in entries if e["status"] >= 500)
    orders = sorted(e["ms"] for e in entries if e["method"] == "GET" and e["path"] == "/api/orders")
    hours = Counter(e["ts"][:13] for e in entries)
    answer = {
        "top_5xx_paths": [p for p, _ in sorted(errors.items(), key=lambda i: (-i[1], i[0]))[:3]],
        "p95_ms_get_orders": orders[math.ceil(0.95 * len(orders)) - 1],
        "users_with_4xx": len({e["user"] for e in entries if 400 <= e["status"] < 500}),
        "busiest_hour": sorted(hours.items(), key=lambda i: (-i[1], i[0]))[0][0],
    }
    (folder / "answer.json").write_text(json.dumps(answer))


def refactor(folder: Path) -> None:
    for path in list((folder / "src").glob("*.js")) + list((folder / "test").glob("*.js")):
        text = path.read_text()
        # The handler's key is the public RPC name: it stays.
        text = re.sub(r"\bgetUser\b(?!:)", "fetchUser", text)
        path.write_text(text)


TASKS = {"git-history": git_history, "fix-tests": fix_tests, "log-analysis": log_analysis, "refactor": refactor}

if __name__ == "__main__":
    TASKS[sys.argv[1]](Path(sys.argv[2]))
