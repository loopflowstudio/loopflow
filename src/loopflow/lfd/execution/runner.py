"""Core iteration runner for lfd.

Executes a single iteration of an Agent.
"""

import subprocess
import sys
import uuid
from dataclasses import replace
from datetime import datetime
from pathlib import Path

from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import ContextConfig, format_prompt, gather_prompt_components
from loopflow.lf.flow import (
    build_join_prompt,
    choose_branch,
    collect_fork_diffs,
    load_join_instructions,
)
from loopflow.lf.flows import (
    FlowDef,
    JoinConfig,
    ResolvedStep,
    load_flow,
    resolve_flow,
)
from loopflow.lf.goals import format_goal_section, resolve_goals
from loopflow.lf.launcher import build_model_command, get_runner
from loopflow.lf.logging import write_prompt_file
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.naming import generate_next_branch, parse_branch_base
from loopflow.lf.worktrees import WorktreeError, get_path
from loopflow.lf.worktrees import create as create_worktree
from loopflow.lf.worktrees import remove as remove_worktree
from loopflow.lfd.daemon.client import notify_event
from loopflow.lfd.flow_run import (
    save_run,
    update_run_pr,
    update_run_status,
    update_run_step,
)
from loopflow.lfd.models import Agent, FlowRun, FlowRunStatus
from loopflow.lfops._helpers import get_default_branch
from loopflow.lfops.next import move_worktree


def _build_loop_prompt(
    agent: Agent,
    effective_goals: list,
    worktree_path: Path,
    step_name: str,
    context_paths: list[str] | None,
    extra_context: list[str] | None = None,
    goals: list[str] | None = None,
) -> tuple[str, str] | None:
    merged_context = list(context_paths) if context_paths else []
    if extra_context:
        merged_context.extend(extra_context)

    components = gather_prompt_components(
        worktree_path,
        step=step_name,
        run_mode="auto",
        goals=goals,
        context_config=ContextConfig(pathset=merged_context),
    )

    if not components.step:
        return None

    step_file, step_content = components.step
    goal_parts = [f"<lf:goal:{g.name}>\n{g.content}\n</lf:goal:{g.name}>" for g in effective_goals]
    goal_section = "\n\n".join(goal_parts)
    combined = f"{goal_section}\n\n---\n\n{step_content}"
    components = replace(components, step=(step_file, combined))
    prompt = format_prompt(components)

    return prompt, step_file


class StepTimeoutError(Exception):
    """Raised when a step exceeds its timeout."""

    def __init__(self, step_label: str, timeout: int, pid: int):
        self.step_label = step_label
        self.timeout = timeout
        self.pid = pid
        super().__init__(f"Step '{step_label}' timed out after {timeout}s (pid={pid})")


# Default step timeout: 30 minutes
DEFAULT_STEP_TIMEOUT = 30 * 60


def _run_collector_step(
    prompt: str,
    worktree_path: Path,
    backend: str,
    model_variant: str | None,
    skip_permissions: bool,
    step_run_id: str,
    step_label: str,
    autocommit: bool = True,
    prefix: str | None = None,
    timeout: int | None = None,
) -> int:
    """Run a step via collector subprocess.

    Args:
        timeout: Max seconds to wait. None uses DEFAULT_STEP_TIMEOUT.
                 0 means no timeout.

    Returns:
        Exit code from the collector process.

    Raises:
        StepTimeoutError: If step exceeds timeout.
    """
    if timeout is None:
        timeout = DEFAULT_STEP_TIMEOUT

    prompt_file = write_prompt_file(prompt)

    command = build_model_command(
        backend,
        auto=True,
        stream=True,
        skip_permissions=skip_permissions,
        yolo=skip_permissions,
        model_variant=model_variant,
        workdir=worktree_path,
    )

    collector_cmd = [
        sys.executable,
        "-m",
        "loopflow.lfd.execution.collector",
        "--step-run-id",
        step_run_id,
        "--step",
        step_label,
        "--repo-root",
        str(worktree_path),
        "--prompt-file",
        prompt_file,
    ]
    if autocommit:
        collector_cmd.append("--autocommit")
    if prefix:
        collector_cmd.extend(["--prefix", prefix])
    collector_cmd.extend(["--", *command])

    process = subprocess.Popen(collector_cmd, cwd=worktree_path)

    try:
        result_code = process.wait(timeout=timeout if timeout > 0 else None)
    except subprocess.TimeoutExpired:
        # Kill the process group (collector and its children)
        _kill_process_tree(process.pid)
        process.wait()  # Reap the zombie
        try:
            Path(prompt_file).unlink()
        except OSError:
            pass
        raise StepTimeoutError(step_label, timeout, process.pid)

    try:
        Path(prompt_file).unlink()
    except OSError:
        pass

    return result_code


def _kill_process_tree(pid: int) -> None:
    """Kill a process and all its children."""
    import os
    import signal

    try:
        # Try to kill the process group first
        os.killpg(os.getpgid(pid), signal.SIGTERM)
    except (ProcessLookupError, PermissionError, OSError):
        pass

    # Also try direct kill in case process group kill failed
    try:
        os.kill(pid, signal.SIGTERM)
    except (ProcessLookupError, PermissionError, OSError):
        pass


class _VariantResult:
    def __init__(self, label: str, worktree: Path, exit_code: int, step_run_id: str):
        self.label = label
        self.worktree = worktree
        self.exit_code = exit_code
        self.step_run_id = step_run_id


def _cleanup_variant_worktrees(repo_root: Path, results: list[_VariantResult]) -> None:
    for r in results:
        remove_worktree(repo_root, r.worktree.name.split(".")[-1])


def _build_loop_inline_prompt(
    agent: Agent,
    effective_goals: list,
    worktree_path: Path,
    inline_text: str,
    context_paths: list[str] | None,
    goals: list[str] | None = None,
) -> str | None:
    components = gather_prompt_components(
        worktree_path,
        inline=inline_text,
        run_mode="auto",
        goals=goals,
        context_config=ContextConfig(pathset=context_paths),
    )
    if not components.step:
        return None

    step_file, step_content = components.step
    goal_parts = [f"<lf:goal:{g.name}>\n{g.content}\n</lf:goal:{g.name}>" for g in effective_goals]
    goal_section = "\n\n".join(goal_parts)
    combined = f"{goal_section}\n\n---\n\n{step_content}"
    components = replace(components, step=(step_file, combined))
    return format_prompt(components)


def _run_fork_join_group(
    agent: Agent,
    flow_name: str,
    worktree_path: Path,
    branch: str,
    steps: list[ResolvedStep],
    join_config: JoinConfig,
    context_paths: list[str] | None,
    effective_goals: list,
    skip_permissions: bool,
    backend: str,
    model_variant: str | None,
) -> int:
    results: list[_VariantResult] = []
    label_counts: dict[str, int] = {}

    for step in steps:
        if not step.step:
            continue

        step_backend = backend
        step_variant = model_variant
        step_context = list(context_paths) if context_paths else []
        step_goals = None

        if step.config:
            if step.config.model:
                step_backend, step_variant = parse_model(step.config.model)
            if step.config.context:
                step_context.extend(step.config.context)
            if step.config.goal:
                step_goals = step.config.goal

        label_base = step.step
        label_counts[label_base] = label_counts.get(label_base, 0) + 1
        label = label_base
        if label_counts[label_base] > 1:
            label = f"{label_base}:{label_counts[label_base]}"

        wt_name = f"_fork-{branch.replace('/', '-')}-{label.replace(':', '-')}"
        try:
            wt_path = create_worktree(agent.repo, wt_name, base=branch)
        except Exception:
            _cleanup_variant_worktrees(agent.repo, results)
            return 1

        subprocess.run(
            ["git", "reset", "--hard", branch],
            cwd=wt_path,
            capture_output=True,
        )
        subprocess.run(["git", "clean", "-fd"], cwd=wt_path, capture_output=True)

        prompt_result = _build_loop_prompt(
            agent,
            effective_goals,
            wt_path,
            step.step,
            step_context or None,
            goals=step_goals,
        )
        if not prompt_result:
            remove_worktree(agent.repo, wt_path.name.split(".")[-1])
            return 1

        prompt, _step_file = prompt_result
        step_run_id = str(uuid.uuid4())
        step_label = f"{agent.area_display}:{step.step}:{label}"

        try:
            exit_code = _run_collector_step(
                prompt,
                wt_path,
                step_backend,
                step_variant,
                skip_permissions,
                step_run_id,
                step_label,
                autocommit=True,
                prefix=f"[{label}] ",
            )
        except StepTimeoutError:
            # Clean up and re-raise for caller to handle
            _cleanup_variant_worktrees(agent.repo, results)
            remove_worktree(agent.repo, wt_path.name.split(".")[-1])
            raise

        results.append(_VariantResult(label, wt_path, exit_code, step_run_id))

    successes = [r for r in results if r.exit_code == 0]
    if not successes:
        _cleanup_variant_worktrees(agent.repo, results)
        return 1

    fork_worktrees = [(r.label, r.worktree) for r in successes]
    join_prompt = build_join_prompt(
        collect_fork_diffs(fork_worktrees),
        load_join_instructions(join_config.step, agent.repo),
        format_goal_section(join_config.goal, agent.repo),
        flow_name,
    )
    join_prompt = _build_loop_inline_prompt(
        agent,
        effective_goals,
        worktree_path,
        join_prompt,
        context_paths,
        goals=None,
    )
    if not join_prompt:
        _cleanup_variant_worktrees(agent.repo, results)
        return 1

    join_backend = backend
    join_variant = model_variant
    if join_config.agent_model:
        join_backend, join_variant = parse_model(join_config.agent_model)

    try:
        join_result = _run_collector_step(
            join_prompt,
            worktree_path,
            join_backend,
            join_variant,
            skip_permissions,
            str(uuid.uuid4()),
            f"{agent.area_display}:join",
            autocommit=True,
        )
    except StepTimeoutError:
        _cleanup_variant_worktrees(agent.repo, results)
        raise

    _cleanup_variant_worktrees(agent.repo, results)
    return join_result


def run_iteration(
    agent: Agent,
    iteration: int,
    run_id: str | None = None,
) -> tuple[bool, Path | None, str | None]:
    """Run a single iteration of an agent.

    Returns (success, worktree_path, new_branch).
    - worktree_path: the agent's worktree (same path, persists across iterations)
    - new_branch: the branch after moving (for next iteration)
    """
    config = load_config(agent.repo)
    base_branch = get_default_branch(agent.repo)

    # Use agent's persistent worktree, or create on first iteration
    if agent.worktree and agent.worktree.exists():
        worktree_path = agent.worktree
        branch = agent.branch
    else:
        # First iteration: generate initial branch and create worktree
        area_slug = agent.area_slug
        branch = generate_next_branch(area_slug, agent.repo)
        worktree_path = get_path(agent.repo, area_slug)
        try:
            git_cmd = [
                "git",
                "worktree",
                "add",
                "-b",
                branch,
                str(worktree_path),
                f"origin/{base_branch}",
            ]
            subprocess.run(git_cmd, cwd=agent.repo, capture_output=True, check=True)
            subprocess.run(
                ["git", "push", "-u", "origin", branch],
                cwd=worktree_path,
                capture_output=True,
            )
        except (subprocess.CalledProcessError, WorktreeError) as e:
            error_msg = f"Failed to create worktree: {e}"
            notify_event("agent.error", {"agent_id": agent.id, "error": error_msg})
            return False, None, None

    run = FlowRun(
        id=run_id or str(uuid.uuid4()),
        agent_id=agent.id,
        flow=agent.flow,
        goal=agent.goal,
        area=agent.area,
        repo=agent.repo,
        status=FlowRunStatus.RUNNING,
        iteration=iteration,
        worktree=str(worktree_path),
        branch=branch,
        started_at=datetime.now(),
    )
    save_run(run)

    notify_event(
        "agent.started",
        {
            "agent_id": agent.id,
            "area": agent.area_display,
            "goal": agent.goal_display,
            "flow": agent.flow,
            "iteration": iteration,
        },
    )

    effective_goals = resolve_goals(agent.repo, agent.goal)
    # Goals are optional - proceed even if none specified

    flow = agent.flow
    if not flow:
        update_run_status(run.id, FlowRunStatus.FAILED, error="Flow is required")
        return False, None, None

    try:
        flow_def = load_flow(flow, agent.repo)
    except ValueError as exc:
        update_run_status(run.id, FlowRunStatus.FAILED, error=str(exc))
        return False, None, None

    if not flow_def:
        update_run_status(run.id, FlowRunStatus.FAILED, error=f"Unknown flow '{flow}'")
        return False, None, None

    resolved = resolve_flow(flow_def, agent.repo)
    if not resolved:
        update_run_status(run.id, FlowRunStatus.FAILED, error=f"Empty flow '{flow}'")
        return False, None, None

    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    runner = get_runner(backend)
    if not runner.is_available():
        update_run_status(run.id, FlowRunStatus.FAILED, error=f"'{backend}' CLI not found")
        return False, None, None

    skip_permissions = config.yolo if config else False

    # Use agent's area as context paths
    context_paths = list(agent.area) if agent.area[0] != "." else None

    i = 0
    while i < len(resolved):
        step = resolved[i]
        if step.parallel_group is not None:
            group_steps = []
            group = step.parallel_group
            while i < len(resolved) and resolved[i].parallel_group == group:
                group_steps.append(resolved[i])
                i += 1

            if i >= len(resolved) or resolved[i].join is None:
                update_run_status(
                    run.id, FlowRunStatus.FAILED, error="Fork must be immediately followed by join"
                )
                return False, None, None

            try:
                result_code = _run_fork_join_group(
                    agent,
                    flow_def.name,
                    worktree_path,
                    branch,
                    group_steps,
                    resolved[i].join.join,
                    context_paths,
                    effective_goals,
                    skip_permissions,
                    backend,
                    model_variant,
                )
            except StepTimeoutError as e:
                update_run_status(run.id, FlowRunStatus.FAILED, error=str(e))
                return False, None, None
            if result_code != 0:
                update_run_status(run.id, FlowRunStatus.FAILED, error="join failed")
                return False, None, None

            i += 1
            continue

        if step.choose is not None:
            try:
                choice = choose_branch(
                    step.choose,
                    flow_def.name,
                    worktree_path,
                    backend,
                    model_variant,
                    skip_permissions,
                )
            except RuntimeError as exc:
                update_run_status(run.id, FlowRunStatus.FAILED, error=str(exc))
                return False, None, None

            branch_steps = step.choose.options[choice]
            branch_flow = FlowDef.from_dict(f"{flow_def.name}:{choice}", {"steps": branch_steps})
            branch_resolved = resolve_flow(branch_flow, agent.repo)
            resolved = resolved[:i] + branch_resolved + resolved[i + 1 :]
            continue

        if step.join is not None:
            update_run_status(run.id, FlowRunStatus.FAILED, error="Join must follow fork")
            return False, None, None

        if not step.step:
            i += 1
            continue

        step_name = step.step
        update_run_step(run.id, step_name)
        notify_event(
            "agent.step.started",
            {
                "agent_id": agent.id,
                "step": step_name,
                "iteration": iteration,
            },
        )

        step_backend = backend
        step_variant = model_variant
        step_context = list(context_paths) if context_paths else []
        step_goals = None

        if step.config:
            if step.config.model:
                step_backend, step_variant = parse_model(step.config.model)
            if step.config.context:
                step_context.extend(step.config.context)
            if step.config.goal:
                step_goals = step.config.goal

        prompt_result = _build_loop_prompt(
            agent,
            effective_goals,
            worktree_path,
            step_name,
            step_context or None,
            goals=step_goals,
        )
        if not prompt_result:
            update_run_status(
                run.id, FlowRunStatus.FAILED, error=f"Step file not found: {step_name}"
            )
            return False, None, None

        prompt, _step_file = prompt_result
        try:
            result_code = _run_collector_step(
                prompt,
                worktree_path,
                step_backend,
                step_variant,
                skip_permissions,
                run.id,
                f"{agent.area_display}:{step_name}",
            )
        except StepTimeoutError as e:
            notify_event(
                "agent.step.completed",
                {
                    "agent_id": agent.id,
                    "step": step_name,
                    "status": "timeout",
                },
            )
            update_run_status(run.id, FlowRunStatus.FAILED, error=str(e))
            return False, None, None

        notify_event(
            "agent.step.completed",
            {
                "agent_id": agent.id,
                "step": step_name,
                "status": "completed" if result_code == 0 else "error",
            },
        )

        if result_code != 0:
            update_run_status(run.id, FlowRunStatus.FAILED, error=f"{step_name} failed")
            return False, None, None

        i += 1

    update_run_step(run.id, None)

    # Create PR targeting main
    pr_url = _create_pr_to_main(agent, worktree_path, branch, iteration, base_branch)
    if pr_url:
        update_run_pr(run.id, pr_url)
        _enable_auto_merge(worktree_path)

    update_run_status(run.id, FlowRunStatus.COMPLETED)

    notify_event(
        "agent.iteration.done",
        {
            "agent_id": agent.id,
            "area": agent.area_display,
            "goal": agent.goal_display,
            "flow": agent.flow,
            "iteration": iteration,
            "pr_url": pr_url,
        },
    )

    # Move worktree to new branch for next iteration
    base_name = parse_branch_base(branch)
    new_branch = generate_next_branch(base_name, agent.repo)
    if not move_worktree(agent.repo, worktree_path, new_branch, base_branch):
        notify_event("agent.error", {"agent_id": agent.id, "error": "Failed to move worktree"})
        return True, worktree_path, None  # Iteration succeeded, but couldn't prep for next

    return True, worktree_path, new_branch


def _create_pr_to_main(
    agent: Agent, worktree_path: Path, branch: str, iteration: int, base_branch: str
) -> str | None:
    """Push branch and create PR targeting base branch (usually main)."""
    result = subprocess.run(
        ["git", "push", "-u", "origin", branch],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    try:
        message = generate_pr_message(worktree_path)
        title = f"[{agent.area_slug}] {message.title}"
        body = (
            f"Agent: {agent.area_display} [{agent.goal_display}]\n"
            f"Flow: {agent.flow}\n"
            f"Iteration: {iteration}\n\n{message.body}"
        )
    except Exception:
        title = f"[{agent.area_slug}] Iteration {iteration}"
        body = (
            f"Agent: {agent.area_display} [{agent.goal_display}]\n"
            f"Flow: {agent.flow}\n"
            f"Iteration: {iteration}"
        )

    cmd = [
        "gh",
        "pr",
        "create",
        "--title",
        title,
        "--body",
        body,
        "--base",
        base_branch,
    ]
    result = subprocess.run(cmd, cwd=worktree_path, capture_output=True, text=True)

    if result.returncode == 0:
        return result.stdout.strip()
    return None


def _enable_auto_merge(worktree_path: Path) -> bool:
    """Enable auto-merge on the current PR (squash when CI passes)."""
    result = subprocess.run(
        ["gh", "pr", "merge", "--squash", "--auto"],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0
