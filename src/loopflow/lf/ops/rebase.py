"""Rebase command for rebasing onto main."""

import subprocess

import typer

from loopflow.lf.context import find_worktree_root, gather_step
from loopflow.lf.ops.git import GitError
from loopflow.lf.ops.git import push as git_push
from loopflow.lf.ops.git import rebase as git_rebase


def register_commands(app: typer.Typer) -> None:
    """Register rebase command on the app."""

    @app.command()
    def rebase() -> None:
        """Rebase onto main, or launch assistant if conflicts."""
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        # Fetch latest main
        typer.echo("Fetching origin/main...")
        subprocess.run(["git", "fetch", "origin", "main"], cwd=repo_root, check=False)

        # Attempt rebase via lf-engine
        typer.echo("Rebasing onto origin/main...")
        try:
            result = git_rebase(repo_root, "origin/main")
        except GitError as err:
            typer.echo(f"Rebase failed: {err}", err=True)
            raise typer.Exit(1)

        if result.success:
            typer.echo("Rebase succeeded, pushing...")
            try:
                git_push(repo_root, force_with_lease=True)
            except GitError as err:
                typer.echo(f"Push failed: {err}", err=True)
                raise typer.Exit(1)
            typer.echo("Done")
            return

        # Conflicts - hand off to assistant
        if result.conflicts:
            typer.echo("Conflicts detected, aborting rebase...")
            for path in result.conflicts:
                typer.echo(f"  {path}")

        # Get rebase prompt (custom or built-in)
        step = gather_step(repo_root, "rebase")
        if not step:
            typer.echo("Error: No rebase step found", err=True)
            raise typer.Exit(1)

        typer.echo("Launching rebase assistant...")
        rebase_result = subprocess.run(["lf", "rebase", "-a"], cwd=repo_root)
        if rebase_result.returncode != 0:
            typer.echo("Rebase assistant failed", err=True)
            raise typer.Exit(1)
