"""Pipeline execution for chaining tasks."""

import os
import platform
import subprocess
import sys
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Optional

from loopflow.config import parse_model
from loopflow.context import build_prompt
from loopflow.git import GitError, find_main_repo, open_pr
from loopflow.launcher import build_model_command, get_runner
from loopflow.lfd.pipelines import PipelineDef, RaceConfig, ResolvedStep, resolve_pipeline, StepConfig
from loopflow.llm_http import generate_pr_message
from loopflow.logging import write_prompt_file
from loopflow.lfd.client import log_session_start, log_session_end
from loopflow.lfd.models import Session, SessionStatus
from loopflow.worktrees import create as create_worktree, remove as remove_worktree


@dataclass
class _StepParams:
    """Parameters for executing a single pipeline step."""
    task: str
    backend: str
    model_variant: str | None
    context: list[str] | None
    voices: list[str] | None


def _run_step(
    params: _StepParams,
    repo_root: Path,
    main_repo: Path,
    exclude: list[str] | None,
    skip_permissions: bool,
    should_push: bool,
    step_num: int,
    total_steps: int,
) -> int:
    """Execute a single pipeline step. Returns exit code."""
    print(f"\n{'='*60}")
    print(f"[{step_num}/{total_steps}] {params.task}")
    print(f"{'='*60}\n")

    prompt = build_prompt(
        repo_root,
        params.task,
        context=params.context,
        exclude=exclude,
        run_mode="auto",
        voices=params.voices,
    )
    prompt_file = write_prompt_file(prompt)

    session = Session(
        id=str(uuid.uuid4()),
        task=params.task,
        repo=str(main_repo),
        worktree=str(repo_root),
        status=SessionStatus.RUNNING,
        started_at=datetime.now(),
        pid=None,
        model=params.backend,
        run_mode="auto",
    )
    log_session_start(session)

    command = build_model_command(
        params.backend,
        auto=True,
        stream=True,
        skip_permissions=skip_permissions,
        model_variant=params.model_variant,
        sandbox_root=repo_root.parent,
        workdir=repo_root,
    )
    collector_cmd = [
        sys.executable,
        "-m",
        "loopflow.lfd.collector",
        "--session-id",
        session.id,
        "--task",
        params.task,
        "--repo-root",
        str(repo_root),
        "--prompt-file",
        prompt_file,
        "--autocommit",
        "--foreground",
    ]
    if should_push:
        collector_cmd.append("--push")
    collector_cmd.extend(["--", *command])

    process = subprocess.Popen(collector_cmd, cwd=repo_root)
    result_code = process.wait()

    os.unlink(prompt_file)

    status = SessionStatus.COMPLETED if result_code == 0 else SessionStatus.ERROR
    log_session_end(session.id, status)

    if result_code != 0:
        print(f"\n[{params.task}] failed with exit code {result_code}")

    return result_code


def _finalize_pipeline(
    pipeline_name: str,
    repo_root: Path,
    should_pr: bool,
) -> None:
    """Handle post-pipeline tasks: PR creation and notification."""
    if should_pr:
        try:
            message = generate_pr_message(repo_root)
            pr_url = open_pr(repo_root, title=message.title, body=message.body)
            print(f"\nPR created: {pr_url}")
        except GitError as e:
            print(f"\nPR creation failed: {e}")

    _notify_done(pipeline_name)


def _notify_done(pipeline_name: str) -> None:
    """Show macOS notification. No-op on other platforms."""
    if platform.system() != "Darwin":
        return
    try:
        subprocess.run(
            ["osascript", "-e", f'display notification "Pipeline complete" with title "lf {pipeline_name}"'],
            capture_output=True,
        )
    except FileNotFoundError:
        pass


@dataclass
class _WorktreeTask:
    """A task to run in a temporary worktree."""
    task: str
    label: str  # Display label (task name or model name)
    wt_prefix: str  # Worktree name prefix (e.g., "_parallel" or "_race")
    backend: str
    model_variant: str | None
    context: list[str] | None
    voices: list[str] | None


@dataclass
class _WorktreeResult:
    """Result from running a task in a temporary worktree."""
    label: str
    worktree: Path
    exit_code: int
    session_id: str


def _run_worktree_tasks(
    tasks: list[_WorktreeTask],
    repo_root: Path,
    main_repo: Path,
    exclude: list[str] | None,
    skip_permissions: bool,
) -> list[_WorktreeResult]:
    """Run tasks in parallel temporary worktrees. Returns results for all tasks."""
    processes: list[tuple[_WorktreeTask, subprocess.Popen, Path, str, str]] = []

    for wt_task in tasks:
        label_short = wt_task.label.replace(":", "-")
        wt_name = f"{wt_task.wt_prefix}-{label_short}-{uuid.uuid4().hex[:8]}"
        try:
            wt_path = create_worktree(repo_root, wt_name)
        except Exception as e:
            print(f"[{wt_task.label}] Failed to create worktree: {e}")
            for _, proc, wt, _, _ in processes:
                proc.terminate()
                remove_worktree(repo_root, wt.name.split(".")[-1])
            return [_WorktreeResult(wt_task.label, repo_root, 1, "")]

        prompt = build_prompt(
            wt_path,
            wt_task.task,
            context=wt_task.context,
            exclude=exclude,
            run_mode="auto",
            voices=wt_task.voices,
        )
        prompt_file = write_prompt_file(prompt)

        session = Session(
            id=str(uuid.uuid4()),
            task=wt_task.task,
            repo=str(main_repo),
            worktree=str(wt_path),
            status=SessionStatus.RUNNING,
            started_at=datetime.now(),
            pid=None,
            model=wt_task.backend,
            run_mode="auto",
        )
        log_session_start(session)

        command = build_model_command(
            wt_task.backend,
            auto=True,
            stream=True,
            skip_permissions=skip_permissions,
            model_variant=wt_task.model_variant,
            sandbox_root=wt_path.parent,
            workdir=wt_path,
        )
        collector_cmd = [
            sys.executable, "-m", "loopflow.lfd.collector",
            "--session-id", session.id,
            "--task", wt_task.task,
            "--repo-root", str(wt_path),
            "--prompt-file", prompt_file,
            "--foreground",
            "--prefix", f"[{wt_task.label}] ",
            "--", *command,
        ]

        print(f"[{wt_task.label}] Starting in {wt_path.name}...")
        process = subprocess.Popen(collector_cmd, cwd=wt_path)
        processes.append((wt_task, process, wt_path, prompt_file, session.id))

    # Wait for all to complete
    results: list[_WorktreeResult] = []
    for wt_task, process, wt_path, prompt_file, session_id in processes:
        exit_code = process.wait()

        try:
            os.unlink(prompt_file)
        except OSError:
            pass

        status = SessionStatus.COMPLETED if exit_code == 0 else SessionStatus.ERROR
        log_session_end(session_id, status)

        results.append(_WorktreeResult(wt_task.label, wt_path, exit_code, session_id))

        if exit_code != 0:
            print(f"[{wt_task.label}] failed with exit code {exit_code}")
        else:
            print(f"[{wt_task.label}] completed successfully")

    return results


def _cleanup_worktrees(repo_root: Path, results: list[_WorktreeResult]) -> None:
    """Remove temporary worktrees from results."""
    for r in results:
        wt_name = r.worktree.name.split(".")[-1]
        remove_worktree(repo_root, wt_name)


def _run_parallel_group(
    steps: list[ResolvedStep],
    repo_root: Path,
    main_repo: Path,
    exclude: list[str] | None,
    skip_permissions: bool,
    should_push: bool,
    backend: str,
    model_variant: str | None,
    context: list[str] | None,
    group_num: int,
    total_groups: int,
) -> list[int]:
    """Run parallel steps in temporary worktrees. Returns list of exit codes."""
    task_names = [s.task for s in steps]
    print(f"\n{'='*60}")
    print(f"[{group_num}/{total_groups}] Parallel: {', '.join(task_names)}")
    print(f"{'='*60}\n")

    # Build worktree tasks from steps
    wt_tasks = []
    for step in steps:
        step_backend = backend
        step_variant = model_variant
        step_context = list(context) if context else []
        step_voices = None

        if step.config:
            if step.config.model:
                step_backend, step_variant = parse_model(step.config.model)
            if step.config.context:
                step_context.extend(step.config.context)
            if step.config.voice:
                step_voices = [step.config.voice]

        wt_tasks.append(_WorktreeTask(
            task=step.task,
            label=step.task,
            wt_prefix="_parallel",
            backend=step_backend,
            model_variant=step_variant,
            context=step_context or None,
            voices=step_voices,
        ))

    results = _run_worktree_tasks(wt_tasks, repo_root, main_repo, exclude, skip_permissions)
    _cleanup_worktrees(repo_root, results)
    return [r.exit_code for r in results]


def _run_race_step(
    task: str,
    race: RaceConfig,
    repo_root: Path,
    main_repo: Path,
    exclude: list[str] | None,
    skip_permissions: bool,
    context: list[str] | None,
    step_num: int,
    total_steps: int,
) -> int:
    """Run task with multiple models in parallel, judge and merge winner."""
    models = race.models
    print(f"\n{'='*60}")
    print(f"[{step_num}/{total_steps}] Race: {task} ({', '.join(models)})")
    print(f"{'='*60}\n")

    # Build worktree tasks from models
    wt_tasks = []
    for model in models:
        backend, model_variant = parse_model(model)
        step_context = list(context) if context else []

        wt_tasks.append(_WorktreeTask(
            task=task,
            label=model,
            wt_prefix=f"_race-{task}",
            backend=backend,
            model_variant=model_variant,
            context=step_context or None,
            voices=None,
        ))

    results = _run_worktree_tasks(wt_tasks, repo_root, main_repo, exclude, skip_permissions)

    # Filter to successful results
    successes = [r for r in results if r.exit_code == 0]

    if not successes:
        print("\nAll models failed, no winner to merge")
        _cleanup_worktrees(repo_root, results)
        return 1

    if len(successes) == 1:
        winner = successes[0]
        print(f"\nOnly {winner.label} succeeded, using it as winner")
    else:
        # Run judge to pick winner
        print(f"\n{len(successes)} models succeeded, running judge...")
        winner = _judge_race(successes, repo_root, race.judge, skip_permissions)
        if not winner:
            print("Judge failed to pick winner, using first successful model")
            winner = successes[0]
        else:
            print(f"Judge picked: {winner.label}")

    # Merge winner's changes into main worktree
    _merge_race_winner(repo_root, winner)

    # Clean up all temp worktrees
    _cleanup_worktrees(repo_root, results)

    return 0


def _judge_race(
    results: list[_WorktreeResult],
    repo_root: Path,
    judge_task: str,
    skip_permissions: bool,
) -> _WorktreeResult | None:
    """Run judge to pick winner from race results."""
    diffs = []
    for r in results:
        diff_result = subprocess.run(
            ["git", "diff", "HEAD~1", "--stat"],
            cwd=r.worktree,
            capture_output=True,
            text=True,
        )
        diff_text = diff_result.stdout if diff_result.returncode == 0 else "(no diff)"

        full_diff = subprocess.run(
            ["git", "diff", "HEAD~1"],
            cwd=r.worktree,
            capture_output=True,
            text=True,
        )
        diffs.append({
            "model": r.label,
            "worktree": str(r.worktree),
            "summary": diff_text,
            "diff": full_diff.stdout if full_diff.returncode == 0 else "",
        })

    judge_prompt = _build_judge_prompt(diffs)

    from loopflow.launcher import get_runner
    backend, variant = parse_model("claude:opus")
    runner = get_runner(backend)

    result = runner.launch(
        judge_prompt,
        auto=True,
        stream=False,
        skip_permissions=skip_permissions,
        cwd=repo_root,
    )

    if result.exit_code != 0:
        return None

    output = result.output or ""
    for r in results:
        if r.label in output:
            return r

    return None


def _build_judge_prompt(diffs: list[dict]) -> str:
    """Build prompt for judge to pick race winner."""
    lines = [
        "You are judging a model race. Multiple models attempted the same coding task.",
        "Compare their outputs and pick the best one.",
        "",
        "Criteria:",
        "- Correctness: Does the solution actually work?",
        "- Completeness: Did it address the full task?",
        "- Code quality: Is it clean, readable, well-structured?",
        "- Simplicity: Does it avoid over-engineering?",
        "",
        "Here are the submissions:",
        "",
    ]

    for i, d in enumerate(diffs, 1):
        lines.append(f"## Submission {i}: {d['model']}")
        lines.append("")
        lines.append("Summary of changes:")
        lines.append("```")
        lines.append(d["summary"])
        lines.append("```")
        lines.append("")
        lines.append("Full diff:")
        lines.append("```diff")
        # Truncate long diffs
        diff_lines = d["diff"].split("\n")
        if len(diff_lines) > 200:
            lines.extend(diff_lines[:200])
            lines.append(f"... ({len(diff_lines) - 200} more lines)")
        else:
            lines.append(d["diff"])
        lines.append("```")
        lines.append("")

    lines.extend([
        "## Your verdict",
        "",
        "Reply with ONLY the model name of the winner (e.g., 'claude:opus' or 'codex:o3').",
        "Do not explain your reasoning.",
    ])

    return "\n".join(lines)


def _merge_race_winner(repo_root: Path, winner: _WorktreeResult) -> None:
    """Merge winner's changes into main worktree."""
    result = subprocess.run(
        ["git", "diff", "--name-only", "HEAD~1"],
        cwd=winner.worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"Warning: Could not get changed files from {winner.label}")
        return

    changed_files = [f.strip() for f in result.stdout.strip().split("\n") if f.strip()]

    for file_path in changed_files:
        src = winner.worktree / file_path
        dst = repo_root / file_path

        if src.exists():
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes(src.read_bytes())
            print(f"  Copied: {file_path}")
        else:
            if dst.exists():
                dst.unlink()
                print(f"  Deleted: {file_path}")

    print(f"\nMerged {len(changed_files)} files from {winner.label}")


def _count_logical_steps(resolved: list[ResolvedStep]) -> int:
    """Count logical steps (parallel groups count as 1)."""
    count = 0
    seen_groups: set[int] = set()
    for step in resolved:
        if step.parallel_group is not None:
            if step.parallel_group not in seen_groups:
                seen_groups.add(step.parallel_group)
                count += 1
        else:
            count += 1
    return count


def run_pipeline_def(
    pipeline: PipelineDef,
    repo_root: Path,
    context: Optional[list[str]] = None,
    exclude: Optional[list[str]] = None,
    skip_permissions: bool = False,
    push_enabled: bool = False,
    pr_enabled: bool = False,
    backend: str = "claude",
    model_variant: str | None = "opus",
) -> int:
    """Run a PipelineDef (from .lf/pipelines/). Returns first non-zero exit code, or 0."""
    should_push = push_enabled
    should_pr = pr_enabled
    if should_pr:
        should_push = True

    runner = get_runner(backend)
    if not runner.is_available():
        print(f"Error: '{backend}' CLI not found")
        return 1

    main_repo = find_main_repo(repo_root) or repo_root
    resolved = resolve_pipeline(pipeline, repo_root)
    total = _count_logical_steps(resolved)

    i = 0
    step_num = 0
    while i < len(resolved):
        step = resolved[i]
        step_num += 1

        if step.parallel_group is not None:
            # Collect all steps in this parallel group
            group_steps = []
            group = step.parallel_group
            while i < len(resolved) and resolved[i].parallel_group == group:
                group_steps.append(resolved[i])
                i += 1

            # Run parallel group
            exit_codes = _run_parallel_group(
                group_steps,
                repo_root,
                main_repo,
                exclude,
                skip_permissions,
                should_push,
                backend,
                model_variant,
                context,
                step_num,
                total,
            )
            if any(code != 0 for code in exit_codes):
                return max(exit_codes)
        elif step.race is not None:
            # Race step: run task with multiple models
            step_context = list(context) if context else []
            if step.config and step.config.context:
                step_context.extend(step.config.context)

            result_code = _run_race_step(
                step.task,
                step.race,
                repo_root,
                main_repo,
                exclude,
                skip_permissions,
                step_context or None,
                step_num,
                total,
            )
            if result_code != 0:
                return result_code

            i += 1
        else:
            # Sequential step
            step_backend = backend
            step_variant = model_variant
            step_context = list(context) if context else []
            step_voices = None

            if step.config:
                if step.config.model:
                    step_backend, step_variant = parse_model(step.config.model)
                if step.config.context:
                    step_context.extend(step.config.context)
                if step.config.voice:
                    step_voices = [step.config.voice]

            params = _StepParams(
                task=step.task,
                backend=step_backend,
                model_variant=step_variant,
                context=step_context or None,
                voices=step_voices,
            )
            result_code = _run_step(
                params, repo_root, main_repo, exclude,
                skip_permissions, should_push, step_num, total,
            )
            if result_code != 0:
                return result_code

            i += 1

    _finalize_pipeline(pipeline.name, repo_root, should_pr)
    return 0
