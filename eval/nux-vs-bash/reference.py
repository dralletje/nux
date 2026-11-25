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


sys.path.insert(0, str(Path(__file__).parent))
import check  # noqa: E402


def git_messy(folder: Path) -> None:
    truth = check.git_messy_truth(folder)
    answer = {
        "breaking_commits": sorted(truth["breaking"]),
        "renames": [{"from": a, "to": b} for a, b in sorted(truth["renames"])],
        "api_commits_by_author": truth["by_author"],
        "feature_lines_deleted": truth["deleted"],
    }
    (folder / "answer.json").write_text(json.dumps(answer))


def test_triage(folder: Path) -> None:
    failing = [{"file": f, "test": t, "message": m} for (f, t), m in check.TRIAGE_FAILING.items()]
    (folder / "answer.json").write_text(json.dumps({"failing": failing, "skipped": 2, "todo": 1}))


def csv_join(folder: Path) -> None:
    truth = check.csv_join_truth(folder)
    answer = {
        "top_countries": [{"country": c, "eur": e} for c, e in truth["top"]],
        "notes_with_line_breaks": truth["line_breaks"],
        "customers_without_orders": truth["without"],
    }
    (folder / "answer.json").write_text(json.dumps(answer))


REPORT_CSV = """
if (process.argv.includes('--format') && process.argv[process.argv.indexOf('--format') + 1] === 'csv') {
  const field = (text) => (/[",\\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text)
  console.log(['customer,orders,total', ...rows.map((r) => [field(r.customer), r.orders, r.total.toFixed(2)].join(','))].join('\\n'))
} else {
  console.log(JSON.stringify(rows, null, 2))
}
"""


def long_session(folder: Path) -> None:
    git(folder, "checkout", "-q", "-b", "report-csv")
    money = folder / "src/money.js"
    money.write_text(money.read_text().replace("Math.floor(sum * 100)", "Math.round(sum * 100)"))
    discounts = folder / "src/discounts.js"
    discounts.write_text(discounts.read_text().replace("total > FREE_SHIPPING_FROM", "total >= FREE_SHIPPING_FROM"))
    git(folder, "commit", "-qam", "fix: round to the nearest cent, free shipping from 50 itself")
    for place in ["src", "test", "bin"]:
        for path in (folder / place).rglob("*.js"):
            path.write_text(path.read_text().replace("calcTotal", "computeTotal"))
    git(folder, "commit", "-qam", "refactor: calcTotal is computeTotal")
    report = folder / "bin/report.js"
    report.write_text(report.read_text().replace("console.log(JSON.stringify(rows, null, 2))\n", REPORT_CSV.lstrip("\n")))
    git(folder, "commit", "-qam", "feat: report --format csv")
    truth = check.long_session_truth(folder)
    answer = {"rounding_bug_commit": truth["bug"], "q1_2026_by_region": truth["q1"], "top_rep_2025": truth["top_rep"]}
    (folder / "answer.json").write_text(json.dumps(answer))


def multi_turn(folder: Path) -> None:
    long_session(folder)
    report = folder / "bin/report.js"
    filtered = """const at = process.argv.indexOf('--min-total')
const least = at === -1 ? -Infinity : Number(process.argv[at + 1])
const rows = [...byCustomer.values()].filter((r) => r.total >= least)"""
    report.write_text(report.read_text().replace("const rows = [...byCustomer.values()]", filtered))
    git(folder, "commit", "-qam", "feat: report --min-total")


TASKS = {"git-history": git_history, "fix-tests": fix_tests, "log-analysis": log_analysis, "refactor": refactor,
         "git-messy": git_messy, "test-triage": test_triage, "csv-join": csv_join, "long-session": long_session,
         "multi-turn": multi_turn}


def multi_turn_ops(folder: Path) -> None:
    multi_turn(folder)
    truth = check.ops_truth(folder)
    answer = json.loads((folder / "answer.json").read_text())
    answer.update({
        "build_warnings": [{"file": f, "line": n} for f, n in check.setup.BUILD_WARNINGS],
        "health_status": "ok",
        "report_customers": truth["report_customers"],
        "top_5xx_path": truth["top_5xx_path"],
        "top_5xx_count": truth["top_5xx_count"],
    })
    (folder / "answer.json").write_text(json.dumps(answer))


TASKS["multi-turn-ops"] = multi_turn_ops

if __name__ == "__main__":
    TASKS[sys.argv[1]](Path(sys.argv[2]))
