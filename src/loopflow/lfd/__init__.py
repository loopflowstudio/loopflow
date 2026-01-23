"""lfd: Loopflow daemon.

Commands for managing agent loops, subscriptions, and schedules.
"""

import asyncio
import json
import socket
import subprocess
import sys
from pathlib import Path

import typer

from loopflow.lf.flows import load_flow
from loopflow.lf.logging import get_log_dir
from loopflow.lf.voices import list_voices, load_voice, voice_exists
from loopflow.lfd.daemon.launchd import install as launchd_install
from loopflow.lfd.daemon.launchd import is_running
from loopflow.lfd.daemon.launchd import uninstall as launchd_uninstall
from loopflow.lfd.daemon.server import run_server
from loopflow.lfd.db import list_all_triggers
from loopflow.lfd.models import Loop, MergeMode, Schedule, Subscription, Trigger, TriggerStatus
from loopflow.lfd.runs.loop import (
    create_loop,
    delete_loop,
    get_loop,
    get_wt_from_cwd,
    list_loops,
    start_loop,
    stop_loop,
)
from loopflow.lfd.runs.run import list_runs_for_trigger
from loopflow.lfd.runs.schedule import create_schedule, delete_schedule, get_schedule
from loopflow.lfd.runs.subscription import (
    create_subscription,
    delete_subscription,
    get_subscription,
)

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"

app = typer.Typer(help="Loopflow daemon - agent loops and triggers")


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


def _status_color(status: TriggerStatus, c: dict[str, str]) -> str:
    if status == TriggerStatus.RUNNING:
        return c["green"]
    elif status == TriggerStatus.ERROR:
        return c["red"]
    elif status == TriggerStatus.WAITING:
        return c["yellow"]
    return c["dim"]


def _trigger_display(trigger: Trigger) -> str:
    """Return area and voices for display."""
    return f"{trigger.area} [{trigger.flow_display}] [{trigger.voices_display}]"


def _trigger_type_name(trigger: Trigger) -> str:
    """Return the type name for a trigger."""
    if isinstance(trigger, Loop):
        return "loop"
    elif isinstance(trigger, Subscription):
        return "subscription"
    elif isinstance(trigger, Schedule):
        return "schedule"
    return "unknown"


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
    all_triggers: bool = typer.Option(False, "--all", help="Include waiting triggers"),
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
            loop = None
            for lp in list_loops(repo=repo):
                if lp.area == area:
                    loop = lp
                    break
            if not loop:
                typer.echo(
                    f"{c['yellow']}Warning:{c['reset']} Loop for '{area}' not found, skipping",
                    err=True,
                )
            else:
                loops_to_start.append(loop)
    else:
        # Start all eligible loops
        loops_to_start = []
        for loop in list_loops(repo=repo):
            if loop.status == TriggerStatus.IDLE:
                loops_to_start.append(loop)
            elif all_triggers and loop.status == TriggerStatus.WAITING:
                loops_to_start.append(loop)

    if not loops_to_start:
        typer.echo(f"{c['dim']}No loops to start{c['reset']}")
        return

    # Start each loop
    started = 0
    for loop in loops_to_start:
        result = start_loop(loop.id)
        if result:
            msg = f"{c['green']}Started{c['reset']} {c['bold']}{loop.area}{c['reset']}"
            typer.echo(f"{msg} ({loop.short_id()})")
            started += 1
        elif result.reason == "already_running":
            typer.echo(f"{c['dim']}Already running:{c['reset']} {loop.area}")
        elif result.reason == "waiting":
            msg = f"{c['yellow']}Waiting:{c['reset']} {loop.area}"
            typer.echo(f"{msg} ({result.outstanding} outstanding)")
        else:
            typer.echo(f"{c['red']}Failed:{c['reset']} {loop.area}")

    typer.echo(f"\nStarted {started}/{len(loops_to_start)} loops")


# Loop command


@app.command()
def loop(
    flow: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., swift/, src/, .)"),
    voices: list[str] = typer.Option(None, "-L", "--voice", help="Voice to add (repeatable)"),
    limit: int = typer.Option(None, "-l", "--limit", help="PR limit override"),
    merge_mode: str = typer.Option(None, "--merge-mode", help="Merge mode: pr or land"),
    foreground: bool = typer.Option(False, "-f", "--foreground", help="Run in foreground"),
):
    """Start a continuous homeostasis loop.

    Flow is required - specifies which flow to run from .lf/flows/.
    Area is required - scopes the work (e.g., swift/, src/, or . for whole repo).
    Voices are optional - adaptive mode is implicit if no mode voice is specified.

    Examples:
        lfd loop ship swift/                              # adaptive mode
        lfd loop ship swift/ -L product-engineer          # adaptive + role
        lfd loop ship swift/ -L product-engineer -L designer  # adaptive + roles
        lfd loop ship .                                   # whole repo
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
        typer.echo(f"\nDid you mean: lfd loop {flow} {area}/ ?")
        raise typer.Exit(1)

    voices = voices or []

    # Validate voices exist
    for voice_name in voices:
        if not voice_exists(repo, voice_name):
            typer.echo(
                f"{c['red']}Error:{c['reset']} Voice '{voice_name}' not found",
                err=True,
            )
            available = list_voices(repo)
            if available:
                typer.echo(f"Available voices: {', '.join(available)}")
            raise typer.Exit(1)

    flow = _validate_flow(repo, flow, c)

    # Validate merge_mode if specified
    if merge_mode and merge_mode not in ("pr", "land"):
        typer.echo(f"{c['red']}Error:{c['reset']} merge-mode must be 'pr' or 'land'", err=True)
        raise typer.Exit(1)

    # Create or get loop
    pr_limit = limit if limit is not None else 5
    mm = MergeMode(merge_mode) if merge_mode else MergeMode.PR

    lp = create_loop(area, repo, flow, voices=voices, pr_limit=pr_limit, merge_mode=mm)

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
            typer.echo(f"  Main branch: {lp.main_branch}")
            typer.echo(f"  Voices: {lp.voices_display}")
            typer.echo(f"  Flow: {lp.flow_display}")
            typer.echo(f"  PR limit: {lp.pr_limit}")
    elif result.reason == "already_running":
        typer.echo(f"Loop already running (PID {lp.pid})")
        raise typer.Exit(1)
    elif result.reason == "waiting":
        msg = f"{c['yellow']}Waiting:{c['reset']} {result.outstanding} outstanding PRs"
        typer.echo(f"{msg} (limit {lp.pr_limit})")
        typer.echo(f"Run 'lfops land --squash' from {lp.main_branch} worktree to land work to main")
        raise typer.Exit(0)
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to start loop", err=True)
        raise typer.Exit(1)


@app.command()
def run(
    flow_name: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., swift/, src/, .)"),
    voices: list[str] = typer.Option(None, "-L", "--voice", help="Voice to add (repeatable)"),
    paste: bool = typer.Option(False, "-v", "--paste", help="Include clipboard as prompt"),
):
    """Run a flow once (direct execution, no trigger).

    Examples:
        lfd run ship swift/                        # one-off adaptive iteration
        lfd run ship swift/ -L product-engineer    # with role
        lfd run ship . -v                          # whole repo with clipboard
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

    voices = voices or []

    # Validate voices exist
    for voice_name in voices:
        if not voice_exists(repo, voice_name):
            typer.echo(f"{c['red']}Error:{c['reset']} Voice '{voice_name}' not found", err=True)
            raise typer.Exit(1)

    flow_name = _validate_flow(repo, flow_name, c)

    # Handle clipboard paste
    if paste:
        result = subprocess.run(["pbpaste"], capture_output=True, text=True)
        if result.returncode == 0 and result.stdout.strip():
            typer.echo(f"{c['dim']}Clipboard content will be included{c['reset']}")

    # For now, create a temporary loop and run it once
    # TODO: Implement direct run without creating a loop
    lp = create_loop(area, repo, flow_name, voices=voices)

    # Start it in foreground (runs once)
    result = start_loop(lp.id, foreground=True)
    if result:
        typer.echo(
            f"{c['green']}Completed{c['reset']} run {c['bold']}{area}{c['reset']} ({lp.short_id()})"
        )
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to run", err=True)
        raise typer.Exit(1)


@app.command()
def subscribe(
    flow: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., swift/, src/, .)"),
    path: list[str] = typer.Option(
        ..., "-p", "-P", "--path", help="Paths to watch (repeatable, supports globs)"
    ),
    voices: list[str] = typer.Option(None, "-L", "--voice", help="Voice to add (repeatable)"),
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

    voices = voices or []
    for voice_name in voices:
        if not voice_exists(repo, voice_name):
            typer.echo(f"{c['red']}Error:{c['reset']} Voice '{voice_name}' not found", err=True)
            raise typer.Exit(1)

    flow = _validate_flow(repo, flow, c)

    # Convert path list to comma-separated pathset
    pathset = ",".join(path)

    # Create subscription
    sub = create_subscription(area, repo, flow, pathset, voices=voices)

    msg = f"{c['green']}Subscribed{c['reset']} {c['bold']}{area}{c['reset']} to {pathset}"
    typer.echo(f"{msg} ({sub.short_id()})")
    typer.echo(f"  Voices: {sub.voices_display}")
    typer.echo(f"  Flow: {sub.flow_display}")
    typer.echo("  Will run when paths change on main")


@app.command()
def schedule(
    flow: str = typer.Argument(..., help="Flow to run (from .lf/flows/<name>.py)"),
    area: str = typer.Argument(..., help="Area of responsibility (e.g., swift/, src/, .)"),
    cron_expr: str = typer.Argument(..., help="Cron expression (e.g., '0 9 * * *')"),
    voices: list[str] = typer.Option(None, "-L", "--voice", help="Voice to add (repeatable)"),
):
    """Schedule a flow to run on cron."""
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

    voices = voices or []
    for voice_name in voices:
        if not voice_exists(repo, voice_name):
            typer.echo(f"{c['red']}Error:{c['reset']} Voice '{voice_name}' not found", err=True)
            raise typer.Exit(1)

    flow = _validate_flow(repo, flow, c)

    # Create schedule
    sched = create_schedule(area, repo, flow, cron_expr, voices=voices)

    typer.echo(
        f"{c['green']}Scheduled{c['reset']} {c['bold']}{area}{c['reset']} ({sched.short_id()})"
    )
    typer.echo(f"  Voices: {sched.voices_display}")
    typer.echo(f"  Flow: {sched.flow_display}")
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
    trigger_id: str = typer.Argument(None, help="Trigger ID (optional, shows all if omitted)"),
    ids_only: bool = typer.Option(False, "--ids", help="Print trigger IDs only (for scripting)"),
):
    """Show status of loops, subscriptions, and schedules."""
    c = _colors()

    # Machine-readable output for scripting
    if ids_only:
        for trigger in list_all_triggers():
            typer.echo(trigger.id)
        return

    if trigger_id:
        # Try to find the trigger
        trigger = get_loop(trigger_id) or get_subscription(trigger_id) or get_schedule(trigger_id)
        if not trigger:
            typer.echo(f"{c['red']}Error:{c['reset']} Trigger '{trigger_id}' not found", err=True)
            raise typer.Exit(1)
        _print_trigger_detail(trigger, c)
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

        triggers = list_all_triggers()
        if not triggers:
            typer.echo(f"{c['dim']}No triggers configured{c['reset']}")
            typer.echo("Start a loop with: lfd loop <flow> <area>")
            return

        typer.echo(f"{'ID':<9} {'TYPE':<12} {'AREA':<30} {'STATUS':<10} {'ITER':<6} REPO")
        typer.echo("-" * 95)

        for trigger in triggers:
            status_c = _status_color(trigger.status, c)
            display_str = _trigger_display(trigger)
            if len(display_str) > 30:
                display_str = display_str[:27] + "..."

            repo_short = str(trigger.repo).replace(str(Path.home()), "~")
            if len(repo_short) > 20:
                repo_short = "..." + repo_short[-17:]

            type_name = _trigger_type_name(trigger)

            typer.echo(
                f"{trigger.short_id():<9} {type_name:<12} {display_str:<30} "
                f"{status_c}{trigger.status.value:<10}{c['reset']} "
                f"{trigger.iteration:<6} {repo_short}"
            )


def _print_trigger_detail(trigger: Trigger, c: dict[str, str]) -> None:
    """Print detailed info for a single trigger."""
    status_c = _status_color(trigger.status, c)
    type_name = _trigger_type_name(trigger)

    typer.echo(f"{c['bold']}{trigger.area}{c['reset']} ({trigger.short_id()})")
    typer.echo(f"  Type: {type_name}")
    typer.echo(f"  Status: {status_c}{trigger.status.value}{c['reset']}")
    typer.echo(f"  Repo: {trigger.repo}")
    typer.echo(f"  Main branch: {trigger.main_branch}")
    typer.echo(f"  Voices: {trigger.voices_display}")
    typer.echo(f"  Flow: {trigger.flow_display}")
    typer.echo(f"  Iteration: {trigger.iteration}")

    if isinstance(trigger, Subscription):
        typer.echo(f"  Pathset: {trigger.pathset}")
    elif isinstance(trigger, Schedule):
        typer.echo(f"  Cron: {trigger.cron}")

    # Show recent runs
    runs = list_runs_for_trigger(type_name, trigger.id, limit=5)
    if runs:
        typer.echo(f"\n  {c['dim']}Recent runs:{c['reset']}")
        for run in runs:
            from loopflow.lfd.models import RunStatus

            run_status_c = (
                c["green"]
                if run.status == RunStatus.COMPLETED
                else c["red"]
                if run.status == RunStatus.FAILED
                else c["dim"]
            )
            pr_info = f" → {run.pr_url}" if run.pr_url else ""
            started = run.started_at.strftime("%Y-%m-%d %H:%M") if run.started_at else "pending"
            typer.echo(
                f"    #{run.iteration} {run_status_c}{run.status.value}{c['reset']}"
                f" {started}{pr_info}"
            )


@app.command()
def stop(
    trigger_id: str = typer.Argument(None, help="Trigger ID to stop (omit with --all)"),
    all_triggers: bool = typer.Option(False, "--all", help="Stop all running triggers"),
    force: bool = typer.Option(False, "-f", "--force", help="Force kill (SIGKILL)"),
):
    """Stop a running loop."""
    c = _colors()

    if all_triggers:
        # Stop all running loops
        stopped = 0
        for loop in list_loops():
            if loop.status == TriggerStatus.RUNNING:
                if stop_loop(loop.id, force=force):
                    msg = f"{c['yellow']}Stopped{c['reset']} {_trigger_display(loop)}"
                    typer.echo(f"{msg} ({loop.short_id()})")
                    stopped += 1
        if stopped == 0:
            typer.echo(f"{c['dim']}No running loops to stop{c['reset']}")
        else:
            typer.echo(f"\nStopped {stopped} loop{'s' if stopped != 1 else ''}")
        return

    if not trigger_id:
        typer.echo(f"{c['red']}Error:{c['reset']} Provide a trigger ID or use --all", err=True)
        raise typer.Exit(1)

    loop = get_loop(trigger_id)
    if not loop:
        typer.echo(f"{c['red']}Error:{c['reset']} Loop '{trigger_id}' not found", err=True)
        raise typer.Exit(1)

    if stop_loop(loop.id, force=force):
        msg = f"{c['yellow']}Stopped{c['reset']} {c['bold']}{_trigger_display(loop)}{c['reset']}"
        typer.echo(f"{msg} ({loop.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to stop loop", err=True)
        raise typer.Exit(1)


@app.command()
def prs(
    trigger_id: str = typer.Argument(..., help="Trigger ID"),
    limit: int = typer.Option(10, "-n", "--limit", help="Number of PRs to show"),
):
    """Show PRs created by a trigger."""
    c = _colors()

    trigger = get_loop(trigger_id) or get_subscription(trigger_id) or get_schedule(trigger_id)
    if not trigger:
        typer.echo(f"{c['red']}Error:{c['reset']} Trigger '{trigger_id}' not found", err=True)
        raise typer.Exit(1)

    type_name = _trigger_type_name(trigger)
    runs = list_runs_for_trigger(type_name, trigger.id, limit=limit)
    runs_with_prs = [r for r in runs if r.pr_url]

    if not runs_with_prs:
        typer.echo(f"{c['dim']}No PRs found for '{trigger.area}'{c['reset']}")
        return

    typer.echo(f"{c['bold']}{trigger.area}{c['reset']} PRs ({trigger.short_id()})")
    typer.echo("")

    from loopflow.lfd.models import RunStatus

    for run in runs_with_prs:
        status_c = c["green"] if run.status == RunStatus.COMPLETED else c["red"]
        started = run.started_at.strftime("%Y-%m-%d") if run.started_at else "?"
        typer.echo(
            f"  #{run.iteration:<3} {status_c}{run.status.value:<10}{c['reset']} "
            f"{c['dim']}{started}{c['reset']}  {run.pr_url}"
        )


@app.command()
def rm(
    trigger_id: str = typer.Argument(..., help="Trigger ID to remove"),
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove a trigger and its history."""
    c = _colors()

    trigger = get_loop(trigger_id) or get_subscription(trigger_id) or get_schedule(trigger_id)
    if not trigger:
        typer.echo(f"{c['red']}Error:{c['reset']} Trigger '{trigger_id}' not found", err=True)
        raise typer.Exit(1)

    if trigger.status == TriggerStatus.RUNNING:
        typer.echo(
            f"{c['red']}Error:{c['reset']} Trigger is running. Stop it first with: "
            f"lfd stop {trigger_id}",
            err=True,
        )
        raise typer.Exit(1)

    if not force:
        confirm = typer.confirm(f"Delete trigger '{trigger.area}' ({trigger.short_id()})?")
        if not confirm:
            raise typer.Abort()

    # Delete based on type
    deleted = False
    if isinstance(trigger, Loop):
        deleted = delete_loop(trigger.id)
    elif isinstance(trigger, Subscription):
        deleted = delete_subscription(trigger.id)
    elif isinstance(trigger, Schedule):
        deleted = delete_schedule(trigger.id)

    if deleted:
        typer.echo(f"Deleted: {trigger.area} ({trigger.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to delete trigger", err=True)
        raise typer.Exit(1)


@app.command()
def logs(
    trigger_id: str = typer.Argument(..., help="Trigger ID"),
    follow: bool = typer.Option(False, "-f", "--follow", help="Follow output (like tail -f)"),
    lines: int = typer.Option(50, "-n", "--lines", help="Number of lines to show"),
):
    """Show logs for a trigger's current run."""
    c = _colors()
    trigger = get_loop(trigger_id) or get_subscription(trigger_id) or get_schedule(trigger_id)
    if not trigger:
        typer.echo(f"{c['red']}Error:{c['reset']} Trigger '{trigger_id}' not found", err=True)
        raise typer.Exit(1)

    # Get latest run for this trigger
    type_name = _trigger_type_name(trigger)
    runs = list_runs_for_trigger(type_name, trigger.id, limit=1)
    if not runs:
        typer.echo(f"{c['dim']}No runs found for '{trigger.area}'{c['reset']}")
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


@app.command("list-voices")
def list_voices_cmd():
    """Show available voices in current repo."""
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    voices_dir = repo / ".lf" / "voices"
    if not voices_dir.exists():
        typer.echo(f"{c['dim']}No voices directory found at {voices_dir}{c['reset']}")
        typer.echo(
            "Create one with: mkdir -p .lf/voices && echo '# My Voice' > .lf/voices/my-voice.md"
        )
        return

    all_voices = list_voices(repo)
    if not all_voices:
        typer.echo(f"{c['dim']}No voices found in {voices_dir}{c['reset']}")
        return

    typer.echo(f"Voices in {c['dim']}{voices_dir}/{c['reset']}")
    typer.echo("")

    for voice_name in all_voices:
        voice = load_voice(repo, voice_name)
        if voice:
            area_str = f"area: [{', '.join(voice.area)}]" if voice.area else ""
            pipeline_str = f"pipeline: {voice.pipeline}" if voice.pipeline else ""
            details = "  ".join(filter(None, [area_str, pipeline_str]))
            typer.echo(f"  {c['bold']}{voice_name:<20}{c['reset']} {c['dim']}{details}{c['reset']}")
        else:
            typer.echo(f"  {c['bold']}{voice_name:<20}{c['reset']}")

    typer.echo("")
    typer.echo(f"{len(all_voices)} voice{'es' if len(all_voices) != 1 else ''} found")


def main() -> None:
    """Entry point for lfd command."""
    if len(sys.argv) == 1:
        sys.argv.append("status")
    app()
