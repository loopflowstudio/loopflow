"""lfd: Agent orchestration daemon.

Unix socket daemon that owns agent lifecycle, trigger evaluation, and session tracking.
"""

import asyncio
import os
import subprocess
from pathlib import Path

import typer

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
    from loopflow.lfd.launchd import is_running
    from loopflow.lfd.client import DaemonClient

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
    from loopflow.lfd.launchd import install as do_install, is_running

    if is_running():
        typer.echo("lfd is already running")
        return

    if do_install():
        typer.echo("lfd installed and started")
    else:
        typer.echo("Failed to install lfd")
        raise typer.Exit(1)


@app.command()
def uninstall():
    """Remove launchd plist and stop daemon."""
    from loopflow.lfd.launchd import uninstall as do_uninstall

    if do_uninstall():
        typer.echo("lfd uninstalled")
    else:
        typer.echo("Failed to uninstall lfd")
        raise typer.Exit(1)


# Agent commands


@app.command(name="list")
def list_cmd():
    """List all agents."""
    from loopflow.lfd.agents import list_agents
    from loopflow.lfd.db import get_latest_run

    agents = list_agents()

    if not agents:
        typer.echo("No agents defined")
        typer.echo("")
        typer.echo("Create one with: lfd new <name>")
        return

    typer.echo(f"{'NAME':<20} {'STATUS':<12} {'TRIGGER':<15} {'PIPELINE':<30}")
    typer.echo("-" * 77)

    for agent in agents:
        pipeline_str = " → ".join(agent.pipeline)
        if len(pipeline_str) > 28:
            pipeline_str = pipeline_str[:25] + "..."

        trigger_str = agent.trigger.kind.value
        if agent.trigger.kind.value == "interval" and agent.trigger.interval_seconds:
            trigger_str = f"interval ({agent.trigger.interval_seconds}s)"

        # Get status from latest run
        latest = get_latest_run(agent.name)
        status_str = latest.status.value if latest else "idle"

        typer.echo(f"{agent.name:<20} {status_str:<12} {trigger_str:<15} {pipeline_str:<30}")


@app.command()
def start(
    name: str = typer.Argument(help="Agent name"),
):
    """Start an agent."""
    from loopflow.lfd.launchd import is_running
    from loopflow.lfd.client import DaemonClient

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
    from loopflow.lfd.launchd import is_running
    from loopflow.lfd.client import DaemonClient

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
    from loopflow.lfd.agents import get_agent
    from loopflow.lfd.db import get_latest_run
    from loopflow.maestro.worktree import get_agent_worktree_path
    from loopflow.maestro.markdown import get_agent_file

    agent = get_agent(name)
    if not agent:
        typer.echo(f"Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    typer.echo(f"Agent: {agent.name}")
    typer.echo(f"  Repo: {agent.repo}")
    typer.echo(f"  Pipeline: {' → '.join(agent.pipeline)}")
    typer.echo(f"  Trigger: {agent.trigger.kind.value}")
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
    agent_file = get_agent_file(name)
    if agent_file:
        worktree = get_agent_worktree_path(agent_file)
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
    pipeline: str = typer.Option(
        "implement,polish,land",
        "-p",
        "--pipeline",
        help="Comma-separated pipeline tasks",
    ),
    trigger: str = typer.Option(
        "manual",
        "-t",
        "--trigger",
        help="Trigger: manual, main-changed, or interval",
    ),
    interval: int = typer.Option(
        None,
        "-i",
        "--interval",
        help="Interval in seconds (for interval trigger)",
    ),
    context: str = typer.Option(
        None,
        "-x",
        "--context",
        help="Comma-separated context paths",
    ),
):
    """Create a new agent."""
    from loopflow.context import find_worktree_root
    from loopflow.git import find_main_repo
    from loopflow.maestro.markdown import create_agent_file, get_agent_file

    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo_root) or repo_root

    existing = get_agent_file(name)
    if existing:
        typer.echo(f"Agent '{name}' already exists at {existing.path}", err=True)
        raise typer.Exit(1)

    pipeline_list = [t.strip() for t in pipeline.split(",") if t.strip()]
    context_list = [c.strip() for c in context.split(",")] if context else None

    if trigger == "interval" and not interval:
        typer.echo("Error: --interval required for interval trigger", err=True)
        raise typer.Exit(1)

    path = create_agent_file(
        name=name,
        repo=main_repo,
        pipeline=pipeline_list,
        trigger=trigger,
        context=context_list,
        interval_seconds=interval,
    )

    typer.echo(f"Created agent: {path}")
    typer.echo(f"  Pipeline: {' → '.join(pipeline_list)}")
    typer.echo(f"  Trigger: {trigger}")
    typer.echo("")
    typer.echo(f"Edit the prompt: lfd edit {name}")


@app.command()
def edit(
    name: str = typer.Argument(help="Agent name"),
):
    """Open agent file in $EDITOR."""
    from loopflow.maestro.markdown import get_agent_file, list_agent_files

    agent = get_agent_file(name)
    if not agent:
        typer.echo(f"Agent '{name}' not found", err=True)
        typer.echo("Available agents:")
        for a in list_agent_files():
            typer.echo(f"  {a.name}")
        raise typer.Exit(1)

    editor = os.environ.get("EDITOR", "vi")
    subprocess.run([editor, str(agent.path)])


@app.command()
def rm(
    name: str = typer.Argument(help="Agent name"),
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove an agent."""
    from loopflow.maestro.markdown import delete_agent_file, get_agent_file

    agent = get_agent_file(name)
    if not agent:
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
    import sys

    if len(sys.argv) == 1:
        sys.argv.append("status")

    app()
