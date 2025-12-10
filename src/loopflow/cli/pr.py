"""Pull request workflow commands."""

import shutil
import subprocess

import typer

from loopflow.context import find_repo_root
from loopflow.git import open_pr

app = typer.Typer(help="Pull request workflow.")


@app.command()
def create():
    """Create a GitHub PR for this branch."""
    repo_root = find_repo_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    if not shutil.which("gh"):
        typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
        raise typer.Exit(1)

    pr_url, error = open_pr(repo_root, draft=False)
    if pr_url:
        typer.echo(pr_url)
        subprocess.run(["open", pr_url])
    else:
        typer.echo(f"Error: {error}", err=True)
        raise typer.Exit(1)


@app.command()
def land(
    message: str = typer.Option(None, "-m", "--message", help="Commit message"),
):
    """Land this branch: squash-merge to main and clean up."""
    repo_root = find_repo_root()
    if not repo_root:
        typer.echo("Error: Not in a git repository", err=True)
        raise typer.Exit(1)

    # Get current branch
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()

    if not branch or branch == "main":
        typer.echo("Error: Already on main (or detached HEAD)", err=True)
        raise typer.Exit(1)

    # Check for uncommitted changes
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        typer.echo("Error: Uncommitted changes. Commit or stash first.", err=True)
        raise typer.Exit(1)

    # Get commit message from -m flag or .lf/COMMIT file
    commit_file = repo_root / ".lf" / "COMMIT"
    if message:
        commit_msg = message
    elif commit_file.exists():
        commit_msg = commit_file.read_text().strip()
        if not commit_msg:
            typer.echo("Error: .lf/COMMIT is empty", err=True)
            raise typer.Exit(1)
    else:
        typer.echo("Error: No commit message. Use -m or create .lf/COMMIT", err=True)
        raise typer.Exit(1)

    # Remove COMMIT file before merge so it doesn't end up in main
    if commit_file.exists():
        commit_file.unlink()
        subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)
        subprocess.run(
            ["git", "commit", "-m", "remove .lf/COMMIT before land"],
            cwd=repo_root,
            check=True,
        )

    # Land it
    subprocess.run(["git", "checkout", "main"], cwd=repo_root, check=True)
    subprocess.run(["git", "merge", "--squash", branch], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", commit_msg], cwd=repo_root, check=True)
    subprocess.run(["git", "branch", "-D", branch], cwd=repo_root, check=True)
    subprocess.run(["git", "push"], cwd=repo_root, check=True)

    typer.echo(f"Landed {branch} to main and pushed.")
