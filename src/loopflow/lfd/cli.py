"""lfd: Loopflow daemon.

Commands for managing waves of autonomous work.
"""

import os

# Suppress gRPC fork handler spam when running subprocesses
os.environ.setdefault("GRPC_ENABLE_FORK_SUPPORT", "0")

import asyncio
import json
import socket
import subprocess
import sys
from pathlib import Path
from typing import Optional

import typer

from loopflow.lf.config import load_config, parse_model
from loopflow.lf.context import ContextConfig, format_prompt, gather_prompt_components
from loopflow.lf.directions import list_directions, parse_list_arg, resolve_directions
from loopflow.lf.flows import load_flow
from loopflow.lf.git import autocommit, find_main_repo
from loopflow.lf.launcher import build_model_interactive_command
from loopflow.lf.logging import get_log_dir
from loopflow.lf.naming import generate_next_branch
from loopflow.lfd.daemon.launchd import install as launchd_install
from loopflow.lfd.daemon.launchd import is_running
from loopflow.lfd.daemon.launchd import uninstall as launchd_uninstall
from loopflow.lfd.daemon.server import run_server
from loopflow.lfd.db import reset_db
from loopflow.lfd.flow_run import list_runs_for_wave
from loopflow.lfd.git_hooks import (
    hooks_status,
    install_hooks,
    uninstall_hooks,
)
from loopflow.lfd.models import MergeMode, StepRunStatus, Wave, WaveStatus
from loopflow.lfd.step_run import get_waiting_step_run, update_step_run_status
from loopflow.lfd.stimulus import (
    create_stimulus,
    delete_stimulus,
    disable_stimulus,
    enable_stimulus,
    get_stimulus,
    list_stimuli,
)
from loopflow.lfd.wave import (
    create_wave,
    delete_wave,
    get_wave,
    get_wave_by_name,
    get_wave_by_worktree,
    get_wt_from_cwd,
    list_waves,
    start_wave,
    stop_wave,
    update_wave,
    update_wave_stacking,
    update_wave_status,
    update_wave_worktree_branch,
)
from loopflow.lf.ops.shell import write_directive

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"

app = typer.Typer(help="Loopflow daemon - waves of autonomous work")


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


def _status_color(status: WaveStatus, c: dict[str, str]) -> str:
    if status == WaveStatus.RUNNING:
        return c["green"]
    elif status == WaveStatus.ERROR:
        return c["red"]
    elif status == WaveStatus.WAITING:
        return c["yellow"]
    return c["dim"]


def _stimulus_display(wave_id: str) -> str:
    """Get stimulus display string for a wave."""
    stimuli = list_stimuli(wave_id=wave_id)
    if not stimuli:
        return "(none)"
    kinds = sorted(set(s.kind for s in stimuli))
    return ",".join(kinds)


def _wave_display(wave: Wave) -> str:
    """Return area, flow, and direction for display."""
    return f"{wave.area_display} [{wave.flow}] [{wave.direction_display}]"


def _validate_flow(repo: Path, flow: str, c: dict[str, str]) -> str:
    """Validate and normalize flow name."""
    normalized = flow.strip()
    if not normalized:
        typer.echo(f"{c['red']}Error:{c['reset']} Flow cannot be empty", err=True)
        raise typer.Exit(1)

    loaded_flow = load_flow(normalized, repo)
    if not loaded_flow:
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
def reset(
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Stop all waves, delete database, reinitialize with latest schema."""
    c = _colors()

    if not force:
        confirm = typer.confirm("This will stop all waves and delete all wave history. Continue?")
        if not confirm:
            raise typer.Abort()

    # Stop all running waves
    stopped = 0
    for wave in list_waves():
        if wave.status == WaveStatus.RUNNING:
            if stop_wave(wave.id, force=True):
                stopped += 1

    if stopped > 0:
        typer.echo(f"Stopped {stopped} running wave{'s' if stopped != 1 else ''}")

    # Reset the database
    reset_db()
    typer.echo(f"{c['green']}Database reset.{c['reset']} All waves and history cleared.")


@app.command()
def start(
    areas: list[str] = typer.Argument(None, help="Areas to start (all idle if omitted)"),
    all_waves: bool = typer.Option(False, "--all", help="Include waiting waves"),
):
    """Start multiple waves in parallel.

    Without arguments, starts all idle waves. With --all, also starts waiting waves.
    """
    c = _colors()
    repo = get_wt_from_cwd()

    # Get waves to start
    if areas:
        # Start specific areas
        waves_to_start = []
        for area in areas:
            wave = None
            for w in list_waves(repo=repo):
                if area in w.area:
                    wave = w
                    break
            if not wave:
                typer.echo(
                    f"{c['yellow']}Warning:{c['reset']} Wave for '{area}' not found, skipping",
                    err=True,
                )
            else:
                waves_to_start.append(wave)
    else:
        # Start all eligible waves
        waves_to_start = []
        for wave in list_waves(repo=repo):
            if wave.status == WaveStatus.IDLE:
                waves_to_start.append(wave)
            elif all_waves and wave.status == WaveStatus.WAITING:
                waves_to_start.append(wave)

    if not waves_to_start:
        typer.echo(f"{c['dim']}No waves to start{c['reset']}")
        return

    # Start each wave
    started = 0
    for wave in waves_to_start:
        result = start_wave(wave.id)
        if result:
            msg = f"{c['green']}Started{c['reset']} {c['bold']}{wave.area_display}{c['reset']}"
            typer.echo(f"{msg} ({wave.short_id()})")
            started += 1
        elif result.reason == "already_running":
            typer.echo(f"{c['dim']}Already running:{c['reset']} {wave.area_display}")
        elif result.reason == "waiting":
            msg = f"{c['yellow']}Waiting:{c['reset']} {wave.area_display}"
            typer.echo(f"{msg} ({result.outstanding} outstanding)")
        else:
            typer.echo(f"{c['red']}Failed:{c['reset']} {wave.area_display}")

    typer.echo(f"\nStarted {started}/{len(waves_to_start)} waves")


# Wave commands


def _resolve_wave(name_or_id: str, repo: Path, c: dict[str, str]) -> Wave | None:
    """Resolve wave by name or ID. Returns None if not found."""
    # Try by name first
    wave = get_wave_by_name(name_or_id, repo)
    if wave:
        return wave
    # Then try by ID
    return get_wave(name_or_id)


def _validate_wave_for_run(wave: Wave, c: dict[str, str]) -> None:
    """Validate wave has required config for running. Exits on error."""
    if wave.area is None:
        typer.echo(
            f"{c['red']}Error:{c['reset']} Wave '{wave.name}' has no area configured",
            err=True,
        )
        typer.echo(f"Set area with: lfd area {wave.name} <path>")
        raise typer.Exit(1)


@app.command()
def create(
    name: str = typer.Argument(None, help="Wave name (generated if omitted)"),
):
    """Create a new wave.

    Creates a wave with the given name (or generates one).
    Configure with `lfd area`, `lfd direction`, `lfd flow` before running.

    Examples:
        lfd create                    # create with generated name
        lfd create swift-falcon       # create with specific name
    """
    c = _colors()
    # Use main repo (waves are shared across worktrees)
    repo = find_main_repo() or get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Check if name already exists
    if name:
        existing = get_wave_by_name(name, repo)
        if existing:
            typer.echo(f"Wave '{name}' already exists ({existing.short_id()})")
            return

    wave = create_wave(repo=repo, name=name)

    typer.echo(
        f"{c['green']}Created{c['reset']} {c['bold']}{wave.name}{c['reset']} ({wave.short_id()})"
    )

    if wave.worktree:
        typer.echo(f"  Worktree: {wave.worktree}")
        typer.echo(f"  Branch: {wave.branch}")

        # Switch to worktree via shell integration
        if not write_directive(f"cd {wave.worktree}"):
            typer.echo(f"\ncd {wave.worktree}")
    else:
        typer.echo(f"  Repo: {repo}")
        typer.echo(f"  {c['yellow']}(worktree creation failed){c['reset']}")


@app.command()
def area(
    name: str = typer.Argument(..., help="Wave name or ID"),
    paths: list[str] = typer.Argument(..., help="Area paths (e.g., src/, swift/)"),
):
    """Set the working area for a wave.

    Examples:
        lfd area swift-falcon src/
        lfd area swift-falcon src/ tests/
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    wave = _resolve_wave(name, repo, c)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{name}' not found", err=True)
        raise typer.Exit(1)

    updated = update_wave(wave.id, area=paths)
    if updated:
        typer.echo(f"Set area: {', '.join(paths)}")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to update wave", err=True)
        raise typer.Exit(1)


@app.command()
def direction(
    name: str = typer.Argument(..., help="Wave name or ID"),
    direction_text: str = typer.Argument(..., help="Direction (inline text or preset name)"),
):
    """Set the direction for a wave.

    Examples:
        lfd direction swift-falcon "fix lint errors"
        lfd direction swift-falcon product-engineer
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    wave = _resolve_wave(name, repo, c)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{name}' not found", err=True)
        raise typer.Exit(1)

    updated = update_wave(wave.id, direction=[direction_text])
    if updated:
        typer.echo(f"Set direction: {direction_text}")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to update wave", err=True)
        raise typer.Exit(1)


@app.command("flow")
def set_flow(
    name: str = typer.Argument(..., help="Wave name or ID"),
    flow_name: str = typer.Argument(..., help="Flow name (from .lf/flows/)"),
):
    """Set the flow for a wave.

    Examples:
        lfd flow swift-falcon ship
        lfd flow swift-falcon polish
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    wave = _resolve_wave(name, repo, c)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{name}' not found", err=True)
        raise typer.Exit(1)

    # Validate flow exists
    flow_name = _validate_flow(repo, flow_name, c)

    updated = update_wave(wave.id, flow=flow_name)
    if updated:
        typer.echo(f"Set flow: {flow_name}")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to update wave", err=True)
        raise typer.Exit(1)


@app.command()
def show(
    name: str = typer.Argument(..., help="Wave name or ID"),
):
    """Show details for a wave.

    Examples:
        lfd show swift-falcon
    """
    c = _colors()
    repo = get_wt_from_cwd()

    wave = _resolve_wave(name, repo, c) if repo else get_wave(name)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{name}' not found", err=True)
        raise typer.Exit(1)

    _print_wave_detail(wave, c)


@app.command("list")
def list_cmd():
    """List all waves.

    Examples:
        lfd list
    """
    c = _colors()
    # Use main repo to find waves (worktrees share waves with their main repo)
    repo = find_main_repo() or get_wt_from_cwd()

    waves = list_waves(repo=repo)
    if not waves:
        typer.echo(f"{c['dim']}No waves configured{c['reset']}")
        typer.echo("Create a wave with: lfd create")
        return

    typer.echo(f"{'NAME':<20} {'AREA':<20} {'STATUS':<10} {'STIMULUS':<12} ID")
    typer.echo("-" * 75)

    for wave in waves:
        status_c = _status_color(wave.status, c)
        area_str = wave.area_display if wave.area else f"{c['dim']}(not set){c['reset']}"
        if len(area_str) > 20:
            area_str = area_str[:17] + "..."

        name_str = wave.name
        if len(name_str) > 20:
            name_str = name_str[:17] + "..."

        typer.echo(
            f"{name_str:<20} {area_str:<20} "
            f"{status_c}{wave.status.value:<10}{c['reset']} "
            f"{_stimulus_display(wave.id):<12} {wave.short_id()}"
        )


@app.command()
def loop(
    name: str = typer.Argument(..., help="Wave name or ID"),
    area_opt: str = typer.Option(None, "--area", "-a", help="Set area (creates wave if needed)"),
    direction_opt: Optional[list[str]] = typer.Option(
        None, "-d", "-D", "--direction", help="Direction (repeatable, or comma-separated)"
    ),
    flow_opt: str = typer.Option(None, "--flow", help="Set flow"),
    limit: int = typer.Option(None, "-l", "--limit", help="PR limit override"),
    merge_mode: str = typer.Option(None, "--merge-mode", help="Merge mode: pr or land"),
    foreground: bool = typer.Option(False, "-f", "--foreground", help="Run in foreground"),
):
    """Start a continuous wave loop (loop stimulus).

    Wave name is required. Creates the wave if it doesn't exist and --area is provided.
    Validates that area is configured before starting.

    Examples:
        lfd loop swift-falcon                                   # run existing wave
        lfd loop swift-falcon --area src/                       # create + set area + run
        lfd loop swift-falcon --area src/ -d concise -d fast
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Get or create wave
    wave = _resolve_wave(name, repo, c)
    if not wave:
        if area_opt:
            # Create new wave with options
            wave = create_wave(repo=repo, name=name)
        else:
            typer.echo(f"{c['red']}Error:{c['reset']} Wave '{name}' not found", err=True)
            typer.echo(f"Create with: lfd create {name}")
            raise typer.Exit(1)

    # Apply options if provided
    if area_opt:
        update_wave(wave.id, area=[area_opt])
        wave.area = [area_opt]
    if direction_opt:
        parsed_direction = parse_list_arg(direction_opt)
        update_wave(wave.id, direction=parsed_direction)
        wave.direction = parsed_direction
    if flow_opt:
        flow_opt = _validate_flow(repo, flow_opt, c)
        update_wave(wave.id, flow=flow_opt)
        wave.flow = flow_opt

    # Validate merge_mode if specified
    if merge_mode and merge_mode not in ("pr", "land"):
        typer.echo(f"{c['red']}Error:{c['reset']} merge-mode must be 'pr' or 'land'", err=True)
        raise typer.Exit(1)

    if limit is not None:
        update_wave(wave.id, pr_limit=limit)
    if merge_mode:
        update_wave(wave.id, merge_mode=MergeMode(merge_mode))

    # Create loop stimulus for the wave
    create_stimulus(wave.id, "loop")

    # Validate wave is configured
    _validate_wave_for_run(wave, c)

    # Validate flow exists
    _validate_flow(repo, wave.flow, c)

    # Refresh wave after updates
    wave = get_wave(wave.id)

    # Start it
    result = start_wave(wave.id, foreground=foreground)
    if result:
        if foreground:
            msg = f"{c['green']}Completed{c['reset']} loop {c['bold']}{wave.name}{c['reset']}"
            typer.echo(f"{msg} ({wave.short_id()})")
        else:
            msg = f"{c['green']}Started{c['reset']} loop {c['bold']}{wave.name}{c['reset']}"
            typer.echo(f"{msg} ({wave.short_id()})")
            typer.echo(f"  Repo: {repo}")
            typer.echo(f"  Area: {wave.area_display}")
            typer.echo(f"  Directions: {wave.direction_display}")
            typer.echo(f"  Flow: {wave.flow}")
            typer.echo(f"  PR limit: {wave.pr_limit}")
    elif result.reason == "already_running":
        typer.echo(f"Wave already running (PID {wave.pid})")
        raise typer.Exit(1)
    elif result.reason == "waiting":
        msg = f"{c['yellow']}Waiting:{c['reset']} {result.outstanding} outstanding PRs"
        typer.echo(f"{msg} (limit {wave.pr_limit})")
        typer.echo("Run 'lf ops land --squash' to land work")
        raise typer.Exit(0)
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to start wave", err=True)
        raise typer.Exit(1)


@app.command()
def run(
    name: str = typer.Argument(..., help="Wave name or ID"),
    area_opt: str = typer.Option(None, "--area", "-a", help="Set area (creates wave if needed)"),
    direction_opt: Optional[list[str]] = typer.Option(
        None, "-d", "-D", "--direction", help="Direction (repeatable, or comma-separated)"
    ),
    flow_opt: str = typer.Option(None, "--flow", help="Set flow"),
):
    """Run a wave once (once stimulus - single run).

    Wave name is required. Creates the wave if it doesn't exist and --area is provided.

    Examples:
        lfd run swift-falcon                                   # run existing wave once
        lfd run swift-falcon --area src/                       # create + set area + run once
        lfd run swift-falcon --area src/ -d concise
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Get or create wave
    wave = _resolve_wave(name, repo, c)
    if not wave:
        if area_opt:
            wave = create_wave(repo=repo, name=name)
        else:
            typer.echo(f"{c['red']}Error:{c['reset']} Wave '{name}' not found", err=True)
            typer.echo(f"Create with: lfd create {name}")
            raise typer.Exit(1)

    # Apply options if provided
    if area_opt:
        update_wave(wave.id, area=[area_opt])
        wave.area = [area_opt]
    if direction_opt:
        parsed_direction = parse_list_arg(direction_opt)
        update_wave(wave.id, direction=parsed_direction)
        wave.direction = parsed_direction
    if flow_opt:
        flow_opt = _validate_flow(repo, flow_opt, c)
        update_wave(wave.id, flow=flow_opt)
        wave.flow = flow_opt

    # Create once stimulus for the wave (single run)
    create_stimulus(wave.id, "once")

    # Validate wave is configured
    _validate_wave_for_run(wave, c)

    # Validate flow exists
    _validate_flow(repo, wave.flow, c)

    # Refresh wave after updates
    wave = get_wave(wave.id)

    # Start it in foreground (runs once)
    result = start_wave(wave.id, foreground=True)

    if result:
        msg = f"{c['green']}Completed{c['reset']} run {c['bold']}{wave.name}{c['reset']}"
        typer.echo(msg)
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to run", err=True)
        raise typer.Exit(1)


@app.command()
def connect(
    name: str = typer.Argument(..., help="Wave name or ID"),
):
    """Attach terminal to a wave's waiting interactive step.

    When a flow reaches an interactive step, the wave pauses in WAITING status.
    This command connects your terminal to that step, running an interactive
    Claude Code session.

    On successful exit (exit code 0):
    - Changes are autocommitted
    - Wave status returns to RUNNING
    - Daemon continues with next steps

    On abort (Ctrl+C, exit code != 0):
    - No commit
    - Wave returns to IDLE (can retry)

    Examples:
        lfd connect swift-falcon    # connect to waiting wave
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Resolve wave
    wave = _resolve_wave(name, repo, c)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{name}' not found", err=True)
        raise typer.Exit(1)

    # Check wave is waiting
    if wave.status != WaveStatus.WAITING:
        typer.echo(
            f"{c['red']}Error:{c['reset']} Wave '{wave.name}' is not waiting "
            f"(status: {wave.status.value})",
            err=True,
        )
        typer.echo("Only waves paused at interactive steps can be connected to.")
        raise typer.Exit(1)

    # Find the waiting StepRun
    step_run = get_waiting_step_run(wave.id)
    if not step_run:
        typer.echo(f"{c['red']}Error:{c['reset']} No waiting step found for wave", err=True)
        raise typer.Exit(1)

    step_name = step_run.step
    worktree_path = Path(step_run.worktree)

    # Update statuses to RUNNING
    update_step_run_status(step_run.id, StepRunStatus.RUNNING)
    update_wave_status(wave.id, WaveStatus.RUNNING)

    # Load config and resolve directions
    config = load_config(wave.repo)
    agent_model = config.agent_model if config else "claude:opus"
    backend, model_variant = parse_model(agent_model)
    skip_permissions = config.yolo if config else False

    direction = resolve_directions(wave.repo, wave.direction)
    context_paths = list(wave.area) if wave.area and wave.area[0] != "." else None

    # Assemble prompt
    components = gather_prompt_components(
        worktree_path,
        step=step_name,
        run_mode="interactive",
        direction=direction,
        context_config=ContextConfig(pathset=context_paths),
    )

    if not components.step:
        typer.echo(f"{c['red']}Error:{c['reset']} Step '{step_name}' not found", err=True)
        update_step_run_status(step_run.id, StepRunStatus.FAILED)
        update_wave_status(wave.id, WaveStatus.IDLE)
        raise typer.Exit(1)

    prompt = format_prompt(components)

    # Build interactive command
    command = build_model_interactive_command(
        backend,
        skip_permissions=skip_permissions,
        yolo=skip_permissions,
        model_variant=model_variant,
        sandbox_root=worktree_path.parent,
        workdir=worktree_path,
    )

    # Display session header
    typer.echo(f"\n{c['cyan']}━━━ {step_name} [interactive] ━━━{c['reset']}", err=True)
    typer.echo(f"{c['dim']}Wave: {wave.name} | Worktree: {worktree_path}{c['reset']}", err=True)
    typer.echo(err=True)

    # Change to worktree and run
    os.chdir(worktree_path)

    # Remove API keys so CLIs use subscriptions
    os.environ.pop("ANTHROPIC_API_KEY", None)
    os.environ.pop("OPENAI_API_KEY", None)

    # Run interactive session
    cmd_with_prompt = command + [prompt]
    result = subprocess.run(cmd_with_prompt, cwd=worktree_path)
    exit_code = result.returncode

    # Handle completion
    if exit_code == 0:
        typer.echo(f"\n{c['green']}Session completed successfully{c['reset']}")

        # Autocommit changes
        autocommit(worktree_path, step_name)

        # Update statuses
        update_step_run_status(step_run.id, StepRunStatus.COMPLETED)
        update_wave_status(wave.id, WaveStatus.RUNNING)

        typer.echo(f"{c['dim']}Changes committed. Daemon will continue flow.{c['reset']}")
    else:
        typer.echo(f"\n{c['yellow']}Session aborted (exit code {exit_code}){c['reset']}")

        # Mark failed, return to idle
        update_step_run_status(step_run.id, StepRunStatus.FAILED)
        update_wave_status(wave.id, WaveStatus.IDLE)

        msg = f"Wave returned to idle. Run 'lfd connect {wave.name}' to retry."
        typer.echo(f"{c['dim']}{msg}{c['reset']}")
        raise typer.Exit(exit_code)


@app.command()
def watch(
    name: str = typer.Argument(..., help="Wave name or ID"),
    area_opt: str = typer.Option(None, "--area", "-a", help="Set area (creates wave if needed)"),
    path_opt: str = typer.Option(None, "--path", "-p", help="Watch path (defaults to area)"),
    direction_opt: Optional[list[str]] = typer.Option(
        None, "-d", "-D", "--direction", help="Direction (repeatable, or comma-separated)"
    ),
    flow_opt: str = typer.Option(None, "--flow", help="Set flow"),
):
    """Watch for changes on origin/main (watch stimulus).

    Triggers when files in the watched path change on main.
    Watch path defaults to area but can be overridden.

    Examples:
        lfd watch swift-falcon                            # watch existing wave
        lfd watch swift-falcon --area src/                # create + set area + watch
        lfd watch swift-falcon --area src/ --path tests/  # watch tests/, work on src/
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    # Get or create wave
    wave = _resolve_wave(name, repo, c)
    if not wave:
        if area_opt:
            wave = create_wave(repo=repo, name=name)
        else:
            typer.echo(f"{c['red']}Error:{c['reset']} Wave '{name}' not found", err=True)
            typer.echo(f"Create with: lfd create {name}")
            raise typer.Exit(1)

    # Apply options if provided
    if area_opt:
        update_wave(wave.id, area=[area_opt])
        wave.area = [area_opt]
    if direction_opt:
        parsed_direction = parse_list_arg(direction_opt)
        update_wave(wave.id, direction=parsed_direction)
        wave.direction = parsed_direction
    if flow_opt:
        flow_opt = _validate_flow(repo, flow_opt, c)
        update_wave(wave.id, flow=flow_opt)
        wave.flow = flow_opt

    # Create watch stimulus for the wave
    create_stimulus(wave.id, "watch")

    # Validate wave is configured
    _validate_wave_for_run(wave, c)

    # Validate flow exists
    _validate_flow(repo, wave.flow, c)

    # Refresh wave after updates
    wave = get_wave(wave.id)

    watch_path = path_opt or wave.area_display
    msg = f"{c['green']}Watching{c['reset']} {c['bold']}{wave.name}{c['reset']}"
    typer.echo(f"{msg} ({wave.short_id()})")
    typer.echo(f"  Area: {wave.area_display}")
    typer.echo(f"  Directions: {wave.direction_display}")
    typer.echo(f"  Flow: {wave.flow}")
    typer.echo(f"  Activates when {watch_path} changes on main")


@app.command("cron")
def cron_cmd(
    name: str = typer.Argument(..., help="Wave name or ID"),
    cron_expr: str = typer.Argument(..., help="Cron expression (e.g., '0 9 * * *')"),
    area_opt: str = typer.Option(None, "--area", "-a", help="Set area (creates wave if needed)"),
    direction_opt: Optional[list[str]] = typer.Option(
        None, "-d", "-D", "--direction", help="Direction (repeatable, or comma-separated)"
    ),
    flow_opt: str = typer.Option(None, "--flow", help="Set flow"),
):
    """Schedule a wave to run on cron (cron stimulus).

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

    # Get or create wave
    wave = _resolve_wave(name, repo, c)
    if not wave:
        if area_opt:
            wave = create_wave(repo=repo, name=name)
        else:
            typer.echo(f"{c['red']}Error:{c['reset']} Wave '{name}' not found", err=True)
            typer.echo(f"Create with: lfd create {name}")
            raise typer.Exit(1)

    # Apply options if provided
    if area_opt:
        update_wave(wave.id, area=[area_opt])
        wave.area = [area_opt]
    if direction_opt:
        parsed_direction = parse_list_arg(direction_opt)
        update_wave(wave.id, direction=parsed_direction)
        wave.direction = parsed_direction
    if flow_opt:
        flow_opt = _validate_flow(repo, flow_opt, c)
        update_wave(wave.id, flow=flow_opt)
        wave.flow = flow_opt

    # Create cron stimulus for the wave
    create_stimulus(wave.id, "cron", cron_expr)

    # Validate wave is configured
    _validate_wave_for_run(wave, c)

    # Validate flow exists
    _validate_flow(repo, wave.flow, c)

    # Refresh wave after updates
    wave = get_wave(wave.id)

    msg = f"{c['green']}Scheduled{c['reset']} {c['bold']}{wave.name}{c['reset']}"
    typer.echo(f"{msg} ({wave.short_id()})")
    typer.echo(f"  Area: {wave.area_display}")
    typer.echo(f"  Directions: {wave.direction_display}")
    typer.echo(f"  Flow: {wave.flow}")
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
    wave_id: str = typer.Argument(None, help="Wave name or ID (optional, shows all if omitted)"),
    ids_only: bool = typer.Option(False, "--ids", help="Print wave IDs only (for scripting)"),
):
    """Show status of waves."""
    c = _colors()

    # Machine-readable output for scripting
    if ids_only:
        for wave in list_waves():
            typer.echo(wave.id)
        return

    if wave_id:
        # Try to find the wave
        repo = find_main_repo() or get_wt_from_cwd()
        wave = _resolve_wave(wave_id, repo, c) if repo else get_wave(wave_id)
        if not wave:
            typer.echo(f"{c['red']}Error:{c['reset']} Wave '{wave_id}' not found", err=True)
            raise typer.Exit(1)
        _print_wave_detail(wave, c)
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

        waves = list_waves()
        if not waves:
            typer.echo(f"{c['dim']}No waves configured{c['reset']}")
            typer.echo("Start a wave with: lfd loop <flow> <area>")
            return

        typer.echo(f"{'ID':<9} {'STIMULUS':<12} {'AREA':<30} {'STATUS':<10} {'ITER':<6} REPO")
        typer.echo("-" * 95)

        for wave in waves:
            status_c = _status_color(wave.status, c)
            display_str = _wave_display(wave)
            if len(display_str) > 30:
                display_str = display_str[:27] + "..."

            repo_short = str(wave.repo).replace(str(Path.home()), "~")
            if len(repo_short) > 20:
                repo_short = "..." + repo_short[-17:]

            typer.echo(
                f"{wave.short_id():<9} {_stimulus_display(wave.id):<12} {display_str:<30} "
                f"{status_c}{wave.status.value:<10}{c['reset']} "
                f"{wave.iteration:<6} {repo_short}"
            )


def _print_wave_detail(wave: Wave, c: dict[str, str]) -> None:
    """Print detailed info for a wave."""
    status_c = _status_color(wave.status, c)

    typer.echo(f"{c['bold']}{wave.area_display}{c['reset']} ({wave.short_id()})")
    typer.echo(f"  Stimulus: {_stimulus_display(wave.id)}")
    typer.echo(f"  Status: {status_c}{wave.status.value}{c['reset']}")
    typer.echo(f"  Repo: {wave.repo}")
    typer.echo(f"  Main branch: {wave.main_branch}")
    typer.echo(f"  Directions: {wave.direction_display}")
    typer.echo(f"  Flow: {wave.flow}")
    typer.echo(f"  Iteration: {wave.iteration}")

    # Show recent runs
    runs = list_runs_for_wave(wave.id, limit=5)
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
    wave_id: str = typer.Argument(None, help="Wave name or ID to stop (omit with --all)"),
    all_waves: bool = typer.Option(False, "--all", help="Stop all running waves"),
    force: bool = typer.Option(False, "-f", "--force", help="Force kill (SIGKILL)"),
):
    """Stop a running wave."""
    c = _colors()
    repo = find_main_repo() or get_wt_from_cwd()

    if all_waves:
        # Stop all running waves
        stopped = 0
        for wave in list_waves():
            if wave.status == WaveStatus.RUNNING:
                if stop_wave(wave.id, force=force):
                    msg = f"{c['yellow']}Stopped{c['reset']} {_wave_display(wave)}"
                    typer.echo(f"{msg} ({wave.short_id()})")
                    stopped += 1
        if stopped == 0:
            typer.echo(f"{c['dim']}No running waves to stop{c['reset']}")
        else:
            typer.echo(f"\nStopped {stopped} wave{'s' if stopped != 1 else ''}")
        return

    if not wave_id:
        typer.echo(
            f"{c['red']}Error:{c['reset']} Provide a wave name or ID, or use --all",
            err=True,
        )
        raise typer.Exit(1)

    wave = _resolve_wave(wave_id, repo, c) if repo else get_wave(wave_id)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{wave_id}' not found", err=True)
        raise typer.Exit(1)

    if stop_wave(wave.id, force=force):
        msg = f"{c['yellow']}Stopped{c['reset']} {c['bold']}{_wave_display(wave)}{c['reset']}"
        typer.echo(f"{msg} ({wave.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to stop wave", err=True)
        raise typer.Exit(1)


@app.command()
def prs(
    wave_id: str = typer.Argument(..., help="Wave name or ID"),
    limit: int = typer.Option(10, "-n", "--limit", help="Number of PRs to show"),
):
    """Show PRs created by a wave."""
    c = _colors()
    repo = find_main_repo() or get_wt_from_cwd()

    wave = _resolve_wave(wave_id, repo, c) if repo else get_wave(wave_id)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{wave_id}' not found", err=True)
        raise typer.Exit(1)

    runs = list_runs_for_wave(wave.id, limit=limit)
    runs_with_prs = [r for r in runs if r.pr_url]

    if not runs_with_prs:
        typer.echo(f"{c['dim']}No PRs found for '{wave.area_display}'{c['reset']}")
        return

    typer.echo(f"{c['bold']}{wave.area_display}{c['reset']} PRs ({wave.short_id()})")
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
    wave_id: str = typer.Argument(..., help="Wave name or ID to remove"),
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove a wave and its history."""
    c = _colors()
    repo = find_main_repo() or get_wt_from_cwd()

    wave = _resolve_wave(wave_id, repo, c) if repo else get_wave(wave_id)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{wave_id}' not found", err=True)
        raise typer.Exit(1)

    if wave.status == WaveStatus.RUNNING:
        typer.echo(
            f"{c['red']}Error:{c['reset']} Wave is running. Stop it first with: lfd stop {wave_id}",
            err=True,
        )
        raise typer.Exit(1)

    if not force:
        confirm = typer.confirm(f"Delete wave '{wave.area_display}' ({wave.short_id()})?")
        if not confirm:
            raise typer.Abort()

    if delete_wave(wave.id):
        typer.echo(f"Deleted: {wave.area_display} ({wave.short_id()})")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to delete wave", err=True)
        raise typer.Exit(1)


@app.command()
def logs(
    wave_id: str = typer.Argument(..., help="Wave name or ID"),
    follow: bool = typer.Option(False, "-f", "--follow", help="Follow output (like tail -f)"),
    lines: int = typer.Option(50, "-n", "--lines", help="Number of lines to show"),
):
    """Show logs for a wave's current run."""
    c = _colors()
    repo = find_main_repo() or get_wt_from_cwd()
    wave = _resolve_wave(wave_id, repo, c) if repo else get_wave(wave_id)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{wave_id}' not found", err=True)
        raise typer.Exit(1)

    # Get latest run for this wave
    runs = list_runs_for_wave(wave.id, limit=1)
    if not runs:
        typer.echo(f"{c['dim']}No runs found for '{wave.area_display}'{c['reset']}")
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


@app.command("list-directions")
def list_directions_cmd():
    """Show available directions in current repo."""
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    directions_dir = repo / ".lf" / "directions"
    if not directions_dir.exists():
        typer.echo(f"{c['dim']}No directions directory found at {directions_dir}{c['reset']}")
        typer.echo(
            "Create one with: mkdir -p .lf/directions && "
            "echo '# My Direction' > .lf/directions/my-direction.md"
        )
        return

    all_directions = list_directions(repo)
    if not all_directions:
        typer.echo(f"{c['dim']}No directions found in {directions_dir}{c['reset']}")
        return

    typer.echo(f"Directions in {c['dim']}{directions_dir}/{c['reset']}")
    typer.echo("")

    for direction_name in all_directions:
        typer.echo(f"  {c['bold']}{direction_name}{c['reset']}")

    typer.echo("")
    typer.echo(f"{len(all_directions)} direction{'s' if len(all_directions) != 1 else ''} found")


# Stacking commands


def _resolve_wave_from_worktree_or_name(
    name: str | None, repo: Path, c: dict[str, str]
) -> Wave | None:
    """Resolve wave from name argument or current worktree."""
    if name:
        return _resolve_wave(name, repo, c)

    # Try to find wave by current worktree
    worktree = get_wt_from_cwd()
    if worktree:
        wave = get_wave_by_worktree(worktree, repo)
        if wave:
            return wave

    return None


def _get_pr_number(repo_root: Path) -> int | None:
    """Get the PR number for the current branch."""
    result = subprocess.run(
        ["gh", "pr", "view", "--json", "number", "-q", ".number"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return int(result.stdout.strip())
    return None


def _get_pr_state(repo_root: Path, branch: str) -> str | None:
    """Get the state of a PR for a branch (OPEN, MERGED, CLOSED)."""
    result = subprocess.run(
        ["gh", "pr", "view", branch, "--json", "state", "-q", ".state"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip().upper()
    return None


def _enable_auto_merge(repo_root: Path, pr_number: int) -> bool:
    """Enable auto-merge on a PR. Returns True if successful."""
    merge_cmd = ["gh", "pr", "merge", str(pr_number), "--squash", "--auto"]
    result = subprocess.run(merge_cmd, cwd=repo_root, capture_output=True, text=True)
    return result.returncode == 0


@app.command("next")
def next_cmd(
    name: str = typer.Argument(None, help="Wave name (inferred from worktree if omitted)"),
    create_pr: bool = typer.Option(False, "-c", "--create-pr", help="Create PR if none exists"),
):
    """Stack a new branch on top of current work.

    Enables auto-merge on current PR, creates new branch from HEAD,
    and records base tracking for rebase-on-land.

    \b
    Examples:
        lfd next                    # from within wave worktree
        lfd next rust               # explicit wave name
        lfd next --create-pr        # create PR first, then stack
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo) or repo

    # Resolve wave
    wave = _resolve_wave_from_worktree_or_name(name, main_repo, c)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave not found", err=True)
        if name:
            typer.echo(f"Create with: lfd create {name}")
        else:
            typer.echo("Run from a wave worktree or specify wave name")
        raise typer.Exit(1)

    worktree = wave.worktree
    if not worktree or not worktree.exists():
        typer.echo(f"{c['red']}Error:{c['reset']} Wave has no worktree", err=True)
        raise typer.Exit(1)

    # Get current branch
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        typer.echo(f"{c['red']}Error:{c['reset']} Not on a branch", err=True)
        raise typer.Exit(1)

    old_branch = result.stdout.strip()

    # Get current HEAD SHA for base_commit
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    old_head = result.stdout.strip() if result.returncode == 0 else None

    # Get or create PR
    pr_number = _get_pr_number(worktree)
    if pr_number is None:
        if create_pr:
            typer.echo("Creating PR...")
            result = subprocess.run(["lf", "ops", "pr"], cwd=worktree)
            if result.returncode != 0:
                typer.echo(f"{c['red']}Error:{c['reset']} Failed to create PR", err=True)
                raise typer.Exit(1)
            pr_number = _get_pr_number(worktree)
            if pr_number is None:
                typer.echo(
                    f"{c['red']}Error:{c['reset']} Could not find PR after creation", err=True
                )
                raise typer.Exit(1)
        else:
            msg = "No open PR found. Run 'lf ops pr' first, or use --create-pr."
            typer.echo(f"{c['red']}Error:{c['reset']} {msg}", err=True)
            raise typer.Exit(1)

    # Enable auto-merge
    typer.echo(f"Enabling auto-merge for PR #{pr_number}...")
    if not _enable_auto_merge(worktree, pr_number):
        typer.echo(f"{c['yellow']}Warning:{c['reset']} Could not enable auto-merge", err=True)

    # Generate new branch name
    new_branch = generate_next_branch(wave.name, main_repo)

    # Create new branch from current HEAD
    typer.echo(f"Creating stacked branch {c['bold']}{new_branch}{c['reset']}...")
    result = subprocess.run(
        ["git", "checkout", "-b", new_branch],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to create branch", err=True)
        typer.echo(result.stderr)
        raise typer.Exit(1)

    # Push to origin with tracking
    result = subprocess.run(
        ["git", "push", "-u", "origin", new_branch],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo(f"{c['yellow']}Warning:{c['reset']} Failed to push to origin", err=True)

    # Update wave: new branch, record base tracking
    wave.branch = new_branch
    wave.base_branch = old_branch
    wave.base_commit = old_head
    update_wave_worktree_branch(wave.id, worktree, new_branch)
    update_wave_stacking(wave.id, old_branch, old_head)

    typer.echo(f"{c['green']}Stacked{c['reset']} on {c['dim']}{old_branch}{c['reset']}")
    typer.echo(f"New branch: {c['bold']}{new_branch}{c['reset']}")


@app.command("rebase")
def rebase_cmd(
    name: str = typer.Argument(None, help="Wave name (inferred from worktree if omitted)"),
):
    """Rebase stacked branch after base PR lands.

    Detects if base branch was squash-merged to main and rebases appropriately.

    \b
    Examples:
        lfd rebase                  # from within wave worktree
        lfd rebase rust             # explicit wave name
    """
    c = _colors()
    repo = get_wt_from_cwd()
    if not repo:
        typer.echo(f"{c['red']}Error:{c['reset']} Not in a git repository", err=True)
        raise typer.Exit(1)

    main_repo = find_main_repo(repo) or repo

    # Resolve wave
    wave = _resolve_wave_from_worktree_or_name(name, main_repo, c)
    if not wave:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave not found", err=True)
        raise typer.Exit(1)

    worktree = wave.worktree
    if not worktree or not worktree.exists():
        typer.echo(f"{c['red']}Error:{c['reset']} Wave has no worktree", err=True)
        raise typer.Exit(1)

    # Fetch latest
    typer.echo("Fetching origin...")
    subprocess.run(["git", "fetch", "origin"], cwd=worktree, capture_output=True)

    if not wave.base_branch:
        # No stacking - just rebase onto origin/main
        typer.echo("No base branch tracked, rebasing onto origin/main...")
        result = subprocess.run(
            ["git", "rebase", "origin/main"],
            cwd=worktree,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            typer.echo(f"{c['red']}Error:{c['reset']} Rebase failed", err=True)
            typer.echo(result.stderr)
            typer.echo("Resolve conflicts and run: git rebase --continue")
            raise typer.Exit(1)
        typer.echo(f"{c['green']}Rebased{c['reset']} onto origin/main")
        return

    # Check if base PR is merged
    pr_state = _get_pr_state(main_repo, wave.base_branch)
    if pr_state == "OPEN":
        typer.echo(f"{c['yellow']}Warning:{c['reset']} Base PR is still open")
        typer.echo(
            f"Wait for {c['dim']}{wave.base_branch}{c['reset']} to merge, then run lfd rebase again"
        )
        raise typer.Exit(1)

    if pr_state != "MERGED":
        typer.echo(f"Base PR state: {pr_state or 'unknown'}")

    # Squash-aware rebase using base_commit
    base_commit = wave.base_commit
    if not base_commit:
        typer.echo(
            f"{c['yellow']}Warning:{c['reset']} No base_commit recorded, using normal rebase"
        )
        result = subprocess.run(
            ["git", "rebase", "origin/main"],
            cwd=worktree,
            capture_output=True,
            text=True,
        )
    else:
        typer.echo(f"Rebasing onto origin/main from {c['dim']}{base_commit[:7]}{c['reset']}...")
        result = subprocess.run(
            ["git", "rebase", "--onto", "origin/main", base_commit],
            cwd=worktree,
            capture_output=True,
            text=True,
        )

    if result.returncode != 0:
        typer.echo(f"{c['red']}Error:{c['reset']} Rebase failed", err=True)
        typer.echo(result.stderr)
        typer.echo("Resolve conflicts and run: git rebase --continue")
        raise typer.Exit(1)

    # Push with force-with-lease
    typer.echo("Pushing rebased branch...")
    result = subprocess.run(
        ["git", "push", "--force-with-lease"],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo(f"{c['yellow']}Warning:{c['reset']} Failed to push", err=True)
        typer.echo(result.stderr)

    # Clear stacking info
    update_wave_stacking(wave.id, None, None)

    typer.echo(f"{c['green']}Rebased{c['reset']} onto origin/main")
    typer.echo(f"Cleared stacking from {c['dim']}{wave.base_branch}{c['reset']}")


# Stimuli sub-app
stimuli_app = typer.Typer(help="Manage wave stimuli (triggers)")
app.add_typer(stimuli_app, name="stimuli")


@stimuli_app.command("list")
def stimuli_list(
    wave: str = typer.Argument(..., help="Wave name or ID"),
):
    """List stimuli for a wave.

    Examples:
        lfd stimuli list swift-falcon
    """
    c = _colors()
    repo = get_wt_from_cwd()

    wave_obj = _resolve_wave(wave, repo, c)
    if not wave_obj:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{wave}' not found", err=True)
        raise typer.Exit(1)

    stimuli = list_stimuli(wave_id=wave_obj.id)
    if not stimuli:
        typer.echo(f"{c['dim']}No stimuli configured for {wave_obj.name}{c['reset']}")
        return

    typer.echo(f"Stimuli for {c['bold']}{wave_obj.name}{c['reset']} ({wave_obj.short_id()})")
    typer.echo("")

    for stim in stimuli:
        enabled_str = "" if stim.enabled else f" {c['dim']}(disabled){c['reset']}"
        cron_str = f" cron={stim.cron}" if stim.cron else ""
        typer.echo(f"  {stim.short_id()}  {stim.kind:<8}{cron_str}{enabled_str}")

    typer.echo("")
    typer.echo(f"{len(stimuli)} stimulus{'es' if len(stimuli) != 1 else ''}")


@stimuli_app.command("add")
def stimuli_add(
    wave: str = typer.Argument(..., help="Wave name or ID"),
    kind: str = typer.Option(..., "--kind", "-k", help="Stimulus kind: loop, watch, cron"),
    cron: str = typer.Option(None, "--cron", "-c", help="Cron expression (required for cron kind)"),
):
    """Add a stimulus to a wave.

    Examples:
        lfd stimuli add swift-falcon --kind watch
        lfd stimuli add swift-falcon --kind cron --cron "0 9 * * *"
    """
    c = _colors()
    repo = get_wt_from_cwd()

    wave_obj = _resolve_wave(wave, repo, c)
    if not wave_obj:
        typer.echo(f"{c['red']}Error:{c['reset']} Wave '{wave}' not found", err=True)
        raise typer.Exit(1)

    if kind not in ("once", "loop", "watch", "cron"):
        typer.echo(f"{c['red']}Error:{c['reset']} Invalid kind: {kind}", err=True)
        typer.echo("Valid kinds: once, loop, watch, cron")
        raise typer.Exit(1)

    if kind == "cron" and not cron:
        typer.echo(f"{c['red']}Error:{c['reset']} --cron is required for cron stimulus", err=True)
        raise typer.Exit(1)

    stimulus = create_stimulus(wave_obj.id, kind, cron)
    msg = f"{c['green']}Created{c['reset']} {kind} stimulus for {wave_obj.name}"
    typer.echo(f"{msg} ({stimulus.short_id()})")


@stimuli_app.command("enable")
def stimuli_enable(
    stimulus_id: str = typer.Argument(..., help="Stimulus ID"),
):
    """Enable a stimulus.

    Examples:
        lfd stimuli enable abc1234
    """
    c = _colors()

    if enable_stimulus(stimulus_id):
        typer.echo(f"{c['green']}Enabled{c['reset']} stimulus {stimulus_id}")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Stimulus '{stimulus_id}' not found", err=True)
        raise typer.Exit(1)


@stimuli_app.command("disable")
def stimuli_disable(
    stimulus_id: str = typer.Argument(..., help="Stimulus ID"),
):
    """Disable a stimulus.

    Examples:
        lfd stimuli disable abc1234
    """
    c = _colors()

    if disable_stimulus(stimulus_id):
        typer.echo(f"{c['yellow']}Disabled{c['reset']} stimulus {stimulus_id}")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Stimulus '{stimulus_id}' not found", err=True)
        raise typer.Exit(1)


@stimuli_app.command("rm")
def stimuli_rm(
    stimulus_id: str = typer.Argument(..., help="Stimulus ID"),
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove a stimulus.

    Examples:
        lfd stimuli rm abc1234
        lfd stimuli rm abc1234 -f
    """
    c = _colors()

    stim = get_stimulus(stimulus_id)
    if not stim:
        typer.echo(f"{c['red']}Error:{c['reset']} Stimulus '{stimulus_id}' not found", err=True)
        raise typer.Exit(1)

    if not force:
        confirm = typer.confirm(f"Delete {stim.kind} stimulus {stim.short_id()}?")
        if not confirm:
            typer.echo("Cancelled")
            raise typer.Exit(0)

    if delete_stimulus(stimulus_id):
        typer.echo(f"{c['red']}Deleted{c['reset']} stimulus {stimulus_id}")
    else:
        typer.echo(f"{c['red']}Error:{c['reset']} Failed to delete stimulus", err=True)
        raise typer.Exit(1)


def main() -> None:
    """Entry point for lfd command."""
    if len(sys.argv) == 1:
        sys.argv.append("status")
    app()


if __name__ == "__main__":
    main()
