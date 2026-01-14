"""Commit with generated message."""

import subprocess

import typer

from loopflow.context import find_worktree_root
from loopflow.git import has_upstream
from loopflow.llm_http import generate_commit_message


def commit(
    push: bool = typer.Option(False, "-p", "--push", help="Push after committing"),
    add: bool = typer.Option(
        True, "-a/-A", "--add/--no-add", help="Stage all changes before committing"
    ),
) -> None:
    """Generate commit message from diff and commit."""
    repo_root = find_worktree_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # Check for changes
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        typer.echo("Nothing to commit", err=True)
        raise typer.Exit(0)

    # Stage changes if requested
    if add:
        subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)

    # Check if there are staged changes
    staged = subprocess.run(
        ["git", "diff", "--cached", "--quiet"],
        cwd=repo_root,
    )
    if staged.returncode == 0:
        typer.echo("Nothing staged to commit", err=True)
        raise typer.Exit(0)

    # Generate commit message
    typer.echo("Generating commit message...")
    try:
        message = generate_commit_message(repo_root)
    except Exception as e:
        typer.echo(f"Error generating commit message: {e}", err=True)
        raise typer.Exit(1)

    # Build full message
    commit_msg = message.title
    if message.body:
        commit_msg += f"\n\n{message.body}"

    # Commit
    result = subprocess.run(
        ["git", "commit", "-m", commit_msg],
        cwd=repo_root,
    )
    if result.returncode != 0:
        typer.echo("Commit failed", err=True)
        raise typer.Exit(1)

    typer.echo(f"Committed: {message.title}")

    # Push if requested
    if push:
        if has_upstream(repo_root):
            result = subprocess.run(["git", "push"], cwd=repo_root)
            if result.returncode == 0:
                typer.echo("Pushed to origin")
            else:
                typer.echo("Push failed", err=True)
                raise typer.Exit(1)
        else:
            typer.echo("No upstream branch, skipping push", err=True)
