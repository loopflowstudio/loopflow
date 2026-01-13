"""Session management commands."""

import os
import signal
from pathlib import Path

import typer

from loopflow.context import find_worktree_root
from loopflow.logging import get_log_dir
from loopflow.maestro.db import (
    DEFAULT_DB_PATH,
    delete_session_data,
    load_sessions,
    update_session_status,
)
from loopflow.maestro.session import SessionStatus

app = typer.Typer(help="Session management.")


def _resolve_session(sessions, prefix: str):
    matches = [session for session in sessions if session.id.startswith(prefix)]
    if not matches:
        typer.echo(f"Error: No session matching '{prefix}'", err=True)
        raise typer.Exit(1)
    if len(matches) > 1:
        ids = ", ".join(session.id[:8] for session in matches)
        typer.echo(f"Error: Ambiguous session id '{prefix}': {ids}", err=True)
        raise typer.Exit(1)
    return matches[0]


@app.command()
def stop(
    session_id: str = typer.Argument(help="Session id (prefix ok)"),
    all_repos: bool = typer.Option(False, "--all", "-a", help="Search sessions from all repos"),
    force: bool = typer.Option(False, "--force", help="Send SIGKILL instead of SIGTERM"),
):
    """Stop a running session."""
    repo = None if all_repos else find_worktree_root()
    sessions = load_sessions(DEFAULT_DB_PATH, repo=repo, include_completed=True)
    session = _resolve_session(sessions, session_id)

    if session.status not in (SessionStatus.RUNNING, SessionStatus.WAITING):
        typer.echo(f"Session {session.id[:8]} is not running")
        raise typer.Exit(0)

    if not session.pid:
        typer.echo(f"Error: Session {session.id[:8]} has no PID to stop", err=True)
        raise typer.Exit(1)

    try:
        os.kill(session.pid, signal.SIGKILL if force else signal.SIGTERM)
    except OSError as e:
        typer.echo(f"Error: Failed to stop session {session.id[:8]}: {e}", err=True)
        raise typer.Exit(1)

    update_session_status(DEFAULT_DB_PATH, session.id, SessionStatus.ERROR)
    typer.echo(f"Stopped session {session.id[:8]}")


@app.command()
def prune(
    all_repos: bool = typer.Option(False, "--all", "-a", help="Prune sessions from all repos"),
):
    """Remove completed sessions and their logs."""
    repo = None if all_repos else find_worktree_root()
    sessions = load_sessions(DEFAULT_DB_PATH, repo=repo, include_completed=True)

    removed = 0
    for session in sessions:
        if session.status in (SessionStatus.RUNNING, SessionStatus.WAITING):
            continue

        log_dir = get_log_dir(Path(session.worktree))
        for suffix in (".log", ".jsonl"):
            log_path = log_dir / f"{session.id}{suffix}"
            if log_path.exists():
                try:
                    log_path.unlink()
                except OSError:
                    pass

        if delete_session_data(DEFAULT_DB_PATH, session.id):
            removed += 1

    typer.echo(f"Pruned {removed} sessions")
