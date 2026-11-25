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


AUTHORS = {
    "alice": ("Alice Example", "alice@example.com"),
    "bob": ("Bob Example", "bob@example.com"),
    "carol": ("Carol Example", "carol@example.com"),
    "dave": ("Dave Example", "dave@example.com"),
}


def commit_as(folder: Path, message: str, author: str, when: str, *extra: str) -> None:
    name, email = AUTHORS[author]
    env = {**os.environ, "GIT_AUTHOR_NAME": name, "GIT_AUTHOR_EMAIL": email, "GIT_COMMITTER_NAME": name,
           "GIT_COMMITTER_EMAIL": email, "GIT_AUTHOR_DATE": when, "GIT_COMMITTER_DATE": when}
    git(folder, "add", "-A")
    git(folder, "commit", "-q", *extra, "-m", message, env=env)


def git_messy(folder: Path) -> None:
    """Commit messages with bodies, pipes, tabs and quotes; a merged branch; two renames.

    Traps: "BREAKING" in a subject (only a body line counts), "BREAKING CHANGE:" in the
    middle of a body line, a merge commit whose body has one, a rename with an edit.
    """
    random.seed(5)
    git(folder, "init", "-q", "-b", "main")
    start = {"src/api/user.js": 8, "src/api/orders.js": 8, "src/core/a.js": 12, "src/core/b.js": 6, "docs/notes.md": 10}
    for path, count in start.items():
        (folder / path).parent.mkdir(parents=True, exist_ok=True)
        (folder / path).write_text("".join(f"// {path} line {i}\n" for i in range(count)))
    commit_as(folder, "Start", "alice", "2026-03-01T09:00:00+00:00")
    names = list(AUTHORS)
    clock = [0]

    def when() -> str:
        clock[0] += 1
        return f"2026-03-{2 + clock[0] // 3:02d}T{9 + clock[0] % 3 * 3:02d}:00:00+00:00"

    def touch(path: str, n: int) -> None:
        (folder / path).parent.mkdir(parents=True, exist_ok=True)
        with open(folder / path, "a") as f:
            f.write("".join(f"// edit {n}.{i}\n" for i in range(random.randint(1, 4))))

    def message(n: int) -> str:
        if n % 7 == 3:
            return f"Change the api ({n})\n\nBREAKING CHANGE: callers must pass an id\n\nMore | with pipes\tand tabs"
        if n % 5 == 0:
            return f'Fix "quoted" | piped\tthing {n}\n\nLine one\nThis mentions BREAKING CHANGE: in the middle'
        if n % 11 == 4:
            return f"BREAKING: tidy up ({n})"
        return f"Update things ({n})"

    paths = ["src/api/user.js", "src/api/orders.js", "src/core/a.js", "src/core/b.js"]
    for n in range(1, 10):
        touch(random.choice(paths), n)
        commit_as(folder, message(n), random.choice(names), when())
    git(folder, "checkout", "-q", "-b", "feature/export")
    for n in range(100, 106):
        if n == 101:
            # Lines deleted: core/a.js shrinks.
            lines = (folder / "src/core/a.js").read_text().splitlines(keepends=True)
            (folder / "src/core/a.js").write_text("".join(lines[:-5]))
        else:
            touch("src/api/export.js", n)
        body = "\n\nBREAKING CHANGE: export needs a format" if n == 103 else ""
        commit_as(folder, f"Export: step {n - 99}{body}", random.choice(names), when())
    git(folder, "checkout", "-q", "main")
    for n in range(10, 15):
        # Not src/core/a.js: the branch deleted its last lines, the merge must not conflict.
        touch(random.choice([p for p in paths if p != "src/core/a.js"]), n)
        commit_as(folder, message(n), random.choice(names), when())
    name, email = AUTHORS["bob"]
    stamp = when()
    env = {**os.environ, "GIT_AUTHOR_NAME": name, "GIT_AUTHOR_EMAIL": email, "GIT_COMMITTER_NAME": name,
           "GIT_COMMITTER_EMAIL": email, "GIT_AUTHOR_DATE": stamp, "GIT_COMMITTER_DATE": stamp}
    git(folder, "merge", "-q", "--no-ff", "feature/export", "-m", "Merge branch 'feature/export'\n\nBREAKING CHANGE: see the branch", env=env)
    for n in range(15, 31):
        if n == 18:
            git(folder, "mv", "src/api/user.js", "src/api/users.js")
            paths[0] = "src/api/users.js"
        elif n == 24:
            git(folder, "mv", "docs/notes.md", "docs/NOTES.md")
            with open(folder / "docs/NOTES.md", "a") as f:
                f.write("one more line\n")
        else:
            touch(random.choice(paths), n)
        commit_as(folder, message(n), random.choice(names), when())


def test_triage(folder: Path) -> None:
    """40 test files of 10 tests that log a lot; 3 fail, 2 skipped, 1 todo.

    Traps: log lines that say "not ok" and "fail".
    """
    failing = {7: ("parses dates in March", "March 31 should stay in March"),
               22: ("rounds half up", "2.5 should round to 3"),
               35: ("keeps order of equal keys", "equal keys must keep their order")}
    (folder / "test").mkdir()
    for module in range(40):
        lines = ["import { test } from 'node:test'", "import assert from 'node:assert/strict'", ""]
        for case in range(10):
            name = f"case {case} of mod{module:02d}"
            options = ""
            body = [f"  console.log('[mod{module:02d}] step {step}: ok')" for step in range(4)]
            body.append("  console.log('not ok yet, retrying once')" if case == 3 else "  console.log('fail-safe engaged')" if case == 6 else "  console.log('done')")
            check = f"  assert.equal({case} + 1, {case + 1})"
            if case == 5 and module in failing:
                name, text = failing[module]
                check = f"  assert.equal(1, 2, '{text}')"
            if case == 8 and module in (4, 30):
                name, options = f"slow path of mod{module:02d}", ", { skip: 'too slow on CI' }"
            if case == 9 and module == 12:
                name, options = "streams large files", ", { todo: true }"
            lines.append(f"test('{name}'{options}, () => {{")
            lines.extend(body)
            lines.append(check)
            lines.append("})")
        (folder / "test" / f"mod{module:02d}.test.js").write_text("\n".join(lines) + "\n")
    (folder / "package.json").write_text(json.dumps({"name": "test-triage", "type": "module", "scripts": {"test": "node --test"}}, indent=2) + "\n")


def csv_join(folder: Path) -> None:
    """orders.csv (quoted fields with commas, quotes and newlines), customers.json, rates.json."""
    import csv

    random.seed(9)
    countries = ["NL", "DE", "FR", "BE", "ES", "IT", "SE", "PL"]
    customers = []
    for n in range(900):
        year = random.choice([2024, 2025, 2025, 2026])
        customers.append({"id": f"c{n:04d}", "country": random.choice(countries), "signup": f"{year}-{random.randint(1, 12):02d}-{random.randint(1, 28):02d}"})
    (folder / "customers.json").write_text(json.dumps(customers, indent=1) + "\n")
    (folder / "rates.json").write_text(json.dumps({"EUR": 1.0, "USD": 0.9, "GBP": 1.15}) + "\n")
    notes = ["", "gift", "leave at door, please", 'said "fragile"', "line one\nline two", "a, b, and c", 'mix: "x", y\nz']
    with open(folder / "orders.csv", "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["id", "customer_id", "date", "amount", "currency", "note"])
        # Customers that never order: every 9th.
        buyers = [c["id"] for i, c in enumerate(customers) if i % 9 != 0]
        for n in range(20000):
            writer.writerow([f"o{n:05d}", random.choice(buyers), f"{random.choice([2025, 2026])}-{random.randint(1, 12):02d}-{random.randint(1, 28):02d}",
                             f"{random.uniform(5, 400):.2f}", random.choice(["EUR", "EUR", "USD", "GBP"]), random.choice(notes)])


MONEY_JS = """export function calcTotal(items) {
  const sum = items.reduce((total, item) => total + item.price * item.qty, 0)
  return Math.round(sum * 100) / 100
}

export function formatEur(amount) {
  return `€${amount.toFixed(2)}`
}
"""

DISCOUNTS_JS = """const CODES = { SAVE10: 0.1, HALF: 0.5 }
export const FREE_SHIPPING_FROM = 50

export function applyDiscount(total, code) {
  const off = CODES[code] ?? 0
  return Math.round(total * (1 - off) * 100) / 100
}

export function shipping(total) {
  return total >= FREE_SHIPPING_FROM ? 0 : 4.95
}
"""

CART_JS = """import { calcTotal } from './money.js'
import { applyDiscount, shipping } from './discounts.js'

export function checkout(items, code) {
  const subtotal = calcTotal(items)
  const discounted = applyDiscount(subtotal, code)
  const cost = shipping(discounted)
  return { subtotal, discounted, shipping: cost, total: Math.round((discounted + cost) * 100) / 100 }
}
"""

INDEX_JS = """export { calcTotal, formatEur } from './money.js'
export { applyDiscount, shipping, FREE_SHIPPING_FROM } from './discounts.js'
export { checkout } from './cart.js'
"""

REPORT_JS = """#!/usr/bin/env node
// Orders per customer: how many, and their total in EUR. Highest total first.
import { readFileSync } from 'node:fs'
import { calcTotal } from '../src/money.js'

const orders = JSON.parse(readFileSync(new URL('../data/orders.json', import.meta.url), 'utf8'))
const byCustomer = new Map()
for (const order of orders) {
  const row = byCustomer.get(order.customer) ?? { customer: order.customer, orders: 0, total: 0 }
  row.orders += 1
  row.total = ROUNDED
  byCustomer.set(order.customer, row)
}
const rows = [...byCustomer.values()].sort((a, b) => b.total - a.total || a.customer.localeCompare(b.customer))
console.log(JSON.stringify(rows, null, 2))
"""

LONG_TESTS = {
    "test/money.test.js": """import { test } from 'node:test'
import assert from 'node:assert/strict'
import { calcTotal, formatEur } from '../src/money.js'

test('total of nothing is 0', () => assert.equal(calcTotal([]), 0))
test('total multiplies price by quantity', () => assert.equal(calcTotal([{ price: 2.5, qty: 4 }, { price: 1, qty: 1 }]), 11))
test('total rounds to the nearest cent', () => assert.equal(calcTotal([{ price: 10.006, qty: 1 }]), 10.01))
test('formats euros', () => assert.equal(formatEur(3), '€3.00'))
""",
    "test/discounts.test.js": """import { test } from 'node:test'
import assert from 'node:assert/strict'
import { applyDiscount, shipping } from '../src/discounts.js'

test('SAVE10 takes 10% off', () => assert.equal(applyDiscount(80, 'SAVE10'), 72))
test('an unknown code changes nothing', () => assert.equal(applyDiscount(80, 'NOPE'), 80))
test('shipping is free from 50', () => assert.equal(shipping(50), 0))
test('shipping below 50 costs 4.95', () => assert.equal(shipping(49.99), 4.95))
""",
    "test/cart.test.js": """import { test } from 'node:test'
import assert from 'node:assert/strict'
import { checkout } from '../src/cart.js'

test('checkout with a code', () => assert.deepEqual(checkout([{ price: 30, qty: 2 }], 'SAVE10'), { subtotal: 60, discounted: 54, shipping: 0, total: 54 }))
test('checkout below free shipping', () => assert.deepEqual(checkout([{ price: 10, qty: 1 }]), { subtotal: 10, discounted: 10, shipping: 4.95, total: 14.95 }))
""",
}

LONG_CUSTOMERS = ["Ada Park", "Ben Okafor", "Smith, J.", 'Ann "Annie" Lee', "Chen Wei", "Dara Novak", "Eli Haddad",
                  "Femi Adeyemi", "Gus Lindqvist", "Hana Sato", "Ines Duarte", "Jon Berg", "Kofi Mensah", "Lea Roux"]
LONG_REPS = ["Mara", "Nico", "Omar", "Pia", "Quinn", "Rosa", "Sven", "Tara", "Umar", "Vera", "Wes", "Yara"]
LONG_NOTES = ["", "", "renewal", "upsell, Q-end", 'customer said "maybe"', "two lines\nof note", "paid, late"]


def long_session(folder: Path) -> None:
    """A Node project with a history, for a session of many calls: two bugs (one commit made
    the rounding one, and a decoy "fix: round report totals" did not), a rename that must
    leave the API schema alone, a CSV option for a report whose customers have commas and
    quotes in their names, a sales CSV whose large amounts are written "1,234.50", and a
    branch of conventional commits."""
    random.seed(11)
    git(folder, "init", "-q", "-b", "main")
    write = lambda path, text: ((folder / path).parent.mkdir(parents=True, exist_ok=True), (folder / path).write_text(text))
    changelog = ["# Changelog\n"]

    def note(line: str) -> None:
        changelog.append(f"- {line}\n")
        write("CHANGELOG.md", "".join(changelog))

    write("package.json", json.dumps({"name": "shop", "version": "0.1.0", "type": "module", "private": True, "scripts": {"test": "node --test"}}, indent=2) + "\n")
    write("README.md", "# shop\n\nMoney, discounts and checkout for the shop.\n\n    npm test\n")
    commit_as(folder, "chore: start the shop package", "alice", "2026-01-05T10:00:00+01:00")
    write("src/money.js", MONEY_JS)
    commit_as(folder, "feat: calcTotal and formatEur", "alice", "2026-01-07T10:00:00+01:00")
    write("test/money.test.js", LONG_TESTS["test/money.test.js"])
    commit_as(folder, "test: money", "alice", "2026-01-08T10:00:00+01:00")
    write("src/discounts.js", DISCOUNTS_JS)
    commit_as(folder, "feat: discount codes and shipping", "bob", "2026-01-12T10:00:00+01:00")
    write("test/discounts.test.js", LONG_TESTS["test/discounts.test.js"])
    commit_as(folder, "test: discounts", "bob", "2026-01-13T10:00:00+01:00")
    write("src/cart.js", CART_JS)
    write("src/index.js", INDEX_JS)
    commit_as(folder, "feat: checkout", "carol", "2026-01-20T10:00:00+01:00")
    write("test/cart.test.js", LONG_TESTS["test/cart.test.js"])
    commit_as(folder, "test: checkout", "carol", "2026-01-21T10:00:00+01:00")
    orders = []
    for n in range(80):
        items = [{"sku": f"sku-{random.randint(1, 40):02d}", "price": round(random.uniform(1, 60), random.choice([2, 2, 3])), "qty": random.randint(1, 4)}
                 for _ in range(random.randint(1, 4))]
        orders.append({"id": f"o{n:03d}", "customer": random.choice(LONG_CUSTOMERS), "items": items})
    write("data/orders.json", json.dumps(orders, indent=1) + "\n")
    write("bin/report.js", REPORT_JS.replace("ROUNDED", "row.total + calcTotal(order.items)"))
    commit_as(folder, "feat: per-customer report", "dave", "2026-02-02T10:00:00+01:00")
    write("README.md", "# shop\n\nMoney, discounts and checkout for the shop.\n\n    npm test\n\nOrders per customer, as JSON:\n\n    node bin/report.js\n")
    commit_as(folder, "docs: how to run the report", "dave", "2026-02-03T10:00:00+01:00")
    write("data/api-schema.json", json.dumps({"service": "shop", "operations": [
        {"operationId": "calcTotal", "method": "POST", "path": "/cart/total"},
        {"operationId": "applyDiscount", "method": "POST", "path": "/cart/discount"},
        {"operationId": "checkout", "method": "POST", "path": "/cart/checkout"}]}, indent=2) + "\n")
    commit_as(folder, "chore: public api schema", "alice", "2026-02-15T10:00:00+01:00")
    note("0.2.0: checkout and the report")
    commit_as(folder, "docs: changelog for 0.2.0", "alice", "2026-02-20T10:00:00+01:00")
    write("bin/report.js", REPORT_JS.replace("ROUNDED", "Math.round((row.total + calcTotal(order.items)) * 100) / 100"))
    commit_as(folder, "fix: round report totals to cents", "dave", "2026-03-03T10:00:00+01:00")
    note("report totals are whole cents")
    commit_as(folder, "docs: changelog", "dave", "2026-03-04T10:00:00+01:00")
    write("package.json", json.dumps({"name": "shop", "version": "0.2.0", "type": "module", "private": True, "scripts": {"test": "node --test"}}, indent=2) + "\n")
    commit_as(folder, "chore: 0.2.0", "alice", "2026-03-10T10:00:00+01:00")
    write("src/money.js", MONEY_JS.replace("Math.round(sum * 100)", "Math.floor(sum * 100)"))
    commit_as(folder, "refactor: simpler rounding in calcTotal", "bob", "2026-04-02T10:00:00+01:00")
    note("calcTotal is simpler")
    commit_as(folder, "docs: changelog", "bob", "2026-04-03T10:00:00+01:00")
    write("README.md", "# shop\n\nMoney, discounts and checkout for the shop.\n\n    npm test\n\nOrders per customer, as JSON:\n\n    node bin/report.js\n\nThe public API is described in data/api-schema.json; its operation names are fixed.\n")
    commit_as(folder, "docs: the api schema is public", "alice", "2026-05-11T10:00:00+01:00")
    write("src/discounts.js", DISCOUNTS_JS.replace("total >= FREE_SHIPPING_FROM", "total > FREE_SHIPPING_FROM").replace("const CODES = { SAVE10: 0.1, HALF: 0.5 }", "const CODES = {\n  SAVE10: 0.1,\n  HALF: 0.5,\n}"))
    commit_as(folder, "style: tidy discount thresholds", "carol", "2026-06-10T10:00:00+01:00")
    note("0.3.0: tidier code")
    commit_as(folder, "docs: changelog for 0.3.0", "carol", "2026-06-12T10:00:00+01:00")
    import csv

    with open(folder / "data/sales.csv", "w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["date", "region", "rep", "amount", "note"])
        for _ in range(3000):
            amount = random.uniform(20, 2600)
            writer.writerow([f"{random.choice([2025, 2026])}-{random.randint(1, 12):02d}-{random.randint(1, 28):02d}", random.choice(["North", "South", "East", "West"]),
                             random.choice(LONG_REPS), f"{amount:,.2f}", random.choice(LONG_NOTES)])
    commit_as(folder, "data: sales export", "dave", "2026-09-01T10:00:00+01:00")


TASKS = {"git-history": git_history, "fix-tests": fix_tests, "log-analysis": log_analysis, "refactor": refactor,
         "git-messy": git_messy, "test-triage": test_triage, "csv-join": csv_join, "long-session": long_session}

if __name__ == "__main__":
    task, folder = sys.argv[1], Path(sys.argv[2])
    folder.mkdir(parents=True, exist_ok=False)
    TASKS[task](folder)
