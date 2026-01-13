"""Agent management CLI commands."""

import os
import subprocess
import sys
from pathlib import Path

import typer

from loopflow.config import PipelineConfig
from loopflow.context import find_worktree_root
from loopflow.git import find_main_repo
from loopflow.maestro.daemon import run_daemon
from loopflow.maestro.db import DEFAULT_DB_PATH, get_db
from loopflow.maestro.launchd import (
    get_log_path,
    install as launchd_install,
    is_installed,
    is_running,
    restart as launchd_restart,
    uninstall as launchd_uninstall,
)
from loopflow.maestro.markdown import (
    create_agent_file,
    delete_agent_file,
    get_agent_file,
    list_agent_files,
)
from loopflow.pipeline import run_pipeline

app = typer.Typer(help="Background agent management.")
daemon_app = typer.Typer(help="Daemon management.")
app.add_typer(daemon_app, name="daemon")


# Daemon commands


@daemon_app.callback(invoke_without_command=True)
def daemon_default(ctx: typer.Context):
    """Run the daemon in foreground (for launchd)."""
    if ctx.invoked_subcommand is None:
        run_daemon()


@daemon_app.command("install")
def daemon_install():
    """Install the launchd plist and start the daemon."""
    if is_installed():
        typer.echo("Daemon already installed")
        if not is_running():
            typer.echo("Starting daemon...")
            launchd_restart()
        return

    typer.echo("Installing daemon...")
    if launchd_install():
        typer.echo("Daemon installed and started")
        typer.echo(f"  Log: {get_log_path()}")
    else:
        typer.echo("Failed to install daemon", err=True)
        raise typer.Exit(1)


@daemon_app.command("uninstall")
def daemon_uninstall():
    """Stop the daemon and remove the launchd plist."""
    if not is_installed():
        typer.echo("Daemon not installed")
        return

    typer.echo("Uninstalling daemon...")
    if launchd_uninstall():
        typer.echo("Daemon uninstalled")
    else:
        typer.echo("Failed to uninstall daemon", err=True)
        raise typer.Exit(1)


@daemon_app.command("status")
def daemon_status():
    """Show daemon status."""
    if not is_installed():
        typer.echo("Daemon: not installed")
        return

    if is_running():
        typer.echo("Daemon: running")
    else:
        typer.echo("Daemon: installed but not running")

    typer.echo(f"  Log: {get_log_path()}")


# Agent management commands


@app.command("new")
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
    """Create a new agent markdown file."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo_root) or repo_root

    # Check if agent already exists
    existing = get_agent_file(name)
    if existing:
        typer.echo(f"Agent '{name}' already exists at {existing.path}", err=True)
        raise typer.Exit(1)

    # Parse options
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
    typer.echo(f"Edit the file to customize the prompt:")
    typer.echo(f"  lf agent edit {name}")


@app.command("edit")
def edit(
    name: str = typer.Argument(help="Agent name"),
):
    """Open agent markdown file in $EDITOR."""
    agent = get_agent_file(name)
    if not agent:
        typer.echo(f"Agent '{name}' not found", err=True)
        typer.echo("Available agents:")
        for a in list_agent_files():
            typer.echo(f"  {a.name}")
        raise typer.Exit(1)

    editor = os.environ.get("EDITOR", "vi")
    subprocess.run([editor, str(agent.path)])


@app.command("rm")
def rm(
    name: str = typer.Argument(help="Agent name"),
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove an agent definition."""
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


@app.command(name="list")
def list_cmd():
    """List all agents and their status."""
    agents = list_agent_files()

    if not agents:
        typer.echo("No agents defined")
        typer.echo("")
        typer.echo("Create one with:")
        typer.echo("  lf agent new my-agent")
        return

    typer.echo(f"{'NAME':<20} {'TRIGGER':<15} {'PIPELINE':<30}")
    typer.echo("-" * 65)

    for agent in agents:
        pipeline_str = " → ".join(agent.pipeline)
        if len(pipeline_str) > 28:
            pipeline_str = pipeline_str[:25] + "..."

        trigger_str = agent.trigger
        if agent.trigger == "interval" and agent.interval_seconds:
            trigger_str = f"interval ({agent.interval_seconds}s)"

        typer.echo(f"{agent.name:<20} {trigger_str:<15} {pipeline_str:<30}")


@app.command()
def show(
    name: str = typer.Argument(help="Agent name"),
):
    """Show details of an agent."""
    agent = get_agent_file(name)
    if not agent:
        typer.echo(f"Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    typer.echo(f"Agent: {agent.name}")
    typer.echo(f"  Path: {agent.path}")
    typer.echo(f"  Repo: {agent.repo}")
    typer.echo(f"  Pipeline: {' → '.join(agent.pipeline)}")
    typer.echo(f"  Trigger: {agent.trigger}")
    if agent.interval_seconds:
        typer.echo(f"  Interval: {agent.interval_seconds}s")
    if agent.context:
        typer.echo(f"  Context: {', '.join(agent.context)}")
    typer.echo(f"  Prompt:")
    typer.echo("")
    for line in agent.prompt.split("\n")[:10]:
        typer.echo(f"    {line}")
    if agent.prompt.count("\n") > 10:
        typer.echo("    ...")


@app.command("run")
def run(
    name: str = typer.Argument(help="Agent name"),
):
    """Run a single agent iteration (used by daemon)."""
    agent = get_agent_file(name)
    if not agent:
        typer.echo(f"Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    typer.echo(f"Running agent: {agent.name}")
    typer.echo(f"  Repo: {agent.repo}")
    typer.echo(f"  Pipeline: {' → '.join(agent.pipeline)}")

    pipeline_config = PipelineConfig(
        name=agent.name,
        tasks=agent.pipeline,
    )

    try:
        result = run_pipeline(
            pipeline=pipeline_config,
            repo_root=agent.repo,
            context=agent.context if agent.context else None,
        )
        if result == 0:
            typer.echo("Agent completed successfully")
        else:
            typer.echo("Agent completed with errors", err=True)
            raise typer.Exit(1)
    except Exception as e:
        typer.echo(f"Agent failed: {e}", err=True)
        raise typer.Exit(1)


@app.command()
def start(
    name: str = typer.Argument(help="Agent name"),
    foreground: bool = typer.Option(False, "-f", "--foreground", help="Run in foreground"),
):
    """Start an agent run now (bypasses trigger)."""
    agent = get_agent_file(name)
    if not agent:
        typer.echo(f"Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    if foreground:
        # Run directly in foreground
        typer.echo(f"Running agent: {agent.name}")
        subprocess.run([sys.executable, "-m", "loopflow.cli", "agent", "run", name])
    else:
        # Spawn as background process
        log_dir = Path.home() / ".lf" / "logs" / "agents"
        log_dir.mkdir(parents=True, exist_ok=True)
        log_path = log_dir / f"{name}.log"

        cmd = [sys.executable, "-m", "loopflow.cli", "agent", "run", name]

        with log_path.open("a") as log_file:
            process = subprocess.Popen(
                cmd,
                stdout=log_file,
                stderr=log_file,
                start_new_session=True,
            )

        typer.echo(f"Started agent: {agent.name} (PID {process.pid})")
        typer.echo(f"  Log: {log_path}")


@app.command()
def stop(
    name: str = typer.Argument(help="Agent name"),
):
    """Stop a running agent (sends SIGTERM)."""
    # Find running process for this agent from the runs table
    conn = get_db(DEFAULT_DB_PATH)
    cursor = conn.execute(
        """SELECT pid FROM agent_runs
           WHERE agent_name = ? AND status = 'running'
           ORDER BY started_at DESC LIMIT 1""",
        (name,),
    )
    row = cursor.fetchone()
    conn.close()

    if not row or not row["pid"]:
        typer.echo(f"No running process found for agent '{name}'")
        return

    pid = row["pid"]

    try:
        os.kill(pid, 15)  # SIGTERM
        typer.echo(f"Sent SIGTERM to agent '{name}' (PID {pid})")
    except ProcessLookupError:
        typer.echo(f"Process {pid} not found (already stopped?)")
    except PermissionError:
        typer.echo(f"Permission denied to stop process {pid}", err=True)
        raise typer.Exit(1)


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
