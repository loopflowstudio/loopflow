"""Background task for auto-creating draft PRs.

Creates draft PRs for pushed branches that don't have PRs yet.
"""

import subprocess
from pathlib import Path

from loopflow.lf.git import find_main_repo
from loopflow.lf.worktrees import list_all


def _create_draft_pr(worktree_path: Path) -> bool:
    """Create a draft PR for the branch. Returns True on success."""
    result = subprocess.run(
        ["gh", "pr", "create", "--draft", "--fill"],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def run_draft_pr_check(repo_root: Path | None = None) -> list[str]:
    """Check all worktrees and create draft PRs for eligible branches.

    Returns list of branches that got draft PRs created.
    """
    if repo_root is None:
        repo_root = find_main_repo()
        if repo_root is None:
            return []

    created = []
    try:
        worktrees = list_all(repo_root)
    except Exception:
        return []

    for wt in worktrees:
        # Skip main branch
        if wt.branch in ("main", "master"):
            continue

        # Skip if already has PR
        if wt.pr_number is not None:
            continue

        # Skip if no commits ahead of main
        if wt.ahead_main == 0:
            continue

        # Skip if no diff (shouldn't happen if ahead_main > 0, but check anyway)
        if wt.lines_added == 0 and wt.lines_removed == 0:
            continue

        # Skip if branch not pushed to remote
        if not wt.on_origin:
            continue

        # Create draft PR
        if _create_draft_pr(wt.path):
            created.append(wt.branch)

    return created
