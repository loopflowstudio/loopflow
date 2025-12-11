"""Git operations for push and PR automation."""

import subprocess
from dataclasses import dataclass
from pathlib import Path


class GitError(Exception):
    """Git operation failed."""
    pass


@dataclass
class WorktreeInfo:
    name: str
    path: Path
    branch: str
    on_origin: bool
    is_dirty: bool


def find_main_repo(start: Path | None = None) -> Path | None:
    """Find the main repo root, even from inside a worktree."""
    cwd = start or Path.cwd()
    result = subprocess.run(
        ["git", "rev-parse", "--git-common-dir"],
        cwd=cwd,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    # --git-common-dir returns the .git directory; parent is repo root
    git_dir = Path(result.stdout.strip())
    if not git_dir.is_absolute():
        git_dir = (cwd / git_dir).resolve()
    return git_dir.parent


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


def autocommit(
    repo_root: Path,
    task: str,
    arg: str | None = None,
    push: bool = False,
    verbose: bool = False,
) -> bool:
    """Commit changes with task name as message. Returns True if committed."""
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if not result.stdout.strip():
        if verbose:
            print(f"\n[{task}] no changes to commit")
        return False

    msg = f"lf {task}"
    if arg:
        msg += f" {arg}"

    subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)
    subprocess.run(["git", "commit", "-m", msg], cwd=repo_root, check=True)

    if verbose:
        print(f"\n[{task}] committed: {msg}")

    if push and has_upstream(repo_root):
        result = subprocess.run(
            ["git", "push"],
            cwd=repo_root,
            capture_output=True,
        )
        if verbose:
            print(f"[{task}] pushed to origin")

    return True


def create_worktree(repo_root: Path, name: str) -> Path:
    """Create a worktree with a new branch. Raises GitError on failure."""
    worktree_path = repo_root / ".lf" / "worktrees" / name

    if worktree_path.exists():
        return worktree_path

    worktree_path.parent.mkdir(parents=True, exist_ok=True)

    result = subprocess.run(
        ["git", "worktree", "add", "-b", name, str(worktree_path)],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        # Extract useful part from git error
        error = result.stderr.strip()
        if "already exists" in error:
            raise GitError(f"Branch '{name}' already exists")
        raise GitError(error or "Failed to create worktree")

    return worktree_path


def open_pr(
    repo_root: Path,
    title: str | None = None,
    body: str | None = None,
) -> str:
    """Open GitHub PR for current branch. Returns URL. Raises GitError on failure."""
    # Push to origin
    subprocess.run(
        ["git", "push", "-u", "origin", "HEAD"],
        cwd=repo_root,
        capture_output=True,
    )

    if title:
        cmd = ["gh", "pr", "create", "--title", title, "--body", body or ""]
    else:
        cmd = ["gh", "pr", "create", "--fill"]

    result = subprocess.run(
        cmd,
        cwd=repo_root,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        # Check if PR already exists
        if "already exists" in result.stderr:
            view_result = subprocess.run(
                ["gh", "pr", "view", "--json", "url", "-q", ".url"],
                cwd=repo_root,
                capture_output=True,
                text=True,
            )
            if view_result.returncode == 0:
                return view_result.stdout.strip()
        raise GitError(result.stderr.strip() or "Failed to create PR")

    return result.stdout.strip()


def update_pr(
    repo_root: Path,
    title: str,
    body: str,
) -> str:
    """Update existing PR title and body. Returns URL. Raises GitError on failure."""
    # Push any new commits
    subprocess.run(
        ["git", "push"],
        cwd=repo_root,
        capture_output=True,
    )

    # Update PR
    result = subprocess.run(
        ["gh", "pr", "edit", "--title", title, "--body", body],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "Failed to update PR")

    # Get PR URL
    view_result = subprocess.run(
        ["gh", "pr", "view", "--json", "url", "-q", ".url"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if view_result.returncode != 0:
        raise GitError("PR updated but could not get URL")

    return view_result.stdout.strip()
