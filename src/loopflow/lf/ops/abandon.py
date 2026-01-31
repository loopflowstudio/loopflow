"""Abandon command for closing PRs and removing worktrees."""

import subprocess
from pathlib import Path

import typer

from loopflow.lf.git import find_main_repo
from loopflow.lf.ops._helpers import get_default_branch, remove_worktree


def _find_worktree_by_branch(main_repo: Path, branch: str) -> Path | None:
    """Find worktree path for a given branch name."""
    result = subprocess.run(
        ["git", "worktree", "list", "--porcelain"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    worktree_path = None
    for line in result.stdout.split("\n"):
        if line.startswith("worktree "):
            worktree_path = Path(line[9:])
        elif line.startswith("branch refs/heads/"):
            if line[18:] == branch:
                return worktree_path
    return None


def _is_worktree_clean(worktree_path: Path) -> bool:
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0 and not result.stdout.strip()


def _close_pr(repo_root: Path, branch: str) -> bool:
    """Close PR for branch if one exists. Returns True if closed."""
    result = subprocess.run(
        ["gh", "pr", "close", branch],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def _delete_remote_branch(repo_root: Path, branch: str) -> bool:
    """Delete remote branch if it exists. Returns True if deleted."""
    result = subprocess.run(
        ["git", "push", "origin", "--delete", branch],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def register_commands(app: typer.Typer) -> None:
    """Register abandon command on the app."""

    @app.command()
    def abandon(
        branch: str = typer.Argument(..., help="Branch name to abandon"),
        force: bool = typer.Option(
            False,
            "-f",
            "--force",
            help="Skip confirmation and force abandon with uncommitted changes",
        ),
    ) -> None:
        """Abandon a branch: close PR, remove worktree, delete branch."""
        main_repo = find_main_repo()
        if not main_repo:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        # Find worktree for this branch
        worktree_path = _find_worktree_by_branch(main_repo, branch)
        if not worktree_path:
            typer.echo(f"Error: No worktree found for branch '{branch}'", err=True)
            raise typer.Exit(1)

        if not worktree_path.exists():
            typer.echo(f"Error: Worktree path does not exist: {worktree_path}", err=True)
            raise typer.Exit(1)

        # Check for uncommitted changes
        if not _is_worktree_clean(worktree_path):
            if not force:
                typer.echo(
                    "Error: Worktree has uncommitted changes. Use --force to abandon anyway.",
                    err=True,
                )
                raise typer.Exit(1)
            typer.echo("Warning: Abandoning worktree with uncommitted changes")

        # Confirm
        if not force:
            typer.confirm(
                f"Abandon branch '{branch}'? "
                "This will close the PR, delete the remote branch, and remove the worktree.",
                abort=True,
            )

        # Close PR if exists
        typer.echo("Closing PR...")
        if _close_pr(main_repo, branch):
            typer.echo("PR closed")
        else:
            typer.echo("No open PR found (or already closed)")

        # Delete remote branch
        typer.echo("Deleting remote branch...")
        if _delete_remote_branch(main_repo, branch):
            typer.echo("Remote branch deleted")
        else:
            typer.echo("No remote branch found (or already deleted)")

        # Remove worktree and local branch
        typer.echo("Removing worktree...")
        base_branch = get_default_branch(main_repo)
        remove_worktree(main_repo, branch, worktree_path, base_branch)
        typer.echo("Worktree removed")

        typer.echo(f"Abandoned '{branch}'")
