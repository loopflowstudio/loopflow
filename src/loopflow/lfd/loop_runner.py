"""Loop iteration runner for lfd.

Runs iterations of a loop until the PR limit is reached or an error occurs.
Can be invoked directly as a subprocess for background execution.
"""

import subprocess
import sys
import uuid
from datetime import datetime
from pathlib import Path

from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import gather_prompt_components, format_prompt
from loopflow.lf.goals import load_goal
from loopflow.lf.launcher import build_model_command, get_runner
from loopflow.lf.logging import write_prompt_file
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.worktrees import WorktreeError, create as create_worktree, remove as remove_worktree
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
from loopflow.lfd.models import Loop, LoopRun, LoopStatus


def run_loop_iterations(loop: Loop) -> None:
    """Run loop iterations until PR limit is reached or error occurs."""
    while True:
        # Check if we should pause
        outstanding = count_outstanding(loop)
        if outstanding >= loop.pr_limit:
            update_loop_status(loop.id, LoopStatus.WAITING)
            notify_event("loop.waiting", {
                "loop_id": loop.id,
                "goal": loop.goal,
                "outstanding": outstanding,
                "limit": loop.pr_limit,
            })
            break

        # Run one iteration
        iteration = loop.iteration + 1
        try:
            success = run_iteration(loop, iteration)
            if success:
                update_loop_iteration(loop.id, iteration)
                loop.iteration = iteration
            else:
                update_loop_status(loop.id, LoopStatus.ERROR)
                break
        except Exception as e:
            notify_event("loop.error", {
                "loop_id": loop.id,
                "goal": loop.goal,
                "error": str(e),
            })
            update_loop_status(loop.id, LoopStatus.ERROR)
            break

    # Clear pid when done
    update_loop_pid(loop.id, None)


def run_iteration(loop: Loop, iteration: int) -> bool:
    """Run a single iteration of the loop.

    Returns True if successful, False on error.
    """
    config = load_config(loop.repo)

    # Create iteration branch from personal-main
    branch = f"{loop.goal}/{iteration:03d}"
    try:
        worktree_path = create_worktree(loop.repo, branch, base=loop.personal_main)
    except WorktreeError as e:
        notify_event("loop.error", {"loop_id": loop.id, "error": f"Failed to create worktree: {e}"})
        return False

    # Create loop_run record
    run = LoopRun(
        id=str(uuid.uuid4()),
        loop_id=loop.id,
        iteration=iteration,
        status=LoopStatus.RUNNING,
        started_at=datetime.now(),
        worktree=str(worktree_path),
    )
    save_loop_run(run)

    notify_event("loop.started", {
        "loop_id": loop.id,
        "goal": loop.goal,
        "iteration": iteration,
    })

    # Load goal content
    goal_spec = load_goal(loop.repo, loop.goal)
    if not goal_spec:
        update_loop_run_status(run.id, LoopStatus.ERROR, error="Goal file not found")
        return False

    # Parse pipeline tasks from goal or default
    pipeline = goal_spec.pipeline if goal_spec.pipeline else "design,implement,polish"
    if pipeline.startswith("@") and config and config.pipelines:
        pipeline_name = pipeline[1:]
        if pipeline_name in config.pipelines:
            tasks = config.pipelines[pipeline_name].tasks
        else:
            tasks = [t.strip() for t in pipeline.split(",")]
    else:
        tasks = [t.strip() for t in pipeline.split(",")]

    # Get model configuration
    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    runner = get_runner(backend)
    if not runner.is_available():
        update_loop_run_status(run.id, LoopStatus.ERROR, error=f"'{backend}' CLI not found")
        return False

    skip_permissions = config.yolo if config else False

    # Run each task in the pipeline
    for task_name in tasks:
        update_loop_run_step(run.id, task_name)
        notify_event("loop.step.started", {
            "loop_id": loop.id,
            "step": task_name,
            "iteration": iteration,
        })

        # Gather prompt components
        context_paths = goal_spec.area if goal_spec.area else None
        if loop.area:
            context_paths = [a.strip() for a in loop.area.split(",")]

        components = gather_prompt_components(
            worktree_path,
            task=task_name,
            context=context_paths,
            run_mode="auto",
        )

        # Inject goal content
        if components.task:
            task_file, task_content = components.task
            goal_section = f"<lf:goal:{loop.goal}>\n{goal_spec.content}\n</lf:goal:{loop.goal}>"
            combined = f"{goal_section}\n\n---\n\n{task_content}"
            components = components._replace(task=(task_file, combined))

        prompt = format_prompt(components)
        prompt_file = write_prompt_file(prompt)

        # Build and run command
        command = build_model_command(
            backend,
            auto=True,
            stream=True,
            skip_permissions=skip_permissions,
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
            "--task",
            f"{loop.goal}:{task_name}",
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

        notify_event("loop.step.completed", {
            "loop_id": loop.id,
            "step": task_name,
            "status": "completed" if result_code == 0 else "error",
        })

        if result_code != 0:
            update_loop_run_status(run.id, LoopStatus.ERROR, error=f"{task_name} failed")
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

    # Clear current step
    update_loop_run_step(run.id, None)

    # Create PR to personal-main
    pr_url = _create_pr_to_personal_main(loop, worktree_path, branch, iteration)
    if pr_url:
        update_loop_run_pr(run.id, pr_url)

        # Auto-merge if configured
        if loop.merge_mode.value == "auto":
            _auto_merge_pr(worktree_path)

    update_loop_run_status(run.id, LoopStatus.IDLE)

    notify_event("loop.iteration.done", {
        "loop_id": loop.id,
        "goal": loop.goal,
        "iteration": iteration,
        "pr_url": pr_url,
    })

    # Cleanup worktree
    _cleanup_worktree(loop.repo, worktree_path, branch)

    return True


def _create_pr_to_personal_main(loop: Loop, worktree_path: Path, branch: str, iteration: int) -> str | None:
    """Push branch and create PR targeting personal-main."""
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
        title = f"[{loop.goal}] {message.title}"
        body = f"Loop: {loop.goal}\nIteration: {iteration}\n\n{message.body}"
    except Exception:
        title = f"[{loop.goal}] Iteration {iteration}"
        body = f"Loop: {loop.goal}\nIteration: {iteration}"

    # Create PR
    cmd = [
        "gh", "pr", "create",
        "--title", title,
        "--body", body,
        "--base", loop.personal_main,
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
