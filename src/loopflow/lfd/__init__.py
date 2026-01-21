"""lfd: Loopflow daemon.

Commands for managing agent loops.
"""

import asyncio
import sys
from pathlib import Path

import typer

from loopflow.lf.flows import load_flow
from loopflow.lf.goals import goal_exists, list_goals, load_goal
from loopflow.lfd.db import (
    delete_loop,
    get_loop,
    get_loop_runs,
    list_loops,
)
from loopflow.lfd.launchd import install as launchd_install
from loopflow.lfd.launchd import is_running
from loopflow.lfd.launchd import uninstall as launchd_uninstall
from loopflow.lfd.loops import create_loop, get_wt_from_cwd, start_loop, stop_loop
from loopflow.lfd.models import Loop, LoopStatus, LoopType
from loopflow.lfd.server import run_server

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"

app = typer.Typer(help="Loopflow daemon - agent loops")


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


def _status_color(status: LoopStatus, c: dict[str, str]) -> str:
    if status == LoopStatus.RUNNING:
        return c["green"]
    elif status == LoopStatus.ERROR:
        return c["red"]
    elif status == LoopStatus.WAITING:
        return c["yellow"]
    return c["dim"]


def _loop_display(lp: Loop) -> str:
    """Return area and goals for display."""
    return f"{lp.area} [{lp.flow_display}] [{lp.goals_display}]"


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
    all_loops: bool = typer.Option(False, "--all", help="Include waiting loops"),
):
    """Start multiple loops in parallel.

    Without arguments, starts all idle loops. With --all, also starts waiting loops.
    """
    c = _colors()
    repo = get_wt_from_cwd()

    # Get loops to start
    if areas:
        # Start specific areas
        loops_to_start = []
        for area in areas:
            lp = None
            for loop in list_loops(repo=repo):
                if loop.area == area:
                    lp = loop
                    break
            if not lp:
                typer.echo(
                    f"{c['yellow']}Warning:{c['reset']} Loop for '{area}' not found, skipping",
                    err=True,
                )
            else:
                loops_to_start.append(lp)
    else:
        # Start all eligible loops
        loops_to_start = []
        for lp in list_loops(repo=repo):
            if lp.status == LoopStatus.IDLE:
                loops_to_start.append(lp)
            elif all_loops and lp.status == LoopStatus.WAITING:
                loops_to_start.append(lp)

    if not loops_to_start:
        typer.echo(f"{c['dim']}No loops to start{c['reset']}")
        return

    # Start each loop
    started = 0
    for lp in loops_to_start:
        result = start_loop(lp.id)
        if result:
            msg = f"{c['green']}Started{c['reset']} {c['bold']}{lp.area}{c['reset']}"
            typer.echo(f"{msg} ({lp.short_id()})")
            started += 1
        elif result.reason == "already_running":
            typer.echo(f"{c['dim']}Already running:{c['reset']} {lp.area}")
        elif result.reason == "waiting":
            msg = f"{c['yellow']}Waiting:{c['reset']} {lp.area}"
            typer.echo(f"{msg} ({result.outstanding} outstanding)")
        else:
            typer.echo(f"{c['red']}Failed:{c['reset']} {lp.area}")

    typer.echo(f"\nStarted {started}/{len(loops_to_start)} loops")


# Loop commands


@app.command()
def loop(
    flow: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., Maestro/, src/, .)"),
    goals: list[str] = typer.Option(None, "-g", "--goal", help="Goal to add (repeatable)"),
    limit: int = typer.Option(None, "-l", "--limit", help="PR limit override"),
    merge_mode: str = typer.Option(None, "--merge-mode", help="Merge mode: pr or land"),
    foreground: bool = typer.Option(False, "-f", "--foreground", help="Run in foreground"),
):
    """Start a continuous homeostasis loop.

    Flow is required - specifies which flow to run from .lf/flows/.
    Area is required - scopes the work (e.g., Maestro/, src/, or . for whole repo).
    Goals are optional - adaptive mode is implicit if no mode goal is specified.

    Examples:
        lfd loop ship Maestro/                              # adaptive mode
        lfd loop ship Maestro/ -g product-engineer          # adaptive + role
        lfd loop ship Maestro/ -g product-engineer -g designer  # adaptive + roles
        lfd loop ship .                                     # whole repo
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
            "Use a path like Maestro/, src/, or . for whole repo.",
            err=True,
        )
        typer.echo(f"\nDid you mean: lfd loop {area}/ ?")
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

    # Create or get loop
    lp = create_loop(LoopType.LOOP, area, repo, goals=goals, flow=flow)

    # Override settings if specified
    changed = False
    if limit is not None:
        lp.pr_limit = limit
        changed = True
    if merge_mode:
        from loopflow.lfd.models import MergeMode

        lp.merge_mode = MergeMode(merge_mode)
        changed = True
    if changed:
        from loopflow.lfd.db import save_loop

        save_loop(lp)

    # Start it
    result = start_loop(lp.id, foreground=foreground)
    if result:
        if foreground:
            msg = f"{c['green']}Completed{c['reset']} loop {c['bold']}{area}{c['reset']}"
            typer.echo(f"{msg} ({lp.short_id()})")
        else:
            msg = f"{c['green']}Started{c['reset']} loop {c['bold']}{area}{c['reset']}"
            typer.echo(f"{msg} ({lp.short_id()})")
            typer.echo(f"  Repo: {repo}")
            typer.echo(f"  Loop main: {lp.loop_main}")
            typer.echo(f"  Goals: {lp.goals_display}")
            typer.echo(f"  Flow: {lp.flow_display}")
            typer.echo(f"  PR limit: {lp.pr_limit}")
    elif result.reason == "already_running":
        typer.echo(f"Loop already running (PID {lp.pid})")
        raise typer.Exit(1)
    elif result.reason == "waiting":
        msg = f"{c['yellow']}Waiting:{c['reset']} {result.outstanding} outstanding PRs"
        typer.echo(f"{msg} (limit {lp.pr_limit})")
        typer.echo(f"Run 'lfops land --squash' from {lp.loop_main} worktree to land work to main")
        raise typer.Exit(0)
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to start loop", err=True)
        raise typer.Exit(1)


@app.command()
def flow(
    flow_name: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., Maestro/, src/, .)"),
    goals: list[str] = typer.Option(None, "-g", "--goal", help="Goal to add (repeatable)"),
    paste: bool = typer.Option(False, "-v", "--paste", help="Include clipboard as prompt"),
):
    """Start a one-off flow (runs once then stops).

    Examples:
        lfd flow ship Maestro/                        # one-off adaptive iteration
        lfd flow ship Maestro/ -g product-engineer    # with role
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
            "Use a path like Maestro/, src/, or . for whole repo.",
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
        import subprocess

        result = subprocess.run(["pbpaste"], capture_output=True, text=True)
        if result.returncode == 0 and result.stdout.strip():
            import tempfile

            with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False) as f:
                f.write(result.stdout)
                project_file = f.name

    # Create or get loop
    lp = create_loop(
        LoopType.FLOW, area, repo, goals=goals, flow=flow_name, project_file=project_file
    )

    # Start it
    if start_loop(lp.id):
        typer.echo(
            f"{c['green']}Started{c['reset']} flow {c['bold']}{area}{c['reset']} ({lp.short_id()})"
        )
        typer.echo(f"  Goals: {lp.goals_display}")
        typer.echo(f"  Flow: {lp.flow_display}")
        if paste:
            typer.echo("  Clipboard: included")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to start flow", err=True)
        raise typer.Exit(1)


@app.command()
def subscribe(
    flow: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., Maestro/, src/, .)"),
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
    lp = create_loop(LoopType.SUBSCRIBE, area, repo, goals=goals, flow=flow, pathset=pathset)

    msg = f"{c['green']}Subscribed{c['reset']} {c['bold']}{area}{c['reset']} to {pathset}"
    typer.echo(f"{msg} ({lp.short_id()})")
    typer.echo(f"  Goals: {lp.goals_display}")
    typer.echo(f"  Flow: {lp.flow_display}")
    typer.echo("  Will run when paths change on main")


@app.command()
def schedule(
    flow: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., Maestro/, src/, .)"),
    cron_expr: str = typer.Argument(..., help="Cron expression (e.g., '0 9 * * *')"),
    goals: list[str] = typer.Option(None, "-g", "--goal", help="Goal to add (repeatable)"),
):
    """Schedule a loop to run on cron."""
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
    lp = create_loop(
        LoopType.SCHEDULE,
        area,
        repo,
        goals=goals,
        flow=flow,
        cron=cron_expr,
    )

    typer.echo(f"{c['green']}Scheduled{c['reset']} {c['bold']}{area}{c['reset']} ({lp.short_id()})")
    typer.echo(f"  Goals: {lp.goals_display}")
    typer.echo(f"  Flow: {lp.flow_display}")
    typer.echo(f"  Cron: {cron_expr}")


def _get_scheduler_status() -> dict | None:
    """Get scheduler status from daemon if running."""
    import json
    import socket

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
    loop_id: str = typer.Argument(None, help="Loop ID (optional, shows all if omitted)"),
    ids_only: bool = typer.Option(False, "--ids", help="Print loop IDs only (for scripting)"),
):
    """Show status of loops."""
    c = _colors()

    # Machine-readable output for scripting
    if ids_only:
        for lp in list_loops():
            typer.echo(lp.id)
        return

    if loop_id:
        lp = get_loop(loop_id)
        if not lp:
            typer.echo(f"{c['red']}Error:{c['reset']} Loop '{loop_id}' not found", err=True)
            raise typer.Exit(1)
        _print_loop_detail(lp, c)
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

        loops = list_loops()
        if not loops:
            typer.echo(f"{c['dim']}No loops configured{c['reset']}")
            typer.echo("Start one with: lfd loop <goal>")
            return

        typer.echo(f"{'ID':<9} {'TYPE':<10} {'AREA':<30} {'STATUS':<10} {'ITER':<6} REPO")
        typer.echo("-" * 90)

        for lp in loops:
            status_c = _status_color(lp.status, c)
            goal_str = _loop_display(lp)
            if len(goal_str) > 30:
                goal_str = goal_str[:27] + "..."

            repo_short = str(lp.repo).replace(str(Path.home()), "~")
            if len(repo_short) > 20:
                repo_short = "..." + repo_short[-17:]

            typer.echo(
                f"{lp.short_id():<9} {lp.type.value:<10} {goal_str:<30} "
                f"{status_c}{lp.status.value:<10}{c['reset']} {lp.iteration:<6} {repo_short}"
            )


def _print_loop_detail(lp: Loop, c: dict[str, str]) -> None:
    """Print detailed info for a single loop."""
    status_c = _status_color(lp.status, c)

    typer.echo(f"{c['bold']}{lp.area}{c['reset']} ({lp.short_id()})")
    typer.echo(f"  Type: {lp.type.value}")
    typer.echo(f"  Status: {status_c}{lp.status.value}{c['reset']}")
    typer.echo(f"  Repo: {lp.repo}")
    typer.echo(f"  Loop main: {lp.loop_main}")
    typer.echo(f"  Goals: {lp.goals_display}")
    typer.echo(f"  Flow: {lp.flow_display}")
    typer.echo(f"  Iteration: {lp.iteration}")
    if lp.project_file:
        typer.echo(f"  Project: {lp.project_file}")
    if lp.pathset:
        typer.echo(f"  Pathset: {lp.pathset}")
    if lp.cron:
        typer.echo(f"  Cron: {lp.cron}")

    # Show recent runs
    runs = get_loop_runs(lp.id, limit=5)
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
    loop_id: str = typer.Argument(None, help="Loop ID to stop (omit with --all)"),
    all_loops: bool = typer.Option(False, "--all", help="Stop all running loops"),
    force: bool = typer.Option(False, "-f", "--force", help="Force kill (SIGKILL)"),
):
    """Stop a running loop."""
    c = _colors()

    if all_loops:
        # Stop all running loops
        stopped = 0
        for lp in list_loops():
            if lp.status == LoopStatus.RUNNING:
                if stop_loop(lp.id, force=force):
                    msg = f"{c['yellow']}Stopped{c['reset']} {_loop_display(lp)}"
                    typer.echo(f"{msg} ({lp.short_id()})")
                    stopped += 1
        if stopped == 0:
            typer.echo(f"{c['dim']}No running loops to stop{c['reset']}")
        else:
            typer.echo(f"\nStopped {stopped} loop{'s' if stopped != 1 else ''}")
        return

    if not loop_id:
        typer.echo(f"{c['red']}Error:{c['reset']} Provide a loop ID or use --all", err=True)
        raise typer.Exit(1)

    lp = get_loop(loop_id)
    if not lp:
        typer.echo(f"{c['red']}Error:{c['reset']} Loop '{loop_id}' not found", err=True)
        raise typer.Exit(1)

    if stop_loop(lp.id, force=force):
        msg = f"{c['yellow']}Stopped{c['reset']} {c['bold']}{_loop_display(lp)}{c['reset']}"
        typer.echo(f"{msg} ({lp.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to stop loop", err=True)
        raise typer.Exit(1)


@app.command()
def prs(
    loop_id: str = typer.Argument(..., help="Loop ID"),
    limit: int = typer.Option(10, "-n", "--limit", help="Number of PRs to show"),
):
    """Show PRs created by a loop."""
    c = _colors()

    lp = get_loop(loop_id)
    if not lp:
        typer.echo(f"{c['red']}Error:{c['reset']} Loop '{loop_id}' not found", err=True)
        raise typer.Exit(1)

    runs = get_loop_runs(lp.id, limit=limit)
    runs_with_prs = [r for r in runs if r.pr_url]

    if not runs_with_prs:
        typer.echo(f"{c['dim']}No PRs found for '{lp.area}'{c['reset']}")
        return

    typer.echo(f"{c['bold']}{lp.area}{c['reset']} PRs ({lp.short_id()})")
    typer.echo("")

    for run in runs_with_prs:
        status_c = _status_color(run.status, c)
        typer.echo(
            f"  #{run.iteration:<3} {status_c}{run.status.value:<10}{c['reset']} "
            f"{c['dim']}{run.started_at.strftime('%Y-%m-%d')}{c['reset']}  {run.pr_url}"
        )


@app.command()
def rm(
    loop_id: str = typer.Argument(..., help="Loop ID to remove"),
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove a loop and its history."""
    c = _colors()

    lp = get_loop(loop_id)
    if not lp:
        typer.echo(f"{c['red']}Error:{c['reset']} Loop '{loop_id}' not found", err=True)
        raise typer.Exit(1)

    if lp.status == LoopStatus.RUNNING:
        typer.echo(
            f"{c['red']}Error:{c['reset']} Loop is running. Stop it first with: lfd stop {loop_id}",
            err=True,
        )
        raise typer.Exit(1)

    if not force:
        confirm = typer.confirm(f"Delete loop '{lp.area}' ({lp.short_id()})?")
        if not confirm:
            raise typer.Abort()

    if delete_loop(lp.id):
        typer.echo(f"Deleted loop: {lp.area} ({lp.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to delete loop", err=True)
        raise typer.Exit(1)


@app.command()
def logs(
    loop_id: str = typer.Argument(..., help="Loop ID"),
    follow: bool = typer.Option(False, "-f", "--follow", help="Follow output (like tail -f)"),
    lines: int = typer.Option(50, "-n", "--lines", help="Number of lines to show"),
):
    """Show logs for a loop's current run."""
    import subprocess

    c = _colors()
    lp = get_loop(loop_id)
    if not lp:
        typer.echo(f"{c['red']}Error:{c['reset']} Loop '{loop_id}' not found", err=True)
        raise typer.Exit(1)

    # Get latest run for this loop
    runs = get_loop_runs(lp.id, limit=1)
    if not runs:
        typer.echo(f"{c['dim']}No runs found for '{lp.area}'{c['reset']}")
        return

    run = runs[0]
    if not run.worktree:
        typer.echo(f"{c['dim']}No worktree for current run{c['reset']}")
        return

    # Find log file
    from loopflow.lf.logging import get_log_dir

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
