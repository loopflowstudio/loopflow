"""Maestro daemon management commands."""

import os
import signal
import subprocess
import sys
from pathlib import Path

import typer

app = typer.Typer(help="Maestro daemon management.")


def _get_paths() -> tuple[Path, Path, Path]:
    """Get maestro file paths."""
    lf_dir = Path.home() / ".lf"
    return (
        lf_dir / "maestro.sock",
        lf_dir / "maestro.json",
        lf_dir / "maestro.pid",
    )


def _is_running(pid_path: Path) -> bool:
    """Check if maestro is running."""
    if not pid_path.exists():
        return False

    try:
        pid = int(pid_path.read_text().strip())
        os.kill(pid, 0)  # Signal 0 checks if process exists
        return True
    except (OSError, ValueError):
        # Process doesn't exist or invalid PID
        return False


@app.command()
def start():
    """Start the maestro daemon."""
    socket_path, state_path, pid_path = _get_paths()

    if _is_running(pid_path):
        typer.echo("Maestro already running")
        raise typer.Exit(0)

    # Start daemon as background process
    process = subprocess.Popen(
        [sys.executable, "-m", "loopflow.maestro.daemon"],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )

    # Save PID
    pid_path.parent.mkdir(parents=True, exist_ok=True)
    pid_path.write_text(str(process.pid))

    typer.echo(f"Maestro listening on {socket_path}")


@app.command()
def stop():
    """Stop the maestro daemon."""
    socket_path, state_path, pid_path = _get_paths()

    if not _is_running(pid_path):
        typer.echo("Maestro not running")
        raise typer.Exit(0)

    pid = int(pid_path.read_text().strip())

    try:
        os.kill(pid, signal.SIGTERM)
        pid_path.unlink()
        if socket_path.exists():
            socket_path.unlink()
        typer.echo("Maestro stopped")
    except OSError:
        typer.echo("Failed to stop maestro", err=True)
        raise typer.Exit(1)
