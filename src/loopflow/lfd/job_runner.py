"""Job iteration runner for lfd.

Runs iterations of a job until the PR limit is reached or an error occurs.
Can be invoked directly as a subprocess for background execution.

When running in background, coordinates with the daemon scheduler to respect
global concurrency and PR limits.
"""

import json
import socket
import subprocess
import sys
import time
import uuid
from dataclasses import dataclass, replace
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
from loopflow.lfd.client import notify_event
from loopflow.lfd.db import (
    get_job,
    save_job_run,
    update_job_iteration,
    update_job_pid,
    update_job_run_pr,
    update_job_run_status,
    update_job_run_step,
    update_job_status,
)
from loopflow.lfd.jobs import count_outstanding
from loopflow.lfd.models import Job, JobRun, JobStatus

# Backwards compatibility aliases
Loop = Job
LoopRun = JobRun
LoopStatus = JobStatus
get_loop = get_job
save_loop_run = save_job_run
update_loop_iteration = update_job_iteration
update_loop_pid = update_job_pid
update_loop_run_pr = update_job_run_pr
update_loop_run_status = update_job_run_status
update_loop_run_step = update_job_run_step
update_loop_status = update_job_status

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"
SCHEDULER_POLL_INTERVAL = 30  # seconds between slot checks


@dataclass
class _VariantResult:
    label: str
    worktree: Path
    exit_code: int
    session_id: str


def _iteration_branch_prefix(job_main: str) -> str:
    """Derive iteration branch prefix from job-main.

    'product-engineer-main' → 'product-engineer'
    'product-engineer-1-main' → 'product-engineer-1'
    """
    if job_main.endswith("-main"):
        return job_main[:-5]
    return job_main


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


def run_job_iterations(job: Job) -> None:
    """Run job iterations until PR limit is reached or error occurs.

    For FLOW jobs, runs exactly one iteration then stops.
    For LOOP jobs, runs continuously until pr_limit outstanding.
    """
    while True:
        # Check if we should pause (per-job limit) - skip for one-shot which runs once
        if not job.is_one_shot:
            outstanding = count_outstanding(job)
            if outstanding >= job.pr_limit:
                update_job_status(job.id, JobStatus.WAITING)
                notify_event(
                    "job.waiting",
                    {
                        "job_id": job.id,
                        "area": job.area,
                        "outstanding": outstanding,
                        "limit": job.pr_limit,
                    },
                )
                break

        # Run one iteration
        iteration = job.iteration + 1
        run_id = str(uuid.uuid4())

        # Wait for scheduler slot (global concurrency)
        while True:
            acquired, reason = _scheduler_acquire(run_id)
            if acquired:
                break
            notify_event(
                "scheduler.waiting",
                {
                    "job_id": job.id,
                    "area": job.area,
                    "reason": reason or "concurrency",
                },
            )
            time.sleep(SCHEDULER_POLL_INTERVAL)

        try:
            success = run_iteration(job, iteration, run_id)
            if success:
                update_job_iteration(job.id, iteration)
                job.iteration = iteration
            else:
                update_job_status(job.id, JobStatus.ERROR)
                break
        except Exception as e:
            notify_event(
                "job.error",
                {
                    "job_id": job.id,
                    "area": job.area,
                    "error": str(e),
                },
            )
            update_job_status(job.id, JobStatus.ERROR)
            break
        finally:
            _scheduler_release(run_id)

        # One-shot jobs run exactly once then stop
        if job.is_one_shot:
            update_job_status(job.id, JobStatus.IDLE)
            break

    # Clear pid when done
    update_job_pid(job.id, None)


# Backwards compatibility alias
run_loop_iterations = run_job_iterations


def _build_job_prompt(
    job: Job,
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

    if job.is_one_shot and job.project_file:
        try:
            prompt_content = Path(job.project_file).read_text()
        except OSError:
            prompt_content = None
        if prompt_content:
            goal_section += f"\n\n<lf:prompt>\n{prompt_content}\n</lf:prompt>"

    combined = f"{goal_section}\n\n---\n\n{step_content}"
    components = replace(components, step=(step_file, combined))
    prompt = format_prompt(components)

    return prompt, step_file


# Backwards compatibility alias
_build_loop_prompt = _build_job_prompt


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


def _cleanup_variant_worktrees(repo_root: Path, results: list[_VariantResult]) -> None:
    for r in results:
        remove_worktree(repo_root, r.worktree.name.split(".")[-1])


def _build_job_inline_prompt(
    job: Job,
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

    if job.is_one_shot and job.project_file:
        try:
            prompt_content = Path(job.project_file).read_text()
        except OSError:
            prompt_content = None
        if prompt_content:
            goal_section += f"\n\n<lf:prompt>\n{prompt_content}\n</lf:prompt>"

    combined = f"{goal_section}\n\n---\n\n{step_content}"
    components = replace(components, step=(step_file, combined))
    return format_prompt(components)


# Backwards compatibility alias
_build_loop_inline_prompt = _build_job_inline_prompt


def _run_fork_join_group(
    job: Job,
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
            wt_path = create_worktree(job.repo, wt_name, base=branch)
        except Exception:
            _cleanup_variant_worktrees(job.repo, results)
            return 1

        subprocess.run(
            ["git", "reset", "--hard", branch],
            cwd=wt_path,
            capture_output=True,
        )
        subprocess.run(["git", "clean", "-fd"], cwd=wt_path, capture_output=True)

        prompt_result = _build_job_prompt(
            job,
            effective_goals,
            wt_path,
            step.step,
            step_context or None,
            voices=step_voices,
        )
        if not prompt_result:
            remove_worktree(job.repo, wt_path.name.split(".")[-1])
            return 1

        prompt, _step_file = prompt_result
        session_id = str(uuid.uuid4())
        step_label = f"{job.area}:{step.step}:{label}"

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
        _cleanup_variant_worktrees(job.repo, results)
        return 1

    fork_worktrees = [(r.label, r.worktree) for r in successes]
    join_prompt = build_join_prompt(
        collect_fork_diffs(fork_worktrees),
        load_join_instructions(join_config.step, job.repo),
        format_voice_section(join_config.voice, job.repo),
        flow_name,
    )
    join_prompt = _build_job_inline_prompt(
        job,
        effective_goals,
        worktree_path,
        join_prompt,
        context_paths,
        voices=None,
    )
    if not join_prompt:
        _cleanup_variant_worktrees(job.repo, results)
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
        f"{job.area}:join",
        autocommit=True,
    )

    _cleanup_variant_worktrees(job.repo, results)
    return join_result


def run_iteration(job: Job, iteration: int, run_id: str | None = None) -> bool:
    """Run a single iteration of the job.

    Args:
        job: The job to run
        iteration: Iteration number
        run_id: Optional pre-allocated run ID (for scheduler coordination)

    Returns True if successful, False on error.
    """
    config = load_config(job.repo)

    # Create iteration branch from job-main
    prefix = _iteration_branch_prefix(job.job_main)
    branch = f"{prefix}/{iteration:03d}"
    try:
        worktree_path = create_worktree(job.repo, branch, base=job.job_main)
    except WorktreeError as e:
        notify_event("job.error", {"job_id": job.id, "error": f"Failed to create worktree: {e}"})
        return False

    # Create job_run record
    run = JobRun(
        id=run_id or str(uuid.uuid4()),
        job_id=job.id,
        iteration=iteration,
        status=JobStatus.RUNNING,
        started_at=datetime.now(),
        worktree=str(worktree_path),
    )
    save_job_run(run)

    notify_event(
        "job.started",
        {
            "job_id": job.id,
            "area": job.area,
            "goals": job.goals,
            "flow": job.flow,
            "iteration": iteration,
        },
    )

    # Build effective goals (inject adaptive if no mode present)
    effective_goals = build_effective_goals(job.repo, job.goals)
    if not effective_goals:
        update_job_run_status(run.id, JobStatus.ERROR, error="No valid goals found")
        return False

    flow = job.flow
    if not flow:
        update_job_run_status(run.id, JobStatus.ERROR, error="Flow is required")
        _cleanup_worktree(job.repo, worktree_path, branch)
        return False

    try:
        flow_def = load_flow(flow, job.repo)
    except ValueError as exc:
        update_job_run_status(run.id, JobStatus.ERROR, error=str(exc))
        _cleanup_worktree(job.repo, worktree_path, branch)
        return False

    if not flow_def:
        update_job_run_status(run.id, JobStatus.ERROR, error=f"Unknown flow '{flow}'")
        _cleanup_worktree(job.repo, worktree_path, branch)
        return False

    resolved = resolve_flow(flow_def, job.repo)
    if not resolved:
        update_job_run_status(run.id, JobStatus.ERROR, error=f"Empty flow '{flow}'")
        _cleanup_worktree(job.repo, worktree_path, branch)
        return False

    # Get model configuration
    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)

    runner = get_runner(backend)
    if not runner.is_available():
        update_job_run_status(run.id, JobStatus.ERROR, error=f"'{backend}' CLI not found")
        return False

    skip_permissions = config.yolo if config else False

    context_paths = [job.area] if job.area != "." else None
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
                update_job_run_status(
                    run.id, JobStatus.ERROR, error="Fork must be immediately followed by join"
                )
                _cleanup_worktree(job.repo, worktree_path, branch)
                return False

            result_code = _run_fork_join_group(
                job,
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
                update_job_run_status(run.id, JobStatus.ERROR, error="join failed")
                _cleanup_worktree(job.repo, worktree_path, branch)
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
                update_job_run_status(run.id, JobStatus.ERROR, error=str(exc))
                _cleanup_worktree(job.repo, worktree_path, branch)
                return False

            branch_steps = step.choose.options[choice]
            branch_flow = FlowDef.from_dict(f"{flow_def.name}:{choice}", {"steps": branch_steps})
            branch_resolved = resolve_flow(branch_flow, job.repo)
            resolved = resolved[:i] + branch_resolved + resolved[i + 1 :]
            continue

        if step.join is not None:
            update_job_run_status(run.id, JobStatus.ERROR, error="Join must follow fork")
            _cleanup_worktree(job.repo, worktree_path, branch)
            return False

        if not step.step:
            i += 1
            continue

        step_name = step.step
        update_job_run_step(run.id, step_name)
        notify_event(
            "job.step.started",
            {
                "job_id": job.id,
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

        prompt_result = _build_job_prompt(
            job,
            effective_goals,
            worktree_path,
            step_name,
            step_context or None,
            voices=step_voices,
        )
        if not prompt_result:
            update_job_run_status(
                run.id, JobStatus.ERROR, error=f"Step file not found: {step_name}"
            )
            _cleanup_worktree(job.repo, worktree_path, branch)
            return False

        prompt, _step_file = prompt_result
        result_code = _run_collector_step(
            prompt,
            worktree_path,
            step_backend,
            step_variant,
            skip_permissions,
            run.id,
            f"{job.area}:{step_name}",
        )

        notify_event(
            "job.step.completed",
            {
                "job_id": job.id,
                "step": step_name,
                "status": "completed" if result_code == 0 else "error",
            },
        )

        if result_code != 0:
            update_job_run_status(run.id, JobStatus.ERROR, error=f"{step_name} failed")
            _cleanup_worktree(job.repo, worktree_path, branch)
            return False

        i += 1

    # Clear current step
    update_job_run_step(run.id, None)

    # Create PR to job-main and auto-merge (always)
    pr_url = _create_pr_to_job_main(job, worktree_path, branch, iteration)
    if pr_url:
        update_job_run_pr(run.id, pr_url)
        _auto_merge_pr(worktree_path)

        # If LAND mode, also merge job-main to main
        if job.merge_mode.value == "land":
            _land_to_main(job)

    update_job_run_status(run.id, JobStatus.IDLE)

    notify_event(
        "job.iteration.done",
        {
            "job_id": job.id,
            "area": job.area,
            "goals": job.goals,
            "flow": job.flow,
            "iteration": iteration,
            "pr_url": pr_url,
        },
    )

    # Cleanup worktree
    _cleanup_worktree(job.repo, worktree_path, branch)

    return True


def _create_pr_to_job_main(
    job: Job, worktree_path: Path, branch: str, iteration: int
) -> str | None:
    """Push branch and create PR targeting job-main."""
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
        title = f"[{job.area_slug}] {message.title}"
        body = (
            f"Job: {job.area} [{job.goals_display}]\n"
            f"Flow: {job.flow_display}\n"
            f"Iteration: {iteration}\n\n{message.body}"
        )
    except Exception:
        title = f"[{job.area_slug}] Iteration {iteration}"
        body = (
            f"Job: {job.area} [{job.goals_display}]\n"
            f"Flow: {job.flow_display}\n"
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
        job.job_main,
    ]
    result = subprocess.run(cmd, cwd=worktree_path, capture_output=True, text=True)

    if result.returncode == 0:
        return result.stdout.strip()
    return None


# Backwards compatibility alias
_create_pr_to_loop_main = _create_pr_to_job_main


def _auto_merge_pr(worktree_path: Path) -> bool:
    """Auto-merge the current PR."""
    result = subprocess.run(
        ["gh", "pr", "merge", "--squash", "--delete-branch"],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def _land_to_main(job: Job) -> str | None:
    """Create or update PR from job-main → main, enable auto-merge.

    Returns PR URL on success, None on failure.
    Works from main repo (not worktree, which gets deleted).
    Idempotent: existing PR just gets auto-merge re-enabled.
    """
    repo = job.repo

    # Push job-main
    subprocess.run(["git", "push", "origin", job.job_main], cwd=repo, capture_output=True)

    # Check for existing PR
    result = subprocess.run(
        [
            "gh",
            "pr",
            "list",
            "--head",
            job.job_main,
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
            job.job_main,
            "--title",
            f"[{job.area_slug}] Land accumulated work",
            "--body",
            f"Auto-land from job: {job.area} [{job.goals_display}] (flow: {job.flow_display})",
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
    """Entry point for background job runner."""
    if len(sys.argv) != 2:
        print("Usage: python -m loopflow.lfd.job_runner <job_id>", file=sys.stderr)
        sys.exit(1)

    job_id = sys.argv[1]
    job = get_job(job_id)

    if not job:
        print(f"Job not found: {job_id}", file=sys.stderr)
        sys.exit(1)

    run_job_iterations(job)


if __name__ == "__main__":
    main()
