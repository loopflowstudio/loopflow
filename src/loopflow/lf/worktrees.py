"""Git worktree operations.

Provides worktree management for parallel development workflows.
Interface is tool-agnostic.
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from loopflow.lf.config import BranchNameConfig


class WorktreeError(Exception):
    """Worktree operation failed."""


@dataclass
class Worktree:
    """A git worktree with status information."""

    name: str
    path: Path
    branch: str
    base_branch: str | None
    base_commit: str | None  # SHA when branched - for squash merge recovery
    on_origin: bool
    is_dirty: bool
    main_state: str | None
    integration_reason: str | None
    pr_url: str | None
    pr_number: int | None
    pr_state: str | None  # "open", "merged", "closed", "draft"
    ahead_main: int
    behind_main: int
    ahead_remote: int
    behind_remote: int
    lines_added: int
    lines_removed: int
    has_staged: bool
    has_modified: bool
    has_untracked: bool
    is_rebasing: bool
    is_merging: bool


def _find_wt_binary() -> str | None:
    """Find the wt binary, checking common locations."""
    wt_path = shutil.which("wt")
    if wt_path:
        return wt_path

    for path in [
        "/opt/homebrew/bin/wt",
        "/usr/local/bin/wt",
        str(Path.home() / ".cargo" / "bin" / "wt"),
    ]:
        if Path(path).exists():
            return path

    return None


def is_wt_available() -> bool:
    """Check if wt (worktrunk) is available."""
    return _find_wt_binary() is not None


def ensure_wt_available() -> bool:
    """Ensure wt is available, installing via homebrew if needed."""
    if _find_wt_binary():
        return True

    brew_path = shutil.which("brew")
    if not brew_path:
        return False

    result = subprocess.run([brew_path, "install", "worktrunk"], capture_output=True)
    if result.returncode != 0:
        return False

    return _find_wt_binary() is not None


def _run_wt(args: list[str], repo_root: Path) -> str:
    """Run worktree CLI command."""
    wt_binary = _find_wt_binary()
    if not wt_binary:
        raise WorktreeError("Worktree CLI not found. Run: lfd install")

    cmd = [wt_binary, "-C", str(repo_root), *args]
    result = subprocess.run(cmd, cwd=repo_root, capture_output=True, text=True)

    if result.returncode != 0:
        error = result.stderr.strip() or result.stdout.strip() or "Worktree operation failed"
        raise WorktreeError(error)

    return result.stdout


def _parse_pr_number(pr_url: str | None) -> int | None:
    if not pr_url:
        return None
    match = re.search(r"/pull/(\d+)", pr_url)
    return int(match.group(1)) if match else None


def get_pr_state(repo_root: Path, branch: str) -> str | None:
    """Return PR state using gh pr view --json state."""
    try:
        result = subprocess.run(
            ["gh", "pr", "view", branch, "--json", "state", "-q", ".state"],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip().lower()
    except FileNotFoundError:
        pass
    return None


def diff_against(repo_root: Path, branch: str, base: str = "main") -> str:
    """Get diff of branch against base."""
    result = subprocess.run(
        ["git", "diff", f"{base}...{branch}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.stdout if result.returncode == 0 else ""


def diff_between(repo_root: Path, branch_a: str, branch_b: str) -> str:
    """Get diff between two branches."""
    result = subprocess.run(
        ["git", "diff", f"{branch_a}...{branch_b}"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.stdout if result.returncode == 0 else ""


def get_github_compare_url(repo_root: Path, branch: str, base: str = "main") -> str | None:
    """Get GitHub compare URL for branch vs base."""
    result = subprocess.run(
        ["git", "remote", "get-url", "origin"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None

    remote_url = result.stdout.strip()
    # Convert git@github.com:org/repo.git or https://github.com/org/repo.git to https://github.com/org/repo
    if remote_url.startswith("git@github.com:"):
        repo_path = remote_url[len("git@github.com:") :].removesuffix(".git")
    elif "github.com" in remote_url:
        repo_path = remote_url.split("github.com/")[-1].removesuffix(".git")
    else:
        return None

    return f"https://github.com/{repo_path}/compare/{base}...{branch}"


def get_path(repo_root: Path, name: str) -> Path:
    """Get the path where a worktree lives (or would live).

    Uses sibling directory pattern: ../repo.branch-name
    """
    sanitized = name.replace("/", "-").replace("\\", "-")
    return repo_root.parent / f"{repo_root.name}.{sanitized}"


def list_all(repo_root: Path) -> list[Worktree]:
    """List all worktrees including the main repo."""
    output = _run_wt(["list", "--format", "json", "--full"], repo_root)
    data = json.loads(output) if output.strip() else []

    worktrees: list[Worktree] = []
    for item in data:
        if item.get("kind") == "branch":
            continue

        branch = item.get("branch", "")
        path = Path(item["path"])

        working_tree = item.get("working_tree") or {}
        has_staged = bool(working_tree.get("staged"))
        has_modified = bool(working_tree.get("modified"))
        has_untracked = bool(working_tree.get("untracked"))
        is_dirty = has_staged or has_modified or has_untracked

        diff_vs_main = working_tree.get("diff_vs_main") or {}
        lines_added = int(diff_vs_main.get("added") or 0)
        lines_removed = int(diff_vs_main.get("deleted") or 0)

        main = item.get("main") or {}
        ahead_main = int(main.get("ahead") or 0)
        behind_main = int(main.get("behind") or 0)

        main_state = item.get("main_state")
        integration_reason = item.get("integration_reason")

        remote = item.get("remote") or {}
        on_origin = bool(remote.get("name") or remote.get("branch"))
        ahead_remote = int(remote.get("ahead") or 0)
        behind_remote = int(remote.get("behind") or 0)

        operation_state = item.get("operation_state") or ""
        is_rebasing = operation_state == "rebase"
        is_merging = operation_state == "merge"

        ci = item.get("ci") or {}
        pr_url = ci.get("url") if ci.get("source") == "pr" else None
        pr_number = _parse_pr_number(pr_url)
        pr_state = ci.get("state", "").lower() if ci.get("source") == "pr" else None

        worktrees.append(
            Worktree(
                name=branch,
                path=path,
                branch=branch,
                base_branch=item.get("base_branch"),
                base_commit=item.get("base_commit"),
                on_origin=on_origin,
                is_dirty=is_dirty,
                main_state=main_state,
                integration_reason=integration_reason,
                pr_url=pr_url,
                pr_number=pr_number,
                pr_state=pr_state if pr_state else None,
                ahead_main=ahead_main,
                behind_main=behind_main,
                ahead_remote=ahead_remote,
                behind_remote=behind_remote,
                lines_added=lines_added,
                lines_removed=lines_removed,
                has_staged=has_staged,
                has_modified=has_modified,
                has_untracked=has_untracked,
                is_rebasing=is_rebasing,
                is_merging=is_merging,
            )
        )

    return worktrees


def create(repo_root: Path, name: str, base: str | None = None) -> Path:
    """Create a worktree for a new branch. Returns path.

    If worktree already exists, switches to it and returns its path.
    If branch exists without worktree (orphaned), deletes it first.
    Uses `name` for both worktree path and branch name (no schema).
    """
    existing = {wt.branch for wt in list_all(repo_root)}

    if name in existing:
        output = _run_wt(["switch", name, "--execute", "pwd"], repo_root)
        return Path(output.strip())

    # Delete orphaned branch (exists without worktree) before creating
    if _local_branch_exists(repo_root, name):
        _delete_local_branch(repo_root, name)

    args = ["switch", "--create", name]
    if base:
        args.extend(["--base", base])
    args.extend(["--execute", "pwd"])

    output = _run_wt(args, repo_root)
    return Path(output.strip())


@dataclass
class CreateWorktreeResult:
    """Result from create_with_schema."""

    path: Path
    branch: str
    base_branch: str | None
    base_commit: str | None


def create_with_schema(
    repo_root: Path,
    short_name: str,
    base: str | None = None,
    branch_config: "BranchNameConfig | None" = None,
) -> CreateWorktreeResult:
    """Create worktree with short name for path, schema-based branch name."""
    from loopflow.lf.branch_names import format_branch_name

    branch_name = format_branch_name(short_name, branch_config)
    worktree_path = get_path(repo_root, short_name)

    # Check if worktree path already exists
    if worktree_path.exists():
        raise WorktreeError(f"Worktree path already exists: {worktree_path}")

    # Check if branch already exists (in worktrees or as orphaned git branch)
    existing_branches = {wt.branch for wt in list_all(repo_root)}
    if branch_name in existing_branches:
        raise WorktreeError(f"Branch already exists: {branch_name}")
    if _local_branch_exists(repo_root, branch_name):
        raise WorktreeError(
            f"Branch {branch_name} exists without a worktree. "
            f"Delete with: git branch -D {branch_name}"
        )

    # Resolve base ref and record base_commit for stacking
    base_ref = base or "main"
    base_branch = base if base and base != "main" else None
    base_commit = None

    if base_branch:
        # Record the commit SHA we're branching from for squash merge recovery
        result = subprocess.run(
            ["git", "rev-parse", base_ref],
            cwd=repo_root,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            base_commit = result.stdout.strip()

    # Create worktree with explicit branch name using git directly
    # (wt CLI doesn't support separate worktree name vs branch name)
    result = subprocess.run(
        ["git", "worktree", "add", "-b", branch_name, str(worktree_path), base_ref],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        error = result.stderr.strip() or result.stdout.strip() or "Failed to create worktree"
        raise WorktreeError(error)

    # Push to create remote branch with tracking
    subprocess.run(
        ["git", "push", "-u", "origin", branch_name],
        cwd=worktree_path,
        capture_output=True,
        text=True,
    )

    return CreateWorktreeResult(
        path=worktree_path,
        branch=branch_name,
        base_branch=base_branch,
        base_commit=base_commit,
    )


def remove(repo_root: Path, name: str) -> bool:
    """Remove a worktree and its branch. Returns success."""
    try:
        _run_wt(["remove", name], repo_root)
        return True
    except WorktreeError:
        return False


def is_merged(wt: Worktree, repo_root: Path, base_branch: str = "main") -> bool:
    """Check if worktree's branch has been merged to base branch."""
    from loopflow.lf.git import find_main_repo

    if wt.branch is None:
        return False  # Detached HEAD - can't determine merge status
    if wt.branch in ("main", "master"):
        return False
    try:
        main_repo = find_main_repo(repo_root)
        if main_repo and wt.path == main_repo:
            return False  # Never prune the main repo checkout
    except Exception:
        pass  # In tests with fake paths, skip this check
    if wt.is_dirty:
        return False

    if wt.main_state == "integrated":
        return True

    # Check PR state - from wt list or via gh pr view fallback
    pr_state = wt.pr_state or get_pr_state(repo_root, wt.branch)
    if pr_state == "merged":
        return True
    if pr_state is not None:
        if _cherry_is_empty(repo_root, wt.branch, f"origin/{base_branch}"):
            return True
        if _trees_match(repo_root, wt.branch, f"origin/{base_branch}"):
            return True
        if not _remote_branch_exists(repo_root, wt.branch) and pr_state != "open":
            return True

    # New branch with no work yet and no PR - not merged, just empty
    # This check comes after PR checks to avoid false negatives for squash-merged PRs
    if wt.ahead_main == 0 and pr_state is None:
        return False

    # Check if branch is ancestor of origin/base_branch (handles squash merges)
    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", wt.branch, f"origin/{base_branch}"],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def merge_diagnostics(repo_root: Path, wt: Worktree, base_branch: str = "main") -> dict:
    """Return merge detection diagnostics for a worktree."""
    base_ref = f"origin/{base_branch}"
    diagnostics = {
        "branch": wt.branch,
        "base_ref": base_ref,
        "is_dirty": wt.is_dirty,
        "pr_state": wt.pr_state,
        "main_state": wt.main_state,
        "cherry_empty": _cherry_is_empty(repo_root, wt.branch, base_ref),
        "trees_match": _trees_match(repo_root, wt.branch, base_ref),
        "is_ancestor": False,
    }

    result = subprocess.run(
        ["git", "merge-base", "--is-ancestor", wt.branch, base_ref],
        cwd=repo_root,
        capture_output=True,
    )
    diagnostics["is_ancestor"] = result.returncode == 0
    return diagnostics


def _trees_match(repo_root: Path, branch: str, base_ref: str) -> bool:
    result = subprocess.run(
        ["git", "diff", "--quiet", f"{base_ref}...{branch}"],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def _cherry_is_empty(repo_root: Path, branch: str, base_ref: str) -> bool:
    result = subprocess.run(
        ["git", "cherry", base_ref, branch],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return False
    return result.stdout.strip() == ""


def _local_branch_exists(repo_root: Path, branch: str) -> bool:
    ref = f"refs/heads/{branch}"
    result = subprocess.run(
        ["git", "show-ref", "--verify", ref],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def _delete_local_branch(repo_root: Path, branch: str) -> bool:
    result = subprocess.run(
        ["git", "branch", "-D", branch],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def _remote_branch_exists(repo_root: Path, branch: str) -> bool:
    ref = f"refs/remotes/origin/{branch}"
    result = subprocess.run(
        ["git", "show-ref", "--verify", ref],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def find_merged(repo_root: Path, base_branch: str = "main") -> list[Worktree]:
    """Return worktrees whose changes have been merged into base branch."""
    worktrees = list_all(repo_root)
    return [wt for wt in worktrees if is_merged(wt, repo_root, base_branch)]
