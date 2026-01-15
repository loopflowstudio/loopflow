"""lfd: Agent orchestration daemon.

Unix socket daemon that owns agent lifecycle, trigger evaluation, and session tracking.
"""

import asyncio
import os
import subprocess
import sys
from pathlib import Path

import typer

from loopflow.context import find_worktree_root
from loopflow.git import find_main_repo
from loopflow.lfd.agents import (
    create_agent_file,
    delete_agent_file,
    get_agent,
    get_agent_file_path,
    get_worktree_path,
    list_agents,
)
from loopflow.lfd.client import DaemonClient
from loopflow.lfd.db import get_latest_run
from loopflow.lfd.launchd import install as launchd_install, is_running, uninstall as launchd_uninstall
from loopflow.lfd.server import run_server

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"

app = typer.Typer(help="Loopflow daemon - agent orchestration")


@app.command()
def serve():
    """Run daemon in foreground (for debugging or launchd)."""
    asyncio.run(run_server(SOCKET_PATH))


@app.command()
def status():
    """Show daemon and agent status."""
    if not is_running():
        typer.echo("lfd is not running")
        typer.echo("")
        typer.echo("Start with: lfd install")
        raise typer.Exit(1)

    client = DaemonClient()
    try:
        result = asyncio.run(client.call("status"))
        typer.echo(f"lfd running (pid {result.get('pid', 'unknown')})")
        typer.echo(f"Agents: {result.get('agents_defined', 0)} defined, {result.get('agents_running', 0)} running")
        typer.echo(f"Sessions: {result.get('sessions_active', 0)} active")
    except Exception as e:
        typer.echo(f"lfd running but not responding: {e}")
        raise typer.Exit(1)


@app.command()
def install():
    """Install launchd plist for auto-start."""
    if is_running():
        typer.echo("lfd is already running")
        return

    if launchd_install():
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
def notify(
    event: str = typer.Argument(help="Event name (e.g. worktree.created)"),
    branch: str = typer.Option(None, "--branch", "-b", help="Branch name"),
    path: str = typer.Option(None, "--path", "-p", help="Worktree path"),
):
    """Send an event to lfd for broadcast to subscribers."""
    if not is_running():
        return  # Silently fail if daemon not running

    data = {}
    if branch:
        data["branch"] = branch
    if path:
        data["path"] = path

    client = DaemonClient()
    try:
        asyncio.run(client.call("notify", {"event": event, "data": data}))
    except Exception:
        pass  # Best effort - don't fail hooks


# Agent commands


@app.command(name="list")
def list_cmd():
    """List all agents."""
    agents = list_agents()

    if not agents:
        typer.echo("No agents defined")
        typer.echo("")
        typer.echo("Create one with: lfd new <name>")
        return

    typer.echo(f"{'EMOJI':<6} {'NAME':<18} {'STATUS':<10} {'TRIGGER':<12} {'PIPELINE':<25}")
    typer.echo("-" * 75)

    for agent in agents:
        pipeline_str = agent.pipeline
        if len(pipeline_str) > 23:
            pipeline_str = pipeline_str[:20] + "..."

        trigger_str = agent.trigger.kind.value
        if agent.trigger.kind.value == "interval" and agent.trigger.interval_seconds:
            trigger_str = f"int({agent.trigger.interval_seconds}s)"
        elif agent.trigger.kind.value == "cron" and agent.trigger.cron:
            trigger_str = f"cron"

        # Get status from latest run
        latest = get_latest_run(agent.name)
        status_str = latest.status.value if latest else "idle"

        emoji_str = agent.emoji if agent.emoji else "-"
        name_str = agent.name[:18] if len(agent.name) > 18 else agent.name

        typer.echo(f"{emoji_str:<6} {name_str:<18} {status_str:<10} {trigger_str:<12} {pipeline_str:<25}")


@app.command()
def start(
    name: str = typer.Argument(help="Agent name"),
):
    """Start an agent."""
    if not is_running():
        typer.echo("lfd is not running. Start with: lfd install")
        raise typer.Exit(1)

    client = DaemonClient()
    try:
        result = asyncio.run(client.call("agents.start", {"name": name}))
        typer.echo(f"Started agent '{name}' (PID {result.get('pid')})")
    except Exception as e:
        typer.echo(f"Failed to start agent: {e}", err=True)
        raise typer.Exit(1)


@app.command()
def stop(
    name: str = typer.Argument(help="Agent name"),
):
    """Stop a running agent."""
    if not is_running():
        typer.echo("lfd is not running")
        raise typer.Exit(1)

    client = DaemonClient()
    try:
        asyncio.run(client.call("agents.stop", {"name": name}))
        typer.echo(f"Stopped agent '{name}'")
    except Exception as e:
        typer.echo(f"Failed to stop agent: {e}", err=True)
        raise typer.Exit(1)


@app.command()
def show(
    name: str = typer.Argument(help="Agent name"),
):
    """Show details of an agent."""
    agent = get_agent(name)
    if not agent:
        typer.echo(f"Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    typer.echo(f"Agent: {agent.name}")
    if agent.emoji:
        typer.echo(f"  Emoji: {agent.emoji}")
    typer.echo(f"  Repo: {agent.repo}")
    if agent.goal:
        typer.echo(f"  Goal: {agent.goal}")
    typer.echo(f"  Pipeline: {agent.pipeline}")
    typer.echo(f"  Merge: {agent.merge_strategy.value}")
    typer.echo(f"  Trigger: {agent.trigger.kind.value}")
    if agent.trigger.cron:
        typer.echo(f"  Cron: {agent.trigger.cron}")
        typer.echo(f"  Grace: {agent.trigger.grace_minutes}m")
    if agent.trigger.interval_seconds:
        typer.echo(f"  Interval: {agent.trigger.interval_seconds}s")
    if agent.context:
        typer.echo(f"  Context: {', '.join(agent.context)}")

    # Show runtime status
    latest = get_latest_run(agent.name)
    if latest:
        typer.echo(f"  Status: {latest.status.value}")
        typer.echo(f"  Last run: {latest.started_at.isoformat()}")
        typer.echo(f"  Iteration: {latest.iteration}")
        if latest.pid:
            typer.echo(f"  PID: {latest.pid}")

    # Show worktree status
    worktree = get_worktree_path(agent)
    if worktree:
        typer.echo(f"  Worktree: {worktree}")

    typer.echo(f"  Prompt:")
    typer.echo("")
    for line in agent.prompt.split("\n")[:10]:
        typer.echo(f"    {line}")
    if agent.prompt.count("\n") > 10:
        typer.echo("    ...")


@app.command()
def logs(
    name: str = typer.Argument(help="Agent name"),
    follow: bool = typer.Option(False, "-f", "--follow", help="Follow log output"),
    lines: int = typer.Option(50, "-n", "--lines", help="Number of lines to show"),
):
    """Show agent logs."""
    log_path = Path.home() / ".lf" / "logs" / "agents" / f"{name}.log"

    if not log_path.exists():
        typer.echo(f"No logs found for agent '{name}'")
        typer.echo(f"  Expected: {log_path}")
        return

    if follow:
        subprocess.run(["tail", "-f", str(log_path)])
    else:
        subprocess.run(["tail", f"-{lines}", str(log_path)])


# Agent definition management


@app.command()
def new(
    name: str = typer.Argument(help="Agent name"),
    emoji: str = typer.Option(
        "",
        "-e",
        "--emoji",
        help="Emoji identifier for visual tracking (e.g., 🔧)",
    ),
    goal: str = typer.Option(
        None,
        "-g",
        "--goal",
        help="Path to goal file (relative to repo, e.g., .lf/goals/security.md)",
    ),
    pipeline: str = typer.Option(
        "ship",
        "-p",
        "--pipeline",
        help="Pipeline name to run (from .lf/pipelines/)",
    ),
    trigger: str = typer.Option(
        "manual",
        "-t",
        "--trigger",
        help="Trigger: manual, loop, cron, main-changed, or interval",
    ),
    merge: str = typer.Option(
        "pr",
        "-m",
        "--merge",
        help="Merge strategy: auto or pr",
    ),
    interval: int = typer.Option(
        None,
        "-i",
        "--interval",
        help="Interval in seconds (for interval trigger)",
    ),
    cron: str = typer.Option(
        None,
        "--cron",
        help="Cron expression (for cron trigger, e.g., '0 9 * * *')",
    ),
    grace: int = typer.Option(
        60,
        "--grace",
        help="Grace period in minutes for missed cron schedules",
    ),
    context: str = typer.Option(
        None,
        "-x",
        "--context",
        help="Comma-separated context paths",
    ),
):
    """Create a new agent."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo_root) or repo_root

    existing = get_agent_file_path(name)
    if existing:
        typer.echo(f"Agent '{name}' already exists at {existing}", err=True)
        raise typer.Exit(1)

    context_list = [c.strip() for c in context.split(",")] if context else None
    goal_path = Path(goal) if goal else None

    if trigger == "interval" and not interval:
        typer.echo("Error: --interval required for interval trigger", err=True)
        raise typer.Exit(1)

    if trigger == "cron" and not cron:
        typer.echo("Error: --cron required for cron trigger", err=True)
        raise typer.Exit(1)

    if merge not in ("auto", "pr"):
        typer.echo("Error: --merge must be 'auto' or 'pr'", err=True)
        raise typer.Exit(1)

    path = create_agent_file(
        name=name,
        repo=main_repo,
        pipeline=pipeline,
        trigger=trigger,
        context=context_list,
        interval_seconds=interval,
        emoji=emoji,
        goal=goal_path,
        merge=merge,
        cron=cron,
        grace_minutes=grace if grace != 60 else None,
    )

    typer.echo(f"Created agent: {path}")
    if emoji:
        typer.echo(f"  Emoji: {emoji}")
    typer.echo(f"  Pipeline: {pipeline}")
    typer.echo(f"  Trigger: {trigger}")
    typer.echo(f"  Merge: {merge}")
    typer.echo("")
    typer.echo(f"Edit the prompt: lfd edit {name}")


@app.command()
def edit(
    name: str = typer.Argument(help="Agent name"),
):
    """Open agent file in $EDITOR."""
    agent_path = get_agent_file_path(name)
    if not agent_path:
        typer.echo(f"Agent '{name}' not found", err=True)
        typer.echo("Available agents:")
        for a in list_agents():
            typer.echo(f"  {a.name}")
        raise typer.Exit(1)

    editor = os.environ.get("EDITOR", "vi")
    subprocess.run([editor, str(agent_path)])


@app.command()
def rm(
    name: str = typer.Argument(help="Agent name"),
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove an agent."""
    agent_path = get_agent_file_path(name)
    if not agent_path:
        typer.echo(f"Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    if not force:
        confirm = typer.confirm(f"Delete agent '{name}'?")
        if not confirm:
            raise typer.Abort()

    if delete_agent_file(name):
        typer.echo(f"Deleted agent: {name}")
    else:
        typer.echo("Failed to delete agent", err=True)
        raise typer.Exit(1)


def main() -> None:
    """Entry point for lfd command."""
    if len(sys.argv) == 1:
        sys.argv.append("status")

    app()
