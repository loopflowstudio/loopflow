"""PR command for creating/updating GitHub pull requests."""

import shutil
import subprocess

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lf.git import (
    GitError,
    ensure_ready_pr,
    find_main_repo,
    get_current_branch,
    get_pr_target,
    is_draft_pr,
    open_pr,
)
from loopflow.lf.messages import generate_pr_message, generate_pr_message_from_diff
from loopflow.lf.ops.git import push as git_push
from loopflow.lf.ops.git import rebase as git_rebase
from loopflow.lf.worktrees import list_all
from loopflow.lf.ops._helpers import (
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


def _get_worktree_base_branch(repo_root) -> str | None:
    """Get base_branch for current worktree if it's stacked."""
    main_repo = find_main_repo(repo_root)
    if not main_repo:
        return None

    current_branch = get_current_branch(repo_root)
    if not current_branch:
        return None

    # Find worktree with this branch and get its base_branch
    try:
        worktrees = list_all(main_repo)
        for wt in worktrees:
            if wt.branch == current_branch:
                return wt.base_branch
    except Exception:
        pass

    return None


def _is_behind_main(repo_root) -> int:
    """Check how many commits branch is behind origin/main. Returns count."""
    result = subprocess.run(
        ["git", "rev-list", "--count", "HEAD..origin/main"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return 0
    return int(result.stdout.strip()) if result.stdout.strip() else 0


def _auto_rebase(repo_root) -> bool:
    """Rebase onto origin/main. Returns True if successful."""
    typer.echo("Rebasing onto origin/main...")
    result = git_rebase(repo_root, "origin/main")
    if not result.success:
        if result.conflicts:
            typer.echo("Rebase conflicts detected:", err=True)
            for path in result.conflicts:
                typer.echo(f"  {path}", err=True)
            typer.echo("Resolve conflicts and run 'lf ops pr' again.", err=True)
        return False

    # Force push after rebase
    try:
        git_push(repo_root, force_with_lease=True)
    except Exception as e:
        typer.echo(f"Push failed after rebase: {e}", err=True)
        return False

    return True


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

        # Auto-rebase if behind main
        behind_count = _is_behind_main(repo_root)
        if behind_count > 0:
            typer.echo(f"Branch is {behind_count} commits behind main. Rebasing...")
            if not _auto_rebase(repo_root):
                raise typer.Exit(1)

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

            # Determine PR target for stacked branches
            base_branch = _get_worktree_base_branch(repo_root)
            pr_base = get_pr_target(base_branch)
            if pr_base != "main":
                typer.echo(f"Targeting base branch: {pr_base}")

            try:
                pr_url = open_pr(repo_root, title=message.title, body=message.body, base=pr_base)
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
