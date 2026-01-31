"""Sync command for updating local main to origin/main."""

import typer

from loopflow.lf.git import find_main_repo, get_current_branch
from loopflow.lf.ops._helpers import get_default_branch, is_repo_clean, sync_main_repo


def register_commands(app: typer.Typer) -> None:
    @app.command()
    def sync() -> None:
        """Fetch origin and update local main to match origin/main."""
        repo_root = find_main_repo()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        base_branch = get_default_branch(repo_root)
        current_branch = get_current_branch(repo_root)
        is_clean = is_repo_clean(repo_root)

        typer.echo(f"Fetching origin/{base_branch}...")
        success = sync_main_repo(repo_root, base_branch)

        if success:
            typer.echo(f"Updated {base_branch} to origin/{base_branch}")
        else:
            if current_branch == base_branch and not is_clean:
                typer.echo(
                    f"Refusing to reset {base_branch} with uncommitted changes",
                    err=True,
                )
            else:
                typer.echo(f"Failed to update {base_branch}", err=True)
            raise typer.Exit(1)
