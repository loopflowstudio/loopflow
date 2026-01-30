"""Next command: land current PR, move worktree to new stacked branch."""

import subprocess
import time
from pathlib import Path

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lf.git import find_main_repo, get_current_branch
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.naming import generate_next_branch, parse_branch_base
from loopflow.lfd.wave import get_wave_by_worktree, update_wave_worktree_branch
from loopflow.lfops._helpers import get_default_branch
from loopflow.lfops.shell import write_directive


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


def _is_branch_merged(repo_root: Path, branch: str) -> bool:
    """Check if branch is fully merged into origin/main."""
    subprocess.run(
        ["git", "fetch", "origin", "main"],
        cwd=repo_root,
        capture_output=True,
    )
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", branch, "origin/main"],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def _fresh_start(repo_root: Path, wave_name: str) -> str | None:
    """Reset to origin/main and create fresh branch. Returns new branch name."""
    subprocess.run(
        ["git", "fetch", "origin", "main"],
        cwd=repo_root,
        capture_output=True,
    )
    result = subprocess.run(
        ["git", "checkout", "origin/main"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    main_repo = find_main_repo(repo_root) or repo_root
    new_branch = generate_next_branch(wave_name, main_repo)

    result = subprocess.run(
        ["git", "checkout", "-b", new_branch],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    subprocess.run(
        ["git", "push", "-u", "origin", new_branch],
        cwd=repo_root,
        capture_output=True,
    )
    return new_branch


def _enable_auto_merge(repo_root: Path, pr_number: int) -> bool:
    """Enable auto-merge on a PR. Returns True if successful.

    Regenerates the PR title/body to reflect latest changes before merging.
    """
    # Regenerate PR message to reflect latest changes
    typer.echo("Refreshing PR...")
    message = generate_pr_message(repo_root)
    title = message.title
    body = message.body

    # Update the PR
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


def _open_terminal(path: Path) -> None:
    """Open terminal at path (Warp)."""
    subprocess.run(["open", f"warp://action/new_window?path={path}"])


def move_worktree(
    worktree_path: Path,
    new_branch: str,
) -> bool:
    """Create new branch from current HEAD and switch to it (true stacking).

    The new branch is based on the current branch's HEAD, not on main.
    Returns True if successful.
    """
    # Create new branch from current HEAD
    result = subprocess.run(
        ["git", "checkout", "-b", new_branch],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False

    # Push to create remote branch with tracking
    subprocess.run(
        ["git", "push", "-u", "origin", new_branch],
        cwd=worktree_path,
        capture_output=True,
    )

    return True


def next_worktree(
    repo_root: Path,
    branch: str,
    block: bool = False,
    open_terminal: bool = True,
    create_pr: bool = False,
) -> Path | None:
    """Move to next branch iteration, handling both open PRs and merged branches.

    If branch has an open PR: enables auto-merge, creates stacked branch from HEAD.
    If branch is already merged: creates fresh branch from origin/main.

    Reuses the same worktree directory, just switches to a new branch.
    Returns path to worktree, or None if failed.
    """
    main_repo = find_main_repo(repo_root) or repo_root
    base_branch = get_default_branch(main_repo)

    # Check we're not on main
    if branch in (base_branch, "main", "master"):
        typer.echo(f"Error: Cannot run next from {branch}", err=True)
        return None

    # Check PR state to determine if already merged
    pr_number = _get_pr_number(repo_root)
    pr_state = _get_pr_state(repo_root, pr_number) if pr_number else None

    already_merged = False
    if pr_state == "MERGED":
        already_merged = True
    elif pr_number is None:
        # No PR - check if branch commits are merged into main
        already_merged = _is_branch_merged(repo_root, branch)
        if not already_merged:
            if create_pr:
                # Run lf ops pr to create PR
                typer.echo("Creating PR...")
                result = subprocess.run(["lf", "ops", "pr"], cwd=repo_root)
                if result.returncode != 0:
                    typer.echo("Error: Failed to create PR", err=True)
                    return None
                pr_number = _get_pr_number(repo_root)
                if pr_number is None:
                    typer.echo("Error: Could not find PR after creation", err=True)
                    return None
            else:
                typer.echo(
                    "Error: No open PR found. Run 'lf ops pr' first, or use --create-pr.",
                    err=True,
                )
                return None

    # Parse wave name from branch for new branch generation
    wave_name = parse_branch_base(branch)

    if already_merged:
        # Fresh start from origin/main
        typer.echo("Branch already merged. Starting fresh from main...")
        new_branch = _fresh_start(repo_root, wave_name)
        if not new_branch:
            typer.echo("Error: Failed to create fresh branch", err=True)
            return None

        # Update wave if it exists
        wave = get_wave_by_worktree(repo_root)
        if wave:
            update_wave_worktree_branch(wave.id, repo_root, new_branch)
            typer.echo(f"Updated wave '{wave.name}' to branch {new_branch}")
    else:
        # Land PR then create stacked branch
        # Enable auto-merge
        typer.echo(f"Enabling auto-merge for PR #{pr_number}...")
        if not _enable_auto_merge(repo_root, pr_number):
            typer.echo("Warning: Could not enable auto-merge", err=True)

        # Wait for merge if blocking
        if block:
            _wait_for_merge(repo_root, pr_number)

        # Generate new branch name
        new_branch = generate_next_branch(wave_name, main_repo)

        # Create new stacked branch from current HEAD
        typer.echo(f"Creating stacked branch {new_branch}...")
        if not move_worktree(repo_root, new_branch):
            typer.echo("Error: Failed to create stacked branch", err=True)
            return None

    # Open terminal in worktree (same path, new branch)
    if open_terminal:
        typer.echo("Opening terminal...")
        _open_terminal(repo_root)

    # Write shell directive to cd to worktree
    write_directive(f"cd {repo_root}")

    return repo_root


def register_commands(app: typer.Typer) -> None:
    """Register next command on the app."""

    @app.command("next")
    def next_cmd(
        block: bool = typer.Option(False, "--block", help="Wait for merge before moving"),
        no_open: bool = typer.Option(False, "--no-open", help="Don't open terminal"),
        create_pr: bool = typer.Option(False, "-c", "--create-pr", help="Create PR if none exists"),
    ) -> None:
        """Move to next branch iteration.

        If current branch has an open PR: enables auto-merge, then creates a
        stacked branch from current HEAD.

        If current branch is already merged (PR merged or no PR but commits
        in main): creates a fresh branch from origin/main.

        The worktree directory stays the same, only the branch changes.

        Example:
            lf ops next                 # land PR or start fresh if merged
            lf ops next --block         # wait for merge, then move
            lf ops next --create-pr     # create PR if none exists, then next
        """
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        branch = get_current_branch(repo_root)
        if not branch:
            typer.echo("Error: Not on a branch (detached HEAD)", err=True)
            raise typer.Exit(1)

        result = next_worktree(
            repo_root,
            branch,
            block=block,
            open_terminal=not no_open,
            create_pr=create_pr,
        )

        if result is None:
            raise typer.Exit(1)

        typer.echo(str(result))
