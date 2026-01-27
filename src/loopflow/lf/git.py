"""Git operations for push and PR automation."""

import json
import subprocess
from pathlib import Path
from typing import Optional


class GitError(Exception):
    """Git operation failed."""

    pass


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
    from loopflow.lf.messages import generate_commit_message

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
    try:
        generated = generate_commit_message(repo_root)
        msg = f"{prefix}: {generated.title}"
        if generated.body:
            msg += f"\n\n{generated.body}"
    except Exception as e:
        # Fallback if LLM unavailable (no API key, rate limit, etc)
        if verbose:
            print(f"[{task}] LLM unavailable, using fallback message: {e}")
        msg = f"{prefix}: auto-generated commit"

    subprocess.run(["git", "commit", "-m", msg], cwd=repo_root, check=True)

    if verbose:
        print(f"[{task}] committed: {msg.splitlines()[0]}")

    if push and has_upstream(repo_root):
        result = subprocess.run(
            ["git", "push"],
            cwd=repo_root,
            capture_output=True,
        )
        if verbose:
            print(f"[{task}] pushed to origin")
        # Create draft PR if none exists
        url = ensure_draft_pr(repo_root)
        if url and verbose:
            print(f"[{task}] created draft PR: {url}")

    return True


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


def get_default_base_ref(repo_root: Path) -> str:
    """Return default base ref (origin/HEAD), falling back to main."""
    result = subprocess.run(
        ["git", "rev-parse", "--abbrev-ref", "origin/HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    base_ref = result.stdout.strip() if result.returncode == 0 else ""
    return base_ref or "main"


def get_pr_target(base_branch: str | None) -> str:
    """Determine PR target based on base branch status.

    If base_branch is set (stacked worktree), target it while its PR is open.
    Once base branch PR merges, target main instead.
    """
    if not base_branch or base_branch == "main":
        return "main"

    # Check if base branch PR is merged
    result = subprocess.run(
        ["gh", "pr", "view", base_branch, "--json", "state", "-q", ".state"],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        state = result.stdout.strip().upper()
        if state == "MERGED":
            return "main"

    return base_branch


def open_pr(
    repo_root: Path,
    title: Optional[str] = None,
    body: Optional[str] = None,
    base: Optional[str] = None,
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

    # Add base branch if specified
    if base and base != "main":
        cmd.extend(["--base", base])

    result = subprocess.run(
        cmd,
        cwd=repo_root,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        # Check if PR already exists
        if "already exists" in result.stderr:
            # Update existing PR with new title/body if provided
            if title:
                subprocess.run(
                    ["gh", "pr", "edit", "--title", title, "--body", body or ""],
                    cwd=repo_root,
                    capture_output=True,
                )
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


def has_pr(repo_root: Path) -> bool:
    """Check if current branch has an open or draft PR."""
    result = subprocess.run(
        ["gh", "pr", "view", "--json", "state", "-q", ".state"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False
    state = result.stdout.strip().upper()
    return state in ("OPEN", "DRAFT")


def ensure_draft_pr(repo_root: Path) -> str | None:
    """Create draft PR if none exists. Returns URL or None if skipped/failed."""
    branch = get_current_branch(repo_root)
    if not branch or branch == "main":
        return None

    if has_pr(repo_root):
        return None

    result = subprocess.run(
        ["gh", "pr", "create", "--draft", "--fill"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return result.stdout.strip()
    return None


def _get_pr_info(repo_root: Path, pr_number: int | None = None) -> dict | None:
    cmd = ["gh", "pr", "view", "--json", "number,isDraft"]
    if pr_number is not None:
        cmd.insert(3, str(pr_number))
    result = subprocess.run(
        cmd,
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0 or not result.stdout.strip():
        return None
    return json.loads(result.stdout)


def is_draft_pr(repo_root: Path, pr_number: int | None = None) -> bool | None:
    """Return True if PR is a draft, False if ready, None if not found."""
    info = _get_pr_info(repo_root, pr_number)
    if not info:
        return None
    return bool(info.get("isDraft", False))


def ensure_ready_pr(repo_root: Path, pr_number: int | None = None) -> bool:
    """Mark draft PR as ready. Returns True if ready or updated."""
    info = _get_pr_info(repo_root, pr_number)
    if not info:
        return False
    if not info.get("isDraft", False):
        return True

    cmd = ["gh", "pr", "ready"]
    number = pr_number or info.get("number")
    if number is not None:
        cmd.append(str(number))
    result = subprocess.run(
        cmd,
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0
