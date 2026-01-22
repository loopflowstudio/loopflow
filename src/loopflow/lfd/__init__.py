"""lfd: Loopflow daemon.

Commands for managing agent jobs.
"""

import asyncio
import json
import socket
import subprocess
import sys
from pathlib import Path

import typer

from loopflow.lf.flows import load_flow
from loopflow.lf.goals import goal_exists, list_goals, load_goal
from loopflow.lf.logging import get_log_dir
from loopflow.lfd.db import (
    delete_job,
    get_job,
    get_job_runs,
    list_jobs,
    save_job,
)
from loopflow.lfd.jobs import create_job, get_wt_from_cwd, start_job, stop_job
from loopflow.lfd.launchd import install as launchd_install
from loopflow.lfd.launchd import is_running
from loopflow.lfd.launchd import uninstall as launchd_uninstall
from loopflow.lfd.models import Job, JobStatus, JobType, MergeMode
from loopflow.lfd.server import run_server

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"

app = typer.Typer(help="Loopflow daemon - agent jobs")


def _use_color() -> bool:
    return sys.stdout.isatty()


def _colors() -> dict[str, str]:
    if not _use_color():
        return {
            "cyan": "",
            "bold": "",
            "dim": "",
            "yellow": "",
            "green": "",
            "red": "",
            "reset": "",
        }
    return {
        "cyan": "\033[36m",
        "bold": "\033[1m",
        "dim": "\033[90m",
        "yellow": "\033[33m",
        "green": "\033[32m",
        "red": "\033[31m",
        "reset": "\033[0m",
    }


def _status_color(status: JobStatus, c: dict[str, str]) -> str:
    if status == JobStatus.RUNNING:
        return c["green"]
    elif status == JobStatus.ERROR:
        return c["red"]
    elif status == JobStatus.WAITING:
        return c["yellow"]
    return c["dim"]


def _job_display(job: Job) -> str:
    """Return area and goals for display."""
    return f"{job.area} [{job.flow_display}] [{job.goals_display}]"


def _is_area(s: str) -> bool:
    """Check if string looks like an area (contains / or is .)."""
    return "/" in s or s == "."


def _validate_flow(repo: Path, flow: str, c: dict[str, str]) -> str:
    """Validate and normalize flow name."""
    normalized = flow.strip()
    if not normalized:
        typer.echo(f"{c['red']}Error:{c['reset']} Flow cannot be empty", err=True)
        raise typer.Exit(1)

    flow_def = load_flow(normalized, repo)
    if not flow_def:
        typer.echo(
            f"{c['red']}Error:{c['reset']} Flow '{normalized}' not found in .lf/flows/",
            err=True,
        )
        raise typer.Exit(1)

    return normalized


# Daemon commands


@app.command()
def serve():
    """Run daemon in foreground (for debugging or launchd)."""
    asyncio.run(run_server(SOCKET_PATH))


@app.command()
def install():
    """Install launchd plist for auto-start."""
    was_running = is_running()
    if launchd_install():
        if was_running:
            typer.echo("lfd reinstalled and restarted")
        else:
            typer.echo("lfd installed and started")
    else:
        typer.echo("Failed to install lfd")
        raise typer.Exit(1)


@app.command()
def uninstall():
    """Remove launchd plist and stop daemon."""
    if launchd_uninstall():
        typer.echo("lfd uninstalled")
    else:
        typer.echo("Failed to uninstall lfd")
        raise typer.Exit(1)


@app.command()
def start(
    areas: list[str] = typer.Argument(None, help="Areas to start (all idle if omitted)"),
    all_jobs: bool = typer.Option(False, "--all", help="Include waiting jobs"),
):
    """Start multiple jobs in parallel.

    Without arguments, starts all idle jobs. With --all, also starts waiting jobs.
    """
    c = _colors()
    repo = get_wt_from_cwd()

    # Get jobs to start
    if areas:
        # Start specific areas
        jobs_to_start = []
        for area in areas:
            job = None
            for j in list_jobs(repo=repo):
                if j.area == area:
                    job = j
                    break
            if not job:
                typer.echo(
                    f"{c['yellow']}Warning:{c['reset']} Job for '{area}' not found, skipping",
                    err=True,
                )
            else:
                jobs_to_start.append(job)
    else:
        # Start all eligible jobs
        jobs_to_start = []
        for job in list_jobs(repo=repo):
            if job.status == JobStatus.IDLE:
                jobs_to_start.append(job)
            elif all_jobs and job.status == JobStatus.WAITING:
                jobs_to_start.append(job)

    if not jobs_to_start:
        typer.echo(f"{c['dim']}No jobs to start{c['reset']}")
        return

    # Start each job
    started = 0
    for job in jobs_to_start:
        result = start_job(job.id)
        if result:
            msg = f"{c['green']}Started{c['reset']} {c['bold']}{job.area}{c['reset']}"
            typer.echo(f"{msg} ({job.short_id()})")
            started += 1
        elif result.reason == "already_running":
            typer.echo(f"{c['dim']}Already running:{c['reset']} {job.area}")
        elif result.reason == "waiting":
            msg = f"{c['yellow']}Waiting:{c['reset']} {job.area}"
            typer.echo(f"{msg} ({result.outstanding} outstanding)")
        else:
            typer.echo(f"{c['red']}Failed:{c['reset']} {job.area}")

    typer.echo(f"\nStarted {started}/{len(jobs_to_start)} jobs")


# Job commands (CLI names kept for user familiarity)


@app.command()
def job(
    flow: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., swift/, src/, .)"),
    goals: list[str] = typer.Option(None, "-g", "--goal", help="Goal to add (repeatable)"),
    limit: int = typer.Option(None, "-l", "--limit", help="PR limit override"),
    merge_mode: str = typer.Option(None, "--merge-mode", help="Merge mode: pr or land"),
    foreground: bool = typer.Option(False, "-f", "--foreground", help="Run in foreground"),
):
    """Start a continuous homeostasis job.

    Flow is required - specifies which flow to run from .lf/flows/.
    Area is required - scopes the work (e.g., swift/, src/, or . for whole repo).
    Goals are optional - adaptive mode is implicit if no mode goal is specified.

    Examples:
        lfd job ship swift/                              # adaptive mode
        lfd job ship swift/ -g product-engineer          # adaptive + role
        lfd job ship swift/ -g product-engineer -g designer  # adaptive + roles
        lfd job ship .                                     # whole repo
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Validate area looks like a path
    if not _is_area(area):
        typer.echo(
            f"{c['red']}Error:{c['reset']} '{area}' doesn't look like an area. "
            "Use a path like swift/, src/, or . for whole repo.",
            err=True,
        )
        typer.echo(f"\nDid you mean: lfd job {area}/ ?")
        raise typer.Exit(1)

    goals = goals or []

    # Validate goals exist
    for goal_name in goals:
        if not goal_exists(repo, goal_name):
            typer.echo(
                f"{c['red']}Error:{c['reset']} Goal '{goal_name}' not found",
                err=True,
            )
            available = list_goals(repo)
            if available:
                typer.echo(f"Available goals: {', '.join(available)}")
            raise typer.Exit(1)

    flow = _validate_flow(repo, flow, c)

    # Validate merge_mode if specified
    if merge_mode and merge_mode not in ("pr", "land"):
        typer.echo(f"{c['red']}Error:{c['reset']} merge-mode must be 'pr' or 'land'", err=True)
        raise typer.Exit(1)

    # Create or get job
    j = create_job(JobType.LOOP, area, repo, goals=goals, flow=flow)

    # Override settings if specified
    changed = False
    if limit is not None:
        j.pr_limit = limit
        changed = True
    if merge_mode:
        j.merge_mode = MergeMode(merge_mode)
        changed = True
    if changed:
        save_job(j)

    # Start it
    result = start_job(j.id, foreground=foreground)
    if result:
        if foreground:
            msg = f"{c['green']}Completed{c['reset']} job {c['bold']}{area}{c['reset']}"
            typer.echo(f"{msg} ({j.short_id()})")
        else:
            msg = f"{c['green']}Started{c['reset']} job {c['bold']}{area}{c['reset']}"
            typer.echo(f"{msg} ({j.short_id()})")
            typer.echo(f"  Repo: {repo}")
            typer.echo(f"  Job main: {j.job_main}")
            typer.echo(f"  Goals: {j.goals_display}")
            typer.echo(f"  Flow: {j.flow_display}")
            typer.echo(f"  PR limit: {j.pr_limit}")
    elif result.reason == "already_running":
        typer.echo(f"Job already running (PID {j.pid})")
        raise typer.Exit(1)
    elif result.reason == "waiting":
        msg = f"{c['yellow']}Waiting:{c['reset']} {result.outstanding} outstanding PRs"
        typer.echo(f"{msg} (limit {j.pr_limit})")
        typer.echo(f"Run 'lfops land --squash' from {j.job_main} worktree to land work to main")
        raise typer.Exit(0)
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to start job", err=True)
        raise typer.Exit(1)


# Backwards compatibility alias
@app.command(hidden=True)
def loop(
    flow: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., swift/, src/, .)"),
    goals: list[str] = typer.Option(None, "-g", "--goal", help="Goal to add (repeatable)"),
    limit: int = typer.Option(None, "-l", "--limit", help="PR limit override"),
    merge_mode: str = typer.Option(None, "--merge-mode", help="Merge mode: pr or land"),
    foreground: bool = typer.Option(False, "-f", "--foreground", help="Run in foreground"),
):
    """Alias for 'job' command (deprecated)."""
    job(flow, area, goals, limit, merge_mode, foreground)


@app.command()
def flow(
    flow_name: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., swift/, src/, .)"),
    goals: list[str] = typer.Option(None, "-g", "--goal", help="Goal to add (repeatable)"),
    paste: bool = typer.Option(False, "-v", "--paste", help="Include clipboard as prompt"),
):
    """Start a one-off flow (runs once then stops).

    Examples:
        lfd flow ship swift/                        # one-off adaptive iteration
        lfd flow ship swift/ -g product-engineer    # with role
        lfd flow ship . -v                            # whole repo with clipboard
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Validate area looks like a path
    if not _is_area(area):
        typer.echo(
            f"{c['red']}Error:{c['reset']} '{area}' doesn't look like an area. "
            "Use a path like swift/, src/, or . for whole repo.",
            err=True,
        )
        raise typer.Exit(1)

    goals = goals or []

    # Validate goals exist
    for goal_name in goals:
        if not goal_exists(repo, goal_name):
            typer.echo(f"{c['red']}Error:{c['reset']} Goal '{goal_name}' not found", err=True)
            raise typer.Exit(1)

    flow_name = _validate_flow(repo, flow_name, c)

    # Handle clipboard paste - write to temp file if provided
    project_file = None
    if paste:
        result = subprocess.run(["pbpaste"], capture_output=True, text=True)
        if result.returncode == 0 and result.stdout.strip():
            import tempfile

            with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False) as f:
                f.write(result.stdout)
                project_file = f.name

    # Create or get job
    j = create_job(JobType.FLOW, area, repo, goals=goals, flow=flow_name, project_file=project_file)

    # Start it
    if start_job(j.id):
        typer.echo(
            f"{c['green']}Started{c['reset']} flow {c['bold']}{area}{c['reset']} ({j.short_id()})"
        )
        typer.echo(f"  Goals: {j.goals_display}")
        typer.echo(f"  Flow: {j.flow_display}")
        if paste:
            typer.echo("  Clipboard: included")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to start flow", err=True)
        raise typer.Exit(1)


@app.command()
def subscribe(
    flow: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., swift/, src/, .)"),
    path: list[str] = typer.Option(
        ..., "-p", "-P", "--path", help="Paths to watch (repeatable, supports globs)"
    ),
    goals: list[str] = typer.Option(None, "-g", "--goal", help="Goal to add (repeatable)"),
):
    """Subscribe to path changes on main."""
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    if not _is_area(area):
        typer.echo(
            f"{c['red']}Error:{c['reset']} '{area}' doesn't look like an area.",
            err=True,
        )
        raise typer.Exit(1)

    goals = goals or []
    for goal_name in goals:
        if not goal_exists(repo, goal_name):
            typer.echo(f"{c['red']}Error:{c['reset']} Goal '{goal_name}' not found", err=True)
            raise typer.Exit(1)

    flow = _validate_flow(repo, flow, c)

    # Convert path list to comma-separated pathset
    pathset = ",".join(path)

    # Create subscription
    j = create_job(JobType.SUBSCRIBE, area, repo, goals=goals, flow=flow, pathset=pathset)

    msg = f"{c['green']}Subscribed{c['reset']} {c['bold']}{area}{c['reset']} to {pathset}"
    typer.echo(f"{msg} ({j.short_id()})")
    typer.echo(f"  Goals: {j.goals_display}")
    typer.echo(f"  Flow: {j.flow_display}")
    typer.echo("  Will run when paths change on main")


@app.command()
def schedule(
    flow: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., swift/, src/, .)"),
    cron_expr: str = typer.Argument(..., help="Cron expression (e.g., '0 9 * * *')"),
    goals: list[str] = typer.Option(None, "-g", "--goal", help="Goal to add (repeatable)"),
):
    """Schedule a job to run on cron."""
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    if not _is_area(area):
        typer.echo(
            f"{c['red']}Error:{c['reset']} '{area}' doesn't look like an area.",
            err=True,
        )
        raise typer.Exit(1)

    goals = goals or []
    for goal_name in goals:
        if not goal_exists(repo, goal_name):
            typer.echo(f"{c['red']}Error:{c['reset']} Goal '{goal_name}' not found", err=True)
            raise typer.Exit(1)

    flow = _validate_flow(repo, flow, c)

    # Create schedule
    j = create_job(
        JobType.SCHEDULE,
        area,
        repo,
        goals=goals,
        flow=flow,
        cron=cron_expr,
    )

    typer.echo(f"{c['green']}Scheduled{c['reset']} {c['bold']}{area}{c['reset']} ({j.short_id()})")
    typer.echo(f"  Goals: {j.goals_display}")
    typer.echo(f"  Flow: {j.flow_display}")
    typer.echo(f"  Cron: {cron_expr}")


def _get_scheduler_status() -> dict | None:
    """Get scheduler status from daemon if running."""
    socket_path = Path.home() / ".lf" / "lfd.sock"
    if not socket_path.exists():
        return None

    try:
        sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        sock.settimeout(2.0)
        sock.connect(str(socket_path))
        sock.sendall(b'{"method": "scheduler.status"}\n')

        data = b""
        while b"\n" not in data:
            chunk = sock.recv(1024)
            if not chunk:
                break
            data += chunk
        sock.close()

        if data:
            response = json.loads(data.decode().strip())
            if response.get("ok"):
                return response.get("result")
        return None
    except Exception:
        return None


@app.command()
def status(
    job_id: str = typer.Argument(None, help="Job ID (optional, shows all if omitted)"),
    ids_only: bool = typer.Option(False, "--ids", help="Print job IDs only (for scripting)"),
):
    """Show status of jobs."""
    c = _colors()

    # Machine-readable output for scripting
    if ids_only:
        for j in list_jobs():
            typer.echo(j.id)
        return

    if job_id:
        j = get_job(job_id)
        if not j:
            typer.echo(f"{c['red']}Error:{c['reset']} Job '{job_id}' not found", err=True)
            raise typer.Exit(1)
        _print_job_detail(j, c)
    else:
        # Show scheduler status if daemon is running
        sched = _get_scheduler_status()
        if sched:
            slots_used = sched.get("slots_used", 0)
            slots_total = sched.get("slots_total", 3)
            outstanding = sched.get("outstanding", 0)
            outstanding_limit = sched.get("outstanding_limit", 15)

            slots_color = c["green"] if slots_used < slots_total else c["yellow"]
            outstanding_color = c["green"] if outstanding < outstanding_limit else c["yellow"]

            typer.echo(
                f"Scheduler: {slots_color}{slots_used}/{slots_total}{c['reset']} slots, "
                f"{outstanding_color}{outstanding}/{outstanding_limit}{c['reset']} outstanding"
            )
            typer.echo("")

        jobs = list_jobs()
        if not jobs:
            typer.echo(f"{c['dim']}No jobs configured{c['reset']}")
            typer.echo("Start one with: lfd job <flow> <area>")
            return

        typer.echo(f"{'ID':<9} {'TYPE':<10} {'AREA':<30} {'STATUS':<10} {'ITER':<6} REPO")
        typer.echo("-" * 90)

        for j in jobs:
            status_c = _status_color(j.status, c)
            goal_str = _job_display(j)
            if len(goal_str) > 30:
                goal_str = goal_str[:27] + "..."

            repo_short = str(j.repo).replace(str(Path.home()), "~")
            if len(repo_short) > 20:
                repo_short = "..." + repo_short[-17:]

            typer.echo(
                f"{j.short_id():<9} {j.type.value:<10} {goal_str:<30} "
                f"{status_c}{j.status.value:<10}{c['reset']} {j.iteration:<6} {repo_short}"
            )


def _print_job_detail(j: Job, c: dict[str, str]) -> None:
    """Print detailed info for a single job."""
    status_c = _status_color(j.status, c)

    typer.echo(f"{c['bold']}{j.area}{c['reset']} ({j.short_id()})")
    typer.echo(f"  Type: {j.type.value}")
    typer.echo(f"  Status: {status_c}{j.status.value}{c['reset']}")
    typer.echo(f"  Repo: {j.repo}")
    typer.echo(f"  Job main: {j.job_main}")
    typer.echo(f"  Goals: {j.goals_display}")
    typer.echo(f"  Flow: {j.flow_display}")
    typer.echo(f"  Iteration: {j.iteration}")
    if j.project_file:
        typer.echo(f"  Project: {j.project_file}")
    if j.pathset:
        typer.echo(f"  Pathset: {j.pathset}")
    if j.cron:
        typer.echo(f"  Cron: {j.cron}")

    # Show recent runs
    runs = get_job_runs(j.id, limit=5)
    if runs:
        typer.echo(f"\n  {c['dim']}Recent runs:{c['reset']}")
        for run in runs:
            run_status_c = _status_color(run.status, c)
            pr_info = f" → {run.pr_url}" if run.pr_url else ""
            typer.echo(
                f"    #{run.iteration} {run_status_c}{run.status.value}{c['reset']}"
                f" {run.started_at.strftime('%Y-%m-%d %H:%M')}{pr_info}"
            )


@app.command()
def stop(
    job_id: str = typer.Argument(None, help="Job ID to stop (omit with --all)"),
    all_jobs: bool = typer.Option(False, "--all", help="Stop all running jobs"),
    force: bool = typer.Option(False, "-f", "--force", help="Force kill (SIGKILL)"),
):
    """Stop a running job."""
    c = _colors()

    if all_jobs:
        # Stop all running jobs
        stopped = 0
        for j in list_jobs():
            if j.status == JobStatus.RUNNING:
                if stop_job(j.id, force=force):
                    msg = f"{c['yellow']}Stopped{c['reset']} {_job_display(j)}"
                    typer.echo(f"{msg} ({j.short_id()})")
                    stopped += 1
        if stopped == 0:
            typer.echo(f"{c['dim']}No running jobs to stop{c['reset']}")
        else:
            typer.echo(f"\nStopped {stopped} job{'s' if stopped != 1 else ''}")
        return

    if not job_id:
        typer.echo(f"{c['red']}Error:{c['reset']} Provide a job ID or use --all", err=True)
        raise typer.Exit(1)

    j = get_job(job_id)
    if not j:
        typer.echo(f"{c['red']}Error:{c['reset']} Job '{job_id}' not found", err=True)
        raise typer.Exit(1)

    if stop_job(j.id, force=force):
        msg = f"{c['yellow']}Stopped{c['reset']} {c['bold']}{_job_display(j)}{c['reset']}"
        typer.echo(f"{msg} ({j.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to stop job", err=True)
        raise typer.Exit(1)


@app.command()
def prs(
    job_id: str = typer.Argument(..., help="Job ID"),
    limit: int = typer.Option(10, "-n", "--limit", help="Number of PRs to show"),
):
    """Show PRs created by a job."""
    c = _colors()

    j = get_job(job_id)
    if not j:
        typer.echo(f"{c['red']}Error:{c['reset']} Job '{job_id}' not found", err=True)
        raise typer.Exit(1)

    runs = get_job_runs(j.id, limit=limit)
    runs_with_prs = [r for r in runs if r.pr_url]

    if not runs_with_prs:
        typer.echo(f"{c['dim']}No PRs found for '{j.area}'{c['reset']}")
        return

    typer.echo(f"{c['bold']}{j.area}{c['reset']} PRs ({j.short_id()})")
    typer.echo("")

    for run in runs_with_prs:
        status_c = _status_color(run.status, c)
        typer.echo(
            f"  #{run.iteration:<3} {status_c}{run.status.value:<10}{c['reset']} "
            f"{c['dim']}{run.started_at.strftime('%Y-%m-%d')}{c['reset']}  {run.pr_url}"
        )


@app.command()
def rm(
    job_id: str = typer.Argument(..., help="Job ID to remove"),
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove a job and its history."""
    c = _colors()

    j = get_job(job_id)
    if not j:
        typer.echo(f"{c['red']}Error:{c['reset']} Job '{job_id}' not found", err=True)
        raise typer.Exit(1)

    if j.status == JobStatus.RUNNING:
        typer.echo(
            f"{c['red']}Error:{c['reset']} Job is running. Stop it first with: lfd stop {job_id}",
            err=True,
        )
        raise typer.Exit(1)

    if not force:
        confirm = typer.confirm(f"Delete job '{j.area}' ({j.short_id()})?")
        if not confirm:
            raise typer.Abort()

    if delete_job(j.id):
        typer.echo(f"Deleted job: {j.area} ({j.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to delete job", err=True)
        raise typer.Exit(1)


@app.command()
def logs(
    job_id: str = typer.Argument(..., help="Job ID"),
    follow: bool = typer.Option(False, "-f", "--follow", help="Follow output (like tail -f)"),
    lines: int = typer.Option(50, "-n", "--lines", help="Number of lines to show"),
):
    """Show logs for a job's current run."""
    c = _colors()
    j = get_job(job_id)
    if not j:
        typer.echo(f"{c['red']}Error:{c['reset']} Job '{job_id}' not found", err=True)
        raise typer.Exit(1)

    # Get latest run for this job
    runs = get_job_runs(j.id, limit=1)
    if not runs:
        typer.echo(f"{c['dim']}No runs found for '{j.area}'{c['reset']}")
        return

    run = runs[0]
    if not run.worktree:
        typer.echo(f"{c['dim']}No worktree for current run{c['reset']}")
        return

    # Find log file
    worktree_path = Path(run.worktree)
    log_dir = get_log_dir(worktree_path)

    # Find most recent log file for this session
    log_files = sorted(log_dir.glob("*.log"), key=lambda p: p.stat().st_mtime, reverse=True)
    if not log_files:
        typer.echo(f"{c['dim']}No log files found in {log_dir}{c['reset']}")
        return

    log_file = log_files[0]
    typer.echo(f"{c['dim']}Log: {log_file}{c['reset']}")
    typer.echo("")

    if follow:
        # Use tail -f for following
        subprocess.run(["tail", "-f", str(log_file)])
    else:
        # Show last N lines
        subprocess.run(["tail", f"-{lines}", str(log_file)])


@app.command("list-goals")
def list_goals_cmd():
    """Show available goals in current repo."""
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    goals_dir = repo / ".lf" / "goals"
    if not goals_dir.exists():
        typer.echo(f"{c['dim']}No goals directory found at {goals_dir}{c['reset']}")
        typer.echo("Create one with: mkdir -p .lf/goals && echo '# My Goal' > .lf/goals/my-goal.md")
        return

    goals = list_goals(repo)
    if not goals:
        typer.echo(f"{c['dim']}No goals found in {goals_dir}{c['reset']}")
        return

    typer.echo(f"Goals in {c['dim']}{goals_dir}/{c['reset']}")
    typer.echo("")

    for goal_name in goals:
        goal = load_goal(repo, goal_name)
        if goal:
            area_str = f"area: [{', '.join(goal.area)}]" if goal.area else ""
            pipeline_str = f"pipeline: {goal.pipeline}" if goal.pipeline else ""
            details = "  ".join(filter(None, [area_str, pipeline_str]))
            typer.echo(f"  {c['bold']}{goal_name:<20}{c['reset']} {c['dim']}{details}{c['reset']}")
        else:
            typer.echo(f"  {c['bold']}{goal_name:<20}{c['reset']}")

    typer.echo("")
    typer.echo(f"{len(goals)} goal{'s' if len(goals) != 1 else ''} found")


def main() -> None:
    """Entry point for lfd command."""
    if len(sys.argv) == 1:
        sys.argv.append("status")
    app()
