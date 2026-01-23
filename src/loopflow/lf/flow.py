"""Flow execution for chaining steps."""

import os
import platform
import re
import subprocess
import sys
import uuid
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Optional

import yaml

from loopflow.lf.config import parse_model
from loopflow.lf.context import (
    ContextConfig,
    FilesetConfig,
    format_drop_label,
    format_prompt,
    gather_prompt_components,
    gather_step,
    trim_prompt_components,
)
from loopflow.lf.flows import Choose, FlowDef, JoinConfig, ResolvedStep, resolve_flow
from loopflow.lf.frontmatter import StepConfig
from loopflow.lf.git import GitError, find_main_repo, open_pr
from loopflow.lf.launcher import build_model_command, get_runner
from loopflow.lf.logging import write_prompt_file
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.tokens import MAX_SAFE_TOKENS, analyze_components
from loopflow.lf.voices import format_voice_section
from loopflow.lf.worktrees import create as create_worktree
from loopflow.lf.worktrees import remove as remove_worktree
from loopflow.lfd.models import StepRun, StepRunStatus
from loopflow.lfd.step_run import log_step_run_end, log_step_run_start


@dataclass
class _StepParams:
    """Parameters for executing a single flow step."""

    step: str
    backend: str
    model_variant: str | None
    context: list[str] | None
    voices: list[str] | None


def _build_step_params(
    step: str,
    config: StepConfig | None,
    backend: str,
    model_variant: str | None,
    context: list[str] | None,
) -> _StepParams:
    """Build step params by applying config overrides to defaults."""
    step_backend = backend
    step_variant = model_variant
    step_context = list(context) if context else []
    step_voices = None

    if config:
        if config.model:
            step_backend, step_variant = parse_model(config.model)
        if config.context:
            step_context.extend(config.context)
        if config.voice:
            step_voices = config.voice

    return _StepParams(
        step=step,
        backend=step_backend,
        model_variant=step_variant,
        context=step_context or None,
        voices=step_voices,
    )


def _run_step(
    params: _StepParams,
    repo_root: Path,
    main_repo: Path,
    exclude: list[str] | None,
    skip_permissions: bool,
    should_push: bool,
    step_num: int,
    total_steps: int,
    chrome: bool = False,
) -> int:
    """Execute a single flow step. Returns exit code."""
    print(f"\n{'=' * 60}")
    print(f"[{step_num}/{total_steps}] {params.step}")
    print(f"{'=' * 60}\n")

    components = gather_prompt_components(
        repo_root,
        params.step,
        run_mode="auto",
        voices=params.voices,
        context_config=ContextConfig(
            files=FilesetConfig(paths=params.context or [], exclude=exclude or [])
        ),
    )
    components, dropped = trim_prompt_components(components, MAX_SAFE_TOKENS)
    if dropped:
        dropped_summary = ", ".join(format_drop_label(item) for item in dropped)
        print(
            f"\033[33m⚠ Context trimmed to fit {MAX_SAFE_TOKENS:,} tokens. "
            f"Dropped: {dropped_summary}\033[0m"
        )
    tree = analyze_components(components)
    if tree.total() > MAX_SAFE_TOKENS:
        print(f"\033[33m⚠ Prompt is {tree.total():,} tokens (limit ~{MAX_SAFE_TOKENS:,})\033[0m")
    prompt = format_prompt(components)
    prompt_file = write_prompt_file(prompt)

    step_run = StepRun(
        id=str(uuid.uuid4()),
        step=params.step,
        repo=str(main_repo),
        worktree=str(repo_root),
        status=StepRunStatus.RUNNING,
        started_at=datetime.now(),
        pid=None,
        model=params.backend,
        run_mode="auto",
    )
    log_step_run_start(step_run)

    command = build_model_command(
        params.backend,
        auto=True,
        stream=True,
        skip_permissions=skip_permissions,
        yolo=skip_permissions,
        model_variant=params.model_variant,
        sandbox_root=repo_root.parent,
        workdir=repo_root,
        chrome=chrome,
    )
    collector_cmd = [
        sys.executable,
        "-m",
        "loopflow.lfd.execution.collector",
        "--step-run-id",
        step_run.id,
        "--step",
        params.step,
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

    status = StepRunStatus.COMPLETED if result_code == 0 else StepRunStatus.FAILED
    log_step_run_end(step_run.id, status)

    if result_code != 0:
        print(f"\n[{params.step}] failed with exit code {result_code}")

    return result_code


def _run_inline_prompt(
    prompt: str,
    step_label: str,
    repo_root: Path,
    main_repo: Path,
    backend: str,
    model_variant: str | None,
    skip_permissions: bool,
    chrome: bool = False,
) -> int:
    """Execute a prompt directly in the main worktree."""
    prompt_file = write_prompt_file(prompt)

    step_run = StepRun(
        id=str(uuid.uuid4()),
        step=step_label,
        repo=str(main_repo),
        worktree=str(repo_root),
        status=StepRunStatus.RUNNING,
        started_at=datetime.now(),
        pid=None,
        model=backend,
        run_mode="auto",
    )
    log_step_run_start(step_run)

    command = build_model_command(
        backend,
        auto=True,
        stream=True,
        skip_permissions=skip_permissions,
        yolo=skip_permissions,
        model_variant=model_variant,
        sandbox_root=repo_root.parent,
        workdir=repo_root,
        chrome=chrome,
    )
    collector_cmd = [
        sys.executable,
        "-m",
        "loopflow.lfd.execution.collector",
        "--step-run-id",
        step_run.id,
        "--step",
        step_label,
        "--repo-root",
        str(repo_root),
        "--prompt-file",
        prompt_file,
        "--autocommit",
        "--foreground",
        "--",
        *command,
    ]

    process = subprocess.Popen(collector_cmd, cwd=repo_root)
    result_code = process.wait()

    os.unlink(prompt_file)

    status = StepRunStatus.COMPLETED if result_code == 0 else StepRunStatus.FAILED
    log_step_run_end(step_run.id, status)

    if result_code != 0:
        print(f"\n[{step_label}] failed with exit code {result_code}")

    return result_code


def _finalize_flow(
    flow_name: str,
    repo_root: Path,
    should_pr: bool,
) -> None:
    """Handle post-flow tasks: PR creation and notification."""
    if should_pr:
        try:
            message = generate_pr_message(repo_root)
            pr_url = open_pr(repo_root, title=message.title, body=message.body)
            print(f"\nPR created: {pr_url}")
            subprocess.run(["open", pr_url])
        except GitError as e:
            print(f"\nPR creation failed: {e}")

    _notify_done(flow_name)


def _notify_done(flow_name: str) -> None:
    """Show macOS notification. No-op on other platforms."""
    if platform.system() != "Darwin":
        return
    try:
        notify_cmd = f'display notification "Flow complete" with title "lf {flow_name}"'
        subprocess.run(
            ["osascript", "-e", notify_cmd],
            capture_output=True,
        )
    except FileNotFoundError:
        pass


@dataclass
class _WorktreeTask:
    """A task to run in a temporary worktree."""

    step: str
    label: str  # Display label (step name or model name)
    wt_prefix: str  # Worktree name prefix (e.g., "_fork")
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
    chrome: bool = False,
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

        subprocess.run(["git", "reset", "--hard"], cwd=wt_path, capture_output=True)
        subprocess.run(["git", "clean", "-fd"], cwd=wt_path, capture_output=True)

        components = gather_prompt_components(
            wt_path,
            wt_task.step,
            run_mode="auto",
            voices=wt_task.voices,
            context_config=ContextConfig(
                files=FilesetConfig(paths=wt_task.context or [], exclude=exclude or [])
            ),
        )
        components, dropped = trim_prompt_components(components, MAX_SAFE_TOKENS)
        if dropped:
            dropped_summary = ", ".join(format_drop_label(item) for item in dropped)
            print(
                f"\033[33m⚠ Context trimmed to fit {MAX_SAFE_TOKENS:,} tokens. "
                f"Dropped: {dropped_summary}\033[0m"
            )
        tree = analyze_components(components)
        if tree.total() > MAX_SAFE_TOKENS:
            print(
                f"\033[33m⚠ Prompt is {tree.total():,} tokens (limit ~{MAX_SAFE_TOKENS:,})\033[0m"
            )
        prompt = format_prompt(components)
        prompt_file = write_prompt_file(prompt)

        step_run = StepRun(
            id=str(uuid.uuid4()),
            step=wt_task.step,
            repo=str(main_repo),
            worktree=str(wt_path),
            status=StepRunStatus.RUNNING,
            started_at=datetime.now(),
            pid=None,
            model=wt_task.backend,
            run_mode="auto",
        )
        log_step_run_start(step_run)

        command = build_model_command(
            wt_task.backend,
            auto=True,
            stream=True,
            skip_permissions=skip_permissions,
            yolo=skip_permissions,
            model_variant=wt_task.model_variant,
            sandbox_root=wt_path.parent,
            workdir=wt_path,
            chrome=chrome,
        )
        collector_cmd = [
            sys.executable,
            "-m",
            "loopflow.lfd.execution.collector",
            "--step-run-id",
            step_run.id,
            "--step",
            wt_task.step,
            "--repo-root",
            str(wt_path),
            "--prompt-file",
            prompt_file,
            "--foreground",
            "--prefix",
            f"[{wt_task.label}] ",
            "--",
            *command,
        ]

        print(f"[{wt_task.label}] Starting in {wt_path.name}...")
        process = subprocess.Popen(collector_cmd, cwd=wt_path)
        processes.append((wt_task, process, wt_path, prompt_file, step_run.id))

    # Wait for all to complete
    results: list[_WorktreeResult] = []
    for wt_task, process, wt_path, prompt_file, session_id in processes:
        exit_code = process.wait()

        try:
            os.unlink(prompt_file)
        except OSError:
            pass

        status = StepRunStatus.COMPLETED if exit_code == 0 else StepRunStatus.FAILED
        log_step_run_end(session_id, status)

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


def _run_fork_join_group(
    steps: list[ResolvedStep],
    join_config: JoinConfig,
    flow_name: str,
    repo_root: Path,
    main_repo: Path,
    exclude: list[str] | None,
    skip_permissions: bool,
    backend: str,
    model_variant: str | None,
    context: list[str] | None,
    group_num: int,
    total_groups: int,
    chrome: bool = False,
) -> int:
    """Run fork steps in temporary worktrees, then join results."""
    step_names = [s.step or "step" for s in steps]
    print(f"\n{'=' * 60}")
    print(f"[{group_num}/{total_groups}] Fork: {', '.join(step_names)}")
    print(f"{'=' * 60}\n")

    label_counts: dict[str, int] = {}
    wt_tasks = []
    for step in steps:
        params = _build_step_params(step.step, step.config, backend, model_variant, context)
        label_base = params.step or "step"
        label_counts[label_base] = label_counts.get(label_base, 0) + 1
        label = label_base
        if label_counts[label_base] > 1:
            label = f"{label_base}:{label_counts[label_base]}"

        wt_tasks.append(
            _WorktreeTask(
                step=params.step,
                label=label,
                wt_prefix="_fork",
                backend=params.backend,
                model_variant=params.model_variant,
                context=params.context,
                voices=params.voices,
            )
        )

    results = _run_worktree_tasks(
        wt_tasks, repo_root, main_repo, exclude, skip_permissions, chrome=chrome
    )
    successes = [r for r in results if r.exit_code == 0]

    if not successes:
        print("\nAll forked steps failed, nothing to join")
        _cleanup_worktrees(repo_root, results)
        return 1

    fork_worktrees = [(r.label, r.worktree) for r in successes]
    join_prompt = build_join_prompt(
        collect_fork_diffs(fork_worktrees),
        load_join_instructions(join_config.step, repo_root),
        format_voice_section(join_config.voice, repo_root),
        flow_name,
    )
    join_backend = backend
    join_variant = model_variant
    if join_config.agent_model:
        join_backend, join_variant = parse_model(join_config.agent_model)

    result_code = _run_inline_prompt(
        join_prompt,
        f"join:{flow_name}",
        repo_root,
        main_repo,
        join_backend,
        join_variant,
        skip_permissions,
        chrome=chrome,
    )
    _cleanup_worktrees(repo_root, results)

    return result_code


def load_join_instructions(step_name: str | None, repo_root: Path) -> str | None:
    """Load instructions for the join step."""
    name = step_name or "synthesize"
    step_file = gather_step(repo_root, name)
    if not step_file:
        return None

    return step_file.content.strip() or None


def _run_git(worktree: Path, args: list[str]) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    return result.stdout if result.returncode == 0 else ""


def collect_fork_diffs(worktrees: list[tuple[str, Path]]) -> list[dict]:
    """Collect diffs from forked worktrees. Takes list of (label, path) tuples."""
    diffs = []
    for label, worktree in worktrees:
        diff_text = _run_git(worktree, ["diff", "--stat"])
        full_diff = _run_git(worktree, ["diff"])
        if not diff_text.strip() and not full_diff.strip():
            diff_text = _run_git(worktree, ["show", "--stat", "--format=", "HEAD"])
            full_diff = _run_git(worktree, ["show", "--format=", "HEAD"])

        diffs.append(
            {
                "label": label,
                "worktree": str(worktree),
                "summary": diff_text.strip() or "(no diff)",
                "diff": full_diff,
            }
        )
    return diffs


def build_join_prompt(
    diffs: list[dict],
    instructions: str | None,
    voice_section: str | None,
    flow_name: str,
) -> str:
    """Build prompt for joining forked diffs on the main worktree."""
    lines = [
        "You are joining changes from multiple forked worktrees into the current worktree.",
        "Synthesize the best parts of all forks into a single changeset here.",
        "Do NOT edit the forked worktrees directly.",
        "After applying the changes, commit the result.",
        f"Write a short summary to scratch/joins/{flow_name}.md if that file makes sense.",
        "",
        "Forked worktrees:",
    ]

    for d in diffs:
        lines.append(f"- {d['label']}: {d['worktree']}")

    lines.extend(["", "Diffs from each fork:"])

    for i, d in enumerate(diffs, 1):
        lines.append(f"## Fork {i}: {d['label']}")
        lines.append("")
        lines.append("Summary of changes:")
        lines.append("```")
        lines.append(d["summary"])
        lines.append("```")
        lines.append("")
        lines.append("Full diff:")
        lines.append("```diff")
        diff_lines = d["diff"].split("\n")
        if len(diff_lines) > 200:
            lines.extend(diff_lines[:200])
            lines.append(f"... ({len(diff_lines) - 200} more lines)")
        else:
            lines.append(d["diff"])
        lines.append("```")
        lines.append("")

    if instructions:
        lines.extend(
            [
                "## Join instructions",
                instructions,
                "",
            ]
        )

    body = "\n".join(lines)
    if voice_section:
        return f"The voice.\n\n{voice_section}\n\n{body}"
    return body


def _count_logical_steps(resolved: list[ResolvedStep]) -> int:
    """Count logical steps (fork groups count as 1)."""
    count = 0
    seen_groups: set[int] = set()
    i = 0
    while i < len(resolved):
        step = resolved[i]
        if step.parallel_group is not None:
            group = step.parallel_group
            if group not in seen_groups:
                seen_groups.add(group)
                count += 1
            while i < len(resolved) and resolved[i].parallel_group == group:
                i += 1
            if i < len(resolved) and resolved[i].join is not None:
                i += 1
            continue
        if step.join is not None:
            count += 1
            i += 1
            continue
        count += 1
        i += 1
    return count


def _parse_choice(path: Path) -> tuple[str | None, str | None]:
    if not path.exists():
        return None, None

    text = path.read_text()
    match = re.match(r"^---\n(.*?)\n---\n?", text, re.DOTALL)
    if not match:
        return None, None

    data = yaml.safe_load(match.group(1)) or {}
    return data.get("choice"), data.get("reason")


def _build_choose_prompt(
    flow_name: str,
    options: dict[str, list],
    output_path: Path,
    override: str | None,
) -> str:
    if override:
        return override

    lines = [
        "You are choosing which branch to run in a flow.",
        f"Flow: {flow_name}",
        "",
        "Available options:",
    ]
    for key, steps in options.items():
        steps_str = ", ".join(
            s if isinstance(s, str) else getattr(s, "step", str(s)) for s in steps
        )
        lines.append(f"- {key}: {steps_str}")

    lines.extend(
        [
            "",
            "Decide which option to run based on repository state.",
            "Inspect roadmap/roadmap and .design as needed.",
            "",
            f"Write your decision to {output_path} with this frontmatter:",
            "---",
            "choice: <option>",
            "reason: <short explanation>",
            "options: [<option>, <option>]",
            "---",
            "",
            "Then include a short explanation in the body.",
        ]
    )
    return "\n".join(lines)


def choose_branch(
    choose: Choose,
    flow_name: str,
    repo_root: Path,
    backend: str,
    model_variant: str | None,
    skip_permissions: bool,
) -> str:
    """Run a choose step and return the selected branch name."""
    output_path = Path(choose.output or f"scratch/choices/{flow_name}.md")
    if not output_path.is_absolute():
        output_path = repo_root / output_path
    output_path.parent.mkdir(parents=True, exist_ok=True)

    prompt = _build_choose_prompt(flow_name, choose.options, output_path, choose.prompt)

    runner = get_runner(backend)
    result = runner.launch(
        prompt,
        auto=True,
        stream=False,
        skip_permissions=skip_permissions,
        model_variant=model_variant,
        cwd=repo_root,
    )
    if result.exit_code != 0:
        raise RuntimeError("choose failed to run")

    choice, _reason = _parse_choice(output_path)
    if not choice or choice not in choose.options:
        raise RuntimeError(f"choose wrote invalid choice to {output_path}")

    return choice


def run_flow_def(
    flow: FlowDef,
    repo_root: Path,
    context: Optional[list[str]] = None,
    exclude: Optional[list[str]] = None,
    skip_permissions: bool = False,
    push_enabled: bool = False,
    pr_enabled: bool = False,
    backend: str = "claude",
    model_variant: str | None = "opus",
    chrome: bool = False,
) -> int:
    """Run a FlowDef (from .lf/flows/). Returns first non-zero exit code, or 0."""
    should_push = push_enabled
    should_pr = pr_enabled
    if should_pr:
        should_push = True

    runner = get_runner(backend)
    if not runner.is_available():
        print(f"Error: '{backend}' CLI not found")
        return 1

    main_repo = find_main_repo(repo_root) or repo_root
    resolved = resolve_flow(flow, repo_root)
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
            if i >= len(resolved) or resolved[i].join is None:
                print("Error: fork must be immediately followed by join")
                return 1

            join_config = resolved[i].join.join
            result_code = _run_fork_join_group(
                group_steps,
                join_config,
                flow.name,
                repo_root,
                main_repo,
                exclude,
                skip_permissions,
                backend,
                model_variant,
                context,
                step_num,
                total,
                chrome=chrome,
            )
            if result_code != 0:
                return result_code

            i += 1
            continue
        elif step.choose is not None:
            choice = choose_branch(
                step.choose,
                flow.name,
                repo_root,
                backend,
                model_variant,
                skip_permissions,
            )
            branch_steps = step.choose.options[choice]
            branch_flow = FlowDef.from_dict(f"{flow.name}:{choice}", {"steps": branch_steps})
            branch_resolved = resolve_flow(branch_flow, repo_root)
            resolved = resolved[:i] + branch_resolved + resolved[i + 1 :]
            total = _count_logical_steps(resolved)
            continue
        elif step.join is not None:
            print("Error: join must follow fork")
            return 1
        else:
            # Sequential step
            params = _build_step_params(step.step, step.config, backend, model_variant, context)
            result_code = _run_step(
                params,
                repo_root,
                main_repo,
                exclude,
                skip_permissions,
                should_push,
                step_num,
                total,
                chrome=chrome,
            )
            if result_code != 0:
                return result_code

            i += 1

    _finalize_flow(flow.name, repo_root, should_pr)
    return 0
