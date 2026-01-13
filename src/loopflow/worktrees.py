"""Git worktree operations.

Provides worktree management for parallel development workflows.
Interface is tool-agnostic.
"""

import json
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path


class WorktreeError(Exception):
    """Worktree operation failed."""


@dataclass
class Worktree:
    """A git worktree with status information."""

    name: str
    path: Path
    branch: str
    base_branch: str | None
    on_origin: bool
    is_dirty: bool
    pr_url: str | None
    pr_number: int | None
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


def _run_wt(args: list[str], repo_root: Path) -> str:
    """Run worktree CLI command."""
    cmd = ["wt", "-C", str(repo_root), *args]
    try:
        result = subprocess.run(cmd, cwd=repo_root, capture_output=True, text=True)
    except FileNotFoundError:
        raise WorktreeError("Worktree CLI not found. Run: lf ops install")

    if result.returncode != 0:
        error = result.stderr.strip() or result.stdout.strip() or "Worktree operation failed"
        raise WorktreeError(error)

    return result.stdout


def _parse_pr_number(pr_url: str | None) -> int | None:
    if not pr_url:
        return None
    match = re.search(r"/pull/(\d+)", pr_url)
    return int(match.group(1)) if match else None


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

        worktrees.append(
            Worktree(
                name=branch,
                path=path,
                branch=branch,
                base_branch=item.get("base_branch"),
                on_origin=on_origin,
                is_dirty=is_dirty,
                pr_url=pr_url,
                pr_number=pr_number,
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
    """
    existing = {wt.branch for wt in list_all(repo_root)}

    if name in existing:
        output = _run_wt(["switch", name, "--execute", "pwd"], repo_root)
        return Path(output.strip())

    args = ["switch", "--create", name]
    if base:
        args.extend(["--base", base])
    args.extend(["--execute", "pwd"])

    output = _run_wt(args, repo_root)
    return Path(output.strip())


def remove(repo_root: Path, name: str) -> bool:
    """Remove a worktree and its branch. Returns success."""
    try:
        _run_wt(["remove", name], repo_root)
        return True
    except WorktreeError:
        return False
