"""Git operations for push and PR automation."""

import subprocess
from dataclasses import dataclass
from pathlib import Path


@dataclass
class WorktreeInfo:
    name: str
    path: Path
    branch: str
    on_origin: bool
    is_dirty: bool


def list_worktrees(repo_root: Path) -> list[WorktreeInfo]:
    """List all worktrees in .lf/worktrees/ with their status."""
    worktrees_dir = repo_root / ".lf" / "worktrees"
    if not worktrees_dir.exists():
        return []

    # Get remote branches
    result = subprocess.run(
        ["git", "branch", "-r", "--format=%(refname:short)"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    remote_branches = set()
    if result.returncode == 0:
        for line in result.stdout.strip().split("\n"):
            if line.startswith("origin/"):
                remote_branches.add(line[7:])  # strip "origin/"

    worktrees = []
    for path in sorted(worktrees_dir.iterdir()):
        if not path.is_dir():
            continue

        name = path.name

        # Get branch name
        branch_result = subprocess.run(
            ["git", "branch", "--show-current"],
            cwd=path,
            capture_output=True,
            text=True,
        )
        branch = branch_result.stdout.strip() if branch_result.returncode == 0 else name

        # Check if dirty
        status_result = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=path,
            capture_output=True,
            text=True,
        )
        is_dirty = bool(status_result.stdout.strip())

        worktrees.append(WorktreeInfo(
            name=name,
            path=path,
            branch=branch,
            on_origin=branch in remote_branches,
            is_dirty=is_dirty,
        ))

    return worktrees


def remove_worktree(repo_root: Path, name: str) -> bool:
    """Remove a worktree and its branch. Returns success."""
    worktree_path = repo_root / ".lf" / "worktrees" / name

    if not worktree_path.exists():
        return False

    # Get branch name before removing
    branch_result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )
    branch = branch_result.stdout.strip() if branch_result.returncode == 0 else name

    # Remove worktree
    result = subprocess.run(
        ["git", "worktree", "remove", str(worktree_path), "--force"],
        cwd=repo_root,
        capture_output=True,
    )
    if result.returncode != 0:
        return False

    # Delete branch
    subprocess.run(
        ["git", "branch", "-D", branch],
        cwd=repo_root,
        capture_output=True,
    )

    return True


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


def create_worktree(repo_root: Path, name: str) -> Path | None:
    """Create a worktree with a new branch. Returns worktree path or None on failure."""
    worktree_path = repo_root / ".lf" / "worktrees" / name

    if worktree_path.exists():
        # Worktree already exists, return it
        return worktree_path

    # Create parent directory
    worktree_path.parent.mkdir(parents=True, exist_ok=True)

    result = subprocess.run(
        ["git", "worktree", "add", "-b", name, str(worktree_path)],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        return None

    return worktree_path


def open_pr(repo_root: Path, draft: bool = True) -> tuple[str | None, str | None]:
    """Open GitHub PR for current branch. Returns (url, error)."""
    commit_file = repo_root / ".lf" / "COMMIT"

    # Read COMMIT for PR title/body before deleting
    if commit_file.exists():
        content = commit_file.read_text().strip()
        lines = content.split("\n", 1)
        title = lines[0]
        body = lines[1].strip() if len(lines) > 1 else ""
        cmd = ["gh", "pr", "create", "--title", title, "--body", body]
        # Remove COMMIT before push - PR becomes source of truth
        commit_file.unlink()
        subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)
        subprocess.run(
            ["git", "commit", "-m", "remove .lf/COMMIT"],
            cwd=repo_root,
            check=True,
        )
    else:
        cmd = ["gh", "pr", "create", "--fill"]

    # Push to origin
    subprocess.run(
        ["git", "push", "-u", "origin", "HEAD"],
        cwd=repo_root,
        capture_output=True,
    )

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
