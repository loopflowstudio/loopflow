"""Core iteration runner for lfd.

Executes a single iteration of an Agent.
"""

import concurrent.futures
import json
import subprocess
import sys
import uuid
from dataclasses import dataclass, replace
from datetime import datetime
from pathlib import Path

from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import ContextConfig, format_prompt, gather_prompt_components
from loopflow.lf.flow import (
    ForkResult,
    build_synthesize_prompt,
    choose_branch,
    load_synthesize_instructions,
    run_flow_def,
    topological_batches,
)
from loopflow.lf.flows import (
    Choose,
    Fork,
    ForkAgent,
    Step,
    build_step_dag,
    load_flow,
)
from loopflow.lf.goals import resolve_goals
from loopflow.lf.launcher import build_model_command, get_runner
from loopflow.lf.logging import write_prompt_file
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.worktrees import WorktreeError
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


@dataclass
class IterationResult:
    """Result of running a single agent iteration."""

    success: bool
    worktree: Path | None = None
    branch: str | None = None


def _iteration_branch_prefix(main_branch: str) -> str:
    """Derive iteration branch prefix from main branch."""
    if main_branch.endswith("-main"):
        return main_branch[:-5]
    return main_branch


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


def _read_scratch_notes(worktree: Path) -> str:
    scratch_dir = worktree / "scratch"
    if not scratch_dir.exists():
        return ""
    notes = []
    for path in sorted(scratch_dir.glob("*.md")):
        try:
            contents = path.read_text().strip()
        except OSError:
            continue
        if contents:
            notes.append(f"## {path.name}\n{contents}")
    return "\n\n".join(notes)


def _current_branch(worktree: Path) -> str | None:
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()
    return branch or None


def _git_rev_parse(worktree: Path, ref: str) -> str:
    result = subprocess.run(
        ["git", "rev-parse", ref],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip() if result.returncode == 0 else ref


def _merge_branch(worktree: Path, branch: str) -> bool:
    result = subprocess.run(
        ["git", "merge", "--no-edit", branch],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        error = result.stderr.strip() or result.stdout.strip()
        notify_event("agent.error", {"error": f"Merge failed for {branch}: {error}"})
        return False
    return True


def _run_git(worktree: Path, args: list[str]) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    return result.stdout if result.returncode == 0 else ""


def _cleanup_fork_worktrees(repo_root: Path, results: list[ForkResult]) -> None:
    for result in results:
        remove_worktree(repo_root, result.worktree.name.split(".")[-1])


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


def _run_fork_synthesize(
    agent: Agent,
    flow_name: str,
    worktree_path: Path,
    branch: str,
    fork: Fork,
    context_paths: list[str] | None,
    effective_goals: list,
    skip_permissions: bool,
    backend: str,
    model_variant: str | None,
) -> int:
    results: list[ForkResult] = []
    base_commit = _git_rev_parse(worktree_path, "HEAD")

    def _run_agent(agent_config: ForkAgent, index: int) -> ForkResult:
        wt_name = f"fork-{flow_name}-{index}"
        try:
            wt_path = create_worktree(agent.repo, wt_name, base=branch)
        except Exception:
            return ForkResult(
                worktree=worktree_path,
                config=agent_config,
                diff="",
                status="failed",
                scratch_notes="",
            )
        subprocess.run(
            ["git", "reset", "--hard", branch],
            cwd=wt_path,
            capture_output=True,
        )
        subprocess.run(["git", "clean", "-fd"], cwd=wt_path, capture_output=True)

        agent_backend = backend
        agent_variant = model_variant
        if agent_config.model:
            agent_backend, agent_variant = parse_model(agent_config.model)

        agent_context = list(context_paths) if context_paths else []

        if agent_config.step:
            prompt_result = _build_loop_prompt(
                agent,
                effective_goals,
                wt_path,
                agent_config.step,
                agent_context or None,
            )
            if not prompt_result:
                return ForkResult(
                    worktree=wt_path,
                    config=agent_config,
                    diff="",
                    status="failed",
                    scratch_notes="",
                )

            prompt, _step_file = prompt_result
            step_run_id = str(uuid.uuid4())
            step_label = f"{agent.area_display}:{agent_config.step}:fork-{index}"
            try:
                exit_code = _run_collector_step(
                    prompt,
                    wt_path,
                    agent_backend,
                    agent_variant,
                    skip_permissions,
                    step_run_id,
                    step_label,
                    autocommit=True,
                    prefix=f"[fork-{index}] ",
                )
            except StepTimeoutError as exc:
                return ForkResult(
                    worktree=wt_path,
                    config=agent_config,
                    diff="",
                    status=f"timeout: {exc}",
                    scratch_notes="",
                )

            diff = _run_git(wt_path, ["diff", f"{base_commit}..HEAD"])
            return ForkResult(
                worktree=wt_path,
                config=agent_config,
                diff=diff,
                status="completed" if exit_code == 0 else "failed",
                scratch_notes=_read_scratch_notes(wt_path),
            )

        if agent_config.flow:
            flow_def = load_flow(agent_config.flow, agent.repo)
            if not flow_def:
                return ForkResult(
                    worktree=wt_path,
                    config=agent_config,
                    diff="",
                    status="failed",
                    scratch_notes="",
                )
            exit_code = run_flow_def(
                flow_def,
                wt_path,
                context=agent_context or None,
                exclude=None,
                skip_permissions=skip_permissions,
                push_enabled=False,
                pr_enabled=False,
                backend=agent_backend,
                model_variant=agent_variant,
            )
            diff = _run_git(wt_path, ["diff", f"{base_commit}..HEAD"])
            return ForkResult(
                worktree=wt_path,
                config=agent_config,
                diff=diff,
                status="completed" if exit_code == 0 else "failed",
                scratch_notes=_read_scratch_notes(wt_path),
            )

        return ForkResult(
            worktree=wt_path,
            config=agent_config,
            diff="",
            status="failed",
            scratch_notes="",
        )

    with concurrent.futures.ThreadPoolExecutor(max_workers=len(fork.agents)) as executor:
        futures = [
            executor.submit(_run_agent, config, i + 1) for i, config in enumerate(fork.agents)
        ]
        for future in futures:
            results.append(future.result())

    if not any(result.status == "completed" for result in results):
        _cleanup_fork_worktrees(agent.repo, results)
        return 1

    synth_prompt_override = fork.synthesize.prompt if fork.synthesize else None
    instructions = load_synthesize_instructions(agent.repo, synth_prompt_override)
    synth_prompt = build_synthesize_prompt(results, instructions, base_commit)
    synth_prompt = _build_loop_inline_prompt(
        agent,
        effective_goals,
        worktree_path,
        synth_prompt,
        context_paths,
        goals=None,
    )
    if not synth_prompt:
        _cleanup_fork_worktrees(agent.repo, results)
        return 1

    try:
        exit_code = _run_collector_step(
            synth_prompt,
            worktree_path,
            backend,
            model_variant,
            skip_permissions,
            str(uuid.uuid4()),
            f"{agent.area_display}:synthesize",
            autocommit=True,
        )
    except StepTimeoutError:
        _cleanup_fork_worktrees(agent.repo, results)
        raise

    _cleanup_fork_worktrees(agent.repo, results)
    return exit_code


def run_iteration(
    agent: Agent,
    iteration: int,
    run_id: str | None = None,
) -> IterationResult:
    """Run a single iteration of an agent."""
    config = load_config(agent.repo)

    prefix = _iteration_branch_prefix(agent.main_branch)
    branch = f"{prefix}/{iteration:03d}"
    try:
        worktree_path = create_worktree(agent.repo, branch, base=agent.main_branch)
    except WorktreeError as e:
        error_msg = f"Failed to create worktree: {e}"
        notify_event("agent.error", {"agent_id": agent.id, "error": error_msg})
        return IterationResult(success=False)

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
        _cleanup_worktree(agent.repo, worktree_path, branch)
        return IterationResult(success=False)

    try:
        flow_def = load_flow(flow, agent.repo)
    except ValueError as exc:
        update_run_status(run.id, FlowRunStatus.FAILED, error=str(exc))
        _cleanup_worktree(agent.repo, worktree_path, branch)
        return IterationResult(success=False)

    if not flow_def:
        update_run_status(run.id, FlowRunStatus.FAILED, error=f"Unknown flow '{flow}'")
        _cleanup_worktree(agent.repo, worktree_path, branch)
        return IterationResult(success=False)

    items: list[Step | Fork | Choose] = list(flow_def.steps)
    if not items:
        update_run_status(run.id, FlowRunStatus.FAILED, error=f"Empty flow '{flow}'")
        _cleanup_worktree(agent.repo, worktree_path, branch)
        return IterationResult(success=False)

    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    runner = get_runner(backend)
    if not runner.is_available():
        update_run_status(run.id, FlowRunStatus.FAILED, error=f"'{backend}' CLI not found")
        return IterationResult(success=False)

    skip_permissions = config.yolo if config else False

    # Use agent's area as context paths
    context_paths = list(agent.area) if agent.area[0] != "." else None

    i = 0
    while i < len(items):
        item = items[i]

        if isinstance(item, Step):
            phase: list[Step] = []
            while i < len(items) and isinstance(items[i], Step):
                phase.append(items[i])
                i += 1

            dag = build_step_dag(phase)
            batches = topological_batches(dag)
            for batch in batches:
                if len(batch) == 1:
                    step_def = batch[0]
                    step_name = step_def.name
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
                    if step_def.model:
                        step_backend, step_variant = parse_model(step_def.model)

                    prompt_result = _build_loop_prompt(
                        agent,
                        effective_goals,
                        worktree_path,
                        step_name,
                        context_paths,
                    )
                    if not prompt_result:
                        update_run_status(
                            run.id, FlowRunStatus.FAILED, error=f"Step file not found: {step_name}"
                        )
                        _cleanup_worktree(agent.repo, worktree_path, branch)
                        return IterationResult(success=False)

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
                        _cleanup_worktree(agent.repo, worktree_path, branch)
                        return IterationResult(success=False)

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
                        _cleanup_worktree(agent.repo, worktree_path, branch)
                        return IterationResult(success=False)
                    continue

                base_branch = _current_branch(worktree_path) or branch
                futures = []
                results: list[tuple[Step, Path, int]] = []

                def _run_parallel(step_def: Step, index: int) -> tuple[Step, Path, int]:
                    wt_name = f"parallel-{branch.replace('/', '-')}-{step_def.name}-{index}"
                    wt_path = create_worktree(agent.repo, wt_name, base=base_branch)
                    subprocess.run(
                        ["git", "reset", "--hard", base_branch],
                        cwd=wt_path,
                        capture_output=True,
                    )
                    subprocess.run(["git", "clean", "-fd"], cwd=wt_path, capture_output=True)

                    step_backend = backend
                    step_variant = model_variant
                    if step_def.model:
                        step_backend, step_variant = parse_model(step_def.model)

                    prompt_result = _build_loop_prompt(
                        agent,
                        effective_goals,
                        wt_path,
                        step_def.name,
                        context_paths,
                    )
                    if not prompt_result:
                        return step_def, wt_path, 1

                    prompt, _step_file = prompt_result
                    step_run_id = str(uuid.uuid4())
                    step_label = f"{agent.area_display}:{step_def.name}:parallel"
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
                            prefix=f"[{step_def.name}] ",
                        )
                    except StepTimeoutError:
                        return step_def, wt_path, 1
                    return step_def, wt_path, exit_code

                with concurrent.futures.ThreadPoolExecutor(max_workers=len(batch)) as executor:
                    for index, step_def in enumerate(batch, 1):
                        futures.append(executor.submit(_run_parallel, step_def, index))
                    for future in futures:
                        results.append(future.result())

                if any(exit_code != 0 for _, _, exit_code in results):
                    for _, wt_path, _ in results:
                        remove_worktree(agent.repo, wt_path.name.split(".")[-1])
                    update_run_status(run.id, FlowRunStatus.FAILED, error="Parallel step failed")
                    _cleanup_worktree(agent.repo, worktree_path, branch)
                    return IterationResult(success=False)

                for _, wt_path, _ in results:
                    merge_branch = _current_branch(wt_path) or wt_path.name
                    if not _merge_branch(worktree_path, merge_branch):
                        for _, cleanup_path, _ in results:
                            remove_worktree(agent.repo, cleanup_path.name.split(".")[-1])
                        update_run_status(run.id, FlowRunStatus.FAILED, error="Merge failed")
                        _cleanup_worktree(agent.repo, worktree_path, branch)
                        return IterationResult(success=False)

                for _, wt_path, _ in results:
                    remove_worktree(agent.repo, wt_path.name.split(".")[-1])

            continue

        if isinstance(item, Fork):
            try:
                result_code = _run_fork_synthesize(
                    agent,
                    flow_def.name,
                    worktree_path,
                    branch,
                    item,
                    context_paths,
                    effective_goals,
                    skip_permissions,
                    backend,
                    model_variant,
                )
            except StepTimeoutError as e:
                update_run_status(run.id, FlowRunStatus.FAILED, error=str(e))
                _cleanup_worktree(agent.repo, worktree_path, branch)
                return IterationResult(success=False)
            if result_code != 0:
                update_run_status(run.id, FlowRunStatus.FAILED, error="synthesize failed")
                _cleanup_worktree(agent.repo, worktree_path, branch)
                return IterationResult(success=False)

            i += 1
            continue

        if isinstance(item, Choose):
            try:
                choice = choose_branch(
                    item,
                    flow_def.name,
                    worktree_path,
                    backend,
                    model_variant,
                    skip_permissions,
                )
            except RuntimeError as exc:
                update_run_status(run.id, FlowRunStatus.FAILED, error=str(exc))
                _cleanup_worktree(agent.repo, worktree_path, branch)
                return IterationResult(success=False)

            items = items[:i] + item.options[choice] + items[i + 1 :]
            continue

        i += 1

    update_run_step(run.id, None)

    pr_url = _create_pr_to_main_branch(agent, worktree_path, branch, iteration)
    if pr_url:
        update_run_pr(run.id, pr_url)
        _auto_merge_pr(worktree_path)

        if agent.merge_mode.value == "land":
            _land_to_main(agent)

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

    _cleanup_worktree(agent.repo, worktree_path, branch)

    return IterationResult(success=True, worktree=worktree_path, branch=branch)


def _create_pr_to_main_branch(
    agent: Agent, worktree_path: Path, branch: str, iteration: int
) -> str | None:
    """Push branch and create PR targeting main_branch."""
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
        agent.main_branch,
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


def _land_to_main(agent: Agent) -> str | None:
    """Create or update PR from main_branch to main, enable auto-merge."""
    repo = agent.repo

    subprocess.run(["git", "push", "origin", agent.main_branch], cwd=repo, capture_output=True)

    result = subprocess.run(
        [
            "gh",
            "pr",
            "list",
            "--head",
            agent.main_branch,
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
        pr_number = existing[0]["number"]
        subprocess.run(
            ["gh", "pr", "merge", str(pr_number), "--squash", "--auto"],
            cwd=repo,
            capture_output=True,
        )
        return existing[0]["url"]

    result = subprocess.run(
        [
            "gh",
            "pr",
            "create",
            "--base",
            "main",
            "--head",
            agent.main_branch,
            "--title",
            f"[{agent.area_slug}] Land accumulated work",
            "--body",
            f"Auto-land from agent: {agent.area_display} [{agent.goal_display}] "
            f"(flow: {agent.flow})",
        ],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    pr_url = result.stdout.strip()

    subprocess.run(["gh", "pr", "merge", "--squash", "--auto"], cwd=repo, capture_output=True)

    return pr_url


def _cleanup_worktree(repo: Path, worktree_path: Path, branch: str) -> None:
    """Remove worktree and delete branch."""
    try:
        remove_worktree(repo, branch)
    except Exception:
        pass

    subprocess.run(
        ["git", "push", "origin", "--delete", branch],
        cwd=repo,
        capture_output=True,
    )
