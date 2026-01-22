"""Core iteration runner for lfd.

Executes a single iteration of any trigger type (Loop, Subscription, Schedule).
"""

import json
import subprocess
import sys
import uuid
from dataclasses import replace
from datetime import datetime
from pathlib import Path

from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import format_prompt, gather_prompt_components
from loopflow.lf.flow import (
    build_join_prompt,
    choose_branch,
    collect_fork_diffs,
    format_voice_section,
    load_join_instructions,
)
from loopflow.lf.flows import (
    FlowDef,
    JoinConfig,
    ResolvedStep,
    load_flow,
    resolve_flow,
)
from loopflow.lf.goals import build_effective_goals, render_goals
from loopflow.lf.launcher import build_model_command, get_runner
from loopflow.lf.logging import write_prompt_file
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.worktrees import WorktreeError
from loopflow.lf.worktrees import create as create_worktree
from loopflow.lf.worktrees import remove as remove_worktree
from loopflow.lfd.daemon.client import notify_event
from loopflow.lfd.models import Loop, Run, RunStatus
from loopflow.lfd.runs.run import (
    save_run,
    update_run_pr,
    update_run_status,
    update_run_step,
)


def _iteration_branch_prefix(main_branch: str) -> str:
    """Derive iteration branch prefix from main branch."""
    if main_branch.endswith("-main"):
        return main_branch[:-5]
    return main_branch


def _build_loop_prompt(
    loop: Loop,
    effective_goals: list,
    worktree_path: Path,
    step_name: str,
    context_paths: list[str] | None,
    extra_context: list[str] | None = None,
    voices: list[str] | None = None,
) -> tuple[str, str] | None:
    merged_context = list(context_paths) if context_paths else []
    if extra_context:
        merged_context.extend(extra_context)

    components = gather_prompt_components(
        worktree_path,
        step=step_name,
        context=merged_context or None,
        run_mode="auto",
        voices=voices,
    )

    if not components.step:
        return None

    step_file, step_content = components.step
    goal_section = render_goals(effective_goals)

    combined = f"{goal_section}\n\n---\n\n{step_content}"
    components = replace(components, step=(step_file, combined))
    prompt = format_prompt(components)

    return prompt, step_file


def _run_collector_step(
    prompt: str,
    worktree_path: Path,
    backend: str,
    model_variant: str | None,
    skip_permissions: bool,
    session_id: str,
    step_label: str,
    autocommit: bool = True,
    prefix: str | None = None,
) -> int:
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
        "--session-id",
        session_id,
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
    result_code = process.wait()

    try:
        Path(prompt_file).unlink()
    except OSError:
        pass

    return result_code


class _VariantResult:
    def __init__(self, label: str, worktree: Path, exit_code: int, session_id: str):
        self.label = label
        self.worktree = worktree
        self.exit_code = exit_code
        self.session_id = session_id


def _cleanup_variant_worktrees(repo_root: Path, results: list[_VariantResult]) -> None:
    for r in results:
        remove_worktree(repo_root, r.worktree.name.split(".")[-1])


def _build_loop_inline_prompt(
    loop: Loop,
    effective_goals: list,
    worktree_path: Path,
    inline_text: str,
    context_paths: list[str] | None,
    voices: list[str] | None = None,
) -> str | None:
    components = gather_prompt_components(
        worktree_path,
        inline=inline_text,
        context=context_paths,
        run_mode="auto",
        voices=voices,
    )
    if not components.step:
        return None

    step_file, step_content = components.step
    goal_section = render_goals(effective_goals)

    combined = f"{goal_section}\n\n---\n\n{step_content}"
    components = replace(components, step=(step_file, combined))
    return format_prompt(components)


def _run_fork_join_group(
    loop: Loop,
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
        step_voices = None

        if step.config:
            if step.config.model:
                step_backend, step_variant = parse_model(step.config.model)
            if step.config.context:
                step_context.extend(step.config.context)
            if step.config.voice:
                step_voices = step.config.voice

        label_base = step.step
        label_counts[label_base] = label_counts.get(label_base, 0) + 1
        label = label_base
        if label_counts[label_base] > 1:
            label = f"{label_base}:{label_counts[label_base]}"

        wt_name = f"_fork-{branch.replace('/', '-')}-{label.replace(':', '-')}"
        try:
            wt_path = create_worktree(loop.repo, wt_name, base=branch)
        except Exception:
            _cleanup_variant_worktrees(loop.repo, results)
            return 1

        subprocess.run(
            ["git", "reset", "--hard", branch],
            cwd=wt_path,
            capture_output=True,
        )
        subprocess.run(["git", "clean", "-fd"], cwd=wt_path, capture_output=True)

        prompt_result = _build_loop_prompt(
            loop,
            effective_goals,
            wt_path,
            step.step,
            step_context or None,
            voices=step_voices,
        )
        if not prompt_result:
            remove_worktree(loop.repo, wt_path.name.split(".")[-1])
            return 1

        prompt, _step_file = prompt_result
        session_id = str(uuid.uuid4())
        step_label = f"{loop.area}:{step.step}:{label}"

        exit_code = _run_collector_step(
            prompt,
            wt_path,
            step_backend,
            step_variant,
            skip_permissions,
            session_id,
            step_label,
            autocommit=True,
            prefix=f"[{label}] ",
        )

        results.append(_VariantResult(label, wt_path, exit_code, session_id))

    successes = [r for r in results if r.exit_code == 0]
    if not successes:
        _cleanup_variant_worktrees(loop.repo, results)
        return 1

    fork_worktrees = [(r.label, r.worktree) for r in successes]
    join_prompt = build_join_prompt(
        collect_fork_diffs(fork_worktrees),
        load_join_instructions(join_config.step, loop.repo),
        format_voice_section(join_config.voice, loop.repo),
        flow_name,
    )
    join_prompt = _build_loop_inline_prompt(
        loop,
        effective_goals,
        worktree_path,
        join_prompt,
        context_paths,
        voices=None,
    )
    if not join_prompt:
        _cleanup_variant_worktrees(loop.repo, results)
        return 1

    join_backend = backend
    join_variant = model_variant
    if join_config.agent_model:
        join_backend, join_variant = parse_model(join_config.agent_model)

    join_result = _run_collector_step(
        join_prompt,
        worktree_path,
        join_backend,
        join_variant,
        skip_permissions,
        str(uuid.uuid4()),
        f"{loop.area}:join",
        autocommit=True,
    )

    _cleanup_variant_worktrees(loop.repo, results)
    return join_result


def run_iteration(
    loop: Loop,
    iteration: int,
    run_id: str | None = None,
    parent_type: str = "loop",
) -> bool:
    """Run a single iteration of a trigger.

    Works for any trigger type (Loop, Subscription, Schedule).
    Returns True if successful, False on error.
    """
    config = load_config(loop.repo)

    prefix = _iteration_branch_prefix(loop.main_branch)
    branch = f"{prefix}/{iteration:03d}"
    try:
        worktree_path = create_worktree(loop.repo, branch, base=loop.main_branch)
    except WorktreeError as e:
        notify_event("loop.error", {"loop_id": loop.id, "error": f"Failed to create worktree: {e}"})
        return False

    run = Run(
        id=run_id or str(uuid.uuid4()),
        parent=f"{parent_type}:{loop.id}",
        flow=loop.flow,
        area=loop.area,
        repo=loop.repo,
        goals=loop.goals,
        status=RunStatus.RUNNING,
        iteration=iteration,
        worktree=str(worktree_path),
        branch=branch,
        started_at=datetime.now(),
    )
    save_run(run)

    notify_event(
        "loop.started",
        {
            "loop_id": loop.id,
            "area": loop.area,
            "goals": loop.goals,
            "flow": loop.flow,
            "iteration": iteration,
        },
    )

    effective_goals = build_effective_goals(loop.repo, loop.goals)
    if not effective_goals:
        update_run_status(run.id, RunStatus.FAILED, error="No valid goals found")
        return False

    flow = loop.flow
    if not flow:
        update_run_status(run.id, RunStatus.FAILED, error="Flow is required")
        _cleanup_worktree(loop.repo, worktree_path, branch)
        return False

    try:
        flow_def = load_flow(flow, loop.repo)
    except ValueError as exc:
        update_run_status(run.id, RunStatus.FAILED, error=str(exc))
        _cleanup_worktree(loop.repo, worktree_path, branch)
        return False

    if not flow_def:
        update_run_status(run.id, RunStatus.FAILED, error=f"Unknown flow '{flow}'")
        _cleanup_worktree(loop.repo, worktree_path, branch)
        return False

    resolved = resolve_flow(flow_def, loop.repo)
    if not resolved:
        update_run_status(run.id, RunStatus.FAILED, error=f"Empty flow '{flow}'")
        _cleanup_worktree(loop.repo, worktree_path, branch)
        return False

    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    runner = get_runner(backend)
    if not runner.is_available():
        update_run_status(run.id, RunStatus.FAILED, error=f"'{backend}' CLI not found")
        return False

    skip_permissions = config.yolo if config else False

    context_paths = [loop.area] if loop.area != "." else None
    if not context_paths and effective_goals[0].area:
        context_paths = effective_goals[0].area

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
                    run.id, RunStatus.FAILED, error="Fork must be immediately followed by join"
                )
                _cleanup_worktree(loop.repo, worktree_path, branch)
                return False

            result_code = _run_fork_join_group(
                loop,
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
            if result_code != 0:
                update_run_status(run.id, RunStatus.FAILED, error="join failed")
                _cleanup_worktree(loop.repo, worktree_path, branch)
                return False

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
                update_run_status(run.id, RunStatus.FAILED, error=str(exc))
                _cleanup_worktree(loop.repo, worktree_path, branch)
                return False

            branch_steps = step.choose.options[choice]
            branch_flow = FlowDef.from_dict(f"{flow_def.name}:{choice}", {"steps": branch_steps})
            branch_resolved = resolve_flow(branch_flow, loop.repo)
            resolved = resolved[:i] + branch_resolved + resolved[i + 1 :]
            continue

        if step.join is not None:
            update_run_status(run.id, RunStatus.FAILED, error="Join must follow fork")
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

        if not step.step:
            i += 1
            continue

        step_name = step.step
        update_run_step(run.id, step_name)
        notify_event(
            "loop.step.started",
            {
                "loop_id": loop.id,
                "step": step_name,
                "iteration": iteration,
            },
        )

        step_backend = backend
        step_variant = model_variant
        step_context = list(context_paths) if context_paths else []
        step_voices = None

        if step.config:
            if step.config.model:
                step_backend, step_variant = parse_model(step.config.model)
            if step.config.context:
                step_context.extend(step.config.context)
            if step.config.voice:
                step_voices = step.config.voice

        prompt_result = _build_loop_prompt(
            loop,
            effective_goals,
            worktree_path,
            step_name,
            step_context or None,
            voices=step_voices,
        )
        if not prompt_result:
            update_run_status(run.id, RunStatus.FAILED, error=f"Step file not found: {step_name}")
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

        prompt, _step_file = prompt_result
        result_code = _run_collector_step(
            prompt,
            worktree_path,
            step_backend,
            step_variant,
            skip_permissions,
            run.id,
            f"{loop.area}:{step_name}",
        )

        notify_event(
            "loop.step.completed",
            {
                "loop_id": loop.id,
                "step": step_name,
                "status": "completed" if result_code == 0 else "error",
            },
        )

        if result_code != 0:
            update_run_status(run.id, RunStatus.FAILED, error=f"{step_name} failed")
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

        i += 1

    update_run_step(run.id, None)

    pr_url = _create_pr_to_main_branch(loop, worktree_path, branch, iteration)
    if pr_url:
        update_run_pr(run.id, pr_url)
        _auto_merge_pr(worktree_path)

        if loop.merge_mode.value == "land":
            _land_to_main(loop)

    update_run_status(run.id, RunStatus.COMPLETED)

    notify_event(
        "loop.iteration.done",
        {
            "loop_id": loop.id,
            "area": loop.area,
            "goals": loop.goals,
            "flow": loop.flow,
            "iteration": iteration,
            "pr_url": pr_url,
        },
    )

    _cleanup_worktree(loop.repo, worktree_path, branch)

    return True


def _create_pr_to_main_branch(
    loop: Loop, worktree_path: Path, branch: str, iteration: int
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
        title = f"[{loop.area_slug}] {message.title}"
        body = (
            f"Loop: {loop.area} [{loop.goals_display}]\n"
            f"Flow: {loop.flow_display}\n"
            f"Iteration: {iteration}\n\n{message.body}"
        )
    except Exception:
        title = f"[{loop.area_slug}] Iteration {iteration}"
        body = (
            f"Loop: {loop.area} [{loop.goals_display}]\n"
            f"Flow: {loop.flow_display}\n"
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
        loop.main_branch,
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
    """Create or update PR from main_branch to main, enable auto-merge."""
    repo = loop.repo

    subprocess.run(["git", "push", "origin", loop.main_branch], cwd=repo, capture_output=True)

    result = subprocess.run(
        [
            "gh",
            "pr",
            "list",
            "--head",
            loop.main_branch,
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
            loop.main_branch,
            "--title",
            f"[{loop.area_slug}] Land accumulated work",
            "--body",
            f"Auto-land from loop: {loop.area} [{loop.goals_display}] (flow: {loop.flow_display})",
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
