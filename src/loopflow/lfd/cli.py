"""lfd: Loopflow daemon.

Commands for managing AI coding agents.
"""

import asyncio
import json
import socket
import subprocess
import sys
from pathlib import Path

import typer

from loopflow.lf.flows import load_flow
from loopflow.lf.goals import list_goals
from loopflow.lf.logging import get_log_dir
from loopflow.lfd.agent import (
    create_agent,
    delete_agent,
    get_agent,
    get_agent_by_name,
    get_wt_from_cwd,
    list_agents,
    start_agent,
    stop_agent,
    update_agent,
)
from loopflow.lfd.daemon.launchd import install as launchd_install
from loopflow.lfd.daemon.launchd import is_running
from loopflow.lfd.daemon.launchd import uninstall as launchd_uninstall
from loopflow.lfd.daemon.server import run_server
from loopflow.lfd.flow_run import list_runs_for_agent
from loopflow.lfd.git_hooks import (
    hooks_status,
    install_hooks,
    uninstall_hooks,
)
from loopflow.lfd.models import Agent, AgentStatus, MergeMode, Stimulus

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"

app = typer.Typer(help="Loopflow daemon - AI coding agents")


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


def _status_color(status: AgentStatus, c: dict[str, str]) -> str:
    if status == AgentStatus.RUNNING:
        return c["green"]
    elif status == AgentStatus.ERROR:
        return c["red"]
    elif status == AgentStatus.WAITING:
        return c["yellow"]
    return c["dim"]


def _agent_display(agent: Agent) -> str:
    """Return area, flow, and goal for display."""
    return f"{agent.area_display} [{agent.flow}] [{agent.goal_display}]"


def _parse_goals(goals: list[str] | None) -> list[str]:
    """Parse goal list, handling comma-separated values.

    Accepts both: -g g1 -g g2 and -g g1,g2
    """
    if not goals:
        return ["default"]
    result = []
    for g in goals:
        result.extend(g.split(","))
    return [x.strip() for x in result if x.strip()]


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
    all_agents: bool = typer.Option(False, "--all", help="Include waiting agents"),
):
    """Start multiple agents in parallel.

    Without arguments, starts all idle agents. With --all, also starts waiting agents.
    """
    c = _colors()
    repo = get_wt_from_cwd()

    # Get agents to start
    if areas:
        # Start specific areas
        agents_to_start = []
        for area in areas:
            agent = None
            for ag in list_agents(repo=repo):
                if area in ag.area:
                    agent = ag
                    break
            if not agent:
                typer.echo(
                    f"{c['yellow']}Warning:{c['reset']} Agent for '{area}' not found, skipping",
                    err=True,
                )
            else:
                agents_to_start.append(agent)
    else:
        # Start all eligible agents
        agents_to_start = []
        for agent in list_agents(repo=repo):
            if agent.status == AgentStatus.IDLE:
                agents_to_start.append(agent)
            elif all_agents and agent.status == AgentStatus.WAITING:
                agents_to_start.append(agent)

    if not agents_to_start:
        typer.echo(f"{c['dim']}No agents to start{c['reset']}")
        return

    # Start each agent
    started = 0
    for agent in agents_to_start:
        result = start_agent(agent.id)
        if result:
            msg = f"{c['green']}Started{c['reset']} {c['bold']}{agent.area_display}{c['reset']}"
            typer.echo(f"{msg} ({agent.short_id()})")
            started += 1
        elif result.reason == "already_running":
            typer.echo(f"{c['dim']}Already running:{c['reset']} {agent.area_display}")
        elif result.reason == "waiting":
            msg = f"{c['yellow']}Waiting:{c['reset']} {agent.area_display}"
            typer.echo(f"{msg} ({result.outstanding} outstanding)")
        else:
            typer.echo(f"{c['red']}Failed:{c['reset']} {agent.area_display}")

    typer.echo(f"\nStarted {started}/{len(agents_to_start)} agents")


# Agent commands


def _resolve_agent(name_or_id: str, repo: Path, c: dict[str, str]) -> Agent | None:
    """Resolve agent by name or ID. Returns None if not found."""
    # Try by name first
    agent = get_agent_by_name(name_or_id, repo)
    if agent:
        return agent
    # Then try by ID
    return get_agent(name_or_id)


def _validate_agent_for_run(agent: Agent, c: dict[str, str]) -> None:
    """Validate agent has required config for running. Exits on error."""
    if agent.area is None:
        typer.echo(
            f"{c['red']}Error:{c['reset']} Agent '{agent.name}' has no area configured",
            err=True,
        )
        typer.echo(f"Set area with: lfd area {agent.name} <path>")
        raise typer.Exit(1)


@app.command()
def create(
    name: str = typer.Argument(None, help="Agent name (generated if omitted)"),
):
    """Create a new agent.

    Creates an agent with the given name (or generates one).
    Configure with `lfd area`, `lfd goal`, `lfd flow` before running.

    Examples:
        lfd create                    # create with generated name
        lfd create swift-falcon       # create with specific name
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Check if name already exists
    if name:
        existing = get_agent_by_name(name, repo)
        if existing:
            typer.echo(f"Agent '{name}' already exists ({existing.short_id()})")
            return

    agent = create_agent(repo=repo, name=name)

    typer.echo(
        f"{c['green']}Created{c['reset']} {c['bold']}{agent.name}{c['reset']} ({agent.short_id()})"
    )
    typer.echo(f"  Repo: {repo}")
    typer.echo("")
    typer.echo("Configure before running:")
    typer.echo(f"  lfd area {agent.name} src/")
    typer.echo(f'  lfd goal {agent.name} "fix lint errors"')
    typer.echo(f"  lfd flow {agent.name} ship")
    typer.echo("")
    typer.echo(f"Then run: lfd loop {agent.name}")


@app.command()
def area(
    name: str = typer.Argument(..., help="Agent name or ID"),
    paths: list[str] = typer.Argument(..., help="Area paths (e.g., src/, swift/)"),
):
    """Set the working area for an agent.

    Examples:
        lfd area swift-falcon src/
        lfd area swift-falcon src/ tests/
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    agent = _resolve_agent(name, repo, c)
    if not agent:
        typer.echo(f"{c['red']}Error:{c['reset']} Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    updated = update_agent(agent.id, area=paths)
    if updated:
        typer.echo(f"Set area: {', '.join(paths)}")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to update agent", err=True)
        raise typer.Exit(1)


@app.command()
def goal(
    name: str = typer.Argument(..., help="Agent name or ID"),
    goal_text: str = typer.Argument(..., help="Goal (inline text or preset name)"),
):
    """Set the goal for an agent.

    Examples:
        lfd goal swift-falcon "fix lint errors"
        lfd goal swift-falcon product-engineer
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    agent = _resolve_agent(name, repo, c)
    if not agent:
        typer.echo(f"{c['red']}Error:{c['reset']} Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    updated = update_agent(agent.id, goal=[goal_text])
    if updated:
        typer.echo(f"Set goal: {goal_text}")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to update agent", err=True)
        raise typer.Exit(1)


@app.command("flow")
def set_flow(
    name: str = typer.Argument(..., help="Agent name or ID"),
    flow_name: str = typer.Argument(..., help="Flow name (from .lf/flows/)"),
):
    """Set the flow for an agent.

    Examples:
        lfd flow swift-falcon ship
        lfd flow swift-falcon polish
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    agent = _resolve_agent(name, repo, c)
    if not agent:
        typer.echo(f"{c['red']}Error:{c['reset']} Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    # Validate flow exists
    flow_name = _validate_flow(repo, flow_name, c)

    updated = update_agent(agent.id, flow=flow_name)
    if updated:
        typer.echo(f"Set flow: {flow_name}")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to update agent", err=True)
        raise typer.Exit(1)


@app.command()
def show(
    name: str = typer.Argument(..., help="Agent name or ID"),
):
    """Show details for an agent.

    Examples:
        lfd show swift-falcon
    """
    c = _colors()
    repo = get_wt_from_cwd()

    agent = _resolve_agent(name, repo, c) if repo else get_agent(name)
    if not agent:
        typer.echo(f"{c['red']}Error:{c['reset']} Agent '{name}' not found", err=True)
        raise typer.Exit(1)

    _print_agent_detail(agent, c)


@app.command("list")
def list_cmd():
    """List all agents.

    Examples:
        lfd list
    """
    c = _colors()
    repo = get_wt_from_cwd()

    agents = list_agents(repo=repo)
    if not agents:
        typer.echo(f"{c['dim']}No agents configured{c['reset']}")
        typer.echo("Create an agent with: lfd create")
        return

    typer.echo(f"{'NAME':<20} {'AREA':<20} {'STATUS':<10} {'STIMULUS':<12} ID")
    typer.echo("-" * 75)

    for agent in agents:
        status_c = _status_color(agent.status, c)
        area_str = agent.area_display if agent.area else f"{c['dim']}(not set){c['reset']}"
        if len(area_str) > 20:
            area_str = area_str[:17] + "..."

        name_str = agent.name
        if len(name_str) > 20:
            name_str = name_str[:17] + "..."

        typer.echo(
            f"{name_str:<20} {area_str:<20} "
            f"{status_c}{agent.status.value:<10}{c['reset']} "
            f"{str(agent.stimulus):<12} {agent.short_id()}"
        )


@app.command()
def loop(
    name: str = typer.Argument(..., help="Agent name or ID"),
    area_opt: str = typer.Option(None, "--area", "-a", help="Set area (creates agent if needed)"),
    goal_opt: str = typer.Option(None, "--goal", "-g", help="Set goal"),
    flow_opt: str = typer.Option(None, "--flow", help="Set flow"),
    limit: int = typer.Option(None, "-l", "--limit", help="PR limit override"),
    merge_mode: str = typer.Option(None, "--merge-mode", help="Merge mode: pr or land"),
    foreground: bool = typer.Option(False, "-f", "--foreground", help="Run in foreground"),
):
    """Start a continuous agent loop (loop stimulus).

    Agent name is required. Creates the agent if it doesn't exist and --area is provided.
    Validates that area is configured before starting.

    Examples:
        lfd loop swift-falcon                             # run existing agent
        lfd loop swift-falcon --area src/                 # create + set area + run
        lfd loop swift-falcon --area src/ --goal "fix lint"
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Get or create agent
    agent = _resolve_agent(name, repo, c)
    if not agent:
        if area_opt:
            # Create new agent with options
            agent = create_agent(repo=repo, name=name)
        else:
            typer.echo(f"{c['red']}Error:{c['reset']} Agent '{name}' not found", err=True)
            typer.echo(f"Create with: lfd create {name}")
            raise typer.Exit(1)

    # Apply options if provided
    if area_opt:
        update_agent(agent.id, area=[area_opt])
        agent.area = [area_opt]
    if goal_opt:
        update_agent(agent.id, goal=[goal_opt])
        agent.goal = [goal_opt]
    if flow_opt:
        flow_opt = _validate_flow(repo, flow_opt, c)
        update_agent(agent.id, flow=flow_opt)
        agent.flow = flow_opt

    # Validate merge_mode if specified
    if merge_mode and merge_mode not in ("pr", "land"):
        typer.echo(f"{c['red']}Error:{c['reset']} merge-mode must be 'pr' or 'land'", err=True)
        raise typer.Exit(1)

    if limit is not None:
        update_agent(agent.id, pr_limit=limit)
    if merge_mode:
        update_agent(agent.id, merge_mode=MergeMode(merge_mode))

    # Update stimulus to loop
    update_agent(agent.id, stimulus=Stimulus("loop"))

    # Validate agent is configured
    _validate_agent_for_run(agent, c)

    # Validate flow exists
    _validate_flow(repo, agent.flow, c)

    # Refresh agent after updates
    agent = get_agent(agent.id)

    # Start it
    result = start_agent(agent.id, foreground=foreground)
    if result:
        if foreground:
            msg = f"{c['green']}Completed{c['reset']} loop {c['bold']}{agent.name}{c['reset']}"
            typer.echo(f"{msg} ({agent.short_id()})")
        else:
            msg = f"{c['green']}Started{c['reset']} loop {c['bold']}{agent.name}{c['reset']}"
            typer.echo(f"{msg} ({agent.short_id()})")
            typer.echo(f"  Repo: {repo}")
            typer.echo(f"  Area: {agent.area_display}")
            typer.echo(f"  Goals: {agent.goal_display}")
            typer.echo(f"  Flow: {agent.flow}")
            typer.echo(f"  PR limit: {agent.pr_limit}")
    elif result.reason == "already_running":
        typer.echo(f"Agent already running (PID {agent.pid})")
        raise typer.Exit(1)
    elif result.reason == "waiting":
        msg = f"{c['yellow']}Waiting:{c['reset']} {result.outstanding} outstanding PRs"
        typer.echo(f"{msg} (limit {agent.pr_limit})")
        typer.echo("Run 'lfops land --squash' to land work")
        raise typer.Exit(0)
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to start agent", err=True)
        raise typer.Exit(1)


@app.command()
def run(
    name: str = typer.Argument(..., help="Agent name or ID"),
    area_opt: str = typer.Option(None, "--area", "-a", help="Set area (creates agent if needed)"),
    goal_opt: str = typer.Option(None, "--goal", "-g", help="Set goal"),
    flow_opt: str = typer.Option(None, "--flow", help="Set flow"),
):
    """Run an agent once (once stimulus - single run).

    Agent name is required. Creates the agent if it doesn't exist and --area is provided.

    Examples:
        lfd run swift-falcon                             # run existing agent once
        lfd run swift-falcon --area src/                 # create + set area + run once
        lfd run swift-falcon --area src/ --goal "fix lint"
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Get or create agent
    agent = _resolve_agent(name, repo, c)
    if not agent:
        if area_opt:
            agent = create_agent(repo=repo, name=name)
        else:
            typer.echo(f"{c['red']}Error:{c['reset']} Agent '{name}' not found", err=True)
            typer.echo(f"Create with: lfd create {name}")
            raise typer.Exit(1)

    # Apply options if provided
    if area_opt:
        update_agent(agent.id, area=[area_opt])
        agent.area = [area_opt]
    if goal_opt:
        update_agent(agent.id, goal=[goal_opt])
        agent.goal = [goal_opt]
    if flow_opt:
        flow_opt = _validate_flow(repo, flow_opt, c)
        update_agent(agent.id, flow=flow_opt)
        agent.flow = flow_opt

    # Update stimulus to once
    update_agent(agent.id, stimulus=Stimulus("once"))

    # Validate agent is configured
    _validate_agent_for_run(agent, c)

    # Validate flow exists
    _validate_flow(repo, agent.flow, c)

    # Refresh agent after updates
    agent = get_agent(agent.id)

    # Start it in foreground (runs once)
    result = start_agent(agent.id, foreground=True)

    if result:
        msg = f"{c['green']}Completed{c['reset']} run {c['bold']}{agent.name}{c['reset']}"
        typer.echo(msg)
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to run", err=True)
        raise typer.Exit(1)


@app.command()
def watch(
    name: str = typer.Argument(..., help="Agent name or ID"),
    area_opt: str = typer.Option(None, "--area", "-a", help="Set area (creates agent if needed)"),
    path_opt: str = typer.Option(None, "--path", "-p", help="Watch path (defaults to area)"),
    goal_opt: str = typer.Option(None, "--goal", "-g", help="Set goal"),
    flow_opt: str = typer.Option(None, "--flow", help="Set flow"),
):
    """Watch for changes on origin/main (watch stimulus).

    Triggers when files in the watched path change on main.
    Watch path defaults to area but can be overridden.

    Examples:
        lfd watch swift-falcon                            # watch existing agent
        lfd watch swift-falcon --area src/                # create + set area + watch
        lfd watch swift-falcon --area src/ --path tests/  # watch tests/, work on src/
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Get or create agent
    agent = _resolve_agent(name, repo, c)
    if not agent:
        if area_opt:
            agent = create_agent(repo=repo, name=name)
        else:
            typer.echo(f"{c['red']}Error:{c['reset']} Agent '{name}' not found", err=True)
            typer.echo(f"Create with: lfd create {name}")
            raise typer.Exit(1)

    # Apply options if provided
    if area_opt:
        update_agent(agent.id, area=[area_opt])
        agent.area = [area_opt]
    if goal_opt:
        update_agent(agent.id, goal=[goal_opt])
        agent.goal = [goal_opt]
    if flow_opt:
        flow_opt = _validate_flow(repo, flow_opt, c)
        update_agent(agent.id, flow=flow_opt)
        agent.flow = flow_opt

    # Update stimulus to watch
    update_agent(agent.id, stimulus=Stimulus("watch"))

    # Validate agent is configured
    _validate_agent_for_run(agent, c)

    # Validate flow exists
    _validate_flow(repo, agent.flow, c)

    # Refresh agent after updates
    agent = get_agent(agent.id)

    watch_path = path_opt or agent.area_display
    msg = f"{c['green']}Watching{c['reset']} {c['bold']}{agent.name}{c['reset']}"
    typer.echo(f"{msg} ({agent.short_id()})")
    typer.echo(f"  Area: {agent.area_display}")
    typer.echo(f"  Goals: {agent.goal_display}")
    typer.echo(f"  Flow: {agent.flow}")
    typer.echo(f"  Activates when {watch_path} changes on main")


@app.command("cron")
def cron_cmd(
    name: str = typer.Argument(..., help="Agent name or ID"),
    cron_expr: str = typer.Argument(..., help="Cron expression (e.g., '0 9 * * *')"),
    area_opt: str = typer.Option(None, "--area", "-a", help="Set area (creates agent if needed)"),
    goal_opt: str = typer.Option(None, "--goal", "-g", help="Set goal"),
    flow_opt: str = typer.Option(None, "--flow", help="Set flow"),
):
    """Schedule an agent to run on cron (cron stimulus).

    Examples:
        lfd cron swift-falcon "0 9 * * *"                # run at 9am daily
        lfd cron swift-falcon "0 9 * * *" --area src/    # create + set area + schedule
        lfd cron swift-falcon "0 10 * * MON"             # run at 10am Mondays
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Get or create agent
    agent = _resolve_agent(name, repo, c)
    if not agent:
        if area_opt:
            agent = create_agent(repo=repo, name=name)
        else:
            typer.echo(f"{c['red']}Error:{c['reset']} Agent '{name}' not found", err=True)
            typer.echo(f"Create with: lfd create {name}")
            raise typer.Exit(1)

    # Apply options if provided
    if area_opt:
        update_agent(agent.id, area=[area_opt])
        agent.area = [area_opt]
    if goal_opt:
        update_agent(agent.id, goal=[goal_opt])
        agent.goal = [goal_opt]
    if flow_opt:
        flow_opt = _validate_flow(repo, flow_opt, c)
        update_agent(agent.id, flow=flow_opt)
        agent.flow = flow_opt

    # Update stimulus to cron
    update_agent(agent.id, stimulus=Stimulus("cron", cron=cron_expr))

    # Validate agent is configured
    _validate_agent_for_run(agent, c)

    # Validate flow exists
    _validate_flow(repo, agent.flow, c)

    # Refresh agent after updates
    agent = get_agent(agent.id)

    msg = f"{c['green']}Scheduled{c['reset']} {c['bold']}{agent.name}{c['reset']}"
    typer.echo(f"{msg} ({agent.short_id()})")
    typer.echo(f"  Area: {agent.area_display}")
    typer.echo(f"  Goals: {agent.goal_display}")
    typer.echo(f"  Flow: {agent.flow}")
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


def _get_daemon_health() -> dict | None:
    """Get daemon health from HTTP endpoint."""
    try:
        import urllib.request

        req = urllib.request.Request("http://127.0.0.1:8765/health", method="GET")
        with urllib.request.urlopen(req, timeout=2) as resp:
            import json

            data = json.loads(resp.read().decode())
            if data.get("ok"):
                return data.get("result")
        return None
    except Exception:
        return None


def _format_uptime(seconds: int) -> str:
    """Format uptime in human-readable form."""
    if seconds < 60:
        return f"{seconds}s"
    elif seconds < 3600:
        return f"{seconds // 60}m"
    elif seconds < 86400:
        return f"{seconds // 3600}h {(seconds % 3600) // 60}m"
    else:
        return f"{seconds // 86400}d {(seconds % 86400) // 3600}h"


@app.command()
def status(
    agent_id: str = typer.Argument(None, help="Agent ID (optional, shows all if omitted)"),
    ids_only: bool = typer.Option(False, "--ids", help="Print agent IDs only (for scripting)"),
):
    """Show status of agents."""
    c = _colors()

    # Machine-readable output for scripting
    if ids_only:
        for agent in list_agents():
            typer.echo(agent.id)
        return

    if agent_id:
        # Try to find the agent
        agent = get_agent(agent_id)
        if not agent:
            typer.echo(f"{c['red']}Error:{c['reset']} Agent '{agent_id}' not found", err=True)
            raise typer.Exit(1)
        _print_agent_detail(agent, c)
    else:
        # Show daemon health
        health = _get_daemon_health()
        if health:
            uptime = _format_uptime(health.get("uptime_seconds", 0))
            version = health.get("version", "?")
            pid = health.get("pid", "?")
            status_line = f"v{version}, pid {pid}, up {uptime}"
            typer.echo(f"Daemon: {c['green']}healthy{c['reset']} ({status_line})")
        else:
            typer.echo(f"Daemon: {c['red']}not running{c['reset']}")
            typer.echo(f"  {c['dim']}Start with: lfd install{c['reset']}")
        typer.echo("")

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

        agents = list_agents()
        if not agents:
            typer.echo(f"{c['dim']}No agents configured{c['reset']}")
            typer.echo("Start an agent with: lfd loop <flow> <area>")
            return

        typer.echo(f"{'ID':<9} {'STIMULUS':<12} {'AREA':<30} {'STATUS':<10} {'ITER':<6} REPO")
        typer.echo("-" * 95)

        for agent in agents:
            status_c = _status_color(agent.status, c)
            display_str = _agent_display(agent)
            if len(display_str) > 30:
                display_str = display_str[:27] + "..."

            repo_short = str(agent.repo).replace(str(Path.home()), "~")
            if len(repo_short) > 20:
                repo_short = "..." + repo_short[-17:]

            typer.echo(
                f"{agent.short_id():<9} {str(agent.stimulus):<12} {display_str:<30} "
                f"{status_c}{agent.status.value:<10}{c['reset']} "
                f"{agent.iteration:<6} {repo_short}"
            )


def _print_agent_detail(agent: Agent, c: dict[str, str]) -> None:
    """Print detailed info for an agent."""
    status_c = _status_color(agent.status, c)

    typer.echo(f"{c['bold']}{agent.area_display}{c['reset']} ({agent.short_id()})")
    typer.echo(f"  Stimulus: {agent.stimulus}")
    typer.echo(f"  Status: {status_c}{agent.status.value}{c['reset']}")
    typer.echo(f"  Repo: {agent.repo}")
    typer.echo(f"  Main branch: {agent.main_branch}")
    typer.echo(f"  Goals: {agent.goal_display}")
    typer.echo(f"  Flow: {agent.flow}")
    typer.echo(f"  Iteration: {agent.iteration}")

    # Show recent runs
    runs = list_runs_for_agent(agent.id, limit=5)
    if runs:
        typer.echo(f"\n  {c['dim']}Recent runs:{c['reset']}")
        for run in runs:
            from loopflow.lfd.models import FlowRunStatus

            run_status_c = (
                c["green"]
                if run.status == FlowRunStatus.COMPLETED
                else c["red"]
                if run.status == FlowRunStatus.FAILED
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
    agent_id: str = typer.Argument(None, help="Agent ID to stop (omit with --all)"),
    all_agents: bool = typer.Option(False, "--all", help="Stop all running agents"),
    force: bool = typer.Option(False, "-f", "--force", help="Force kill (SIGKILL)"),
):
    """Stop a running agent."""
    c = _colors()

    if all_agents:
        # Stop all running agents
        stopped = 0
        for agent in list_agents():
            if agent.status == AgentStatus.RUNNING:
                if stop_agent(agent.id, force=force):
                    msg = f"{c['yellow']}Stopped{c['reset']} {_agent_display(agent)}"
                    typer.echo(f"{msg} ({agent.short_id()})")
                    stopped += 1
        if stopped == 0:
            typer.echo(f"{c['dim']}No running agents to stop{c['reset']}")
        else:
            typer.echo(f"\nStopped {stopped} agent{'s' if stopped != 1 else ''}")
        return

    if not agent_id:
        typer.echo(f"{c['red']}Error:{c['reset']} Provide an agent ID or use --all", err=True)
        raise typer.Exit(1)

    agent = get_agent(agent_id)
    if not agent:
        typer.echo(f"{c['red']}Error:{c['reset']} Agent '{agent_id}' not found", err=True)
        raise typer.Exit(1)

    if stop_agent(agent.id, force=force):
        msg = f"{c['yellow']}Stopped{c['reset']} {c['bold']}{_agent_display(agent)}{c['reset']}"
        typer.echo(f"{msg} ({agent.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to stop agent", err=True)
        raise typer.Exit(1)


@app.command()
def prs(
    agent_id: str = typer.Argument(..., help="Agent ID"),
    limit: int = typer.Option(10, "-n", "--limit", help="Number of PRs to show"),
):
    """Show PRs created by an agent."""
    c = _colors()

    agent = get_agent(agent_id)
    if not agent:
        typer.echo(f"{c['red']}Error:{c['reset']} Agent '{agent_id}' not found", err=True)
        raise typer.Exit(1)

    runs = list_runs_for_agent(agent.id, limit=limit)
    runs_with_prs = [r for r in runs if r.pr_url]

    if not runs_with_prs:
        typer.echo(f"{c['dim']}No PRs found for '{agent.area_display}'{c['reset']}")
        return

    typer.echo(f"{c['bold']}{agent.area_display}{c['reset']} PRs ({agent.short_id()})")
    typer.echo("")

    from loopflow.lfd.models import FlowRunStatus

    for run in runs_with_prs:
        status_c = c["green"] if run.status == FlowRunStatus.COMPLETED else c["red"]
        started = run.started_at.strftime("%Y-%m-%d") if run.started_at else "?"
        typer.echo(
            f"  #{run.iteration:<3} {status_c}{run.status.value:<10}{c['reset']} "
            f"{c['dim']}{started}{c['reset']}  {run.pr_url}"
        )


@app.command()
def rm(
    agent_id: str = typer.Argument(..., help="Agent ID to remove"),
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove an agent and its history."""
    c = _colors()

    agent = get_agent(agent_id)
    if not agent:
        typer.echo(f"{c['red']}Error:{c['reset']} Agent '{agent_id}' not found", err=True)
        raise typer.Exit(1)

    if agent.status == AgentStatus.RUNNING:
        typer.echo(
            f"{c['red']}Error:{c['reset']} Agent is running. Stop it first with: "
            f"lfd stop {agent_id}",
            err=True,
        )
        raise typer.Exit(1)

    if not force:
        confirm = typer.confirm(f"Delete agent '{agent.area_display}' ({agent.short_id()})?")
        if not confirm:
            raise typer.Abort()

    if delete_agent(agent.id):
        typer.echo(f"Deleted: {agent.area_display} ({agent.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to delete agent", err=True)
        raise typer.Exit(1)


@app.command()
def logs(
    agent_id: str = typer.Argument(..., help="Agent ID"),
    follow: bool = typer.Option(False, "-f", "--follow", help="Follow output (like tail -f)"),
    lines: int = typer.Option(50, "-n", "--lines", help="Number of lines to show"),
):
    """Show logs for an agent's current run."""
    c = _colors()
    agent = get_agent(agent_id)
    if not agent:
        typer.echo(f"{c['red']}Error:{c['reset']} Agent '{agent_id}' not found", err=True)
        raise typer.Exit(1)

    # Get latest run for this agent
    runs = list_runs_for_agent(agent.id, limit=1)
    if not runs:
        typer.echo(f"{c['dim']}No runs found for '{agent.area_display}'{c['reset']}")
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


# Git hooks commands

hooks_app = typer.Typer(help="Git hook management")
app.add_typer(hooks_app, name="hooks")


@hooks_app.command("install")
def hooks_install_cmd(
    repo_path: str = typer.Argument(None, help="Repository path (default: current directory)"),
):
    """Install lfd notification hooks in a repository."""
    c = _colors()

    if repo_path:
        repo = Path(repo_path).resolve()
    else:
        repo = get_wt_from_cwd()

    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    installed = install_hooks(repo)
    if installed:
        typer.echo(f"{c['green']}Installed{c['reset']} hooks: {', '.join(installed)}")
    else:
        typer.echo(f"{c['dim']}Hooks already installed{c['reset']}")


@hooks_app.command("uninstall")
def hooks_uninstall_cmd(
    repo_path: str = typer.Argument(None, help="Repository path (default: current directory)"),
):
    """Remove lfd notification hooks from a repository."""
    c = _colors()

    if repo_path:
        repo = Path(repo_path).resolve()
    else:
        repo = get_wt_from_cwd()

    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    removed = uninstall_hooks(repo)
    if removed:
        typer.echo(f"{c['yellow']}Removed{c['reset']} hooks: {', '.join(removed)}")
    else:
        typer.echo(f"{c['dim']}No hooks to remove{c['reset']}")


@hooks_app.command("status")
def hooks_status_cmd(
    repo_path: str = typer.Argument(None, help="Repository path (default: current directory)"),
):
    """Check which lfd hooks are installed."""
    c = _colors()

    if repo_path:
        repo = Path(repo_path).resolve()
    else:
        repo = get_wt_from_cwd()

    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    status = hooks_status(repo)
    typer.echo(f"Hooks in {c['dim']}{repo}{c['reset']}")
    for hook, installed in status.items():
        icon = f"{c['green']}✓{c['reset']}" if installed else f"{c['dim']}✗{c['reset']}"
        typer.echo(f"  {icon} {hook}")


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

    all_goals = list_goals(repo)
    if not all_goals:
        typer.echo(f"{c['dim']}No goals found in {goals_dir}{c['reset']}")
        return

    typer.echo(f"Goals in {c['dim']}{goals_dir}/{c['reset']}")
    typer.echo("")

    for goal_name in all_goals:
        typer.echo(f"  {c['bold']}{goal_name}{c['reset']}")

    typer.echo("")
    typer.echo(f"{len(all_goals)} goal{'s' if len(all_goals) != 1 else ''} found")


def main() -> None:
    """Entry point for lfd command."""
    if len(sys.argv) == 1:
        sys.argv.append("status")
    app()


if __name__ == "__main__":
    main()
