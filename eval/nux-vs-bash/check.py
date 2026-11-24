"""Did a run do its task? python3 -I check.py <task> <folder> prints {"passed", "checks"}.

Every check is a name and whether it held; the run passed when all of them did. The truth
is computed from the fixture (git, the logs) where it can be, not written down.
"""

import json
import math
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
import setup  # noqa: E402


def answer(folder: Path) -> dict:
    try:
        return json.loads((folder / "answer.json").read_text())
    except Exception:
        return {}


def git(folder: Path, *args: str) -> str:
    return subprocess.run(["git", *args], cwd=folder, check=True, capture_output=True, text=True).stdout


def same_commit(folder: Path, given, truth: str) -> bool:
    if not isinstance(given, str) or len(given) < 7:
        return False
    try:
        return git(folder, "rev-parse", given.strip()).strip() == truth
    except subprocess.CalledProcessError:
        return False


def check_git_history(folder: Path) -> dict:
    given = answer(folder)
    added = git(folder, "log", "--format=%H", "--reverse", "-S", "function parseConfig", "--", "src/config.js").split()[0]
    deleted = git(folder, "log", "--format=%H", "--diff-filter=D", "--", "src/legacy.js").split()[0]
    touched = Counter(line for line in git(folder, "log", "--format=", "--name-only").splitlines() if line)
    most = touched.most_common(1)[0][0]
    carol = 0
    for line in git(folder, "log", "--author=Carol", "--format=", "--numstat").splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and parts[0].isdigit():
            carol += int(parts[0])
    return {
        "parse_config_added_in": same_commit(folder, given.get("parse_config_added_in"), added),
        "legacy_deleted_in": same_commit(folder, given.get("legacy_deleted_in"), deleted),
        "most_changed_file": given.get("most_changed_file") == most,
        "lines_added_by_carol": given.get("lines_added_by_carol") == carol,
    }


def node_test(folder: Path, *files: str) -> subprocess.CompletedProcess:
    return subprocess.run(["node", "--test", *files], cwd=folder, capture_output=True, text=True)


def check_fix_tests(folder: Path) -> dict:
    given = answer(folder)
    failed_before = {"average of nothing is 0", "clamp keeps a value inside", "clamp lowers a high value", "truncate long text"}
    tests_untouched = all((folder / path).read_text() == text for path, text in setup.TESTS.items())
    return {
        "tests_untouched": tests_untouched,
        "all_tests_pass": node_test(folder).returncode == 0,
        "named_the_failing_tests": set(given.get("failed_before") or []) == failed_before,
    }


def check_log_analysis(folder: Path) -> dict:
    given = answer(folder)
    entries = [json.loads(line) for path in sorted((folder / "logs").glob("*.jsonl")) for line in path.read_text().splitlines() if line]
    errors = Counter(e["path"] for e in entries if e["status"] >= 500)
    top = sorted(errors.items(), key=lambda item: (-item[1], item[0]))[:3]
    orders = sorted(e["ms"] for e in entries if e["method"] == "GET" and e["path"] == "/api/orders")
    p95 = orders[math.ceil(0.95 * len(orders)) - 1]
    users = len({e["user"] for e in entries if 400 <= e["status"] < 500})
    hours = Counter(e["ts"][:13] for e in entries)
    busiest = sorted(hours.items(), key=lambda item: (-item[1], item[0]))[0][0]
    return {
        "top_5xx_paths": given.get("top_5xx_paths") == [path for path, _ in top],
        "p95_ms_get_orders": given.get("p95_ms_get_orders") == p95,
        "users_with_4xx": given.get("users_with_4xx") == users,
        "busiest_hour": given.get("busiest_hour") == busiest,
    }


HIDDEN_TEST = """import { test } from 'node:test'
import assert from 'node:assert/strict'
import * as users from '../src/users.js'
import { greet } from '../src/greet.js'
import { adminName } from '../src/admin.js'
import { report } from '../src/report.js'
import { handle } from '../src/api.js'
import * as index from '../src/index.js'

test('fetchUser, and no getUser', () => {
  assert.equal(users.fetchUser(1).name, 'Ada')
  assert.equal(users.getUser, undefined)
  assert.equal(index.fetchUser(2).name, 'Grace')
})
test('the rest still works', () => {
  assert.equal(greet(1), 'Hello, Ada')
  assert.equal(adminName(2), 'GRACE')
  assert.equal(report([1, 9]), 'Ada, -')
})
test('the public rpc name getUser is still served', () => assert.equal(handle('getUser', { id: '1' }).name, 'Ada'))
"""


def check_refactor(folder: Path) -> dict:
    scratch = folder.parent / (folder.name + "-original")
    if not scratch.exists():
        scratch.mkdir()
        setup.refactor(scratch)
    hidden = folder / "test" / "zz-hidden-check.test.js"
    hidden.write_text(HIDDEN_TEST)
    hidden_ok = node_test(folder, str(hidden.relative_to(folder))).returncode == 0
    hidden.unlink()
    leftovers = []
    for path in sorted(list((folder / "src").rglob("*.js")) + list((folder / "test").rglob("*.js"))):
        for line in path.read_text().splitlines():
            code = re.sub(r"//.*$", "", line)
            # The handler's key is the public RPC name, bare or quoted: `getUser:`, `'getUser':`.
            if re.search(r"\bgetUser\b", code) and not re.match(r"""\s*['"]?getUser['"]?\s*:""", code):
                leftovers.append(f"{path.relative_to(folder)}: {line.strip()}")
    return {
        "hidden_test_passes": hidden_ok,
        "own_tests_pass": node_test(folder).returncode == 0,
        "routes_json_unchanged": (folder / "data/routes.json").read_text() == (scratch / "data/routes.json").read_text(),
        "no_getUser_left_in_code": not leftovers,
    }


def full_hash(folder: Path, given) -> str | None:
    if not isinstance(given, str) or len(given.strip()) < 7:
        return None
    try:
        return git(folder, "rev-parse", given.strip()).strip()
    except subprocess.CalledProcessError:
        return None


def git_messy_truth(folder: Path) -> dict:
    breaking = []
    for record in git(folder, "log", "--no-merges", "--format=%H%x1f%b%x1e").split("\x1e"):
        if "\x1f" not in record:
            continue
        # strip("\n") only: Python counts \x1f as whitespace.
        sha, body = record.strip("\n").split("\x1f", 1)
        if any(line.startswith("BREAKING CHANGE:") for line in body.splitlines()):
            breaking.append(sha)
    renames = set()
    for line in git(folder, "log", "-M", "--diff-filter=R", "--name-status", "--format=").splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and parts[0].startswith("R"):
            renames.add((parts[1], parts[2]))
    by_author = Counter()
    for record in git(folder, "log", "--no-merges", "--format=%x1e%an", "--", "src/api/").split("\x1e"):
        if record.strip():
            by_author[record.strip()] += 1
    merge = git(folder, "log", "--merges", "--format=%H", "-1").strip()
    deleted = 0
    for line in git(folder, "log", "--format=", "--numstat", f"{merge}^1..{merge}^2").splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and parts[1].isdigit():
            deleted += int(parts[1])
    return {"breaking": set(breaking), "renames": renames, "by_author": dict(by_author), "deleted": deleted}


def check_git_messy(folder: Path) -> dict:
    given = answer(folder)
    truth = git_messy_truth(folder)
    breaking = {full_hash(folder, h) for h in (given.get("breaking_commits") or [])}
    renames = {(r.get("from"), r.get("to")) for r in (given.get("renames") or []) if isinstance(r, dict)}
    return {
        "breaking_commits": breaking == truth["breaking"],
        "renames": renames == truth["renames"],
        "api_commits_by_author": given.get("api_commits_by_author") == truth["by_author"],
        "feature_lines_deleted": given.get("feature_lines_deleted") == truth["deleted"],
    }


TRIAGE_FAILING = {
    ("test/mod07.test.js", "parses dates in March"): "March 31 should stay in March",
    ("test/mod22.test.js", "rounds half up"): "2.5 should round to 3",
    ("test/mod35.test.js", "keeps order of equal keys"): "equal keys must keep their order",
}


def check_test_triage(folder: Path) -> dict:
    given = answer(folder)
    failing = {}
    for row in given.get("failing") or []:
        if isinstance(row, dict):
            failing[(str(row.get("file", "")).removeprefix("./"), row.get("test"))] = str(row.get("message", ""))
    untouched = all((folder / "test" / f"mod{m:02d}.test.js").exists() for m in range(40)) and not (folder / "src").exists()
    return {
        "failing_tests": set(failing) == set(TRIAGE_FAILING),
        "messages": all(expected in failing.get(key, "") for key, expected in TRIAGE_FAILING.items()),
        "skipped": given.get("skipped") == 2,
        "todo": given.get("todo") == 1,
        "tests_untouched": untouched,
    }


def csv_join_truth(folder: Path) -> dict:
    import csv

    customers = {c["id"]: c for c in json.loads((folder / "customers.json").read_text())}
    rates = json.loads((folder / "rates.json").read_text())
    with open(folder / "orders.csv", newline="") as f:
        orders = list(csv.DictReader(f))
    totals = Counter()
    for order in orders:
        customer = customers[order["customer_id"]]
        if order["date"].startswith("2026") and customer["signup"].startswith("2025"):
            totals[customer["country"]] += float(order["amount"]) * rates[order["currency"]]
    top = sorted(totals.items(), key=lambda item: -item[1])[:3]
    return {
        "top": [(country, round(eur, 2)) for country, eur in top],
        "line_breaks": sum(1 for o in orders if "\n" in o["note"] or "\r" in o["note"]),
        "without": len(set(customers) - {o["customer_id"] for o in orders}),
    }


def check_csv_join(folder: Path) -> dict:
    given = answer(folder)
    truth = csv_join_truth(folder)
    top = given.get("top_countries") or []
    top_ok = len(top) == 3 and all(
        isinstance(row, dict) and row.get("country") == country and isinstance(row.get("eur"), (int, float)) and abs(row["eur"] - eur) <= 0.01
        for row, (country, eur) in zip(top, truth["top"])
    )
    return {
        "top_countries": top_ok,
        "notes_with_line_breaks": given.get("notes_with_line_breaks") == truth["line_breaks"],
        "customers_without_orders": given.get("customers_without_orders") == truth["without"],
    }


CHECKS = {"git-history": check_git_history, "fix-tests": check_fix_tests, "log-analysis": check_log_analysis, "refactor": check_refactor,
          "git-messy": check_git_messy, "test-triage": check_test_triage, "csv-join": check_csv_join}

if __name__ == "__main__":
    task, folder = sys.argv[1], Path(sys.argv[2])
    checks = CHECKS[task](folder)
    print(json.dumps({"passed": all(checks.values()), "checks": checks}))
