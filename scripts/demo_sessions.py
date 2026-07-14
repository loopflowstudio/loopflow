#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import shlex
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]


def _parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Demo the Wave → Project Session → Task Session product hierarchy."
    )
    parser.add_argument("wave", help="Wave name, for example infrastructure")
    parser.add_argument("--task", help="Existing Linear issue id to select or start")
    parser.add_argument(
        "--start-wave",
        action="store_true",
        help="start the provider-backed Wave in tmux; may spend provider tokens",
    )
    parser.add_argument(
        "--sync",
        action="store_true",
        help="refresh the Wave's Linear snapshot before reading it",
    )
    parser.add_argument(
        "--start-task",
        action="store_true",
        help="start --task in its worktree; creates local state and spends provider tokens",
    )
    parser.add_argument(
        "--app",
        action="store_true",
        help="build and open the Mac app after validating the hierarchy",
    )
    return parser.parse_args()


def _lf_binary() -> Path:
    configured = os.environ.get("LF_BIN")
    if configured:
        binary = Path(configured).expanduser().resolve()
        if not binary.is_file():
            raise RuntimeError(f"LF_BIN is not a file: {binary}")
        return binary

    binary = ROOT / "target" / "debug" / "lf"
    if not binary.is_file():
        subprocess.run(
            ["cargo", "build", "-q", "-p", "loopflow", "--bin", "lf"],
            cwd=ROOT,
            check=True,
        )
    return binary


def _lf(binary: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [str(binary), *args],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=check,
    )


def _status(binary: Path, wave: str) -> dict[str, Any]:
    result = _lf(binary, "status", wave, "--json", check=False)
    if result.returncode != 0:
        message = result.stderr.strip() or result.stdout.strip()
        raise RuntimeError(message or f"lf status exited {result.returncode}")
    return json.loads(result.stdout)


def _start_wave(binary: Path, wave: str) -> None:
    if shutil.which("tmux") is None:
        raise RuntimeError("tmux is required for --start-wave")
    session = f"lf-demo-{wave}"
    exists = subprocess.run(
        ["tmux", "has-session", "-t", session],
        capture_output=True,
        check=False,
    ).returncode == 0
    if not exists:
        subprocess.run(
            [
                "tmux",
                "new-session",
                "-d",
                "-s",
                session,
                "-c",
                str(ROOT),
                shlex.join([str(binary), "wave", wave]),
            ],
            check=True,
        )
    print(f"Wave process: tmux attach -t {session}")

    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        try:
            _status(binary, wave)
            return
        except RuntimeError:
            time.sleep(1)
    raise RuntimeError(f"wave/{wave} did not register within 90 seconds")


def _print_hierarchy(snapshot: dict[str, Any], selected_task: str | None) -> str | None:
    wave = snapshot["wave"]
    print(f"\nWave  {wave['name']}  [{wave['status']}]  {wave['repo']}")
    first_task: str | None = None
    matched_task = False

    for project in snapshot["projects"]:
        planning = project["project"]
        runtime = project["runtime"]
        project_session_id = runtime["session_id"] if runtime else None
        status = runtime["status"] if runtime else "unstarted"
        print(f"  Project  {planning['name']}  [{status}]  {project_session_id or '-'}")

        for task in project["tasks"]:
            task_plan = task["task"]
            task_runtime = task["runtime"]
            identifier = task_plan["identifier"]
            first_task = first_task or identifier
            matched_task = matched_task or selected_task in {
                identifier,
                task_plan["id"],
            }
            if task_runtime is None:
                print(f"    Task  {identifier}  [unstarted]")
                continue
            if project_session_id is None:
                raise RuntimeError(f"{identifier} has a Task Session without a Project Session")
            if task_runtime["project_session_id"] != project_session_id:
                raise RuntimeError(
                    f"{identifier} points to {task_runtime['project_session_id']}, "
                    f"but its Project row is {project_session_id}"
                )
            print(
                f"    Task  {identifier}  [{task_runtime['status']}]  "
                f"{task_runtime['session_id']} → {project_session_id}"
            )
            print(f"      workspace  {task_runtime['worktree']}")

    if selected_task and not matched_task:
        raise RuntimeError(f"Task {selected_task} is absent from wave/{wave['name']}")
    return selected_task or first_task


def _print_walkthrough(binary: Path, wave: str, task: str | None) -> None:
    print("\nLook for:")
    print("  1. Wave Chat remains available while Project and Task Sessions run.")
    print("  2. Every running Task names the Project Session directly above it.")
    print("  3. Child activity appears in the thread; current truth stays in the work map.")
    if task:
        print("  4. Select the Task to inspect changed files, patches, files, and terminals.")
        print("\nUseful terminal reads:")
        print(f"  {binary} task status {task} --json")
        print(f"  {binary} task changes {task}")
        print(f"  {binary} task diff {task}")
    print(f"\nRefresh this view: {binary} status {wave} --json")


def main() -> int:
    args = _parse_args()
    if args.start_task and not args.task:
        raise RuntimeError("--start-task requires --task <existing-linear-issue>")

    binary = _lf_binary()
    if args.start_wave:
        _start_wave(binary, args.wave)
    if args.sync:
        result = _lf(binary, "pm", "sync", "--wave", args.wave, check=False)
        if result.returncode != 0:
            raise RuntimeError(result.stderr.strip() or result.stdout.strip())
    if args.start_task:
        result = _lf(binary, "task", "run", args.task, "--json", check=False)
        if result.returncode != 0:
            raise RuntimeError(result.stderr.strip() or result.stdout.strip())

    try:
        snapshot = _status(binary, args.wave)
    except RuntimeError as error:
        if not args.start_wave:
            raise RuntimeError(
                f"{error}\nStart the demo Wave explicitly (this may spend provider tokens):\n"
                f"  uv run python scripts/demo_sessions.py {args.wave} --start-wave"
            ) from error
        raise

    task = _print_hierarchy(snapshot, args.task)
    _print_walkthrough(binary, args.wave, task)

    if args.app:
        subprocess.run(
            [sys.executable, str(ROOT / "scripts" / "loopflow-dev.py"), "run"],
            cwd=ROOT,
            check=True,
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (RuntimeError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"demo: {error}", file=sys.stderr)
        raise SystemExit(1) from error
