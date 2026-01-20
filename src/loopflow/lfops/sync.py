"""Sync command for updating local main to origin/main."""

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lfops._helpers import get_default_branch, sync_main_repo


def register_commands(app: typer.Typer) -> None:
    @app.command()
    def sync() -> None:
        """Fetch origin and update local main to match origin/main."""
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        base_branch = get_default_branch(repo_root)

        typer.echo(f"Fetching origin/{base_branch}...")
        success = sync_main_repo(repo_root, base_branch)

        if success:
            typer.echo(f"Updated {base_branch} to origin/{base_branch}")
        else:
            typer.echo(f"Failed to update {base_branch}", err=True)
            raise typer.Exit(1)
