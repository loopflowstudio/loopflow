"""Worktree management commands."""

import subprocess

import typer

from pathlib import Path

from loopflow.config import Config, load_config
from loopflow.git import GitError, create_worktree, find_main_repo, list_worktrees, remove_worktree

app = typer.Typer(help="Worktree management.")


def _find_workspace(worktree_path: Path, config: Config | None) -> Path | None:
    """Find workspace file in the worktree."""
    if config and config.ide.workspace:
        workspace_path = worktree_path / config.ide.workspace
        if workspace_path.exists():
            return workspace_path
        return None

    workspaces = list(worktree_path.glob("*.code-workspace"))
    if len(workspaces) == 1:
        return workspaces[0]

    return None


def _open_ide(worktree_path: Path, config: Config | None) -> None:
    """Open configured IDEs at worktree path."""
    ide = config.ide if config else None

    if not ide or ide.warp:
        subprocess.run(["open", f"warp://action/new_window?path={worktree_path}"])

    if not ide or ide.cursor:
        workspace = _find_workspace(worktree_path, config)
        if workspace:
            subprocess.run(["cursor", str(workspace)])
        else:
            subprocess.run(["cursor", str(worktree_path)])


@app.command()
def create(
    name: str = typer.Argument(help="Branch/worktree name"),
):
    """Create a worktree and branch, open IDEs."""
    main_repo = find_main_repo()
    if not main_repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    try:
        worktree_path = create_worktree(main_repo, name)
    except GitError as e:
        typer.echo(f"Error: {e}", err=True)
        raise typer.Exit(1)

    config = load_config(main_repo)
    _open_ide(worktree_path, config)
    typer.echo(f"cd {worktree_path}")


@app.command(name="open")
def open_cmd(
    name: str = typer.Argument(help="Branch/worktree name"),
):
    """Open IDEs at an existing worktree."""
    main_repo = find_main_repo()
    if not main_repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    worktree_path = main_repo / ".lf" / "worktrees" / name
    if not worktree_path.exists():
        typer.echo(f"Error: Worktree '{name}' not found at {worktree_path}", err=True)
        raise typer.Exit(1)

    config = load_config(main_repo)
    _open_ide(worktree_path, config)
    typer.echo(f"cd {worktree_path}")


@app.command(name="list")
def list_cmd():
    """List all worktrees with status."""
    main_repo = find_main_repo()
    if not main_repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    worktrees = list_worktrees(main_repo)
    if not worktrees:
        typer.echo("No worktrees found")
        return

    for wt in worktrees:
        status = []
        if wt.is_dirty:
            status.append("dirty")
        if not wt.on_origin:
            status.append("local")

        status_str = f" ({', '.join(status)})" if status else ""
        typer.echo(f"{wt.name}{status_str}")


@app.command()
def clean(
    force: bool = typer.Option(False, "-f", "--force", help="Skip confirmation"),
):
    """Remove worktrees for branches no longer on origin."""
    main_repo = find_main_repo()
    if not main_repo:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # Prune stale remote-tracking branches
    subprocess.run(["git", "fetch", "--prune"], cwd=main_repo, capture_output=True)

    worktrees = list_worktrees(main_repo)
    to_remove = [wt for wt in worktrees if not wt.on_origin and not wt.is_dirty]

    if not to_remove:
        typer.echo("No worktrees to clean")
        return

    typer.echo("Worktrees to remove:")
    for wt in to_remove:
        typer.echo(f"  {wt.name}")

    if not force:
        confirm = typer.confirm("Remove these worktrees?")
        if not confirm:
            raise typer.Exit(0)

    for wt in to_remove:
        if remove_worktree(main_repo, wt.name):
            typer.echo(f"Removed {wt.name}")
        else:
            typer.echo(f"Failed to remove {wt.name}", err=True)
