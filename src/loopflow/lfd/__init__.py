"""lfd: Loopflow daemon.

Commands for managing agent loops.
"""

import asyncio
import sys
from pathlib import Path

import typer

from loopflow.lf.goals import goal_exists, list_goals, load_goal
from loopflow.lfd.db import (
    delete_loop,
    get_loop,
    get_loop_runs,
    list_loops,
    update_loop_status,
)
from loopflow.lfd.launchd import install as launchd_install, is_running, uninstall as launchd_uninstall
from loopflow.lfd.loops import create_loop, get_repo_from_cwd, start_loop, stop_loop
from loopflow.lfd.models import Loop, LoopStatus, LoopType
from loopflow.lfd.server import run_server

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"

app = typer.Typer(help="Loopflow daemon - agent loops")


def _use_color() -> bool:
    return sys.stdout.isatty()


def _colors() -> dict[str, str]:
    if not _use_color():
        return {"cyan": "", "bold": "", "dim": "", "yellow": "", "green": "", "red": "", "reset": ""}
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


def _goal_display(lp: Loop) -> str:
    """Return goal name with area suffix if present."""
    if lp.area_slug:
        return f"{lp.goal_name} ({lp.area_slug}/)"
    return lp.goal_name


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
    goals: list[str] = typer.Argument(None, help="Goal names to start (all idle if omitted)"),
    all_loops: bool = typer.Option(False, "-a", "--all", help="Include waiting loops"),
):
    """Start multiple loops in parallel.

    Without arguments, starts all idle loops. With --all, also starts waiting loops.
    """
    c = _colors()
    repo = get_repo_from_cwd()

    # Get loops to start
    if goals:
        # Start specific goals
        loops_to_start = []
        for goal in goals:
            lp = None
            for loop in list_loops(repo=repo):
                if loop.goal_name == goal:
                    lp = loop
                    break
            if not lp:
                typer.echo(f"{c['yellow']}Warning:{c['reset']} Loop '{goal}' not found, skipping", err=True)
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
            typer.echo(f"{c['green']}Started{c['reset']} {c['bold']}{lp.goal_name}{c['reset']} ({lp.short_id()})")
            started += 1
        elif result.reason == "already_running":
            typer.echo(f"{c['dim']}Already running:{c['reset']} {lp.goal_name}")
        elif result.reason == "waiting":
            typer.echo(f"{c['yellow']}Waiting:{c['reset']} {lp.goal_name} ({result.outstanding} outstanding)")
        else:
            typer.echo(f"{c['red']}Failed:{c['reset']} {lp.goal_name}")

    typer.echo(f"\nStarted {started}/{len(loops_to_start)} loops")


# Loop commands


@app.command()
def loop(
    goal: str = typer.Argument(..., help="Goal name from .lf/goals/"),
    area: str = typer.Option(None, "-a", "--area", help="Area of responsibility (pathset override)"),
    limit: int = typer.Option(None, "-l", "--limit", help="PR limit override"),
    merge_mode: str = typer.Option(None, "--merge-mode", help="Merge mode: pr or land"),
    foreground: bool = typer.Option(False, "-f", "--foreground", help="Run in foreground"),
):
    """Start a continuous homeostasis loop."""
    c = _colors()
    repo = get_repo_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Validate goal exists
    if not goal_exists(repo, goal):
        typer.echo(f"{c['red']}Error:{c['reset']} Goal '{goal}' not found in {repo}/.lf/goals/", err=True)
        available = list_goals(repo)
        if available:
            typer.echo(f"Available goals: {', '.join(available)}")
        raise typer.Exit(1)

    # Validate merge_mode if specified
    if merge_mode and merge_mode not in ("pr", "land"):
        typer.echo(f"{c['red']}Error:{c['reset']} merge-mode must be 'pr' or 'land'", err=True)
        raise typer.Exit(1)

    # Create or get loop
    lp = create_loop(LoopType.LOOP, goal, repo, area=area)

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
            typer.echo(f"{c['green']}Completed{c['reset']} loop {c['bold']}{goal}{c['reset']} ({lp.short_id()})")
        else:
            typer.echo(f"{c['green']}Started{c['reset']} loop {c['bold']}{goal}{c['reset']} ({lp.short_id()})")
            typer.echo(f"  Repo: {repo}")
            typer.echo(f"  Loop main: {lp.loop_main}")
            typer.echo(f"  PR limit: {lp.pr_limit}")
            if area:
                typer.echo(f"  Area: {area}")
    elif result.reason == "already_running":
        typer.echo(f"Loop already running (PID {lp.pid})")
        raise typer.Exit(1)
    elif result.reason == "waiting":
        typer.echo(f"{c['yellow']}Waiting:{c['reset']} {result.outstanding} outstanding PRs (limit {lp.pr_limit})")
        typer.echo(f"Run 'lfops land --squash' from {lp.loop_main} worktree to land work to main")
        raise typer.Exit(0)
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to start loop", err=True)
        raise typer.Exit(1)


@app.command()
def flow(
    goal: str = typer.Argument(..., help="Goal name from .lf/goals/"),
    project: str = typer.Option(None, "--project", "-p", help="Project/prompt file path"),
    paste: bool = typer.Option(False, "-v", "--paste", help="Include clipboard as prompt"),
    area: str = typer.Option(None, "-r", help="Area of responsibility (pathset override)"),
):
    """Start a one-off flow (runs once then stops).

    The flow runs a single iteration of the goal's pipeline, optionally with
    additional context from a project file or clipboard.
    """
    c = _colors()
    repo = get_repo_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Validate goal exists
    if not goal_exists(repo, goal):
        typer.echo(f"{c['red']}Error:{c['reset']} Goal '{goal}' not found", err=True)
        raise typer.Exit(1)

    # Resolve project file if specified
    project_file = None
    if project:
        project_path = Path(project)
        if not project_path.is_absolute():
            project_path = repo / project
        if not project_path.exists():
            typer.echo(f"{c['red']}Error:{c['reset']} Project file not found: {project}", err=True)
            raise typer.Exit(1)
        project_file = str(project_path)

    # Handle clipboard paste - write to temp file if provided
    if paste:
        import subprocess
        result = subprocess.run(["pbpaste"], capture_output=True, text=True)
        if result.returncode == 0 and result.stdout.strip():
            import tempfile
            with tempfile.NamedTemporaryFile(mode="w", suffix=".md", delete=False) as f:
                f.write(result.stdout)
                if project_file:
                    # Append clipboard to project file content
                    typer.echo(f"{c['yellow']}Note:{c['reset']} Both -p and -v provided; clipboard appended to project file")
                else:
                    project_file = f.name

    # Create or get loop
    lp = create_loop(LoopType.FLOW, goal, repo, area=area, project_file=project_file)

    # Start it
    if start_loop(lp.id):
        typer.echo(f"{c['green']}Started{c['reset']} flow {c['bold']}{goal}{c['reset']} ({lp.short_id()})")
        if project:
            typer.echo(f"  Project: {project}")
        if paste:
            typer.echo(f"  Clipboard: included")
        if area:
            typer.echo(f"  Area: {area}")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to start flow", err=True)
        raise typer.Exit(1)


@app.command()
def subscribe(
    pathset: str = typer.Argument(..., help="Pathset to watch (comma-separated)"),
    goal: str = typer.Argument(..., help="Goal name from .lf/goals/"),
    area: str = typer.Option(None, "-r", help="Area of responsibility override"),
):
    """Subscribe to pathset changes on main."""
    c = _colors()
    repo = get_repo_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    if not goal_exists(repo, goal):
        typer.echo(f"{c['red']}Error:{c['reset']} Goal '{goal}' not found", err=True)
        raise typer.Exit(1)

    # Create subscription
    lp = create_loop(LoopType.SUBSCRIBE, goal, repo, area=area, pathset=pathset)

    typer.echo(f"{c['green']}Subscribed{c['reset']} {c['bold']}{goal}{c['reset']} to {pathset} ({lp.short_id()})")
    typer.echo(f"  Will run when {pathset} changes on main")


@app.command()
def schedule(
    cron_expr: str = typer.Argument(..., help="Cron expression (e.g., '0 9 * * *')"),
    goal: str = typer.Argument(..., help="Goal name from .lf/goals/"),
    project: str = typer.Option(None, "--project", "-p", help="Project file path"),
    area: str = typer.Option(None, "-r", help="Area of responsibility override"),
):
    """Schedule a loop to run on cron."""
    c = _colors()
    repo = get_repo_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    if not goal_exists(repo, goal):
        typer.echo(f"{c['red']}Error:{c['reset']} Goal '{goal}' not found", err=True)
        raise typer.Exit(1)

    project_file = None
    if project:
        project_path = Path(project)
        if not project_path.is_absolute():
            project_path = repo / project
        if not project_path.exists():
            typer.echo(f"{c['red']}Error:{c['reset']} Project file not found: {project}", err=True)
            raise typer.Exit(1)
        project_file = str(project_path)

    # Create schedule
    lp = create_loop(LoopType.SCHEDULE, goal, repo, area=area, cron=cron_expr, project_file=project_file)

    typer.echo(f"{c['green']}Scheduled{c['reset']} {c['bold']}{goal}{c['reset']} ({lp.short_id()})")
    typer.echo(f"  Cron: {cron_expr}")
    if project:
        typer.echo(f"  Project: {project}")


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
            typer.echo(f"Start one with: lfd loop <goal>")
            return

        typer.echo(f"{'ID':<9} {'TYPE':<10} {'GOAL':<30} {'STATUS':<10} {'ITER':<6} REPO")
        typer.echo("-" * 90)

        for lp in loops:
            status_c = _status_color(lp.status, c)
            goal_str = _goal_display(lp)
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

    typer.echo(f"{c['bold']}{lp.goal_name}{c['reset']} ({lp.short_id()})")
    typer.echo(f"  Type: {lp.type.value}")
    typer.echo(f"  Status: {status_c}{lp.status.value}{c['reset']}")
    typer.echo(f"  Repo: {lp.repo}")
    typer.echo(f"  Loop main: {lp.loop_main}")
    typer.echo(f"  Iteration: {lp.iteration}")

    if lp.area:
        typer.echo(f"  Area: {lp.area}")
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
                    typer.echo(f"{c['yellow']}Stopped{c['reset']} {_goal_display(lp)} ({lp.short_id()})")
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
        typer.echo(f"{c['yellow']}Stopped{c['reset']} {c['bold']}{_goal_display(lp)}{c['reset']} ({lp.short_id()})")
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
        typer.echo(f"{c['dim']}No PRs found for '{lp.goal_name}'{c['reset']}")
        return

    typer.echo(f"{c['bold']}{lp.goal_name}{c['reset']} PRs ({lp.short_id()})")
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
        typer.echo(f"{c['red']}Error:{c['reset']} Loop is running. Stop it first with: lfd stop {loop_id}", err=True)
        raise typer.Exit(1)

    if not force:
        confirm = typer.confirm(f"Delete loop '{lp.goal_name}' ({lp.short_id()})?")
        if not confirm:
            raise typer.Abort()

    if delete_loop(lp.id):
        typer.echo(f"Deleted loop: {lp.goal_name} ({lp.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to delete loop", err=True)
        raise typer.Exit(1)


@app.command("list-goals")
def list_goals_cmd():
    """Show available goals in current repo."""
    c = _colors()
    repo = get_repo_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    goals_dir = repo / ".lf" / "goals"
    if not goals_dir.exists():
        typer.echo(f"{c['dim']}No goals directory found at {goals_dir}{c['reset']}")
        typer.echo(f"Create one with: mkdir -p .lf/goals && echo '# My Goal' > .lf/goals/my-goal.md")
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
