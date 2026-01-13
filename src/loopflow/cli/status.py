"""Session status commands."""

from datetime import datetime

import typer

from loopflow.context import find_worktree_root
from loopflow.maestro.db import DEFAULT_DB_PATH, load_sessions

app = typer.Typer(help="Session status.")


def _format_time_ago(started_at: datetime) -> str:
    """Format time difference as '2m ago', '5h ago', etc."""
    delta = datetime.now() - started_at
    seconds = int(delta.total_seconds())

    if seconds < 60:
        return f"{seconds}s ago"
    elif seconds < 3600:
        return f"{seconds // 60}m ago"
    elif seconds < 86400:
        return f"{seconds // 3600}h ago"
    else:
        return f"{seconds // 86400}d ago"


@app.command()
def status(
    all_repos: bool = typer.Option(False, "--all", "-a", help="Show sessions from all repos"),
):
    """Show running sessions."""
    repo = None if all_repos else find_worktree_root()
    sessions = load_sessions(DEFAULT_DB_PATH, repo=repo)

    if not sessions:
        typer.echo("No running sessions")
        raise typer.Exit(0)

    # Print header
    typer.echo(f"{'ID':<10} {'TASK':<14} {'WORKTREE':<24} {'STATUS':<10} {'STARTED'}")

    # Print sessions
    for session in sessions:
        worktree_name = session.worktree.name
        time_ago = _format_time_ago(session.started_at)
        typer.echo(
            f"{session.id[:8]:<10} {session.task:<14} {worktree_name:<24} {session.status.value:<10} {time_ago}"
        )
