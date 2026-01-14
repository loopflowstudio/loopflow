"""lfd: Agent orchestration daemon.

Unix socket daemon that owns agent lifecycle, trigger evaluation, and session tracking.
"""

import asyncio
from pathlib import Path

from loopflow.lfd.server import run_server

SOCKET_PATH = Path.home() / ".lf" / "lfd.sock"


def main() -> None:
    """Entry point for lfd command."""
    import sys
    import typer

    app = typer.Typer(help="Loopflow daemon")

    @app.command()
    def start(foreground: bool = typer.Option(False, "--foreground", "-f", help="Run in foreground")):
        """Start the daemon."""
        if foreground:
            asyncio.run(run_server(SOCKET_PATH))
        else:
            from loopflow.lfd.launchd import is_running, install
            if is_running():
                typer.echo("lfd is already running")
                raise typer.Exit(1)
            if install():
                typer.echo("lfd started")
            else:
                typer.echo("Failed to start lfd")
                raise typer.Exit(1)

    @app.command()
    def stop():
        """Stop the daemon."""
        from loopflow.lfd.launchd import uninstall
        if uninstall():
            typer.echo("lfd stopped")
        else:
            typer.echo("Failed to stop lfd")
            raise typer.Exit(1)

    @app.command()
    def status():
        """Show daemon status."""
        from loopflow.lfd.launchd import is_running
        from loopflow.lfd.client import DaemonClient

        if not is_running():
            typer.echo("lfd is not running")
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
        from loopflow.lfd.launchd import install as do_install
        if do_install():
            typer.echo("lfd installed and started")
        else:
            typer.echo("Failed to install lfd")
            raise typer.Exit(1)

    @app.command()
    def uninstall():
        """Remove launchd plist."""
        from loopflow.lfd.launchd import uninstall as do_uninstall
        if do_uninstall():
            typer.echo("lfd uninstalled")
        else:
            typer.echo("Failed to uninstall lfd")
            raise typer.Exit(1)

    if len(sys.argv) == 1:
        # Default to status if no command
        sys.argv.append("status")

    app()
