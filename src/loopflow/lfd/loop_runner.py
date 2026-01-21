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
from loopflow.lf.context import format_prompt, gather_prompt_components, gather_step
from loopflow.lf.flows import (
    Choose,
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
from loopflow.lf.voices import load_voice
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


def _choose_branch(
    choose: Choose,
    flow_name: str,
    repo_root: Path,
    backend: str,
    model_variant: str | None,
    skip_permissions: bool,
) -> str:
    output_path = Path(choose.output or f".design/choices/{flow_name}.md")
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


def _format_voice_section(voice_names: list[str] | None, repo_root: Path) -> str | None:
    if not voice_names:
        return None

    voices = [load_voice(name, repo_root) for name in voice_names]
    if len(voices) == 1:
        v = voices[0]
        return f"<lf:voice:{v.name}>\n{v.content}\n</lf:voice:{v.name}>"

    voice_parts = [f"<lf:voice:{v.name}>\n{v.content}\n</lf:voice:{v.name}>" for v in voices]
    return f"<lf:voices>\n{chr(10).join(voice_parts)}\n</lf:voices>"


def _load_join_instructions(step_name: str | None, repo_root: Path) -> str | None:
    name = step_name or "synthesize"
    step_file = gather_step(repo_root, name)
    if not step_file:
        return None

    return step_file.content.strip() or None


def _collect_fork_diffs(results: list[_VariantResult]) -> list[dict]:
    diffs = []
    for r in results:
        diff_result = subprocess.run(
            ["git", "diff", "--stat"],
            cwd=r.worktree,
            capture_output=True,
            text=True,
        )
        diff_text = diff_result.stdout if diff_result.returncode == 0 else "(no diff)"

        full_diff = subprocess.run(
            ["git", "diff"],
            cwd=r.worktree,
            capture_output=True,
            text=True,
        )
        diffs.append(
            {
                "label": r.label,
                "worktree": str(r.worktree),
                "summary": diff_text,
                "diff": full_diff.stdout if full_diff.returncode == 0 else "",
            }
        )
    return diffs


def _build_join_prompt(
    diffs: list[dict],
    instructions: str | None,
    voice_section: str | None,
    flow_name: str,
) -> str:
    lines = [
        "You are joining changes from multiple forked worktrees into the current worktree.",
        "Synthesize the best parts of all forks into a single changeset here.",
        "Do NOT edit the forked worktrees directly.",
        "After applying the changes, commit the result.",
        f"Write a short summary to .design/joins/{flow_name}.md if that file makes sense.",
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

    if loop.type == LoopType.FLOW and loop.project_file:
        try:
            prompt_content = Path(loop.project_file).read_text()
        except OSError:
            prompt_content = None
        if prompt_content:
            goal_section += f"\n\n<lf:prompt>\n{prompt_content}\n</lf:prompt>"

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

    join_prompt = _build_join_prompt(
        _collect_fork_diffs(successes),
        _load_join_instructions(join_config.step, loop.repo),
        _format_voice_section(join_config.voice, loop.repo),
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
            group_steps = []
            group = step.parallel_group
            while i < len(resolved) and resolved[i].parallel_group == group:
                group_steps.append(resolved[i])
                i += 1

            if i >= len(resolved) or resolved[i].join is None:
                update_loop_run_status(
                    run.id, LoopStatus.ERROR, error="Fork must be immediately followed by join"
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
                update_loop_run_status(run.id, LoopStatus.ERROR, error="join failed")
                _cleanup_worktree(loop.repo, worktree_path, branch)
                return False

            i += 1
            continue

        if step.race is not None:
            update_loop_run_status(
                run.id, LoopStatus.ERROR, error="Race steps are not supported in lfd loops yet"
            )
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

        if step.choose is not None:
            try:
                choice = _choose_branch(
                    step.choose,
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

            branch_steps = step.choose.options[choice]
            branch_flow = FlowDef.from_dict(f"{flow_def.name}:{choice}", {"steps": branch_steps})
            branch_resolved = resolve_flow(branch_flow, loop.repo)
            resolved = resolved[:i] + branch_resolved + resolved[i + 1 :]
            continue

        if step.join is not None:
            update_loop_run_status(run.id, LoopStatus.ERROR, error="Join must follow fork")
            _cleanup_worktree(loop.repo, worktree_path, branch)
            return False

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
