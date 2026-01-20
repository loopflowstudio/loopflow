"""PR command for creating/updating GitHub pull requests."""

import shutil
import subprocess

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lf.git import GitError, open_pr
from loopflow.lf.messages import generate_pr_message
from loopflow.lfops._helpers import add_commit_push


def _get_existing_pr_url(repo_root) -> str | None:
    """Check if an open PR exists for current branch. Returns URL if exists, None otherwise."""
    result = subprocess.run(
        ["gh", "pr", "view", "--json", "url,state", "-q", 'select(.state == "OPEN") | .url'],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip()
    return None


def _has_unpushed_commits(repo_root) -> bool:
    """Check if the current branch has commits not yet pushed to remote."""
    result = subprocess.run(
        ["git", "rev-list", "--count", "@{u}..HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        # No upstream tracking branch - assume there are new commits
        return True
    count = int(result.stdout.strip()) if result.stdout.strip() else 0
    return count > 0


def _update_pr(repo_root, title: str, body: str) -> str:
    """Update existing PR title and body. Returns URL."""
    subprocess.run(
        ["git", "push"],
        cwd=repo_root,
        capture_output=True,
    )
    result = subprocess.run(
        ["gh", "pr", "edit", "--title", title, "--body", body],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "Failed to update PR")
    return _get_existing_pr_url(repo_root) or ""


def register_commands(app: typer.Typer) -> None:
    """Register PR command on the app."""

    @app.command("pr")
    def pr() -> None:
        """Create or update a GitHub PR, then open it in browser.

        Auto-commits any uncommitted changes before creating/updating the PR.
        """
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        if not shutil.which("gh"):
            typer.echo("Error: 'gh' CLI not found. Install with: brew install gh", err=True)
            raise typer.Exit(1)

        # Always auto-commit and push any pending changes
        add_commit_push(repo_root)

        # Check if PR already exists
        existing_url = _get_existing_pr_url(repo_root)

        if existing_url:
            # Skip regeneration if no new commits to push
            if not _has_unpushed_commits(repo_root):
                typer.echo("No new commits. Opening existing PR...")
                subprocess.run(["open", existing_url])
                return

            typer.echo("Updating existing PR...")
            message = generate_pr_message(repo_root)
            typer.echo(f"\n{message.title}\n")
            typer.echo(message.body)
            typer.echo("")
            try:
                pr_url = _update_pr(repo_root, title=message.title, body=message.body)
            except GitError as e:
                typer.echo(f"Error: {e}", err=True)
                raise typer.Exit(1)
            typer.echo(f"Updated: {pr_url}")
        else:
            typer.echo("Creating PR...")
            message = generate_pr_message(repo_root)
            typer.echo(f"\n{message.title}\n")
            typer.echo(message.body)
            typer.echo("")
            try:
                pr_url = open_pr(repo_root, title=message.title, body=message.body)
            except GitError as e:
                typer.echo(f"Error: {e}", err=True)
                raise typer.Exit(1)
            typer.echo(f"Created: {pr_url}")

        subprocess.run(["open", pr_url])
