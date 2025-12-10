"""Git operations for push and PR automation."""

import subprocess
from pathlib import Path


def has_upstream(repo_root: Path) -> bool:
    """Check if current branch tracks a remote."""
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "@{u}"],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def push(repo_root: Path) -> bool:
    """Push current branch to its upstream. Returns success."""
    result = subprocess.run(
        ["git", "push"],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def create_and_track_branch(repo_root: Path, name: str) -> bool:
    """Create branch and set up tracking with origin. Returns success."""
    # Create branch
    result = subprocess.run(
        ["git", "checkout", "-b", name],
        cwd=repo_root,
        capture_output=True,
    )
    if result.returncode != 0:
        return False

    # Set up tracking
    result = subprocess.run(
        ["git", "push", "-u", "origin", name],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def open_pr(repo_root: Path, draft: bool = True) -> tuple[str | None, str | None]:
    """Open GitHub PR for current branch. Returns (url, error)."""
    commit_file = repo_root / ".lf" / "COMMIT"

    if commit_file.exists():
        content = commit_file.read_text().strip()
        lines = content.split("\n", 1)
        title = lines[0]
        body = lines[1].strip() if len(lines) > 1 else ""
        cmd = ["gh", "pr", "create", "--title", title, "--body", body]
    else:
        cmd = ["gh", "pr", "create", "--fill"]

    if draft:
        cmd.append("--draft")

    result = subprocess.run(
        cmd,
        cwd=repo_root,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        # Check if PR already exists
        if "already exists" in result.stderr:
            # Get existing PR URL
            view_result = subprocess.run(
                ["gh", "pr", "view", "--json", "url", "-q", ".url"],
                cwd=repo_root,
                capture_output=True,
                text=True,
            )
            if view_result.returncode == 0:
                return view_result.stdout.strip(), None
        return None, result.stderr.strip()

    return result.stdout.strip(), None
