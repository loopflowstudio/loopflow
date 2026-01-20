"""Shared helpers for lfops commands."""

import subprocess
from pathlib import Path

import typer

from loopflow.lf.git import ensure_draft_pr
from loopflow.lf.messages import generate_commit_message


def add_commit_push(repo_root: Path, push: bool = True) -> bool:
    """Add, commit (with generated message), and optionally push. Returns True if committed."""
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        if push:
            typer.echo("Pushing...")
            subprocess.run(["git", "push"], cwd=repo_root, check=True)
            _maybe_create_draft_pr(repo_root)
        return False

    typer.echo("Staging changes...")
    subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)

    typer.echo("Generating commit message...")
    message = generate_commit_message(repo_root)
    commit_msg = message.title
    if message.body:
        commit_msg += f"\n\n{message.body}"

    typer.echo(f"Committing: {message.title}")
    subprocess.run(["git", "commit", "-m", commit_msg], cwd=repo_root, check=True)

    if push:
        typer.echo("Pushing...")
        subprocess.run(["git", "push"], cwd=repo_root, check=True)
        _maybe_create_draft_pr(repo_root)

    return True


def _maybe_create_draft_pr(repo_root: Path) -> None:
    """Create draft PR after push if none exists. Silent on failure."""
    url = ensure_draft_pr(repo_root)
    if url:
        typer.echo(f"Created draft PR: {url}")


def get_default_branch(repo_root: Path) -> str:
    result = subprocess.run(
        ["git", "symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip().split("/", 1)[-1]
    return "main"


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
    """Update local base_branch to match origin."""
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "HEAD"],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    current_branch = result.stdout.strip() if result.returncode == 0 else ""

    if current_branch == base_branch:
        # Branch is checked out: fetch + reset to origin (fast-forward)
        subprocess.run(["git", "fetch", "origin", base_branch], cwd=main_repo, check=False)
        result = subprocess.run(
            ["git", "reset", "--hard", f"origin/{base_branch}"],
            cwd=main_repo,
            capture_output=True,
        )
        return result.returncode == 0
    else:
        # Branch not checked out: update ref directly
        result = subprocess.run(
            ["git", "fetch", "origin", f"{base_branch}:{base_branch}"],
            cwd=main_repo,
            capture_output=True,
        )
        return result.returncode == 0


def remove_worktree(main_repo: Path, branch: str, worktree_path: Path, base_branch: str = "main") -> None:
    """Remove worktree and branch. Uses wt for events, falls back to git if needed."""
    # Update local base branch to match origin so wt correctly detects squash-merged branches
    sync_main_repo(main_repo, base_branch)

    # Try wt first (emits events for Maestro)
    result = subprocess.run(
        ["wt", "-C", str(main_repo), "remove", branch],
        cwd=main_repo,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return

    # wt failed - fall back to git directly (handles "main already used" errors)
    subprocess.run(
        ["git", "worktree", "remove", "--force", str(worktree_path)],
        cwd=main_repo,
        capture_output=True,
    )
    subprocess.run(
        ["git", "branch", "-D", branch],
        cwd=main_repo,
        capture_output=True,
    )
