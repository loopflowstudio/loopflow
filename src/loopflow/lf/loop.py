"""CLI commands for agent loops."""

import sys
from datetime import datetime
from pathlib import Path
from typing import Optional

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lf.goals import goal_exists, list_goals
from loopflow.lfd.agents import (
    get_agent,
    list_agents,
    start_agent,
    stop_agent,
    create_agent_file,
    delete_agent_file,
)
from loopflow.lfd.db import get_latest_run, get_loop_prs
from loopflow.lfd.models import AgentStatus, TriggerKind

app = typer.Typer(
    name="loop",
    help="Manage agent loops.",
    no_args_is_help=True,
)


def _use_color() -> bool:
    """Check if we should use colored output."""
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


def _status_color(status: AgentStatus, c: dict[str, str]) -> str:
    """Get color code for agent status."""
    if status == AgentStatus.RUNNING:
        return c["green"]
    elif status == AgentStatus.ERROR:
        return c["red"]
    elif status == AgentStatus.IDLE:
        return c["dim"]
    return c["yellow"]


@app.command()
def start(
    name: str = typer.Argument(..., help="Agent name to start"),
) -> None:
    """Start an agent loop.

    The agent will begin its iteration cycle, running its pipeline
    and generating PRs to its personal-main branch.
    """
    import asyncio

    agent = get_agent(name)
    if not agent:
        typer.echo(f"Error: Agent '{name}' not found", err=True)
        typer.echo(f"Available agents: {', '.join(a.name for a in list_agents())}", err=True)
        raise typer.Exit(1)

    # Check if already running
    latest = get_latest_run(name)
    if latest and latest.status == AgentStatus.RUNNING:
        typer.echo(f"Agent '{name}' is already running (PID {latest.pid})")
        raise typer.Exit(1)

    result = asyncio.run(start_agent(name))

    if result.success:
        c = _colors()
        emoji = f"{agent.emoji} " if agent.emoji else ""
        typer.echo(f"{c['green']}Started{c['reset']} {emoji}{c['bold']}{name}{c['reset']} (PID {result.pid})")
        if agent.goal:
            typer.echo(f"  {c['dim']}Goal: {agent.goal}{c['reset']}")
        if agent.area:
            typer.echo(f"  {c['dim']}Area: {', '.join(agent.area)}{c['reset']}")
    else:
        typer.echo(f"Error: {result.error}", err=True)
        raise typer.Exit(1)


@app.command()
def stop(
    name: str = typer.Argument(..., help="Agent name to stop"),
) -> None:
    """Stop a running agent loop."""
    agent = get_agent(name)
    if not agent:
        typer.echo(f"Error: Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    if stop_agent(name):
        c = _colors()
        emoji = f"{agent.emoji} " if agent.emoji else ""
        typer.echo(f"{c['yellow']}Stopped{c['reset']} {emoji}{c['bold']}{name}{c['reset']}")
    else:
        typer.echo(f"Agent '{name}' is not running")
        raise typer.Exit(1)


@app.command()
def status(
    name: Optional[str] = typer.Argument(None, help="Agent name (optional, shows all if omitted)"),
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Show detailed info"),
) -> None:
    """Show status of agent loops."""
    c = _colors()

    if name:
        # Show status for specific agent
        agent = get_agent(name)
        if not agent:
            typer.echo(f"Error: Agent '{name}' not found", err=True)
            raise typer.Exit(1)

        latest = get_latest_run(name)
        _print_agent_status(agent, latest, c, verbose)
    else:
        # Show all agents
        agents = list_agents()
        if not agents:
            typer.echo(f"{c['dim']}No agents configured{c['reset']}")
            typer.echo(f"{c['dim']}Create agents in ~/.lf/agents/*.md{c['reset']}")
            return

        for agent in agents:
            latest = get_latest_run(agent.name)
            _print_agent_status(agent, latest, c, verbose)
            if verbose:
                typer.echo("")


def _print_agent_status(agent, latest, c, verbose: bool) -> None:
    """Print status for a single agent."""
    emoji = f"{agent.emoji} " if agent.emoji else ""

    if latest:
        status_c = _status_color(latest.status, c)
        status_text = latest.status.value.upper()

        # Format duration
        if latest.status == AgentStatus.RUNNING:
            duration = datetime.now() - latest.started_at
            duration_str = f" ({_format_duration(duration)})"
        elif latest.ended_at:
            duration = latest.ended_at - latest.started_at
            duration_str = f" (took {_format_duration(duration)})"
        else:
            duration_str = ""

        step_info = f" [{latest.current_step}]" if latest.current_step else ""

        typer.echo(
            f"{emoji}{c['bold']}{agent.name}{c['reset']}  "
            f"{status_c}{status_text}{c['reset']}"
            f"{step_info}{duration_str}"
        )

        if verbose:
            typer.echo(f"  {c['dim']}Iteration:{c['reset']} {latest.iteration}")
            if latest.pid:
                typer.echo(f"  {c['dim']}PID:{c['reset']} {latest.pid}")
            if latest.worktree:
                typer.echo(f"  {c['dim']}Worktree:{c['reset']} {latest.worktree}")
            if agent.goal:
                typer.echo(f"  {c['dim']}Goal:{c['reset']} {agent.goal}")
            if agent.area:
                typer.echo(f"  {c['dim']}Area:{c['reset']} {', '.join(agent.area)}")
            if latest.error:
                typer.echo(f"  {c['red']}Error:{c['reset']} {latest.error}")

            # Show recent PRs
            prs = get_loop_prs(agent.name, limit=3)
            if prs:
                typer.echo(f"  {c['dim']}Recent PRs:{c['reset']}")
                for pr in prs:
                    typer.echo(f"    #{pr.get('iteration', '?')}: {pr['pr_url']}")
    else:
        typer.echo(
            f"{emoji}{c['bold']}{agent.name}{c['reset']}  "
            f"{c['dim']}NEVER RUN{c['reset']}"
        )
        if verbose:
            if agent.goal:
                typer.echo(f"  {c['dim']}Goal:{c['reset']} {agent.goal}")
            if agent.area:
                typer.echo(f"  {c['dim']}Area:{c['reset']} {', '.join(agent.area)}")


@app.command(name="list")
def list_cmd() -> None:
    """List all configured agents."""
    c = _colors()
    agents = list_agents()

    if not agents:
        typer.echo(f"{c['dim']}No agents configured{c['reset']}")
        typer.echo(f"{c['dim']}Create agents in ~/.lf/agents/*.md{c['reset']}")
        return

    typer.echo(f"{c['cyan']}{c['bold']}AGENTS{c['reset']}")
    typer.echo("")

    for agent in agents:
        emoji = f"{agent.emoji} " if agent.emoji else "  "
        trigger = agent.trigger.kind.value

        latest = get_latest_run(agent.name)
        if latest and latest.status == AgentStatus.RUNNING:
            status = f"{c['green']}running{c['reset']}"
        elif latest and latest.status == AgentStatus.ERROR:
            status = f"{c['red']}error{c['reset']}"
        else:
            status = f"{c['dim']}idle{c['reset']}"

        typer.echo(
            f"{emoji}{c['bold']}{agent.name:<16}{c['reset']} "
            f"{c['dim']}{trigger:<12}{c['reset']} "
            f"{status}"
        )

    typer.echo("")
    typer.echo(f"{c['dim']}Use 'lf loop start <name>' to start an agent{c['reset']}")


@app.command()
def prs(
    name: str = typer.Argument(..., help="Agent name"),
    limit: int = typer.Option(10, "--limit", "-n", help="Number of PRs to show"),
) -> None:
    """Show PRs created by an agent loop."""
    c = _colors()

    agent = get_agent(name)
    if not agent:
        typer.echo(f"Error: Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    prs = get_loop_prs(name, limit=limit)

    if not prs:
        typer.echo(f"{c['dim']}No PRs found for '{name}'{c['reset']}")
        return

    emoji = f"{agent.emoji} " if agent.emoji else ""
    typer.echo(f"{emoji}{c['bold']}{name}{c['reset']} PRs")
    typer.echo("")

    for pr in prs:
        iteration = pr.get("iteration", "?")
        status = pr.get("status", "open")
        created_at = pr.get("created_at", "")

        if status == "merged":
            status_c = c["green"]
        elif status == "closed":
            status_c = c["red"]
        else:
            status_c = c["yellow"]

        typer.echo(
            f"  #{iteration:<3} {status_c}{status:<8}{c['reset']} "
            f"{c['dim']}{created_at[:10] if created_at else ''}{c['reset']}  "
            f"{pr['pr_url']}"
        )


def _format_duration(delta) -> str:
    """Format a timedelta as a human-readable string."""
    seconds = int(delta.total_seconds())
    if seconds < 60:
        return f"{seconds}s"
    minutes = seconds // 60
    if minutes < 60:
        return f"{minutes}m"
    hours = minutes // 60
    minutes = minutes % 60
    return f"{hours}h {minutes}m"
