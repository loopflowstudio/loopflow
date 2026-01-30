"""Flow execution for chaining steps."""

import concurrent.futures
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

from loopflow.lf.config import Config, load_config, parse_model
from loopflow.lf.context import (
    ContextConfig,
    FilesetConfig,
    format_drop_label,
    format_prompt,
    gather_prompt_components,
    gather_step,
    trim_prompt_components,
)
from loopflow.lf.execution import ExecutionParams, execute_step
from loopflow.lf.flows import (
    Choose,
    Flow,
    Fork,
    ForkThread,
    LoopUntilEmpty,
    Step,
    StepDAG,
    SynthesizeConfig,
    build_step_dag,
    load_flow,
)
from loopflow.lf.git import GitError, find_main_repo, open_pr
from loopflow.lf.launcher import build_model_command, get_runner
from loopflow.lf.logging import write_prompt_file
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.tokens import MAX_SAFE_TOKENS, analyze_components
from loopflow.lf.worktrees import create as create_worktree
from loopflow.lf.worktrees import remove as remove_worktree
from loopflow.lfd.models import StepRun, StepRunStatus
from loopflow.lfd.step_run import log_step_run_end, log_step_run_start

FlowItem = Step | Fork | Choose | LoopUntilEmpty


@dataclass
class _StepParams:
    """Parameters for executing a single flow step."""

    step: str
    backend: str
    model_variant: str | None
    context: list[str] | None
    direction: list[str] | None


def _build_step_params(
    step: Step,
    backend: str,
    model_variant: str | None,
    context: list[str] | None,
    flow_direction: list[str] | None,
) -> _StepParams:
    """Build step params by applying overrides to defaults."""
    step_backend = backend
    step_variant = model_variant
    step_context = list(context) if context else []

    # Step direction overrides flow direction
    direction = step.direction if step.direction else flow_direction

    if step.model:
        step_backend, step_variant = parse_model(step.model)

    return _StepParams(
        step=step.name,
        backend=step_backend,
        model_variant=step_variant,
        context=step_context or None,
        direction=direction,
    )


def _is_step_interactive(step: Step, repo_root: Path, config: Config | None) -> bool:
    """Check if step should run interactively.

    Priority: flow step override > frontmatter > default (False)
    """
    if step.interactive is not None:
        return step.interactive
    step_file = gather_step(repo_root, step.name, config)
    if step_file and step_file.config.interactive is not None:
        return step_file.config.interactive
    return False


def _run_interactive_step(
    params: _StepParams,
    repo_root: Path,
    main_repo: Path,
    exclude: list[str] | None,
    skip_permissions: bool,
    step_num: int,
    total_steps: int,
    chrome: bool = False,
) -> int:
    """Run step interactively using unified execution."""
    components = gather_prompt_components(
        repo_root,
        params.step,
        run_mode="interactive",
        direction=params.direction,
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

    return execute_step(
        ExecutionParams(
            step_name=params.step,
            repo_root=repo_root,
            components=components,
            backend=params.backend,
            model_variant=params.model_variant,
            skip_permissions=skip_permissions,
            chrome=chrome,
            is_interactive=True,
            use_execvp=False,  # Flow needs subprocess.run to continue
            step_num=step_num,
            total_steps=total_steps,
            direction=params.direction,
            context=params.context,
            autocommit=False,  # Flow handles autocommit
        )
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
    """Execute a single flow step using unified execution."""
    components = gather_prompt_components(
        repo_root,
        params.step,
        run_mode="auto",
        direction=params.direction,
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

    return execute_step(
        ExecutionParams(
            step_name=params.step,
            repo_root=repo_root,
            components=components,
            backend=params.backend,
            model_variant=params.model_variant,
            skip_permissions=skip_permissions,
            chrome=chrome,
            is_interactive=False,
            step_num=step_num,
            total_steps=total_steps,
            direction=params.direction,
            context=params.context,
            autocommit=True,
            push=should_push,
        )
    )


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
    """Execute a prompt directly in the main worktree using unified execution."""
    from loopflow.lf.context import PromptComponents

    # Create minimal components for inline prompt
    components = PromptComponents(
        run_mode="auto",
        docs=[],
        diff=None,
        diff_files=[],
        step=(step_label, prompt),
        repo_root=repo_root,
    )

    return execute_step(
        ExecutionParams(
            step_name=step_label,
            repo_root=repo_root,
            components=components,
            backend=backend,
            model_variant=model_variant,
            skip_permissions=skip_permissions,
            chrome=chrome,
            is_interactive=False,
            autocommit=True,
        )
    )


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
    direction: list[str] | None


@dataclass
class _WorktreeResult:
    """Result from running a task in a temporary worktree."""

    label: str
    worktree: Path
    branch: str
    exit_code: int
    session_id: str


def _run_worktree_tasks(
    tasks: list[_WorktreeTask],
    repo_root: Path,
    base_branch: str,
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
            wt_path = create_worktree(repo_root, wt_name, base=base_branch)
        except Exception as e:
            print(f"[{wt_task.label}] Failed to create worktree: {e}")
            for _, proc, wt, _, _ in processes:
                proc.terminate()
                remove_worktree(repo_root, wt.name.split(".")[-1])
            return [_WorktreeResult(wt_task.label, repo_root, "", 1, "")]

        subprocess.run(
            ["git", "reset", "--hard", base_branch],
            cwd=wt_path,
            capture_output=True,
        )
        subprocess.run(["git", "clean", "-fd"], cwd=wt_path, capture_output=True)

        components = gather_prompt_components(
            wt_path,
            wt_task.step,
            run_mode="auto",
            direction=wt_task.direction,
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

        branch = _current_branch(wt_path) or wt_path.name
        results.append(_WorktreeResult(wt_task.label, wt_path, branch, exit_code, session_id))

        if exit_code != 0:
            print(f"[{wt_task.label}] failed with exit code {exit_code}")
        else:
            print(f"[{wt_task.label}] completed successfully")

    return results


def _cleanup_worktrees(repo_root: Path, results: list[_WorktreeResult]) -> None:
    """Remove temporary worktrees from results."""
    for r in results:
        if r.worktree == repo_root or not r.branch:
            continue
        wt_name = r.worktree.name.split(".")[-1]
        remove_worktree(repo_root, wt_name)


@dataclass
class ForkResult:
    worktree: Path
    config: ForkThread
    diff: str
    status: str
    scratch_notes: str


def run_fork(
    fork: Fork,
    base_commit: str,
    parent_worktree: Path,
    flow_name: str,
    main_repo: Path,
    exclude: list[str] | None,
    skip_permissions: bool,
    backend: str,
    model_variant: str | None,
    context: list[str] | None,
    chrome: bool = False,
) -> list[ForkResult]:
    """Create worktrees from base_commit, run each thread in parallel, return results."""
    base_branch = _current_branch(parent_worktree) or "HEAD"
    results: list[ForkResult] = []

    def _run_thread(thread: ForkThread, index: int) -> tuple[ForkThread, Path, int]:
        label = f"fork-{flow_name}-{index}-{uuid.uuid4().hex[:8]}"
        try:
            wt_path = create_worktree(parent_worktree, label, base=base_branch)
        except Exception as exc:
            print(f"[{label}] Failed to create worktree: {exc}")
            return thread, parent_worktree, 1

        subprocess.run(
            ["git", "reset", "--hard", base_branch],
            cwd=wt_path,
            capture_output=True,
        )
        subprocess.run(["git", "clean", "-fd"], cwd=wt_path, capture_output=True)

        # Use thread's step, or fall back to fork-level step
        step_name = thread.step or fork.step
        if step_name:
            step = Step(
                name=step_name,
                model=thread.model or fork.model,
                direction=thread.direction,
            )
            params = _build_step_params(step, backend, model_variant, context, None)
            exit_code = _run_step(
                params,
                wt_path,
                main_repo,
                exclude,
                skip_permissions,
                False,
                index,
                len(fork.threads),
                chrome=chrome,
            )
            return thread, wt_path, exit_code

        if thread.flow:
            loaded_flow = load_flow(thread.flow, parent_worktree)
            if not loaded_flow:
                print(f"[{label}] Unknown flow: {thread.flow}")
                return thread, wt_path, 1
            exit_code = run_flow(
                loaded_flow,
                wt_path,
                context=context,
                exclude=exclude,
                skip_permissions=skip_permissions,
                push_enabled=False,
                pr_enabled=False,
                backend=backend,
                model_variant=model_variant,
                chrome=chrome,
            )
            return thread, wt_path, exit_code

        print(f"[{label}] Fork thread must set step or flow")
        return thread, wt_path, 1

    futures = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=len(fork.threads)) as executor:
        for index, thread in enumerate(fork.threads, 1):
            futures.append(executor.submit(_run_thread, thread, index))
        for future in futures:
            thread, wt_path, exit_code = future.result()
            diff = _run_git(wt_path, ["diff", f"{base_commit}..HEAD"])
            results.append(
                ForkResult(
                    worktree=wt_path,
                    config=thread,
                    diff=diff,
                    status="completed" if exit_code == 0 else "failed",
                    scratch_notes=_read_scratch_notes(wt_path),
                )
            )

    return results


def run_synthesize(
    synth: SynthesizeConfig | None,
    fork_results: list[ForkResult],
    base_commit: str,
    target_worktree: Path,
    main_repo: Path,
    skip_permissions: bool,
    backend: str,
    model_variant: str | None,
    chrome: bool = False,
) -> int:
    """Review fork diffs against base, write unified result + analysis to target."""
    synth_prompt = synth.prompt if synth else None
    prompt = build_synthesize_prompt(
        fork_results,
        load_synthesize_instructions(target_worktree, synth_prompt),
        base_commit,
    )
    return _run_inline_prompt(
        prompt,
        "synthesize",
        target_worktree,
        main_repo,
        backend,
        model_variant,
        skip_permissions,
        chrome=chrome,
    )


def cleanup_fork_worktrees(results: list[ForkResult], parent_worktree: Path) -> None:
    """Delete temporary fork worktrees after synthesis."""
    for result in results:
        wt_name = result.worktree.name.split(".")[-1]
        remove_worktree(parent_worktree, wt_name)


def load_synthesize_instructions(
    repo_root: Path,
    prompt_override: str | None,
) -> str | None:
    """Load instructions for the synthesizer step."""
    if prompt_override:
        return prompt_override.strip()

    step_file = gather_step(repo_root, "synthesize")
    if not step_file:
        return None

    return step_file.content.strip() or None


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


def _get_wave_name(repo_root: Path, explicit_wave: str | None) -> str:
    """Get wave name from explicit value, or derive from worktree/branch."""
    if explicit_wave:
        return explicit_wave
    # Derive from worktree directory name (e.g., loopflow.lfflow -> lfflow)
    dir_name = repo_root.name
    if "." in dir_name:
        return dir_name.split(".")[-1]
    # Fall back to branch name
    branch = _current_branch(repo_root)
    if branch:
        return branch
    return "default"


def _is_wave_empty(repo_root: Path, wave: str) -> bool:
    """Check if a wave's backlog is empty (no items in roadmap/<wave>/)."""
    roadmap_dir = repo_root / "roadmap" / wave
    if not roadmap_dir.exists():
        return True
    items = list(roadmap_dir.glob("*.md"))
    return len(items) == 0


def _run_git(worktree: Path, args: list[str]) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    return result.stdout if result.returncode == 0 else ""


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
        print(f"Merge failed for {branch}: {error}")
        return False
    return True


def build_synthesize_prompt(
    results: list[ForkResult],
    instructions: str | None,
    base_commit: str,
) -> str:
    """Build prompt for synthesizing fork results on the main worktree."""
    lines = [
        "You have multiple implementations of the same task from forked agents.",
        "Analyze differences, write synthesis notes, then produce a unified result here.",
        "Write analysis to scratch/synthesis.md before applying code changes.",
        "Do NOT edit the forked worktrees directly.",
        "",
        "## Fork Results",
    ]

    for index, result in enumerate(results, 1):
        config = result.config
        lines.append(f"### Fork {index}")
        lines.append(
            "Config: "
            f"step={config.step}, flow={config.flow}, direction={config.direction}, "
            f"model={config.model}, area={config.area}"
        )
        lines.append(f"Status: {result.status}")
        lines.append(f"Worktree: {result.worktree}")
        lines.append("")
        if result.scratch_notes:
            lines.append("Scratch notes:")
            lines.append("```")
            lines.append(result.scratch_notes)
            lines.append("```")
            lines.append("")
        lines.append("Diff:")
        lines.append("```diff")
        diff_lines = result.diff.split("\n")
        if len(diff_lines) > 200:
            lines.extend(diff_lines[:200])
            lines.append(f"... ({len(diff_lines) - 200} more lines)")
        else:
            lines.append(result.diff)
        lines.append("```")
        lines.append("")

    if instructions:
        lines.extend(
            [
                "## Instructions",
                instructions,
                "",
            ]
        )

    lines.append(f"(Base commit: {base_commit})")

    return "\n".join(lines)


def topological_batches(dag: StepDAG) -> list[list[Step]]:
    """Return batches of steps that can run in parallel."""
    remaining = set(dag.steps.keys())
    deps = {name: set(values) for name, values in dag.dependencies.items()}
    order_index = {name: index for index, name in enumerate(dag.order)}
    batches: list[list[Step]] = []

    while remaining:
        ready = [name for name in remaining if not deps[name]]
        if not ready:
            raise ValueError("Cycle detected in flow steps")
        ready.sort(key=order_index.get)
        batches.append([dag.steps[name] for name in ready])
        for name in ready:
            remaining.remove(name)
            for dep_set in deps.values():
                dep_set.discard(name)

    return batches


def _count_logical_steps(items: list[FlowItem]) -> int:
    """Count logical steps (parallel batches count as 1)."""
    count = 0
    i = 0
    while i < len(items):
        item = items[i]
        if isinstance(item, Step):
            phase = []
            while i < len(items) and isinstance(items[i], Step):
                phase.append(items[i])
                i += 1
            dag = build_step_dag(phase)
            count += len(topological_batches(dag))
            continue
        count += 1
        i += 1
    return count


def _format_flow_outline(items: list[FlowItem]) -> str:
    """Format a compact flow outline like: review → fork(reduce×3) → publish"""
    parts = []
    for item in items:
        if isinstance(item, Step):
            parts.append(item.name)
        elif isinstance(item, Fork):
            step_name = item.step or "?"
            parts.append(f"fork({step_name}×{len(item.threads)})")
        elif isinstance(item, Choose):
            opts = list(item.options.keys())
            parts.append(f"choose({'/'.join(opts[:3])})")
        elif isinstance(item, LoopUntilEmpty):
            parts.append(f"loop({item.source})")
        else:
            parts.append("?")
    return " → ".join(parts)


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
            s if isinstance(s, str) else getattr(s, "name", str(s)) for s in steps
        )
        lines.append(f"- {key}: {steps_str}")

    lines.extend(
        [
            "",
            "Decide which option to run based on repository state.",
            "Inspect reports/ and scratch/ as needed.",
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


def run_flow(
    flow: Flow,
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
    """Run a Flow (from .lf/flows/). Returns first non-zero exit code, or 0."""
    should_push = push_enabled
    should_pr = pr_enabled
    if should_pr:
        should_push = True

    runner = get_runner(backend)
    if not runner.is_available():
        print(f"Error: '{backend}' coding agent not found")
        return 1

    # Display engine header
    engine_display = f"{backend}:{model_variant}" if model_variant else backend
    print(f"\033[90mengine: {engine_display}\033[0m")

    config = load_config(repo_root)
    main_repo = find_main_repo(repo_root) or repo_root
    items: list[FlowItem] = list(flow.steps)
    total = _count_logical_steps(items)

    # Flow overview
    model_str = f"{backend}:{model_variant}" if model_variant else backend
    print(f"\n\033[1;34m{'═' * 60}\033[0m")
    print(f"\033[1;34m  FLOW: {flow.name}\033[0m")
    print(f"\033[1;34m{'═' * 60}\033[0m")
    print(f"\033[90mmodel={model_str} | steps={total}\033[0m")
    print(f"\033[90m{_format_flow_outline(items)}\033[0m")
    print(f"\033[1;34m{'═' * 60}\033[0m")

    i = 0
    step_num = 0
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
                step_num += 1
                if len(batch) == 1:
                    step = batch[0]
                    params = _build_step_params(step, backend, model_variant, context, None)

                    # Check if step should run interactively
                    if _is_step_interactive(step, repo_root, config):
                        result_code = _run_interactive_step(
                            params,
                            repo_root,
                            main_repo,
                            exclude,
                            skip_permissions,
                            step_num,
                            total,
                            chrome=chrome,
                        )
                    else:
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
                    continue

                base_branch = _current_branch(repo_root) or "HEAD"
                wt_tasks = []
                for step in batch:
                    params = _build_step_params(step, backend, model_variant, context, None)
                    wt_tasks.append(
                        _WorktreeTask(
                            step=params.step,
                            label=params.step,
                            wt_prefix="_parallel",
                            backend=params.backend,
                            model_variant=params.model_variant,
                            context=params.context,
                            direction=params.direction,
                        )
                    )

                results = _run_worktree_tasks(
                    wt_tasks,
                    repo_root,
                    base_branch,
                    main_repo,
                    exclude,
                    skip_permissions,
                    chrome=chrome,
                )
                if any(r.exit_code != 0 for r in results):
                    _cleanup_worktrees(repo_root, results)
                    return 1

                for result in results:
                    if not _merge_branch(repo_root, result.branch):
                        _cleanup_worktrees(repo_root, results)
                        return 1

                _cleanup_worktrees(repo_root, results)

            continue

        if isinstance(item, Fork):
            step_num += 1
            base_commit = _git_rev_parse(repo_root, "HEAD")
            fork_results = run_fork(
                item,
                base_commit,
                repo_root,
                flow.name,
                main_repo,
                exclude,
                skip_permissions,
                backend,
                model_variant,
                context,
                chrome=chrome,
            )
            if not fork_results or not any(result.status == "completed" for result in fork_results):
                cleanup_fork_worktrees(fork_results, repo_root)
                return 1

            result_code = run_synthesize(
                item.synthesize,
                fork_results,
                base_commit,
                repo_root,
                main_repo,
                skip_permissions,
                backend,
                model_variant,
                chrome=chrome,
            )
            cleanup_fork_worktrees(fork_results, repo_root)

            if result_code != 0:
                return result_code

            i += 1
            continue

        if isinstance(item, Choose):
            choice = choose_branch(
                item,
                flow.name,
                repo_root,
                backend,
                model_variant,
                skip_permissions,
            )
            branch_steps = item.options[choice]
            items = items[:i] + branch_steps + items[i + 1 :]
            total = _count_logical_steps(items)
            continue

        if isinstance(item, LoopUntilEmpty):
            wave_name = _get_wave_name(repo_root, item.wave)
            iteration = 0
            print(f"\n\033[1m{'─' * 60}\033[0m")
            print(f"\033[1m[loop_until_empty] wave={wave_name}\033[0m")
            print(f"\033[1m{'─' * 60}\033[0m\n")

            while not _is_wave_empty(repo_root, wave_name):
                iteration += 1
                if iteration > item.max_iterations:
                    print(f"[loop_until_empty] max iterations ({item.max_iterations}) reached")
                    return 1

                print(f"\n\033[90m─── Loop iteration {iteration} ───\033[0m\n")

                # Run the loop's steps as a sub-flow
                loop_flow = Flow(name=f"{flow.name}-loop-{iteration}", steps=list(item.steps))
                result_code = run_flow(
                    loop_flow,
                    repo_root,
                    context=context,
                    exclude=exclude,
                    skip_permissions=skip_permissions,
                    push_enabled=False,
                    pr_enabled=False,
                    backend=backend,
                    model_variant=model_variant,
                    chrome=chrome,
                )
                if result_code != 0:
                    return result_code

            print(f"\n[loop_until_empty] wave={wave_name} is empty, loop complete\n")
            i += 1
            continue

        i += 1

    _finalize_flow(flow.name, repo_root, should_pr)
    return 0
