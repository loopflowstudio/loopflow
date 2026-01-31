"""Next command: land current PR, move worktree to new stacked branch."""

import subprocess
import time
from pathlib import Path

import typer

from loopflow.lf.context import find_worktree_root
from loopflow.lf.git import find_main_repo, get_current_branch
from loopflow.lf.messages import generate_pr_message
from loopflow.lf.naming import extract_iteration_suffix, generate_next_branch, parse_branch_base
from loopflow.lf.ops.git import GitError
from loopflow.lf.ops.git import create_branch as git_create_branch
from loopflow.lf.ops.git import push as git_push
from loopflow.lf.ops.git import rebase as git_rebase
from loopflow.lfd.wave import get_wave_by_worktree, update_wave_worktree_branch
from loopflow.lf.ops._helpers import add_commit_push, get_default_branch
from loopflow.lf.ops.shell import write_directive


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


def _fetch_main(repo_root: Path) -> None:
    """Fetch origin/main."""
    subprocess.run(["git", "fetch", "origin", "main"], cwd=repo_root, capture_output=True)


def _is_branch_merged(repo_root: Path, branch: str) -> bool:
    """Check if branch is fully merged into origin/main. Assumes origin/main is fetched."""
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", branch, "origin/main"],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def _fresh_start(repo_root: Path, wave_name: str) -> str | None:
    """Reset to origin/main and create fresh branch. Returns new branch name."""
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

    try:
        git_create_branch(repo_root, new_branch)
    except GitError:
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


def _preserve_worktree(repo_root: Path, branch: str, wave_name: str) -> Path | None:
    """Move current worktree to preserve it. Returns new path or None if failed."""
    suffix = extract_iteration_suffix(branch)
    if not suffix:
        # No iteration suffix - use branch name directly
        suffix = branch

    main_repo = find_main_repo(repo_root) or repo_root
    new_path = main_repo.parent / f"{main_repo.name}.{wave_name}.{suffix}"

    if new_path.exists():
        typer.echo(f"Error: Cannot preserve worktree, path exists: {new_path}", err=True)
        return None

    # git worktree move <worktree> <new-path>
    result = subprocess.run(
        ["git", "worktree", "move", str(repo_root), str(new_path)],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo(f"Error: Failed to move worktree: {result.stderr}", err=True)
        return None

    return new_path


def _create_fresh_worktree(
    main_repo: Path,
    wave_name: str,
    new_branch: str,
    base_ref: str = "origin/main",
) -> Path | None:
    """Create new worktree at wave_name path with new branch."""
    worktree_path = main_repo.parent / f"{main_repo.name}.{wave_name}"

    result = subprocess.run(
        ["git", "worktree", "add", "-b", new_branch, str(worktree_path), base_ref],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        typer.echo(f"Error: Failed to create worktree: {result.stderr}", err=True)
        return None

    # Push to create remote branch
    subprocess.run(
        ["git", "push", "-u", "origin", new_branch],
        cwd=worktree_path,
        capture_output=True,
    )

    return worktree_path


def _rebase_onto_main(repo_root: Path, base_branch: str) -> bool:
    """Rebase current branch onto base_branch. Returns True if successful."""
    # Fetch latest
    subprocess.run(["git", "fetch", "origin", base_branch], cwd=repo_root, capture_output=True)

    # Check if rebase is needed
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

    # Force-push rebased branch
    typer.echo("Pushing rebased branch...")
    try:
        git_push(repo_root, force_with_lease=True)
    except GitError as e:
        typer.echo(f"Error: Push failed after rebase: {e}", err=True)
        return False

    return True


def next_worktree(
    repo_root: Path,
    branch: str,
    block: bool = False,
    open_terminal: bool = True,
    create_pr: bool = False,
    rebase: bool = True,
) -> Path | None:
    """Move to next branch iteration, handling both open PRs and merged branches.

    If branch has an open PR: enables auto-merge, creates stacked branch from HEAD.
    If branch is already merged: creates fresh branch from origin/main.

    Preserves the old worktree at a new path (e.g., repo.wave.timestamp.words)
    and creates a new worktree at the original path.
    Returns path to new worktree, or None if failed.
    """
    main_repo = find_main_repo(repo_root) or repo_root
    base_branch = get_default_branch(main_repo)

    # Check we're not on main
    if branch in (base_branch, "main", "master"):
        typer.echo(f"Error: Cannot run next from {branch}", err=True)
        return None

    # Rebase onto main to ensure we're working with latest code
    if rebase:
        if not _rebase_onto_main(repo_root, base_branch):
            return None

    # Check PR state to determine if already merged
    pr_number = _get_pr_number(repo_root)
    pr_state = _get_pr_state(repo_root, pr_number) if pr_number else None

    already_merged = False
    if pr_state == "MERGED":
        already_merged = True
    elif pr_number is None:
        # No PR - check if branch commits are merged into main
        _fetch_main(repo_root)
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

    # Preserve current worktree before making changes
    typer.echo("Preserving current worktree...")
    preserved = _preserve_worktree(repo_root, branch, wave_name)
    if not preserved:
        return None
    typer.echo(f"Old worktree preserved at {preserved}")

    # Look up wave before creating new worktree (it tracks the old path)
    wave = get_wave_by_worktree(preserved)

    if already_merged:
        # Fresh start from origin/main
        typer.echo("Creating fresh branch from main...")
        _fetch_main(main_repo)
        new_branch = generate_next_branch(wave_name, main_repo)
        new_worktree = _create_fresh_worktree(main_repo, wave_name, new_branch)
        if not new_worktree:
            typer.echo("Error: Failed to create fresh worktree", err=True)
            return None
    else:
        # Land PR then create stacked branch
        # Enable auto-merge on the old PR (now at preserved path)
        typer.echo(f"Enabling auto-merge for PR #{pr_number}...")
        if not _enable_auto_merge(preserved, pr_number):
            typer.echo("Warning: Could not enable auto-merge", err=True)

        # Wait for merge if blocking
        if block:
            _wait_for_merge(preserved, pr_number)

        # Get current HEAD from preserved worktree for stacking
        head_result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=preserved,
            capture_output=True,
            text=True,
        )
        head_sha = head_result.stdout.strip()

        # Generate new branch and create stacked worktree
        new_branch = generate_next_branch(wave_name, main_repo)
        typer.echo(f"Creating stacked branch {new_branch}...")
        new_worktree = _create_fresh_worktree(main_repo, wave_name, new_branch, head_sha)
        if not new_worktree:
            typer.echo("Error: Failed to create stacked worktree", err=True)
            return None

    # Update wave if it exists
    if wave:
        update_wave_worktree_branch(wave.id, new_worktree, new_branch)
        typer.echo(f"Updated wave '{wave.name}' to branch {new_branch}")

    # Open terminal in new worktree
    if open_terminal:
        typer.echo("Opening terminal...")
        _open_terminal(new_worktree)

    # Write shell directive to cd to new worktree
    write_directive(f"cd {new_worktree}")

    return new_worktree


def register_commands(app: typer.Typer) -> None:
    """Register next command on the app."""

    @app.command("next")
    def next_cmd(
        block: bool = typer.Option(False, "--block", help="Wait for merge before moving"),
        no_open: bool = typer.Option(False, "--no-open", help="Don't open terminal"),
        create_pr: bool = typer.Option(False, "-c", "--create-pr", help="Create PR if none exists"),
        rebase: bool = typer.Option(True, "--rebase/--no-rebase", help="Rebase onto main first"),
    ) -> None:
        """Move to next branch iteration, preserving the old worktree.

        Auto-commits any uncommitted changes, then rebases onto main (unless --no-rebase).

        If current branch has an open PR: enables auto-merge, then creates a
        stacked branch from current HEAD.

        If current branch is already merged (PR merged or no PR but commits
        in main): creates a fresh branch from origin/main.

        The old worktree is preserved at a new path (e.g., repo.wave.timestamp.words)
        so you can still access it for rebasing or other work. It will be cleaned
        up by `lf ops wt prune` when its PR merges.

        Example:
            lf ops next                 # land PR or start fresh if merged
            lf ops next --block         # wait for merge, then move
            lf ops next --create-pr     # create PR if none exists, then next
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

        result = next_worktree(
            repo_root,
            branch,
            block=block,
            open_terminal=not no_open,
            create_pr=create_pr,
            rebase=rebase,
        )

        if result is None:
            raise typer.Exit(1)

        typer.echo(str(result))
