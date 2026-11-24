"""Make the folder one run works in: python3 -I setup.py <task> <folder>.

Every fixture is deterministic (fixed seeds, fixed dates), so every run of a task starts
from the same files. The task's prompt is in tasks/<task>.md; check.py says whether a run
did it.
"""

import json
import os
import random
import subprocess
import sys
from pathlib import Path


def git(folder: Path, *args: str, env: dict | None = None) -> None:
    subprocess.run(["git", *args], cwd=folder, check=True, capture_output=True, env=env)


def commit(folder: Path, message: str, author: str, when: str) -> None:
    name, email = {
        "alice": ("Alice Example", "alice@example.com"),
        "bob": ("Bob Example", "bob@example.com"),
        "carol": ("Carol Example", "carol@example.com"),
    }[author]
    env = {
        **os.environ,
        "GIT_AUTHOR_NAME": name,
        "GIT_AUTHOR_EMAIL": email,
        "GIT_COMMITTER_NAME": name,
        "GIT_COMMITTER_EMAIL": email,
        "GIT_AUTHOR_DATE": when,
        "GIT_COMMITTER_DATE": when,
    }
    git(folder, "add", "-A")
    git(folder, "commit", "-q", "-m", message, env=env)


def git_history(folder: Path) -> None:
    """A repository of 40 commits by three authors; one function's origin, one deleted file."""
    random.seed(1)
    git(folder, "init", "-q", "-b", "main")
    files = ["src/server.js", "src/routes.js", "src/db.js", "src/util.js", "README.md"]
    for path in files + ["src/legacy.js"]:
        (folder / path).parent.mkdir(parents=True, exist_ok=True)
        (folder / path).write_text(f"// {path}\n")
    commit(folder, "Start the project", "alice", "2026-01-01T09:00:00+00:00")
    authors = ["alice", "bob", "carol"]
    for n in range(1, 40):
        when = f"2026-01-{1 + n // 2:02d}T{9 + n % 8:02d}:00:00+00:00"
        author = authors[random.randrange(3)]
        # Weighted, so one file is clearly the most changed.
        path = random.choice(files + ["src/routes.js"] * 3)
        lines = random.randint(1, 6)
        with open(folder / path, "a") as f:
            for i in range(lines):
                f.write(f"// change {n}.{i} by {author}\n")
        message = f"Change {path.split('/')[-1]} ({n})"
        if n == 13:
            (folder / "src/config.js").write_text("export function parseConfig(text) {\n  return JSON.parse(text)\n}\n")
            message = "Read the config file"
        if n == 21:
            (folder / "src/legacy.js").unlink()
            message = "Clean up old code"
        if n == 27:
            # A later commit that mentions parseConfig too: not the one that added it.
            with open(folder / "src/config.js", "a") as f:
                f.write("// parseConfig: also YAML later\n")
            message = "Note on parseConfig"
        commit(folder, message, author, when)


TESTS = {
    "test/math.test.js": """import { test } from 'node:test'
import assert from 'node:assert/strict'
import { sum, average, clamp, roundTo } from '../src/math.js'

test('sum adds numbers', () => assert.equal(sum([1, 2, 3]), 6))
test('sum of nothing is 0', () => assert.equal(sum([]), 0))
test('average of numbers', () => assert.equal(average([2, 4, 6]), 4))
test('average of nothing is 0', () => assert.equal(average([]), 0))
test('clamp keeps a value inside', () => assert.equal(clamp(5, 0, 10), 5))
test('clamp raises a low value', () => assert.equal(clamp(-3, 0, 10), 0))
test('clamp lowers a high value', () => assert.equal(clamp(42, 0, 10), 10))
test('roundTo two decimals', () => assert.equal(roundTo(1.005, 2), 1.01))
""",
    "test/strings.test.js": """import { test } from 'node:test'
import assert from 'node:assert/strict'
import { slugify, capitalize, truncate } from '../src/strings.js'

test('slugify lowercases and dashes', () => assert.equal(slugify('Hello World'), 'hello-world'))
test('slugify drops punctuation', () => assert.equal(slugify('Hi, there!'), 'hi-there'))
test('slugify collapses spaces', () => assert.equal(slugify('a   b'), 'a-b'))
test('capitalize the first letter', () => assert.equal(capitalize('nux'), 'Nux'))
test('capitalize empty text', () => assert.equal(capitalize(''), ''))
test('truncate long text', () => assert.equal(truncate('abcdefghij', 5), 'abcd…'))
test('truncate short text stays', () => assert.equal(truncate('abc', 5), 'abc'))
""",
    "test/dates.test.js": """import { test } from 'node:test'
import assert from 'node:assert/strict'
import { addDays, isWeekend, daysBetween } from '../src/dates.js'

test('addDays moves forward', () => assert.equal(addDays('2026-01-30', 3), '2026-02-02'))
test('addDays moves back', () => assert.equal(addDays('2026-03-01', -1), '2026-02-28'))
test('a Saturday is weekend', () => assert.equal(isWeekend('2026-01-03'), true))
test('a Monday is not weekend', () => assert.equal(isWeekend('2026-01-05'), false))
test('daysBetween two dates', () => assert.equal(daysBetween('2026-01-01', '2026-01-31'), 30))
test('daysBetween is never negative', () => assert.equal(daysBetween('2026-01-31', '2026-01-01'), 30))
""",
}

SOURCES = {
    "src/math.js": """export function sum(numbers) {
  return numbers.reduce((total, n) => total + n, 0)
}

export function average(numbers) {
  // Bug: NaN for an empty list.
  return sum(numbers) / numbers.length
}

export function clamp(value, low, high) {
  // Bug: the bounds are swapped.
  return Math.min(low, Math.max(high, value))
}

export function roundTo(value, places) {
  const factor = 10 ** places
  return Math.round((value + Number.EPSILON) * factor) / factor
}
""",
    "src/strings.js": """export function slugify(text) {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9\\s-]/g, '')
    .trim()
    .replace(/\\s+/g, '-')
}

export function capitalize(text) {
  return text.charAt(0).toUpperCase() + text.slice(1)
}

export function truncate(text, length) {
  // Bug: one character too many before the ellipsis.
  return text.length > length ? text.slice(0, length) + '…' : text
}
""",
    "src/dates.js": """const DAY = 24 * 60 * 60 * 1000

export function addDays(date, days) {
  const time = new Date(date + 'T00:00:00Z').getTime() + days * DAY
  return new Date(time).toISOString().slice(0, 10)
}

export function isWeekend(date) {
  const day = new Date(date + 'T00:00:00Z').getUTCDay()
  return day === 0 || day === 6
}

export function daysBetween(a, b) {
  return Math.abs(new Date(b) - new Date(a)) / DAY
}
""",
}


def fix_tests(folder: Path) -> None:
    """A small Node project: 21 tests, of which 4 fail because of 3 bugs in the source."""
    for path, text in {**SOURCES, **TESTS}.items():
        (folder / path).parent.mkdir(parents=True, exist_ok=True)
        (folder / path).write_text(text)
    (folder / "package.json").write_text(json.dumps({"name": "fix-tests", "type": "module", "scripts": {"test": "node --test"}}, indent=2) + "\n")


def log_analysis(folder: Path) -> None:
    """Three days of access logs as JSON lines, 21000 requests."""
    random.seed(3)
    paths = ["/api/orders", "/api/users", "/api/search", "/api/cart", "/health", "/api/payments", "/login"]
    weights = [30, 20, 15, 12, 10, 8, 5]
    users = [f"u{n:03d}" for n in range(180)]
    (folder / "logs").mkdir()
    for day in range(3):
        with open(folder / "logs" / f"access-2026-02-{10 + day}.jsonl", "w") as f:
            for n in range(7000):
                path = random.choices(paths, weights)[0]
                fail = {"/api/payments": 0.08, "/api/search": 0.05, "/api/cart": 0.03}.get(path, 0.01)
                roll = random.random()
                status = random.choice([500, 502, 503]) if roll < fail else (random.choice([400, 401, 404]) if roll < fail + 0.04 else 200)
                ms = int(random.lognormvariate(4 if path != "/api/search" else 5, 0.6))
                hour = min(23, int(random.triangular(0, 24, 14)))
                second = random.randrange(3600)
                entry = {
                    "ts": f"2026-02-{10 + day}T{hour:02d}:{second // 60:02d}:{second % 60:02d}Z",
                    "method": "POST" if path in ("/api/cart", "/api/payments", "/login") else "GET",
                    "path": path,
                    "status": status,
                    "ms": ms,
                    "user": random.choice(users),
                }
                f.write(json.dumps(entry) + "\n")


def refactor(folder: Path) -> None:
    """An ESM project where getUser is used in many shapes; one data file must keep the name."""
    files = {
        "src/users.js": "const USERS = { 1: { id: 1, name: 'Ada' }, 2: { id: 2, name: 'Grace' } }\n\nexport function getUser(id) {\n  return USERS[id] ?? null\n}\n\nexport function userName(id) {\n  return getUser(id)?.name ?? 'unknown'\n}\n",
        "src/orders.js": "import { getUser } from './users.js'\n\nexport function orderOwner(order) {\n  return getUser(order.userId)\n}\n",
        "src/greet.js": "import { getUser as loadUser } from './users.js'\n\nexport function greet(id) {\n  const user = loadUser(id)\n  return user ? `Hello, ${user.name}` : 'Hello, stranger'\n}\n",
        "src/admin.js": "import * as users from './users.js'\n\nexport function adminName(id) {\n  return users.getUser(id)?.name.toUpperCase() ?? 'NOBODY'\n}\n",
        "src/report.js": "import { getUser, userName } from './users.js'\n\nexport function report(ids) {\n  // getUser gives null for an unknown id.\n  return ids.map((id) => (getUser(id) ? userName(id) : '-')).join(', ')\n}\n",
        "src/index.js": "export { getUser, userName } from './users.js'\nexport { orderOwner } from './orders.js'\nexport { greet } from './greet.js'\n",
        "src/api.js": "import { getUser } from './users.js'\n\nconst handlers = {\n  getUser: (params) => getUser(Number(params.id)),\n}\n\nexport function handle(name, params) {\n  return handlers[name]?.(params)\n}\n",
        "data/routes.json": json.dumps({"routes": [{"rpc": "getUser", "public": True}, {"rpc": "listOrders", "public": False}]}, indent=2) + "\n",
        "test/users.test.js": "import { test } from 'node:test'\nimport assert from 'node:assert/strict'\nimport { getUser, userName } from '../src/users.js'\nimport { orderOwner } from '../src/orders.js'\nimport { greet } from '../src/greet.js'\nimport { adminName } from '../src/admin.js'\nimport { report } from '../src/report.js'\nimport { handle } from '../src/api.js'\nimport { readFileSync } from 'node:fs'\n\ntest('a known user', () => assert.equal(getUser(1).name, 'Ada'))\ntest('names', () => assert.equal(userName(3), 'unknown'))\ntest('owner', () => assert.equal(orderOwner({ userId: 2 }).name, 'Grace'))\ntest('greet', () => assert.equal(greet(1), 'Hello, Ada'))\ntest('admin', () => assert.equal(adminName(2), 'GRACE'))\ntest('report', () => assert.equal(report([1, 9]), 'Ada, -'))\ntest('the rpc name in routes.json is served', () => {\n  const routes = JSON.parse(readFileSync(new URL('../data/routes.json', import.meta.url))).routes\n  assert.equal(handle(routes[0].rpc, { id: '1' }).name, 'Ada')\n})\n",
        "package.json": json.dumps({"name": "refactor", "type": "module", "scripts": {"test": "node --test"}}, indent=2) + "\n",
    }
    for path, text in files.items():
        (folder / path).parent.mkdir(parents=True, exist_ok=True)
        (folder / path).write_text(text)


TASKS = {"git-history": git_history, "fix-tests": fix_tests, "log-analysis": log_analysis, "refactor": refactor}

if __name__ == "__main__":
    task, folder = sys.argv[1], Path(sys.argv[2])
    folder.mkdir(parents=True, exist_ok=False)
    TASKS[task](folder)
