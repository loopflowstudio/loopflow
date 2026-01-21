"""Loop iteration runner for lfd.

Runs iterations of a loop until the PR limit is reached or an error occurs.
Can be invoked directly as a subprocess for background execution.

When running in background, coordinates with the daemon scheduler to respect
global concurrency and PR limits.
"""

import json
import re
import socket
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass, replace
from datetime import datetime
from pathlib import Path

import yaml

from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import format_prompt, gather_prompt_components
from loopflow.lf.flows import ChooseFork, ChooseResult, FlowDef, load_flow, resolve_flow
from loopflow.lf.goals import build_effective_goals, render_goals
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


@dataclass
class _VariantTask:
    step: str
    label: str
    backend: str
    model_variant: str | None
    context: list[str] | None
    voices: list[str] | None


@dataclass
class _VariantResult:
    label: str
    worktree: Path
    exit_code: int
    session_id: str


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
                        "area": loop.area,
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
                    "area": loop.area,
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
                    "area": loop.area,
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


def _parse_choice(path: Path) -> tuple[str | None, str | None]:
    if not path.exists():
        return None, None

    text = path.read_text()
    match = re.match(r"^---\n(.*?)\n---\n?", text, re.DOTALL)
    if not match:
        return None, None

    data = yaml.safe_load(match.group(1)) or {}
    return data.get("choice"), data.get("reason")


def _build_choose_fork_prompt(
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
            "Inspect .docs/roadmap and .design as needed.",
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


def _choose_fork_branch(
    choose_fork: ChooseFork,
    flow_name: str,
    repo_root: Path,
    backend: str,
    model_variant: str | None,
    skip_permissions: bool,
) -> str:
    output_path = Path(choose_fork.output or f".design/choices/{flow_name}.md")
    if not output_path.is_absolute():
        output_path = repo_root / output_path
    output_path.parent.mkdir(parents=True, exist_ok=True)

    prompt = _build_choose_fork_prompt(flow_name, choose_fork.options, output_path, choose_fork.prompt)

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
        raise RuntimeError("choose_fork failed to run")

    choice, _reason = _parse_choice(output_path)
    if not choice or choice not in choose_fork.options:
        raise RuntimeError(f"choose_fork wrote invalid choice to {output_path}")

    return choice


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

    if loop.type == LoopType.FLOW and loop.project_file:
        try:
            prompt_content = Path(loop.project_file).read_text()
        except OSError:
            prompt_content = None
        if prompt_content:
            goal_section += f"\n\n<lf:prompt>\n{prompt_content}\n</lf:prompt>"

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
        "loopflow.lfd.collector",
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


def _build_judge_prompt(diffs: list[dict]) -> str:
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
        diff_lines = d["diff"].split("\n")
        if len(diff_lines) > 200:
            lines.extend(diff_lines[:200])
            lines.append(f"... ({len(diff_lines) - 200} more lines)")
        else:
            lines.append(d["diff"])
        lines.append("```")
        lines.append("")

    lines.extend(
        [
            "## Your verdict",
            "",
            "Reply with ONLY the model name of the winner (e.g., 'claude:opus' or 'codex:o3').",
            "Do not explain your reasoning.",
        ]
    )

    return "\n".join(lines)


def _judge_variants(
    results: list[_VariantResult],
    repo_root: Path,
    skip_permissions: bool,
) -> _VariantResult | None:
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
        diffs.append(
            {
                "model": r.label,
                "worktree": str(r.worktree),
                "summary": diff_text,
                "diff": full_diff.stdout if full_diff.returncode == 0 else "",
            }
        )

    judge_prompt = _build_judge_prompt(diffs)

    backend, variant = parse_model("claude:opus")
    runner = get_runner(backend)

    result = runner.launch(
        judge_prompt,
        auto=True,
        stream=False,
        skip_permissions=skip_permissions,
        model_variant=variant,
        cwd=repo_root,
    )
    if result.exit_code != 0:
        return None

    output = result.output or ""
    for r in results:
        if r.label in output:
            return r

    return None


def _merge_variant_winner(repo_root: Path, winner: _VariantResult) -> None:
    result = subprocess.run(
        ["git", "diff", "--name-only", "HEAD~1"],
        cwd=winner.worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return

    changed_files = [f.strip() for f in result.stdout.strip().split("\n") if f.strip()]

    for file_path in changed_files:
        src = winner.worktree / file_path
        dst = repo_root / file_path

        if src.exists():
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes(src.read_bytes())
        elif dst.exists():
            dst.unlink()


def _cleanup_variant_worktrees(repo_root: Path, results: list[_VariantResult]) -> None:
    for r in results:
        remove_worktree(repo_root, r.worktree.name.split(".")[-1])


def _run_choose_result(
    choose_result: ChooseResult,
    loop: Loop,
    worktree_path: Path,
    branch: str,
    context_paths: list[str] | None,
    effective_goals: list,
    skip_permissions: bool,
) -> int:
    options = choose_result.options
    if not options:
        return 1

    variants: list[_VariantTask] = []
    for idx, option in enumerate(options, 1):
        label = option.label or f"{option.model}:{idx}"
        backend, model_variant = parse_model(option.model)
        variants.append(
            _VariantTask(
                step=choose_result.step,
                label=label,
                backend=backend,
                model_variant=model_variant,
                context=option.context,
                voices=option.voice,
            )
        )

    processes: list[tuple[_VariantTask, subprocess.Popen, Path, str, str]] = []
    for task in variants:
        label_short = task.label.replace(":", "-")
        wt_name = f"_choose-{branch.replace('/', '-')}-{label_short}-{uuid.uuid4().hex[:8]}"
        try:
            wt_path = create_worktree(loop.repo, wt_name, base=branch)
        except Exception:
            for _, proc, wt, _, _ in processes:
                proc.terminate()
                remove_worktree(loop.repo, wt.name.split(".")[-1])
            return 1

        prompt_result = _build_loop_prompt(
            loop,
            effective_goals,
            wt_path,
            task.step,
            context_paths,
            extra_context=task.context,
            voices=task.voices,
        )
        if not prompt_result:
            remove_worktree(loop.repo, wt_path.name.split(".")[-1])
            return 1

        prompt, _step_file = prompt_result
        session_id = str(uuid.uuid4())
        step_label = f"{loop.area}:{task.step}:{task.label}"

        prompt_file = write_prompt_file(prompt)
        command = build_model_command(
            task.backend,
            auto=True,
            stream=True,
            skip_permissions=skip_permissions,
            yolo=skip_permissions,
            model_variant=task.model_variant,
            workdir=wt_path,
        )
        collector_cmd = [
            sys.executable,
            "-m",
            "loopflow.lfd.collector",
            "--session-id",
            session_id,
            "--step",
            step_label,
            "--repo-root",
            str(wt_path),
            "--prompt-file",
            prompt_file,
            "--autocommit",
            "--prefix",
            f"[{task.label}] ",
            "--",
            *command,
        ]

        process = subprocess.Popen(collector_cmd, cwd=wt_path)
        processes.append((task, process, wt_path, prompt_file, session_id))

    results: list[_VariantResult] = []
    for task, process, wt_path, prompt_file, session_id in processes:
        exit_code = process.wait()
        try:
            Path(prompt_file).unlink()
        except OSError:
            pass
        results.append(_VariantResult(task.label, wt_path, exit_code, session_id))

    successes = [r for r in results if r.exit_code == 0]
    if not successes:
        _cleanup_variant_worktrees(loop.repo, results)
        return 1

    if len(successes) == 1:
        winner = successes[0]
    else:
        winner = _judge_variants(successes, loop.repo, skip_permissions) or successes[0]

    _merge_variant_winner(worktree_path, winner)
    _cleanup_variant_worktrees(loop.repo, results)

    return 0


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
            "area": loop.area,
            "goals": loop.goals,
            "flow": loop.flow,
            "iteration": iteration,
        },
    )

    # Build effective goals (inject adaptive if no mode present)
    effective_goals = build_effective_goals(loop.repo, loop.goals)
    if not effective_goals:
        update_loop_run_status(run.id, LoopStatus.ERROR, error="No valid goals found")
        return False

    flow = loop.flow
    if not flow:
        update_loop_run_status(run.id, LoopStatus.ERROR, error="Flow is required")
        _cleanup_worktree(loop.repo, worktree_path, branch)
        return False

    try:
        flow_def = load_flow(flow, loop.repo)
    except ValueError as exc:
        update_loop_run_status(run.id, LoopStatus.ERROR, error=str(exc))
        _cleanup_worktree(loop.repo, worktree_path, branch)
        return False

    if not flow_def:
        update_loop_run_status(run.id, LoopStatus.ERROR, error=f"Unknown flow '{flow}'")
        _cleanup_worktree(loop.repo, worktree_path, branch)
        return False

    resolved = resolve_flow(flow_def, loop.repo)
    if not resolved:
        update_loop_run_status(run.id, LoopStatus.ERROR, error=f"Empty flow '{flow}'")
        _cleanup_worktree(loop.repo, worktree_path, branch)
        return False

    # Get model configuration
    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    runner = get_runner(backend)
    if not runner.is_available():
        update_loop_run_status(run.id, LoopStatus.ERROR, error=f"'{backend}' CLI not found")
        return False

    skip_permissions = config.yolo if config else False

    context_paths = [loop.area] if loop.area != "." else None
    if not context_paths and effective_goals[0].area:
        context_paths = effective_goals[0].area

    i = 0
    while i < len(resolved):
        step = resolved[i]
        if step.parallel_group is not None:
            update_loop_run_status(
                run.id, LoopStatus.ERROR, error="Parallel steps are not supported in lfd loops yet"
            )
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

        if step.race is not None:
            update_loop_run_status(
                run.id, LoopStatus.ERROR, error="Race steps are not supported in lfd loops yet"
            )
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

        if step.choose_fork is not None:
            try:
                choice = _choose_fork_branch(
                    step.choose_fork,
                    flow_def.name,
                    worktree_path,
                    backend,
                    model_variant,
                    skip_permissions,
                )
            except RuntimeError as exc:
                update_loop_run_status(run.id, LoopStatus.ERROR, error=str(exc))
                _cleanup_worktree(loop.repo, worktree_path, branch)
                return False

            branch_steps = step.choose_fork.options[choice]
            branch_flow = FlowDef.from_dict(f"{flow_def.name}:{choice}", {"steps": branch_steps})
            branch_resolved = resolve_flow(branch_flow, loop.repo)
            resolved = resolved[:i] + branch_resolved + resolved[i + 1 :]
            continue

        if step.choose_result is not None:
            step_name = step.choose_result.step
            update_loop_run_step(run.id, step_name)
            notify_event(
                "loop.step.started",
                {
                    "loop_id": loop.id,
                    "step": step_name,
                    "iteration": iteration,
                },
            )

            result_code = _run_choose_result(
                step.choose_result,
                loop,
                worktree_path,
                branch,
                context_paths,
                effective_goals,
                skip_permissions,
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
                update_loop_run_status(run.id, LoopStatus.ERROR, error=f"{step_name} failed")
                _cleanup_worktree(loop.repo, worktree_path, branch)
                return False

            i += 1
            continue

        if not step.step:
            i += 1
            continue

        step_name = step.step
        update_loop_run_step(run.id, step_name)
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
            update_loop_run_status(
                run.id, LoopStatus.ERROR, error=f"Step file not found: {step_name}"
            )
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
            update_loop_run_status(run.id, LoopStatus.ERROR, error=f"{step_name} failed")
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

        i += 1

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
            "area": loop.area,
            "goals": loop.goals,
            "flow": loop.flow,
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
