"""Rebase command for rebasing onto main."""

import subprocess

import typer

from loopflow.lf.context import find_worktree_root, gather_step


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

        # Attempt rebase
        typer.echo("Rebasing onto origin/main...")
        result = subprocess.run(
            ["git", "rebase", "origin/main"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )

        if result.returncode == 0:
            # Success - push
            typer.echo("Rebase succeeded, pushing...")
            push_result = subprocess.run(
                ["git", "push", "--force-with-lease"],
                cwd=repo_root,
            )
            if push_result.returncode == 0:
                typer.echo("Done")
                return
            typer.echo("Push failed", err=True)
            raise typer.Exit(1)

        # Conflicts - abort and hand off to assistant
        typer.echo("Conflicts detected, aborting rebase...")
        subprocess.run(["git", "rebase", "--abort"], cwd=repo_root)

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
