"""lfwork: Work queue CLI."""

import sys
from pathlib import Path

import typer

from loopflow.lf.config import load_config
from loopflow.lf.context import find_worktree_root
from loopflow.lfd.work.asana_backend import AsanaBackend
from loopflow.lfd.work.backend import WorkBackend
from loopflow.lfd.work.file_backend import FileBackend
from loopflow.lfd.work.models import WorkItem, get_next_work

app = typer.Typer(help="Work queue management")


def _get_backend(repo_root: Path) -> WorkBackend:
    """Get the configured work backend."""
    config = load_config(repo_root)

    if config and config.work and config.work.backend == "asana":
        if not config.work.asana or not config.work.asana.project_id:
            typer.echo("Error: work.asana.project_id not configured", err=True)
            raise typer.Exit(1)
        return AsanaBackend(config.work.asana.project_id)

    return FileBackend(repo_root)


def _format_item(item: WorkItem, verbose: bool = False) -> str:
    """Format a work item for display."""
    status_icons = {
        "proposed": "?",
        "approved": "✓",
        "active": "▶",
        "done": "✔",
    }
    icon = status_icons.get(item.status, " ")

    claimed = " [human]" if item.claimed_by == "human" else ""
    blocked = f" [blocked: {item.blocked_on}]" if item.blocked_on else ""

    line = f"{icon} {item.id:<20} {item.title}{claimed}{blocked}"

    if verbose:
        line += f"\n    status={item.status}"
        if item.worktree:
            line += f" worktree={item.worktree}"

    return line


@app.command("list")
def list_items(
    status: str = typer.Option(None, "--status", "-s", help="Filter by status"),
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Show details"),
) -> None:
    """List work items."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    backend = _get_backend(repo_root)
    items = backend.list_items(status=status)

    if not items:
        typer.echo("No work items")
        return

    for item in items:
        typer.echo(_format_item(item, verbose))


@app.command()
def show(
    item_id: str = typer.Argument(help="Work item ID"),
) -> None:
    """Show a work item."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    backend = _get_backend(repo_root)
    item = backend.get_item(item_id)

    if not item:
        typer.echo(f"Error: Work item '{item_id}' not found", err=True)
        raise typer.Exit(1)

    typer.echo(f"# {item.title}")
    typer.echo(f"ID: {item.id}")
    typer.echo(f"Status: {item.status}")
    if item.claimed_by:
        typer.echo(f"Claimed by: {item.claimed_by}")
    if item.blocked_on:
        typer.echo(f"Blocked on: {item.blocked_on}")
    if item.worktree:
        typer.echo(f"Worktree: {item.worktree}")
    typer.echo("")
    typer.echo(item.description)
    if item.notes:
        typer.echo("\n## Notes")
        typer.echo(item.notes)


@app.command()
def add(
    title: str = typer.Argument(help="Work item title"),
    description: str = typer.Option("", "--description", "-d", help="Description"),
) -> None:
    """Add a new work item (status=proposed)."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    backend = _get_backend(repo_root)
    item = WorkItem(
        id="",
        title=title,
        description=description,
        status="proposed",
    )
    created = backend.create_item(item)
    typer.echo(f"Created: {created.id}")


@app.command()
def approve(
    item_id: str = typer.Argument(help="Work item ID"),
) -> None:
    """Approve a proposed work item."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    backend = _get_backend(repo_root)
    item = backend.update_item(item_id, status="approved")

    if not item:
        typer.echo(f"Error: Work item '{item_id}' not found", err=True)
        raise typer.Exit(1)

    typer.echo(f"Approved: {item_id}")


@app.command()
def reject(
    item_id: str = typer.Argument(help="Work item ID"),
) -> None:
    """Reject (delete) a work item."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    backend = _get_backend(repo_root)
    if backend.delete_item(item_id):
        typer.echo(f"Rejected: {item_id}")
    else:
        typer.echo(f"Error: Work item '{item_id}' not found", err=True)
        raise typer.Exit(1)


@app.command()
def claim(
    item_id: str = typer.Argument(help="Work item ID"),
) -> None:
    """Claim a work item for human work."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    backend = _get_backend(repo_root)
    item = backend.update_item(item_id, claimed_by="human")

    if not item:
        typer.echo(f"Error: Work item '{item_id}' not found", err=True)
        raise typer.Exit(1)

    typer.echo(f"Claimed: {item_id}")


@app.command()
def release(
    item_id: str = typer.Argument(help="Work item ID"),
) -> None:
    """Release a claimed work item."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    backend = _get_backend(repo_root)
    item = backend.update_item(item_id, claimed_by=None)

    if not item:
        typer.echo(f"Error: Work item '{item_id}' not found", err=True)
        raise typer.Exit(1)

    typer.echo(f"Released: {item_id}")


@app.command()
def blocked(
    verbose: bool = typer.Option(False, "--verbose", "-v", help="Show details"),
) -> None:
    """Show items blocked on user input."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    backend = _get_backend(repo_root)
    items = backend.list_items()
    blocked_items = [i for i in items if i.blocked_on]

    if not blocked_items:
        typer.echo("No blocked items")
        return

    for item in blocked_items:
        typer.echo(_format_item(item, verbose))


@app.command("next")
def next_item() -> None:
    """Show the next work item an agent would pick."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    backend = _get_backend(repo_root)
    items = backend.list_items()
    item = get_next_work(items)

    if not item:
        typer.echo("No available work")
        return

    typer.echo(_format_item(item, verbose=True))


def main() -> None:
    """Entry point for lfwork command."""
    if len(sys.argv) == 1:
        sys.argv.append("list")
    app()


if __name__ == "__main__":
    main()
