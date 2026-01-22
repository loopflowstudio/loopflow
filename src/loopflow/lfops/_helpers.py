"""Shared helpers for lfops commands."""

import shutil
import subprocess
from pathlib import Path

import typer

from loopflow.lf.config import Config, load_config
from loopflow.lf.git import ensure_draft_pr, get_current_branch
from loopflow.lf.messages import generate_commit_message


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
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        if push:
            _push(repo_root)
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
        _push(repo_root)
        _maybe_create_draft_pr(repo_root)

    return True


def _push(repo_root: Path) -> None:
    """Push current branch, using --force-with-lease if needed (e.g., after rebase)."""
    typer.echo("Pushing...")
    result = subprocess.run(["git", "push"], cwd=repo_root, capture_output=True)
    if result.returncode != 0:
        # Non-fast-forward - use force-with-lease (safe for feature branches after rebase)
        subprocess.run(["git", "push", "--force-with-lease"], cwd=repo_root, check=True)


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


def is_repo_clean(repo_root: Path) -> bool:
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0 and not result.stdout.strip()


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
    fetch_result = subprocess.run(
        ["git", "fetch", "origin", base_branch],
        cwd=main_repo,
        capture_output=True,
    )
    if fetch_result.returncode != 0:
        return False

    # If base_branch is checked out here, update it to match origin
    current_branch = get_current_branch(main_repo)
    if current_branch == base_branch:
        if not is_repo_clean(main_repo):
            return False
        result = subprocess.run(
            ["git", "reset", "--hard", f"origin/{base_branch}"],
            cwd=main_repo,
            capture_output=True,
        )
        if result.returncode != 0:
            return False

    # origin/base_branch is now up-to-date, which is sufficient for merge-base checks
    return True


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
