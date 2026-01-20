"""Loop iteration runner for lfd.

Runs iterations of a loop until the PR limit is reached or an error occurs.
Can be invoked directly as a subprocess for background execution.

When running in background, coordinates with the daemon scheduler to respect
global concurrency and PR limits.
"""

import json
import socket
import subprocess
import sys
import time
import uuid
from dataclasses import replace
from datetime import datetime
from pathlib import Path

from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import format_prompt, gather_prompt_components
from loopflow.lf.goals import load_goal
from loopflow.lf.launcher import build_model_command, get_runner
from loopflow.lf.logging import write_prompt_file
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.worktrees import WorktreeError
from loopflow.lf.worktrees import create as create_worktree
from loopflow.lf.worktrees import remove as remove_worktree
from loopflow.lfd.client import notify_event
from loopflow.lfd.db import (
    get_loop,
    save_loop_run,
    update_loop_iteration,
    update_loop_pid,
    update_loop_run_pr,
    update_loop_run_status,
    update_loop_run_step,
    update_loop_status,
)
from loopflow.lfd.loops import count_outstanding
from loopflow.lfd.models import Loop, LoopRun, LoopStatus, LoopType

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"
SCHEDULER_POLL_INTERVAL = 30  # seconds between slot checks


def _iteration_branch_prefix(loop_main: str) -> str:
    """Derive iteration branch prefix from loop-main.

    'product-engineer-main' → 'product-engineer'
    'product-engineer-1-main' → 'product-engineer-1'
    """
    if loop_main.endswith("-main"):
        return loop_main[:-5]
    return loop_main


def _scheduler_call(method: str, params: dict | None = None) -> dict | None:
    """Make a synchronous call to the daemon scheduler.

    Returns the result dict on success, None on connection failure.
    """
    if not SOCKET_PATH.exists():
        return None

    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(5.0)
        sock.connect(str(SOCKET_PATH))

        request = {"method": method}
        if params:
            request["params"] = params

        sock.sendall((json.dumps(request) + "\n").encode())

        response_data = b""
        while b"\n" not in response_data:
            chunk = sock.recv(1024)
            if not chunk:
                break
            response_data += chunk

        sock.close()

        if response_data:
            response = json.loads(response_data.decode().strip())
            if response.get("ok"):
                return response.get("result", {})
        return None
    except Exception:
        return None


def _scheduler_acquire(run_id: str) -> tuple[bool, str | None]:
    """Try to acquire a scheduler slot.

    Returns (acquired, reason) when the daemon is available.
    """
    result = _scheduler_call("scheduler.acquire", {"run_id": run_id})
    if result is None:
        # Daemon not running, allow iteration (standalone mode)
        return True, None
    return result.get("acquired", False), result.get("reason")


def _scheduler_release(run_id: str) -> None:
    """Release a scheduler slot."""
    _scheduler_call("scheduler.release", {"run_id": run_id})


def run_loop_iterations(loop: Loop) -> None:
    """Run loop iterations until PR limit is reached or error occurs.

    For FLOW loops, runs exactly one iteration then stops.
    For LOOP loops, runs continuously until pr_limit outstanding.
    """
    while True:
        # Check if we should pause (per-loop limit) - skip for FLOW which runs once
        if loop.type != LoopType.FLOW:
            outstanding = count_outstanding(loop)
            if outstanding >= loop.pr_limit:
                update_loop_status(loop.id, LoopStatus.WAITING)
                notify_event(
                    "loop.waiting",
                    {
                        "loop_id": loop.id,
                        "goal": loop.goal_name,
                        "outstanding": outstanding,
                        "limit": loop.pr_limit,
                    },
                )
                break

        # Run one iteration
        iteration = loop.iteration + 1
        run_id = str(uuid.uuid4())

        # Wait for scheduler slot (global concurrency)
        while True:
            acquired, reason = _scheduler_acquire(run_id)
            if acquired:
                break
            notify_event(
                "scheduler.waiting",
                {
                    "loop_id": loop.id,
                    "goal": loop.goal_name,
                    "reason": reason or "concurrency",
                },
            )
            time.sleep(SCHEDULER_POLL_INTERVAL)

        try:
            success = run_iteration(loop, iteration, run_id)
            if success:
                update_loop_iteration(loop.id, iteration)
                loop.iteration = iteration
            else:
                update_loop_status(loop.id, LoopStatus.ERROR)
                break
        except Exception as e:
            notify_event(
                "loop.error",
                {
                    "loop_id": loop.id,
                    "goal": loop.goal_name,
                    "error": str(e),
                },
            )
            update_loop_status(loop.id, LoopStatus.ERROR)
            break
        finally:
            _scheduler_release(run_id)

        # FLOW loops run exactly once then stop
        if loop.type == LoopType.FLOW:
            update_loop_status(loop.id, LoopStatus.IDLE)
            break

    # Clear pid when done
    update_loop_pid(loop.id, None)


def run_iteration(loop: Loop, iteration: int, run_id: str | None = None) -> bool:
    """Run a single iteration of the loop.

    Args:
        loop: The loop to run
        iteration: Iteration number
        run_id: Optional pre-allocated run ID (for scheduler coordination)

    Returns True if successful, False on error.
    """
    config = load_config(loop.repo)

    # Create iteration branch from loop-main
    prefix = _iteration_branch_prefix(loop.loop_main)
    branch = f"{prefix}/{iteration:03d}"
    try:
        worktree_path = create_worktree(loop.repo, branch, base=loop.loop_main)
    except WorktreeError as e:
        notify_event("loop.error", {"loop_id": loop.id, "error": f"Failed to create worktree: {e}"})
        return False

    # Create loop_run record
    run = LoopRun(
        id=run_id or str(uuid.uuid4()),
        loop_id=loop.id,
        iteration=iteration,
        status=LoopStatus.RUNNING,
        started_at=datetime.now(),
        worktree=str(worktree_path),
    )
    save_loop_run(run)

    notify_event(
        "loop.started",
        {
            "loop_id": loop.id,
            "goal": loop.goal_name,
            "iteration": iteration,
        },
    )

    # Load goal content
    goal_spec = load_goal(loop.repo, loop.goal_name)
    if not goal_spec:
        update_loop_run_status(run.id, LoopStatus.ERROR, error="Goal file not found")
        return False

    # Parse flow steps from goal or default
    flow = goal_spec.pipeline or "design,implement,polish"
    tasks = [t.strip() for t in flow.split(",")]
    if flow.startswith("@") and config and config.flows:
        config_flow = config.flows.get(flow[1:])
        if config_flow:
            tasks = config_flow.steps

    # Get model configuration
    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    runner = get_runner(backend)
    if not runner.is_available():
        update_loop_run_status(run.id, LoopStatus.ERROR, error=f"'{backend}' CLI not found")
        return False

    skip_permissions = config.yolo if config else False

    # Run each step in the flow
    for task_name in tasks:
        update_loop_run_step(run.id, task_name)
        notify_event(
            "loop.step.started",
            {
                "loop_id": loop.id,
                "step": task_name,
                "iteration": iteration,
            },
        )

        # Gather prompt components
        context_paths = goal_spec.area if goal_spec.area else None
        if loop.area:
            context_paths = [a.strip() for a in loop.area.split(",")]

        components = gather_prompt_components(
            worktree_path,
            step=task_name,
            context=context_paths,
            run_mode="auto",
        )

        # Verify step file exists
        if not components.step:
            update_loop_run_status(
                run.id, LoopStatus.ERROR, error=f"Step file not found: {task_name}"
            )
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

        # Inject goal content
        step_file, step_content = components.step
        goal_name = loop.goal_name
        goal_section = f"<lf:goal:{goal_name}>\n{goal_spec.content}\n</lf:goal:{goal_name}>"

        # For FLOW loops, inject project file content if present
        if loop.type == LoopType.FLOW and loop.project_file:
            try:
                prompt_content = Path(loop.project_file).read_text()
                goal_section += f"\n\n<lf:prompt>\n{prompt_content}\n</lf:prompt>"
            except OSError:
                pass  # Project file may have been removed

        combined = f"{goal_section}\n\n---\n\n{step_content}"
        components = replace(components, step=(step_file, combined))

        prompt = format_prompt(components)
        prompt_file = write_prompt_file(prompt)

        # Build and run command
        command = build_model_command(
            backend,
            auto=True,
            stream=True,
            skip_permissions=skip_permissions,
            yolo=skip_permissions,
            model_variant=model_variant,
            workdir=worktree_path,
        )

        # Run via collector for output capture
        collector_cmd = [
            sys.executable,
            "-m",
            "loopflow.lfd.collector",
            "--session-id",
            run.id,
            "--step",
            f"{loop.goal_name}:{task_name}",
            "--repo-root",
            str(worktree_path),
            "--prompt-file",
            prompt_file,
            "--autocommit",
            "--",
            *command,
        ]

        process = subprocess.Popen(collector_cmd, cwd=worktree_path)
        result_code = process.wait()

        # Clean up prompt file
        try:
            Path(prompt_file).unlink()
        except OSError:
            pass

        notify_event(
            "loop.step.completed",
            {
                "loop_id": loop.id,
                "step": task_name,
                "status": "completed" if result_code == 0 else "error",
            },
        )

        if result_code != 0:
            update_loop_run_status(run.id, LoopStatus.ERROR, error=f"{task_name} failed")
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

    # Clear current step
    update_loop_run_step(run.id, None)

    # Create PR to loop-main and auto-merge (always)
    pr_url = _create_pr_to_loop_main(loop, worktree_path, branch, iteration)
    if pr_url:
        update_loop_run_pr(run.id, pr_url)
        _auto_merge_pr(worktree_path)

        # If LAND mode, also merge loop-main to main
        if loop.merge_mode.value == "land":
            _land_to_main(loop)

    update_loop_run_status(run.id, LoopStatus.IDLE)

    notify_event(
        "loop.iteration.done",
        {
            "loop_id": loop.id,
            "goal": loop.goal_name,
            "iteration": iteration,
            "pr_url": pr_url,
        },
    )

    # Cleanup worktree
    _cleanup_worktree(loop.repo, worktree_path, branch)

    return True


def _create_pr_to_loop_main(
    loop: Loop, worktree_path: Path, branch: str, iteration: int
) -> str | None:
    """Push branch and create PR targeting loop-main."""
    # Push the branch
    result = subprocess.run(
        ["git", "push", "-u", "origin", branch],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    # Generate PR message
    try:
        message = generate_pr_message(worktree_path)
        title = f"[{loop.goal_name}] {message.title}"
        body = f"Loop: {loop.goal_name}\nIteration: {iteration}\n\n{message.body}"
    except Exception:
        title = f"[{loop.goal_name}] Iteration {iteration}"
        body = f"Loop: {loop.goal_name}\nIteration: {iteration}"

    # Create PR
    cmd = [
        "gh",
        "pr",
        "create",
        "--title",
        title,
        "--body",
        body,
        "--base",
        loop.loop_main,
    ]
    result = subprocess.run(cmd, cwd=worktree_path, capture_output=True, text=True)

    if result.returncode == 0:
        return result.stdout.strip()
    return None


def _auto_merge_pr(worktree_path: Path) -> bool:
    """Auto-merge the current PR."""
    result = subprocess.run(
        ["gh", "pr", "merge", "--squash", "--delete-branch"],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def _land_to_main(loop: Loop) -> str | None:
    """Create or update PR from loop-main → main, enable auto-merge.

    Returns PR URL on success, None on failure.
    Works from main repo (not worktree, which gets deleted).
    Idempotent: existing PR just gets auto-merge re-enabled.
    """
    repo = loop.repo

    # Push loop-main
    subprocess.run(["git", "push", "origin", loop.loop_main], cwd=repo, capture_output=True)

    # Check for existing PR
    result = subprocess.run(
        [
            "gh",
            "pr",
            "list",
            "--head",
            loop.loop_main,
            "--base",
            "main",
            "--json",
            "number,url",
            "--state",
            "open",
        ],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    existing = json.loads(result.stdout) if result.returncode == 0 and result.stdout.strip() else []

    if existing:
        # PR exists - ensure auto-merge is enabled
        pr_number = existing[0]["number"]
        subprocess.run(
            ["gh", "pr", "merge", str(pr_number), "--squash", "--auto"],
            cwd=repo,
            capture_output=True,
        )
        return existing[0]["url"]

    # Create new PR
    result = subprocess.run(
        [
            "gh",
            "pr",
            "create",
            "--base",
            "main",
            "--head",
            loop.loop_main,
            "--title",
            f"[{loop.goal_name}] Land accumulated work",
            "--body",
            f"Auto-land from loop: {loop.goal_name}",
        ],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    pr_url = result.stdout.strip()

    # Enable auto-merge on the new PR
    subprocess.run(["gh", "pr", "merge", "--squash", "--auto"], cwd=repo, capture_output=True)

    return pr_url


def _cleanup_worktree(repo: Path, worktree_path: Path, branch: str) -> None:
    """Remove worktree and delete branch."""
    try:
        remove_worktree(repo, branch)
    except Exception:
        pass

    # Delete remote branch
    subprocess.run(
        ["git", "push", "origin", "--delete", branch],
        cwd=repo,
        capture_output=True,
    )


def main() -> None:
    """Entry point for background loop runner."""
    if len(sys.argv) != 2:
        print("Usage: python -m loopflow.lfd.loop_runner <loop_id>", file=sys.stderr)
        sys.exit(1)

    loop_id = sys.argv[1]
    loop = get_loop(loop_id)

    if not loop:
        print(f"Loop not found: {loop_id}", file=sys.stderr)
        sys.exit(1)

    run_loop_iterations(loop)


if __name__ == "__main__":
    main()
