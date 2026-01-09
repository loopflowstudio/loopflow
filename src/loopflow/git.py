"""Git operations for push and PR automation."""

import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


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
    base_branch: str | None = None  # PR target branch, if PR exists
    pr_url: str | None = None
    pr_number: int | None = None
    commit_sha: str | None = None
    commit_age: str | None = None
    commit_message: str | None = None
    ahead_main: int = 0
    behind_main: int = 0
    has_staged: bool = False
    has_modified: bool = False
    has_untracked: bool = False


def find_main_repo(start: Optional[Path] = None) -> Path | None:
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


def worktree_path(repo_root: Path, branch: str) -> Path:
    """Get the path for a worktree (worktrunk-compatible).

    Pattern: ../{repo}.{branch} with / replaced by -
    """
    sanitized = branch.replace("/", "-").replace("\\", "-")
    return repo_root.parent / f"{repo_root.name}.{sanitized}"


def list_worktrees(repo_root: Path) -> list[WorktreeInfo]:
    """List all worktrees (worktrunk-compatible sibling directories).

    Looks for directories matching ../{repo}.* pattern.
    """
    parent = repo_root.parent
    prefix = f"{repo_root.name}."

    wt_paths = sorted([
        p for p in parent.iterdir()
        if p.is_dir() and p.name.startswith(prefix)
    ])
    if not wt_paths:
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
    for path in wt_paths:
        # Extract branch name: loopflow.feature-x -> feature-x
        name = path.name[len(prefix):]

        # Get branch name
        branch_result = subprocess.run(
            ["git", "branch", "--show-current"],
            cwd=path,
            capture_output=True,
            text=True,
        )
        branch = branch_result.stdout.strip() if branch_result.returncode == 0 else name

        # Check detailed status
        status_result = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=path,
            capture_output=True,
            text=True,
        )
        status_lines = status_result.stdout.strip().split("\n") if status_result.stdout.strip() else []
        has_staged = any(line and line[0] in "MADRC" for line in status_lines)
        has_modified = any(line and len(line) > 1 and line[1] in "MD" for line in status_lines)
        has_untracked = any(line and line.startswith("??") for line in status_lines)
        is_dirty = bool(status_lines)

        # Get PR info
        base_branch = None
        pr_url = None
        pr_number = None
        pr_result = subprocess.run(
            ["gh", "pr", "view", "--json", "baseRefName,url,number", "-q", ".baseRefName,.url,.number"],
            cwd=path,
            capture_output=True,
            text=True,
        )
        if pr_result.returncode == 0 and pr_result.stdout.strip():
            parts = pr_result.stdout.strip().split("\n")
            if len(parts) >= 3:
                base_branch = parts[0]
                pr_url = parts[1]
                try:
                    pr_number = int(parts[2])
                except ValueError:
                    pass

        # Get commit info
        commit_result = subprocess.run(
            ["git", "log", "-1", "--format=%h%n%ar%n%s"],
            cwd=path,
            capture_output=True,
            text=True,
        )
        commit_sha = None
        commit_age = None
        commit_message = None
        if commit_result.returncode == 0:
            parts = commit_result.stdout.strip().split("\n")
            if len(parts) >= 3:
                commit_sha = parts[0]
                commit_age = parts[1]
                commit_message = parts[2][:50]  # truncate

        # Get ahead/behind main
        ahead_main = 0
        behind_main = 0
        rev_result = subprocess.run(
            ["git", "rev-list", "--left-right", "--count", "main...HEAD"],
            cwd=path,
            capture_output=True,
            text=True,
        )
        if rev_result.returncode == 0 and rev_result.stdout.strip():
            parts = rev_result.stdout.strip().split()
            if len(parts) == 2:
                try:
                    behind_main = int(parts[0])
                    ahead_main = int(parts[1])
                except ValueError:
                    pass

        worktrees.append(WorktreeInfo(
            name=name,
            path=path,
            branch=branch,
            on_origin=branch in remote_branches,
            is_dirty=is_dirty,
            base_branch=base_branch,
            pr_url=pr_url,
            pr_number=pr_number,
            commit_sha=commit_sha,
            commit_age=commit_age,
            commit_message=commit_message,
            ahead_main=ahead_main,
            behind_main=behind_main,
            has_staged=has_staged,
            has_modified=has_modified,
            has_untracked=has_untracked,
        ))

    return worktrees


def remove_worktree(repo_root: Path, name: str) -> bool:
    """Remove a worktree and its branch. Returns success."""
    wt_path = worktree_path(repo_root, name)

    if not wt_path.exists():
        return False

    # Get branch name before removing
    branch_result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=wt_path,
        capture_output=True,
        text=True,
    )
    branch = branch_result.stdout.strip() if branch_result.returncode == 0 else name

    # Remove worktree
    result = subprocess.run(
        ["git", "worktree", "remove", str(wt_path), "--force"],
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
    push: bool = False,
    verbose: bool = False,
) -> bool:
    """Commit changes with task name + generated message. Returns True if committed."""
    from loopflow.llm_http import generate_commit_message

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

    # Build prefix: lf {task}
    prefix = f"lf {task}"

    subprocess.run(["git", "add", "-A"], cwd=repo_root, check=True)

    # Generate commit message from staged diff
    if verbose:
        print(f"\n[{task}] generating commit message...")
    generated = generate_commit_message(repo_root)

    # Combine: prefix on first line, then generated title and body
    msg = f"{prefix}: {generated.title}"
    if generated.body:
        msg += f"\n\n{generated.body}"

    subprocess.run(["git", "commit", "-m", msg], cwd=repo_root, check=True)

    if verbose:
        print(f"[{task}] committed: {prefix}: {generated.title}")

    if push and has_upstream(repo_root):
        result = subprocess.run(
            ["git", "push"],
            cwd=repo_root,
            capture_output=True,
        )
        if verbose:
            print(f"[{task}] pushed to origin")

    return True


def _sync_main(repo_root: Path) -> None:
    """Fetch origin/main and fast-forward main to match. Raises GitError if main is dirty."""
    # Check if main repo is dirty
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.stdout.strip():
        raise GitError("Main repo has uncommitted changes. Commit or stash them first.")

    # Fetch latest
    subprocess.run(
        ["git", "fetch", "origin", "main"],
        cwd=repo_root,
        capture_output=True,
    )

    # Reset main to origin/main
    subprocess.run(
        ["git", "reset", "--hard", "origin/main"],
        cwd=repo_root,
        capture_output=True,
    )


def get_current_branch(repo_root: Path) -> str | None:
    """Get current branch name, or None if detached HEAD."""
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    branch = result.stdout.strip()
    return branch if branch else None


def create_worktree(repo_root: Path, name: str, base: str | None = None) -> Path:
    """Create a worktree with a new branch. Raises GitError on failure.

    If base is None, uses current branch. If current is main or detached, syncs main first.
    """
    wt_path = worktree_path(repo_root, name)

    if wt_path.exists():
        return wt_path

    # Determine base branch
    if base is None:
        current = get_current_branch(repo_root)
        base = current if current and current != "main" else None

    # If branching from main (or no base specified), sync main first
    if base is None or base == "main":
        _sync_main(repo_root)
        start_point = "main"
    else:
        # Fetch the base branch to get latest
        subprocess.run(
            ["git", "fetch", "origin", base],
            cwd=repo_root,
            capture_output=True,
        )
        start_point = f"origin/{base}"

    result = subprocess.run(
        ["git", "worktree", "add", "-b", name, str(wt_path), start_point],
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

    return wt_path


def open_pr(
    repo_root: Path,
    title: Optional[str] = None,
    body: Optional[str] = None,
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
