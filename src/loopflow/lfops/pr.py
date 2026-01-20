"""PR command for creating/updating GitHub pull requests."""

import shutil
import subprocess

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lf.git import (
    GitError,
    ensure_ready_pr,
    find_main_repo,
    is_draft_pr,
    open_pr,
)
from loopflow.lf.messages import generate_pr_message, generate_pr_message_from_diff
from loopflow.lfops._helpers import (
    _push,
    add_commit_push,
    get_default_branch,
    run_lint,
    sync_main_repo,
)


def _get_existing_pr_url(repo_root) -> str | None:
    """Check if an open PR exists for current branch. Returns URL if exists, None otherwise."""
    result = subprocess.run(
        [
            "gh",
            "pr",
            "view",
            "--json",
            "url,state",
            "-q",
            'select(.state == "OPEN") | .url',
        ],
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


def _get_pr_diff(repo_root) -> str | None:
    """Fetch combined PR diff via gh for accuracy against the PR base."""
    result = subprocess.run(
        ["gh", "pr", "diff"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None
    return result.stdout


def _update_pr(repo_root, title: str, body: str) -> str:
    """Update existing PR title and body. Returns URL."""
    _push(repo_root)
    result = subprocess.run(
        ["gh", "pr", "edit", "--title", title, "--body", body],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "Failed to update PR")
    return _get_existing_pr_url(repo_root) or ""


def _sync_main_repo(repo_root) -> None:
    main_repo = find_main_repo(repo_root) or repo_root
    base_branch = get_default_branch(main_repo)
    if not sync_main_repo(main_repo, base_branch):
        typer.echo(
            f"Warning: failed to sync {base_branch}; diff may be stale",
            err=True,
        )


def register_commands(app: typer.Typer) -> None:
    """Register PR command on the app."""

    @app.command("pr")
    def pr(
        refresh: bool = typer.Option(
            False, "--refresh", "-r", help="Force regenerate PR title and body"
        ),
        lint: bool = typer.Option(True, "--lint/--no-lint", help="Run lint before PR"),
    ) -> None:
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

        if lint and not run_lint(repo_root):
            typer.echo("Lint failed, aborting PR", err=True)
            raise typer.Exit(1)

        _sync_main_repo(repo_root)

        # Always auto-commit and push any pending changes
        add_commit_push(repo_root)

        # Check if PR already exists
        existing_url = _get_existing_pr_url(repo_root)

        if existing_url:
            # Skip regeneration if no new commits unless refresh flag or draft PR.
            # Drafts are created with gh --fill, so refresh them with LLM output.
            if not refresh and not _has_unpushed_commits(repo_root) and not is_draft_pr(repo_root):
                typer.echo("No new commits. Opening existing PR...")
                subprocess.run(["open", existing_url])
                return

            typer.echo("Updating existing PR...")
            diff = _get_pr_diff(repo_root)
            if diff:
                message = generate_pr_message_from_diff(repo_root, diff)
            else:
                message = generate_pr_message(repo_root)
            typer.echo(f"\n{message.title}\n")
            typer.echo(message.body)
            typer.echo("")
            try:
                pr_url = _update_pr(repo_root, title=message.title, body=message.body)
            except GitError as e:
                typer.echo(f"Error: {e}", err=True)
                raise typer.Exit(1)
            if is_draft_pr(repo_root):
                if not ensure_ready_pr(repo_root):
                    typer.echo("Error: Failed to mark PR as ready", err=True)
                    raise typer.Exit(1)
                typer.echo("Marked PR as ready for review")
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
            if is_draft_pr(repo_root):
                if not ensure_ready_pr(repo_root):
                    typer.echo("Error: Failed to mark PR as ready", err=True)
                    raise typer.Exit(1)
                typer.echo("Marked PR as ready for review")
            typer.echo(f"Created: {pr_url}")

        subprocess.run(["open", pr_url])
