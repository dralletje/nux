"""Run agents on the tasks, with nux or with plain bash, and record what it took.

    python3 runner.py --tasks git-history fix-tests --tools nux bash --models sonnet opus \
        --reps 1 --parallel 4 --nux-bin <folder with a `nux` binary> --work <scratch folder>

Each run is a headless `claude -p` in a fresh copy of the task's fixture (setup.py). Both
tools start from the same place: no user or project settings and no CLAUDE.md
(`--setting-sources ""`, a working folder outside any project), the tools Bash, Read,
Write and Edit. A nux run adds what agents in this workspace get: the nux-only hook
(~/.claude/hooks/nux-only.py) and the instructions in nux-instructions.md, with the nux
under test first on PATH. Results go to results/<task>.<tool>.<model>.<rep>.json.
"""

import argparse
import json
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).parent
HOOK = Path.home() / ".claude/hooks/nux-only.py"


def transcript_stats(session_id: str) -> dict:
    """What the agent's Bash calls were like, from the session's transcript."""
    found = list((Path.home() / ".claude/projects").glob(f"*/{session_id}.jsonl"))
    if not found:
        return {"transcript": None}
    calls, results = {}, {}
    for line in found[0].read_text(errors="replace").splitlines():
        try:
            entry = json.loads(line)
        except Exception:
            continue
        content = (entry.get("message") or {}).get("content")
        if not isinstance(content, list):
            continue
        for part in content:
            if not isinstance(part, dict):
                continue
            if part.get("type") == "tool_use":
                calls[part["id"]] = {"tool": part.get("name"), "command": (part.get("input") or {}).get("command", "")}
            elif part.get("type") == "tool_result":
                body = part.get("content")
                text = "".join(b.get("text", "") for b in body if isinstance(b, dict)) if isinstance(body, list) else (body or "")
                results[part.get("tool_use_id")] = {"error": bool(part.get("is_error")), "chars": len(text), "head": text[:200]}
    bash = [(calls[i], results.get(i, {})) for i in calls if calls[i]["tool"] == "Bash"]
    tools = {}
    for call in calls.values():
        tools[call["tool"]] = tools.get(call["tool"], 0) + 1
    return {
        "transcript": str(found[0]),
        "tool_calls": tools,
        "bash_calls": len(bash),
        "bash_errors": sum(1 for _, r in bash if r.get("error")),
        "bash_blocked": sum(1 for _, r in bash if "Bash runs nux only" in r.get("head", "")),
        "bash_output_chars": sum(r.get("chars", 0) for _, r in bash),
        "nux_help_calls": sum(1 for c, _ in bash if c["command"].strip().startswith("nux help") or "nux help" in c["command"][:40]),
    }


def run_one(task: str, tool: str, model: str, rep: int, work: Path, nux_bin: Path, results: Path) -> dict:
    name = f"{task}.{tool}.{model}.{rep}"
    folder = work / name
    subprocess.run([sys.executable, str(HERE / "setup.py"), task, str(folder)], check=True)
    prompt = (HERE / "tasks" / f"{task}.md").read_text()
    command = [
        "claude", "-p", prompt,
        "--model", model,
        "--setting-sources", "",
        "--allowedTools", "Bash", "Read", "Write", "Edit",
        "--output-format", "json",
    ]
    env = dict(os.environ)
    if tool == "nux":
        hooks = {"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": f"python3 {HOOK}"}]}]}}
        command += ["--settings", json.dumps(hooks), "--append-system-prompt", (HERE / "nux-instructions.md").read_text()]
        env["PATH"] = f"{nux_bin}:{env['PATH']}"
    started = time.time()
    try:
        run = subprocess.run(command, cwd=folder, env=env, capture_output=True, text=True, timeout=1800)
        output, exit_code = run.stdout, run.returncode
    except subprocess.TimeoutExpired as timeout:
        output, exit_code = (timeout.stdout or b"").decode() if isinstance(timeout.stdout, bytes) else (timeout.stdout or ""), "timeout"
    wall = time.time() - started
    try:
        reply = json.loads(output)
    except Exception:
        reply = {"unparsed": output[-2000:]}
    check = subprocess.run([sys.executable, str(HERE / "check.py"), task, str(folder)], capture_output=True, text=True)
    try:
        verdict = json.loads(check.stdout)
    except Exception:
        verdict = {"passed": False, "checks": {}, "error": check.stderr[-1000:]}
    record = {
        "name": name, "task": task, "tool": tool, "model": model, "rep": rep,
        "passed": verdict["passed"], "checks": verdict.get("checks"),
        "exit_code": exit_code, "wall_seconds": round(wall, 1),
        "duration_ms": reply.get("duration_ms"), "num_turns": reply.get("num_turns"),
        "cost_usd": reply.get("total_cost_usd"), "usage": reply.get("usage"),
        "models_used": list((reply.get("modelUsage") or {}).keys()),
        "is_error": reply.get("is_error"), "result": (reply.get("result") or "")[:1500],
        "session_id": reply.get("session_id"),
        **transcript_stats(reply.get("session_id") or "none"),
    }
    results.mkdir(parents=True, exist_ok=True)
    (results / f"{name}.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"{name}: passed={record['passed']} turns={record['num_turns']} cost={record['cost_usd']} wall={record['wall_seconds']}s", flush=True)
    return record


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--tasks", nargs="+", required=True)
    parser.add_argument("--tools", nargs="+", default=["nux", "bash"])
    parser.add_argument("--models", nargs="+", default=["sonnet", "opus"])
    parser.add_argument("--reps", nargs="+", type=int, default=[1])
    parser.add_argument("--parallel", type=int, default=4)
    parser.add_argument("--nux-bin", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--results", type=Path, default=HERE / "results")
    arguments = parser.parse_args()
    jobs = [(t, tool, m, r) for r in arguments.reps for t in arguments.tasks for m in arguments.models for tool in arguments.tools]
    with ThreadPoolExecutor(arguments.parallel) as pool:
        list(pool.map(lambda job: run_one(*job, arguments.work, arguments.nux_bin, arguments.results), jobs))
