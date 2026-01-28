"""Core iteration runner for lfd.

Executes a single iteration of a Wave.
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
from loopflow.lf.context import (
    ContextConfig,
    format_prompt,
    gather_prompt_components,
    gather_step,
)
from loopflow.lf.directions import resolve_directions
from loopflow.lf.flow import (
    ForkResult,
    build_synthesize_prompt,
    choose_branch,
    load_synthesize_instructions,
    run_flow,
    topological_batches,
)
from loopflow.lf.flows import (
    Choose,
    Fork,
    ForkThread,
    Step,
    build_step_dag,
    load_flow,
)
from loopflow.lf.launcher import build_model_command, get_runner
from loopflow.lf.logging import write_prompt_file
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.worktrees import WorktreeError
from loopflow.lf.worktrees import create as create_worktree
from loopflow.lf.worktrees import remove as remove_worktree
from loopflow.lfd.daemon.client import notify_event
from loopflow.lfd.flow_run import (
    get_run,
    save_run,
    update_flow_run_index,
    update_run_pr,
    update_run_status,
    update_run_step,
)
from loopflow.lfd.models import (
    FlowRun,
    FlowRunStatus,
    StepRun,
    StepRunStatus,
    TickResult,
    Wave,
    WaveStatus,
)
from loopflow.lfd.step_run import save_step_run
from loopflow.lfd.wave import get_wave, update_wave_status


@dataclass
class IterationResult:
    """Result of running a single wave iteration."""

    success: bool
    worktree: Path | None = None
    branch: str | None = None


def _iteration_branch_prefix(main_branch: str) -> str:
    """Derive iteration branch prefix from main branch."""
    if main_branch.endswith("-main"):
        return main_branch[:-5]
    return main_branch


def _build_loop_prompt(
    wave: Wave,
    direction: list,
    worktree_path: Path,
    step_name: str,
    context_paths: list[str] | None,
    extra_context: list[str] | None = None,
) -> tuple[str, str] | None:
    from loopflow.lf.context import FilesetConfig

    merged_context = list(context_paths) if context_paths else []
    if extra_context:
        merged_context.extend(extra_context)

    components = gather_prompt_components(
        worktree_path,
        step=step_name,
        run_mode="auto",
        direction=direction,
        context_config=ContextConfig(
            files=FilesetConfig(paths=merged_context),
            wave=wave.name,
        ),
    )

    if not components.step:
        return None

    step_file, step_content = components.step
    direction_parts = [
        f"<lf:direction:{d.name}>\n{d.content}\n</lf:direction:{d.name}>" for d in direction
    ]
    direction_section = "\n\n".join(direction_parts)

    combined = f"{direction_section}\n\n---\n\n{step_content}"
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
    """Run a step via collector subprocess. Raises StepTimeoutError if step exceeds timeout."""
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
        notify_event("wave.error", {"error": f"Merge failed for {branch}: {error}"})
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
    wave: Wave,
    direction: list,
    worktree_path: Path,
    inline_text: str,
    context_paths: list[str] | None,
) -> str | None:
    from loopflow.lf.context import FilesetConfig

    components = gather_prompt_components(
        worktree_path,
        inline=inline_text,
        run_mode="auto",
        direction=direction,
        context_config=ContextConfig(
            files=FilesetConfig(paths=list(context_paths) if context_paths else []),
            wave=wave.name,
        ),
    )
    if not components.step:
        return None

    step_file, step_content = components.step
    direction_parts = [
        f"<lf:direction:{d.name}>\n{d.content}\n</lf:direction:{d.name}>" for d in direction
    ]
    direction_section = "\n\n".join(direction_parts)

    combined = f"{direction_section}\n\n---\n\n{step_content}"
    components = replace(components, step=(step_file, combined))
    return format_prompt(components)


def _run_fork_synthesize(
    wave: Wave,
    flow_name: str,
    worktree_path: Path,
    branch: str,
    fork: Fork,
    context_paths: list[str] | None,
    direction: list,
    skip_permissions: bool,
    backend: str,
    model_variant: str | None,
) -> int:
    results: list[ForkResult] = []
    base_commit = _git_rev_parse(worktree_path, "HEAD")

    def _run_fork_branch(fork_config: ForkThread, index: int) -> ForkResult:
        wt_name = f"fork-{flow_name}-{index}"
        try:
            wt_path = create_worktree(wave.repo, wt_name, base=branch)
        except Exception:
            return ForkResult(
                worktree=worktree_path,
                config=fork_config,
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

        fork_backend = backend
        fork_variant = model_variant
        if fork_config.model:
            fork_backend, fork_variant = parse_model(fork_config.model)

        fork_context = list(context_paths) if context_paths else []

        if fork_config.step:
            prompt_result = _build_loop_prompt(
                wave,
                direction,
                wt_path,
                fork_config.step,
                fork_context or None,
            )
            if not prompt_result:
                return ForkResult(
                    worktree=wt_path,
                    config=fork_config,
                    diff="",
                    status="failed",
                    scratch_notes="",
                )

            prompt, _step_file = prompt_result
            step_run_id = str(uuid.uuid4())
            step_label = f"{wave.area_display}:{fork_config.step}:fork-{index}"
            try:
                exit_code = _run_collector_step(
                    prompt,
                    wt_path,
                    fork_backend,
                    fork_variant,
                    skip_permissions,
                    step_run_id,
                    step_label,
                    autocommit=True,
                    prefix=f"[fork-{index}] ",
                )
            except StepTimeoutError as exc:
                return ForkResult(
                    worktree=wt_path,
                    config=fork_config,
                    diff="",
                    status=f"timeout: {exc}",
                    scratch_notes="",
                )

            diff = _run_git(wt_path, ["diff", f"{base_commit}..HEAD"])
            return ForkResult(
                worktree=wt_path,
                config=fork_config,
                diff=diff,
                status="completed" if exit_code == 0 else "failed",
                scratch_notes=_read_scratch_notes(wt_path),
            )

        if fork_config.flow:
            loaded_flow = load_flow(fork_config.flow, wave.repo)
            if not loaded_flow:
                return ForkResult(
                    worktree=wt_path,
                    config=fork_config,
                    diff="",
                    status="failed",
                    scratch_notes="",
                )
            exit_code = run_flow(
                loaded_flow,
                wt_path,
                context=fork_context or None,
                exclude=None,
                skip_permissions=skip_permissions,
                push_enabled=False,
                pr_enabled=False,
                backend=fork_backend,
                model_variant=fork_variant,
            )
            diff = _run_git(wt_path, ["diff", f"{base_commit}..HEAD"])
            return ForkResult(
                worktree=wt_path,
                config=fork_config,
                diff=diff,
                status="completed" if exit_code == 0 else "failed",
                scratch_notes=_read_scratch_notes(wt_path),
            )

        return ForkResult(
            worktree=wt_path,
            config=fork_config,
            diff="",
            status="failed",
            scratch_notes="",
        )

    with concurrent.futures.ThreadPoolExecutor(max_workers=len(fork.threads)) as executor:
        futures = [
            executor.submit(_run_fork_branch, config, i + 1)
            for i, config in enumerate(fork.threads)
        ]
        for future in futures:
            results.append(future.result())

    if not any(result.status == "completed" for result in results):
        _cleanup_fork_worktrees(wave.repo, results)
        return 1

    synth_prompt_override = fork.synthesize.prompt if fork.synthesize else None
    instructions = load_synthesize_instructions(wave.repo, synth_prompt_override)
    synth_prompt = build_synthesize_prompt(results, instructions, base_commit)

    # Add synthesize direction on top of wave direction
    synth_direction = list(direction)
    if fork.synthesize and fork.synthesize.direction:
        extra = resolve_directions(fork.synthesize.direction, wave.repo)
        synth_direction = synth_direction + extra

    synth_prompt = _build_loop_inline_prompt(
        wave,
        synth_direction,
        worktree_path,
        synth_prompt,
        context_paths,
    )
    if not synth_prompt:
        _cleanup_fork_worktrees(wave.repo, results)
        return 1

    try:
        exit_code = _run_collector_step(
            synth_prompt,
            worktree_path,
            backend,
            model_variant,
            skip_permissions,
            str(uuid.uuid4()),
            f"{wave.area_display}:synthesize",
            autocommit=True,
        )
    except StepTimeoutError:
        _cleanup_fork_worktrees(wave.repo, results)
        raise

    _cleanup_fork_worktrees(wave.repo, results)
    return exit_code


def run_iteration(
    wave: Wave,
    iteration: int,
    run_id: str | None = None,
) -> IterationResult:
    """Run a single iteration of a wave."""
    config = load_config(wave.repo)

    prefix = _iteration_branch_prefix(wave.main_branch)
    branch = f"{prefix}/{iteration:03d}"
    try:
        worktree_path = create_worktree(wave.repo, branch, base=wave.main_branch)
    except WorktreeError as e:
        error_msg = f"Failed to create worktree: {e}"
        notify_event("wave.error", {"wave_id": wave.id, "error": error_msg})
        return IterationResult(success=False)

    run = FlowRun(
        id=run_id or str(uuid.uuid4()),
        wave_id=wave.id,
        flow=wave.flow,
        direction=wave.direction,
        area=wave.area,
        repo=wave.repo,
        status=FlowRunStatus.RUNNING,
        iteration=iteration,
        worktree=str(worktree_path),
        branch=branch,
        started_at=datetime.now(),
    )
    save_run(run)

    notify_event(
        "wave.started",
        {
            "wave_id": wave.id,
            "area": wave.area_display,
            "direction": wave.direction_display,
            "flow": wave.flow,
            "iteration": iteration,
        },
    )

    direction = resolve_directions(wave.repo, wave.direction)
    # Direction is optional - proceed even if none specified

    flow = wave.flow
    if not flow:
        update_run_status(run.id, FlowRunStatus.FAILED, error="Flow is required")
        _cleanup_worktree(wave.repo, worktree_path, branch)
        return IterationResult(success=False)

    try:
        loaded_flow = load_flow(flow, wave.repo)
    except ValueError as exc:
        update_run_status(run.id, FlowRunStatus.FAILED, error=str(exc))
        _cleanup_worktree(wave.repo, worktree_path, branch)
        return IterationResult(success=False)

    if not loaded_flow:
        update_run_status(run.id, FlowRunStatus.FAILED, error=f"Unknown flow '{flow}'")
        _cleanup_worktree(wave.repo, worktree_path, branch)
        return IterationResult(success=False)

    items: list[Step | Fork | Choose] = list(loaded_flow.steps)
    if not items:
        update_run_status(run.id, FlowRunStatus.FAILED, error=f"Empty flow '{flow}'")
        _cleanup_worktree(wave.repo, worktree_path, branch)
        return IterationResult(success=False)

    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    runner = get_runner(backend)
    if not runner.is_available():
        update_run_status(run.id, FlowRunStatus.FAILED, error=f"'{backend}' CLI not found")
        return IterationResult(success=False)

    skip_permissions = config.yolo if config else False

    # Use wave's area as context paths
    context_paths = list(wave.area) if wave.area[0] != "." else None

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
                        "wave.step.started",
                        {
                            "wave_id": wave.id,
                            "step": step_name,
                            "iteration": iteration,
                        },
                    )

                    step_backend = backend
                    step_variant = model_variant
                    if step_def.model:
                        step_backend, step_variant = parse_model(step_def.model)

                    prompt_result = _build_loop_prompt(
                        wave,
                        direction,
                        worktree_path,
                        step_name,
                        context_paths,
                    )
                    if not prompt_result:
                        update_run_status(
                            run.id, FlowRunStatus.FAILED, error=f"Step file not found: {step_name}"
                        )
                        _cleanup_worktree(wave.repo, worktree_path, branch)
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
                            f"{wave.area_display}:{step_name}",
                        )
                    except StepTimeoutError as e:
                        notify_event(
                            "wave.step.completed",
                            {
                                "wave_id": wave.id,
                                "step": step_name,
                                "status": "timeout",
                            },
                        )
                        update_run_status(run.id, FlowRunStatus.FAILED, error=str(e))
                        _cleanup_worktree(wave.repo, worktree_path, branch)
                        return IterationResult(success=False)

                    notify_event(
                        "wave.step.completed",
                        {
                            "wave_id": wave.id,
                            "step": step_name,
                            "status": "completed" if result_code == 0 else "error",
                        },
                    )

                    if result_code != 0:
                        update_run_status(run.id, FlowRunStatus.FAILED, error=f"{step_name} failed")
                        _cleanup_worktree(wave.repo, worktree_path, branch)
                        return IterationResult(success=False)
                    continue

                base_branch = _current_branch(worktree_path) or branch
                futures = []
                results: list[tuple[Step, Path, int]] = []

                def _run_parallel(step_def: Step, index: int) -> tuple[Step, Path, int]:
                    wt_name = f"parallel-{branch.replace('/', '-')}-{step_def.name}-{index}"
                    wt_path = create_worktree(wave.repo, wt_name, base=base_branch)
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
                        wave,
                        direction,
                        wt_path,
                        step_def.name,
                        context_paths,
                    )
                    if not prompt_result:
                        return step_def, wt_path, 1

                    prompt, _step_file = prompt_result
                    step_run_id = str(uuid.uuid4())
                    step_label = f"{wave.area_display}:{step_def.name}:parallel"
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
                        remove_worktree(wave.repo, wt_path.name.split(".")[-1])
                    update_run_status(run.id, FlowRunStatus.FAILED, error="Parallel step failed")
                    _cleanup_worktree(wave.repo, worktree_path, branch)
                    return IterationResult(success=False)

                for _, wt_path, _ in results:
                    merge_branch = _current_branch(wt_path) or wt_path.name
                    if not _merge_branch(worktree_path, merge_branch):
                        for _, cleanup_path, _ in results:
                            remove_worktree(wave.repo, cleanup_path.name.split(".")[-1])
                        update_run_status(run.id, FlowRunStatus.FAILED, error="Merge failed")
                        _cleanup_worktree(wave.repo, worktree_path, branch)
                        return IterationResult(success=False)

                for _, wt_path, _ in results:
                    remove_worktree(wave.repo, wt_path.name.split(".")[-1])

            continue

        if isinstance(item, Fork):
            try:
                result_code = _run_fork_synthesize(
                    wave,
                    loaded_flow.name,
                    worktree_path,
                    branch,
                    item,
                    context_paths,
                    direction,
                    skip_permissions,
                    backend,
                    model_variant,
                )
            except StepTimeoutError as e:
                update_run_status(run.id, FlowRunStatus.FAILED, error=str(e))
                _cleanup_worktree(wave.repo, worktree_path, branch)
                return IterationResult(success=False)
            if result_code != 0:
                update_run_status(run.id, FlowRunStatus.FAILED, error="synthesize failed")
                _cleanup_worktree(wave.repo, worktree_path, branch)
                return IterationResult(success=False)

            i += 1
            continue

        if isinstance(item, Choose):
            try:
                choice = choose_branch(
                    item,
                    loaded_flow.name,
                    worktree_path,
                    backend,
                    model_variant,
                    skip_permissions,
                )
            except RuntimeError as exc:
                update_run_status(run.id, FlowRunStatus.FAILED, error=str(exc))
                _cleanup_worktree(wave.repo, worktree_path, branch)
                return IterationResult(success=False)

            items = items[:i] + item.options[choice] + items[i + 1 :]
            continue

        i += 1

    update_run_step(run.id, None)

    pr_url = _create_pr_to_main_branch(wave, worktree_path, branch, iteration)
    if pr_url:
        update_run_pr(run.id, pr_url)
        _auto_merge_pr(worktree_path)

        if wave.merge_mode.value == "land":
            _land_to_main(wave)

    update_run_status(run.id, FlowRunStatus.COMPLETED)

    notify_event(
        "wave.iteration.done",
        {
            "wave_id": wave.id,
            "area": wave.area_display,
            "direction": wave.direction_display,
            "flow": wave.flow,
            "iteration": iteration,
            "pr_url": pr_url,
        },
    )

    _cleanup_worktree(wave.repo, worktree_path, branch)

    return IterationResult(success=True, worktree=worktree_path, branch=branch)


# Tick-based flow execution for interactive steps


def tick_flow(flow_run_id: str) -> TickResult:
    """Advance a FlowRun by one step. Returns when paused at interactive or complete.

    This is the state machine executor for flows with interactive steps.
    Each call:
    1. Reads current position from DB (step_index)
    2. Checks if next step is interactive
    3. If interactive: creates WAITING StepRun, pauses
    4. If auto: executes step, advances position
    5. Returns result indicating next action
    """
    flow_run = get_run(flow_run_id)
    if not flow_run:
        return TickResult.STEP_FAILED

    if not flow_run.wave_id:
        return TickResult.STEP_FAILED

    wave = get_wave(flow_run.wave_id)
    if not wave:
        return TickResult.STEP_FAILED

    # Load flow definition
    try:
        loaded_flow = load_flow(flow_run.flow, wave.repo)
    except ValueError:
        update_run_status(flow_run.id, FlowRunStatus.FAILED, error="Flow not found")
        return TickResult.STEP_FAILED

    if not loaded_flow:
        update_run_status(flow_run.id, FlowRunStatus.FAILED, error="Flow not found")
        return TickResult.STEP_FAILED

    # Flatten to list for indexing (only handles Step items for now)
    items = [item for item in loaded_flow.steps if isinstance(item, Step)]

    if flow_run.step_index >= len(items):
        # All steps complete
        update_run_status(flow_run.id, FlowRunStatus.COMPLETED)
        return TickResult.FLOW_COMPLETE

    step_def = items[flow_run.step_index]
    step_name = step_def.name

    # Check if step is interactive via frontmatter
    worktree_path = Path(flow_run.worktree) if flow_run.worktree else None
    step_file = gather_step(worktree_path, step_name)
    is_interactive = step_file and step_file.config.interactive

    if is_interactive:
        # Create StepRun with WAITING status
        step_run = StepRun(
            id=str(uuid.uuid4()),
            step=step_name,
            repo=str(wave.repo),
            worktree=flow_run.worktree or str(wave.repo),
            flow_run_id=flow_run.id,
            wave_id=wave.id,
            status=StepRunStatus.WAITING,
            run_mode="interactive",
        )
        save_step_run(step_run)

        # Update wave status to WAITING
        update_wave_status(wave.id, WaveStatus.WAITING)

        # Update flow run current step
        update_run_step(flow_run.id, step_name)

        notify_event(
            "wave.waiting",
            {
                "wave_id": wave.id,
                "step": step_name,
                "step_run_id": step_run.id,
            },
        )

        return TickResult.WAITING_INTERACTIVE

    # Auto step - run it
    update_run_step(flow_run.id, step_name)

    config = load_config(wave.repo)
    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    if step_def.model:
        backend, model_variant = parse_model(step_def.model)

    skip_permissions = config.yolo if config else False
    context_paths = list(wave.area) if wave.area and wave.area[0] != "." else None
    direction = resolve_directions(wave.repo, wave.direction)

    prompt_result = _build_loop_prompt(
        wave,
        direction,
        Path(flow_run.worktree),
        step_name,
        context_paths,
    )

    if not prompt_result:
        update_run_status(flow_run.id, FlowRunStatus.FAILED, error=f"Step not found: {step_name}")
        return TickResult.STEP_FAILED

    prompt, _step_file = prompt_result

    try:
        result_code = _run_collector_step(
            prompt,
            Path(flow_run.worktree),
            backend,
            model_variant,
            skip_permissions,
            flow_run.id,
            f"{wave.area_display}:{step_name}",
        )
    except StepTimeoutError as e:
        update_run_status(flow_run.id, FlowRunStatus.FAILED, error=str(e))
        return TickResult.STEP_FAILED

    if result_code != 0:
        update_run_status(flow_run.id, FlowRunStatus.FAILED, error=f"{step_name} failed")
        return TickResult.STEP_FAILED

    # Advance step index
    update_flow_run_index(flow_run.id, flow_run.step_index + 1)

    notify_event(
        "wave.step.completed",
        {
            "wave_id": wave.id,
            "step": step_name,
            "status": "completed",
        },
    )

    return TickResult.STEP_COMPLETE


def _create_pr_to_main_branch(
    wave: Wave, worktree_path: Path, branch: str, iteration: int
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
        title = f"[{wave.area_slug}] {message.title}"
        body = (
            f"Wave: {wave.area_display} [{wave.direction_display}]\n"
            f"Flow: {wave.flow}\n"
            f"Iteration: {iteration}\n\n{message.body}"
        )
    except Exception:
        title = f"[{wave.area_slug}] Iteration {iteration}"
        body = (
            f"Wave: {wave.area_display} [{wave.direction_display}]\n"
            f"Flow: {wave.flow}\n"
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
        wave.main_branch,
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


def _land_to_main(wave: Wave) -> str | None:
    """Create or update PR from main_branch to main, enable auto-merge."""
    repo = wave.repo

    subprocess.run(["git", "push", "origin", wave.main_branch], cwd=repo, capture_output=True)

    result = subprocess.run(
        [
            "gh",
            "pr",
            "list",
            "--head",
            wave.main_branch,
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
            wave.main_branch,
            "--title",
            f"[{wave.area_slug}] Land accumulated work",
            "--body",
            f"Auto-land from wave: {wave.area_display} [{wave.direction_display}] "
            f"(flow: {wave.flow})",
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
