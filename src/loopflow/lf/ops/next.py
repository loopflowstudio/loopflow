"""Next command: create new branch iteration for current wave."""

import subprocess
import time
from pathlib import Path

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lf.git import find_main_repo, get_current_branch
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.naming import generate_branch_name
from loopflow.lf.ops._helpers import add_commit_push, get_default_branch
from loopflow.lf.ops.git import GitError
from loopflow.lf.ops.git import create_branch as git_create_branch
from loopflow.lf.ops.git import push as git_push
from loopflow.lf.ops.git import rebase as git_rebase
from loopflow.lf.ops.shell import write_directive
from loopflow.lfd.wave import get_wave_by_worktree, update_wave_worktree_branch


def _get_pr_number(repo_root: Path) -> int | None:
    """Get the PR number for the current branch."""
    result = subprocess.run(
        ["gh", "pr", "view", "--json", "number", "-q", ".number"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return int(result.stdout.strip())
    return None


def _get_pr_state(repo_root: Path, pr_number: int) -> str | None:
    """Get the state of a PR (OPEN, MERGED, CLOSED)."""
    result = subprocess.run(
        ["gh", "pr", "view", str(pr_number), "--json", "state", "-q", ".state"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip().upper()
    return None


def _enable_auto_merge(repo_root: Path, pr_number: int) -> bool:
    """Enable auto-merge on a PR. Returns True if successful."""
    typer.echo("Refreshing PR...")
    message = generate_pr_message(repo_root)
    title = message.title
    body = message.body

    subprocess.run(
        ["gh", "pr", "edit", str(pr_number), "--title", title, "--body", body],
        cwd=repo_root,
        capture_output=True,
    )

    merge_cmd = [
        "gh",
        "pr",
        "merge",
        str(pr_number),
        "--squash",
        "--auto",
        "--subject",
        title,
    ]
    if body:
        merge_cmd.extend(["--body", body])

    result = subprocess.run(merge_cmd, cwd=repo_root, capture_output=True, text=True)
    return result.returncode == 0


def _wait_for_merge(repo_root: Path, pr_number: int, timeout: int = 600) -> bool:
    """Wait for PR to merge. Returns True if merged, False if timeout or closed."""
    start = time.time()
    typer.echo(f"Waiting for PR #{pr_number} to merge... (Ctrl+C to continue without waiting)")

    try:
        while time.time() - start < timeout:
            state = _get_pr_state(repo_root, pr_number)
            if state == "MERGED":
                typer.echo("done")
                return True
            if state == "CLOSED":
                typer.echo("PR was closed without merging", err=True)
                return False
            time.sleep(5)
    except KeyboardInterrupt:
        typer.echo("\nContinuing without waiting...")
        return False

    typer.echo("Timeout waiting for merge", err=True)
    return False


def _rebase_onto_main(repo_root: Path, base_branch: str) -> bool:
    """Rebase current branch onto base_branch. Returns True if successful."""
    subprocess.run(["git", "fetch", "origin", base_branch], cwd=repo_root, capture_output=True)

    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", f"origin/{base_branch}", "HEAD"],
        cwd=repo_root,
        capture_output=True,
    )
    if result.returncode == 0:
        return True  # Already up-to-date

    typer.echo(f"Rebasing onto {base_branch}...")
    try:
        rebase_result = git_rebase(repo_root, f"origin/{base_branch}")
    except GitError as e:
        typer.echo(f"Error: Rebase failed: {e}", err=True)
        return False

    if not rebase_result.success:
        typer.echo("Rebase had conflicts. Resolve manually or run 'lf ops rebase'.", err=True)
        return False

    typer.echo("Pushing rebased branch...")
    try:
        git_push(repo_root, force_with_lease=True)
    except GitError as e:
        typer.echo(f"Error: Push failed after rebase: {e}", err=True)
        return False

    return True


def next_iteration(
    repo_root: Path,
    block: bool = False,
    create_pr: bool = False,
    rebase: bool = True,
) -> str | None:
    """Create new branch iteration for current wave.

    1. Get wave from worktree (source of truth for wave name)
    2. Handle current PR (enable auto-merge if open)
    3. Generate new branch name from wave.name
    4. Create new branch at HEAD (stacked on current work)
    5. Update wave metadata

    Returns new branch name, or None if failed.
    """
    # Get wave - this is the source of truth
    wave = get_wave_by_worktree(repo_root)
    if not wave:
        typer.echo("Error: No wave found for this worktree", err=True)
        return None

    main_repo = find_main_repo(repo_root) or repo_root
    base_branch = get_default_branch(main_repo)
    current_branch = get_current_branch(repo_root)

    if current_branch in (base_branch, "main", "master"):
        typer.echo(f"Error: Cannot run next from {current_branch}", err=True)
        return None

    # Rebase onto main
    if rebase:
        if not _rebase_onto_main(repo_root, base_branch):
            return None

    # Handle PR
    pr_number = _get_pr_number(repo_root)
    if pr_number:
        pr_state = _get_pr_state(repo_root, pr_number)
        if pr_state == "OPEN":
            typer.echo(f"Enabling auto-merge for PR #{pr_number}...")
            if not _enable_auto_merge(repo_root, pr_number):
                typer.echo("Warning: Could not enable auto-merge", err=True)
            if block:
                _wait_for_merge(repo_root, pr_number)
    elif create_pr:
        typer.echo("Creating PR...")
        result = subprocess.run(["lf", "ops", "pr"], cwd=repo_root)
        if result.returncode != 0:
            typer.echo("Error: Failed to create PR", err=True)
            return None
        pr_number = _get_pr_number(repo_root)
        if pr_number:
            typer.echo(f"Enabling auto-merge for PR #{pr_number}...")
            _enable_auto_merge(repo_root, pr_number)

    # Generate new branch from wave name (source of truth)
    new_branch = generate_branch_name(wave.name, main_repo)
    typer.echo(f"Creating branch {new_branch}...")

    # Create new branch at HEAD
    try:
        git_create_branch(repo_root, new_branch)
    except GitError as e:
        typer.echo(f"Error: Failed to create branch: {e}", err=True)
        return None

    # Push new branch
    subprocess.run(
        ["git", "push", "-u", "origin", new_branch],
        cwd=repo_root,
        capture_output=True,
    )

    # Update wave metadata
    update_wave_worktree_branch(wave.id, repo_root, new_branch)
    typer.echo(f"Updated wave '{wave.name}' to branch {new_branch}")

    return new_branch


def register_commands(app: typer.Typer) -> None:
    """Register next command on the app."""

    @app.command("next")
    def next_cmd(
        block: bool = typer.Option(False, "--block", help="Wait for merge before continuing"),
        create_pr: bool = typer.Option(False, "-c", "--create-pr", help="Create PR if none exists"),
        rebase: bool = typer.Option(True, "--rebase/--no-rebase", help="Rebase onto main first"),
    ) -> None:
        """Create new branch iteration for current wave.

        Auto-commits any uncommitted changes, rebases onto main (unless --no-rebase),
        then creates a new branch stacked on current HEAD.

        If current branch has an open PR, enables auto-merge before moving on.

        Example:
            lf ops next                 # create next iteration
            lf ops next --block         # wait for PR merge, then continue
            lf ops next --create-pr     # create PR if none exists
            lf ops next --no-rebase     # skip rebasing onto main
        """
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        branch = get_current_branch(repo_root)
        if not branch:
            typer.echo("Error: Not on a branch (detached HEAD)", err=True)
            raise typer.Exit(1)

        # Handle uncommitted changes
        add_commit_push(repo_root, push=True)

        result = next_iteration(
            repo_root,
            block=block,
            create_pr=create_pr,
            rebase=rebase,
        )

        if result is None:
            raise typer.Exit(1)

        typer.echo(result)
