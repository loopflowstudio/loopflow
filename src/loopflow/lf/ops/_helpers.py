"""Shared helpers for lfops commands."""

import shutil
import subprocess
from pathlib import Path

import typer

from loopflow.lf.config import Config, load_config
from loopflow.lf.git import ensure_draft_pr
from loopflow.lf.messages import generate_commit_message
from loopflow.lf.ops.git import GitError
from loopflow.lf.ops.git import commit as git_commit
from loopflow.lf.ops.git import delete_local_branch as git_delete_local_branch
from loopflow.lf.ops.git import get_default_branch as git_get_default_branch
from loopflow.lf.ops.git import is_clean as git_is_clean
from loopflow.lf.ops.git import push as git_push
from loopflow.lf.ops.git import stage_all as git_stage_all
from loopflow.lf.ops.git import sync_main as git_sync_main
from loopflow.lf.ops.git import worktree_remove as git_worktree_remove


def _check_lint(repo_root: Path, config: Config | None) -> bool | None:
    """Run lint check. Returns True if passes, False if fails, None if can't check."""
    # Try user-configured command first
    if config and config.lint_check:
        result = subprocess.run(
            config.lint_check,
            shell=True,
            cwd=repo_root,
            capture_output=True,
        )
        return result.returncode == 0

    # Fall back to auto-detect ruff
    if shutil.which("ruff") is None:
        return None

    targets = []
    if (repo_root / "src").is_dir():
        targets.append("src/")
    if (repo_root / "tests").is_dir():
        targets.append("tests/")
    if not targets:
        return None

    check = subprocess.run(["ruff", "check", *targets], cwd=repo_root, capture_output=True)
    if check.returncode != 0:
        return False

    fmt = subprocess.run(
        ["ruff", "format", "--check", *targets], cwd=repo_root, capture_output=True
    )
    return fmt.returncode == 0


def run_lint(repo_root: Path) -> bool:
    """Check lint first; invoke agent only if checks fail."""
    config = load_config(repo_root)
    result = _check_lint(repo_root, config)

    if result is True:
        typer.echo("Lint passed")
        return True

    if result is False:
        typer.echo("Lint issues found, running fixer...")
    else:
        typer.echo("Running lint...")

    agent_result = subprocess.run(["lf", "lint", "-a"], cwd=repo_root)
    return agent_result.returncode == 0


def add_commit_push(repo_root: Path, push: bool = True) -> bool:
    """Add, commit (with generated message), and optionally push. Returns True if committed."""
    if git_is_clean(repo_root):
        if push:
            _push(repo_root)
            _maybe_create_draft_pr(repo_root)
        return False

    typer.echo("Staging changes...")
    git_stage_all(repo_root)

    typer.echo("Generating commit message...")
    message = generate_commit_message(repo_root)
    commit_msg = message.title
    if message.body:
        commit_msg += f"\n\n{message.body}"

    typer.echo(f"Committing: {message.title}")
    git_commit(repo_root, commit_msg)

    if push:
        _push(repo_root)
        _maybe_create_draft_pr(repo_root)

    return True


def _push(repo_root: Path) -> None:
    """Push current branch, using --force-with-lease if needed (e.g., after rebase)."""
    typer.echo("Pushing...")
    try:
        git_push(repo_root)
    except GitError:
        # Non-fast-forward - use force-with-lease (safe for feature branches after rebase)
        try:
            git_push(repo_root, force_with_lease=True)
        except GitError as e:
            typer.echo(f"Push failed: {e}", err=True)
            raise


def _maybe_create_draft_pr(repo_root: Path) -> None:
    """Create draft PR after push if none exists. Silent on failure."""
    url = ensure_draft_pr(repo_root)
    if url:
        typer.echo(f"Created draft PR: {url}")


def get_default_branch(repo_root: Path) -> str:
    try:
        return git_get_default_branch(repo_root)
    except GitError:
        return "main"


def is_repo_clean(repo_root: Path) -> bool:
    try:
        return git_is_clean(repo_root)
    except GitError:
        return False


def resolve_base_ref(repo_root: Path, base_branch: str) -> str:
    origin_ref = f"origin/{base_branch}"
    result = subprocess.run(
        ["git", "rev-parse", "--verify", origin_ref],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return origin_ref
    return base_branch


def get_diff(repo_root: Path, base_ref: str) -> str:
    result = subprocess.run(
        ["git", "diff", f"{base_ref}...HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.stdout if result.returncode == 0 else ""


def sync_main_repo(main_repo: Path, base_branch: str) -> bool:
    """Fetch origin/base_branch. Updates local ref if base_branch is checked out here."""
    try:
        synced = git_sync_main(main_repo, base_branch)
    except GitError:
        return False
    return synced


def remove_worktree(
    main_repo: Path, branch: str, worktree_path: Path, base_branch: str = "main"
) -> None:
    """Remove worktree and branch. Uses wt for events, falls back to git if needed."""
    # Update local base branch to match origin so wt correctly detects squash-merged branches
    sync_main_repo(main_repo, base_branch)

    # Try wt first (emits events for Concerto)
    result = subprocess.run(
        ["wt", "-C", str(main_repo), "remove", branch],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return

    # wt failed - fall back to git directly (handles "main already used" errors)
    try:
        git_worktree_remove(main_repo, worktree_path)
    except GitError:
        pass

    try:
        git_delete_local_branch(main_repo, branch)
    except GitError:
        pass
