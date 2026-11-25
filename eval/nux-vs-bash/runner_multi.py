"""A session of many requests: the steps of tasks/<task>.json, one `claude -p` each, every
one after the first with `--resume <session>`, as a person asking for one thing after
another. The same start as runner.py (no settings, no CLAUDE.md; for nux the hook, the
instructions and the nux under test on PATH), given again with every step.

    python3 runner_multi.py --task multi-turn --tools bash nux-nofiles --models sonnet opus \
        --reps 1 2 --parallel 8 --nux-bin <folder with nux> --work <scratch> --results results/run5

The fixture is setup.py's for the task (multi-turn: long-session's); check.py checks the
end state. Results: results/<task>.<tool>.<model>.<rep>.json, with every step's cost,
turns and answer.
"""

import argparse
import json
import os
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from runner import HERE, HOOK, transcript_stats

FIXTURE = {"multi-turn": "long-session"}


def run_one(task: str, tool: str, model: str, rep: int, work: Path, nux_bin: Path, results: Path) -> dict:
    name = f"{task}.{tool}.{model}.{rep}"
    folder = work / name
    subprocess.run([sys.executable, str(HERE / "setup.py"), FIXTURE.get(task, task), str(folder)], check=True)
    steps = json.loads((HERE / "tasks" / f"{task}.json").read_text())
    base = ["claude", "--model", model, "--setting-sources", "", "--allowedTools", "Bash", "Read", "Write", "Edit", "--output-format", "json"]
    env = dict(os.environ)
    instructions = {"nux": "nux-instructions.md", "nux-nofiles": "nux-instructions-nofiles.md"}.get(tool)
    if instructions:
        hooks = {"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": f"python3 {HOOK}"}]}]}}
        base += ["--settings", json.dumps(hooks), "--append-system-prompt", (HERE / instructions).read_text()]
        env["PATH"] = f"{nux_bin}:{env['PATH']}"
    session = None
    done = []
    started = time.time()
    for number, prompt in enumerate(steps, 1):
        command = base + ["-p", prompt] + (["--resume", session] if session else [])
        step_started = time.time()
        try:
            run = subprocess.run(command, cwd=folder, env=env, capture_output=True, text=True, timeout=1800)
            reply = json.loads(run.stdout)
        except Exception as error:
            done.append({"step": number, "error": str(error)[-1000:]})
            break
        session = reply.get("session_id") or session
        done.append({
            "step": number, "prompt": prompt, "session_id": reply.get("session_id"),
            "num_turns": reply.get("num_turns"), "cost_usd": reply.get("total_cost_usd"),
            "wall_seconds": round(time.time() - step_started, 1), "is_error": reply.get("is_error"),
            "usage": reply.get("usage"), "result": (reply.get("result") or "")[:2000],
        })
    check = subprocess.run([sys.executable, str(HERE / "check.py"), task, str(folder)], capture_output=True, text=True)
    try:
        verdict = json.loads(check.stdout)
    except Exception:
        verdict = {"passed": False, "checks": {}, "error": check.stderr[-1000:]}
    record = {
        "name": name, "task": task, "tool": tool, "model": model, "rep": rep,
        "passed": verdict["passed"], "checks": verdict.get("checks"),
        "steps_done": len([s for s in done if "error" not in s]), "steps": done,
        "num_turns": sum(s.get("num_turns") or 0 for s in done),
        "cost_usd": round(sum(s.get("cost_usd") or 0 for s in done), 4),
        "wall_seconds": round(time.time() - started, 1),
        "session_id": session,
        **transcript_stats(session or "none"),
    }
    results.mkdir(parents=True, exist_ok=True)
    (results / f"{name}.json").write_text(json.dumps(record, indent=2) + "\n")
    print(f"{name}: passed={record['passed']} steps={record['steps_done']} turns={record['num_turns']} cost={record['cost_usd']} wall={record['wall_seconds']}s", flush=True)
    return record


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--task", required=True)
    parser.add_argument("--tools", nargs="+", default=["nux-nofiles", "bash"])
    parser.add_argument("--models", nargs="+", default=["sonnet", "opus"])
    parser.add_argument("--reps", nargs="+", type=int, default=[1])
    parser.add_argument("--parallel", type=int, default=4)
    parser.add_argument("--nux-bin", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--results", type=Path, default=HERE / "results")
    arguments = parser.parse_args()
    jobs = [(arguments.task, tool, m, r) for r in arguments.reps for m in arguments.models for tool in arguments.tools]
    with ThreadPoolExecutor(arguments.parallel) as pool:
        list(pool.map(lambda job: run_one(*job, arguments.work, arguments.nux_bin, arguments.results), jobs))
