"""Cycle command: land current PR, create fresh worktree in same space."""

import json
import subprocess
import time
from pathlib import Path

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lf.git import find_main_repo, get_current_branch
from loopflow.lf.naming import generate_cycle_branch, parse_branch_for_cycle
from loopflow.lf.worktrees import create_with_schema, get_path
from loopflow.lfops._helpers import get_default_branch, remove_worktree
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


def _enable_auto_merge(repo_root: Path, pr_number: int) -> bool:
    """Enable auto-merge on a PR. Returns True if successful."""
    # Get PR title for squash commit message
    result = subprocess.run(
        ["gh", "pr", "view", str(pr_number), "--json", "title,body"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False

    pr_data = json.loads(result.stdout)
    title = pr_data.get("title", "")
    body = pr_data.get("body", "")

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


def cycle(
    repo_root: Path,
    branch: str,
    wait: bool = True,
    open_terminal: bool = True,
    create_pr: bool = False,
) -> Path | None:
    """Land current branch, create new worktree with magical-musical suffix.

    Returns path to new worktree, or None if cycle failed.
    """
    main_repo = find_main_repo(repo_root) or repo_root
    base_branch = get_default_branch(main_repo)

    # Check we're not on main
    if branch in (base_branch, "main", "master"):
        typer.echo(f"Error: Cannot cycle from {branch}", err=True)
        return None

    # Get or create PR
    pr_number = _get_pr_number(repo_root)
    if pr_number is None:
        if create_pr:
            # Run lfops pr to create PR
            typer.echo("Creating PR...")
            result = subprocess.run(["lfops", "pr"], cwd=repo_root)
            if result.returncode != 0:
                typer.echo("Error: Failed to create PR", err=True)
                return None
            pr_number = _get_pr_number(repo_root)
            if pr_number is None:
                typer.echo("Error: Could not find PR after creation", err=True)
                return None
        else:
            typer.echo("Error: No open PR found. Run 'lfops pr' first, or use --create-pr.", err=True)
            return None

    # Enable auto-merge
    typer.echo(f"Enabling auto-merge for PR #{pr_number}...")
    if not _enable_auto_merge(repo_root, pr_number):
        typer.echo("Warning: Could not enable auto-merge", err=True)

    # Wait for merge
    merged = False
    if wait:
        merged = _wait_for_merge(repo_root, pr_number)

    # Generate new branch name
    base_name = parse_branch_for_cycle(branch)
    new_branch = generate_cycle_branch(base_name, main_repo)

    # Create new worktree
    typer.echo(f"Creating worktree {new_branch}...")

    # Use the short name (just the magical-musical suffix) for worktree directory
    # Extract just the suffix part after the base name
    suffix = new_branch[len(base_name) + 1 :]  # +1 for the hyphen
    worktree_short_name = f"{base_name.split('.')[-1]}-{suffix}"

    # Create worktree with git directly since we need specific branch name
    worktree_path = get_path(main_repo, worktree_short_name)
    result = subprocess.run(
        ["git", "worktree", "add", "-b", new_branch, str(worktree_path), f"origin/{base_branch}"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo(f"Error creating worktree: {result.stderr}", err=True)
        return None

    # Push to create remote branch with tracking
    subprocess.run(
        ["git", "push", "-u", "origin", new_branch],
        cwd=worktree_path,
        capture_output=True,
    )

    # Remove old worktree if merge completed
    if merged and repo_root != main_repo:
        typer.echo(f"Removing worktree {branch}...")
        try:
            remove_worktree(main_repo, branch, repo_root, base_branch)
        except Exception:
            typer.echo("Warning: Could not remove old worktree. Run 'lfops wt prune' later.", err=True)

    # Open terminal in new worktree
    if open_terminal:
        typer.echo("Opening terminal...")
        _open_terminal(worktree_path)

    # Write shell directive to cd to new worktree
    write_directive(f"cd {worktree_path}")

    return worktree_path


def register_commands(app: typer.Typer) -> None:
    """Register cycle command on the app."""

    @app.command("cycle")
    def cycle_cmd(
        no_wait: bool = typer.Option(
            False, "--no-wait", help="Submit to merge queue without waiting"
        ),
        no_open: bool = typer.Option(False, "--no-open", help="Don't open terminal"),
        create_pr: bool = typer.Option(
            False, "-c", "--create-pr", help="Create PR if none exists"
        ),
    ) -> None:
        """Land current PR, create fresh worktree in same space.

        Enables auto-merge on the PR, waits for merge, removes the old worktree,
        and creates a new one with a magical-musical suffix (e.g., aurora-melody).

        Example:
            lfops cycle                # land PR, create next worktree
            lfops cycle --no-wait      # submit to merge queue, don't wait
            lfops cycle --create-pr    # create PR if none exists, then cycle
        """
        repo_root = find_worktree_root()
        if not repo_root:
            typer.echo("Error: Not in a git repository", err=True)
            raise typer.Exit(1)

        branch = get_current_branch(repo_root)
        if not branch:
            typer.echo("Error: Not on a branch (detached HEAD)", err=True)
            raise typer.Exit(1)

        result = cycle(
            repo_root,
            branch,
            wait=not no_wait,
            open_terminal=not no_open,
            create_pr=create_pr,
        )

        if result is None:
            raise typer.Exit(1)

        typer.echo(str(result))
