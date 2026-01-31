"""Git operations with Rust (lf-engine) or Python fallback."""

import json
import shutil
import subprocess
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any

from loopflow.lf.config import get_internal_flag


class GitError(Exception):
    def __init__(self, payload: Any) -> None:
        self.payload = payload
        super().__init__(self._format())

    def _format(self) -> str:
        if isinstance(self.payload, dict):
            message = self.payload.get("message") or self.payload.get("stderr")
            return message or json.dumps(self.payload)
        return str(self.payload)


@dataclass
class RebaseResult:
    success: bool
    conflicts: list[Path] | None
    new_head: str | None


@dataclass
class BranchInfo:
    old_branch: str
    old_head: str
    new_branch: str


@dataclass
class LandResult:
    merged_commit: str
    branch_deleted: bool


# -----------------------------------------------------------------------------
# Rust/Python routing
# -----------------------------------------------------------------------------

_backend_printed = False


@lru_cache(maxsize=1)
def _loopflow_engine_available() -> bool:
    return shutil.which("lf-engine") is not None


def _use_rust() -> bool:
    """Use Rust if flag is set AND lf-engine is available."""
    return get_internal_flag("use_rust") and _loopflow_engine_available()


def _print_backend_once() -> None:
    """Print backend info once per session (grey text)."""
    global _backend_printed
    if _backend_printed:
        return
    _backend_printed = True

    import sys

    backend = "lf-engine (rust)" if _use_rust() else "python"
    # ANSI dim grey: \033[2m for dim, \033[0m to reset
    if sys.stderr.isatty():
        sys.stderr.write(f"\033[2m[git: {backend}]\033[0m\n")
    else:
        sys.stderr.write(f"[git: {backend}]\n")


# -----------------------------------------------------------------------------
# Rust implementations (via lf-engine subprocess)
# -----------------------------------------------------------------------------


def _run_loopflow_engine(args: list[str]) -> Any:
    try:
        result = subprocess.run(args, capture_output=True, text=True)
    except FileNotFoundError as err:
        raise GitError(f"lf-engine not found: {err}") from None
    if result.returncode != 0:
        payload = result.stderr.strip()
        try:
            raise GitError(json.loads(payload))
        except json.JSONDecodeError:
            raise GitError(payload) from None
    output = result.stdout.strip()
    if not output:
        return None
    return json.loads(output)


def _rebase_rust(worktree: Path, onto: str, base_commit: str | None = None) -> RebaseResult:
    args = ["lf-engine", "rebase", "--worktree", str(worktree), "--onto", onto]
    if base_commit:
        args.extend(["--base-commit", base_commit])
    data = _run_loopflow_engine(args)
    conflicts = None
    if data.get("conflicts"):
        conflicts = [Path(path) for path in data["conflicts"]]
    return RebaseResult(
        success=data["success"],
        conflicts=conflicts,
        new_head=data.get("new_head"),
    )


def _push_rust(worktree: Path, force_with_lease: bool = False) -> None:
    args = ["lf-engine", "push", "--worktree", str(worktree)]
    if force_with_lease:
        args.append("--force-with-lease")
    _run_loopflow_engine(args)


def _create_branch_rust(worktree: Path, name: str) -> BranchInfo:
    data = _run_loopflow_engine(["lf-engine", "branch", "--worktree", str(worktree), "--name", name])
    return BranchInfo(**data)


def _land_rust(worktree: Path, strategy: str, main_branch: str = "main") -> LandResult:
    args = [
        "lf-engine",
        "land",
        "--worktree",
        str(worktree),
        "--strategy",
        strategy,
        "--main-branch",
        main_branch,
    ]
    data = _run_loopflow_engine(args)
    return LandResult(**data)


# -----------------------------------------------------------------------------
# Python implementations (fallback)
# -----------------------------------------------------------------------------


def _rebase_python(worktree: Path, onto: str, base_commit: str | None = None) -> RebaseResult:
    if base_commit:
        cmd = ["git", "rebase", "--onto", onto, base_commit]
    else:
        cmd = ["git", "rebase", onto]

    result = subprocess.run(cmd, cwd=worktree, capture_output=True, text=True)

    if result.returncode == 0:
        # Get new HEAD
        head_result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=worktree,
            capture_output=True,
            text=True,
        )
        new_head = head_result.stdout.strip() if head_result.returncode == 0 else None
        return RebaseResult(success=True, conflicts=None, new_head=new_head)

    # Check for conflicts
    status_result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=worktree,
        capture_output=True,
        text=True,
    )

    conflicts = []
    for line in status_result.stdout.splitlines():
        if line.startswith("UU ") or line.startswith("AA "):
            conflicts.append(Path(line[3:]))

    # Abort the rebase
    subprocess.run(["git", "rebase", "--abort"], cwd=worktree, capture_output=True)

    if conflicts:
        return RebaseResult(success=False, conflicts=conflicts, new_head=None)

    # Some other error
    raise GitError(result.stderr.strip() or "Rebase failed")


def _push_python(worktree: Path, force_with_lease: bool = False) -> None:
    cmd = ["git", "push"]
    if force_with_lease:
        cmd.append("--force-with-lease")
    result = subprocess.run(cmd, cwd=worktree, capture_output=True, text=True)
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "Push failed")


def _create_branch_python(worktree: Path, name: str) -> BranchInfo:
    # Get current branch and HEAD
    branch_result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    old_branch = branch_result.stdout.strip()

    head_result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    old_head = head_result.stdout.strip()

    # Create and checkout new branch
    result = subprocess.run(
        ["git", "checkout", "-b", name],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or f"Failed to create branch {name}")

    return BranchInfo(old_branch=old_branch, old_head=old_head, new_branch=name)


def _land_python(worktree: Path, strategy: str, main_branch: str = "main") -> LandResult:
    # Get current branch
    branch_result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    feature_branch = branch_result.stdout.strip()

    if strategy == "squash":
        # Squash merge approach
        subprocess.run(["git", "checkout", main_branch], cwd=worktree, check=True)
        result = subprocess.run(
            ["git", "merge", "--squash", feature_branch],
            cwd=worktree,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            raise GitError(result.stderr.strip() or "Squash merge failed")

        # Commit
        subprocess.run(["git", "commit", "--no-edit"], cwd=worktree, check=True)
    else:
        # Regular merge
        subprocess.run(["git", "checkout", main_branch], cwd=worktree, check=True)
        result = subprocess.run(
            ["git", "merge", "--no-ff", feature_branch],
            cwd=worktree,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            raise GitError(result.stderr.strip() or "Merge failed")

    # Get merged commit
    head_result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    merged_commit = head_result.stdout.strip()

    # Delete feature branch
    delete_result = subprocess.run(
        ["git", "branch", "-D", feature_branch],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    branch_deleted = delete_result.returncode == 0

    return LandResult(merged_commit=merged_commit, branch_deleted=branch_deleted)


# -----------------------------------------------------------------------------
# Public API
# -----------------------------------------------------------------------------


def rebase(worktree: Path, onto: str, base_commit: str | None = None) -> RebaseResult:
    _print_backend_once()
    if _use_rust():
        return _rebase_rust(worktree, onto, base_commit)
    return _rebase_python(worktree, onto, base_commit)


def push(worktree: Path, force_with_lease: bool = False) -> None:
    _print_backend_once()
    if _use_rust():
        return _push_rust(worktree, force_with_lease)
    return _push_python(worktree, force_with_lease)


def create_branch(worktree: Path, name: str) -> BranchInfo:
    _print_backend_once()
    if _use_rust():
        return _create_branch_rust(worktree, name)
    return _create_branch_python(worktree, name)


def land(worktree: Path, strategy: str, main_branch: str = "main") -> LandResult:
    _print_backend_once()
    if _use_rust():
        return _land_rust(worktree, strategy, main_branch)
    return _land_python(worktree, strategy, main_branch)
