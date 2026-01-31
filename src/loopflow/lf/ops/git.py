"""Git operations with Rust (PyO3) or Python fallback."""

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
            return message or str(self.payload)
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
def _loopflow_engine_module() -> Any:
    """Try to import loopflow_engine PyO3 module."""
    try:
        import loopflow_engine

        return loopflow_engine
    except ImportError:
        return None


def _use_rust() -> bool:
    """Use Rust if flag is set AND loopflow_engine module is available."""
    return get_internal_flag("use_rust") and _loopflow_engine_module() is not None


def _print_backend_once() -> None:
    """Print backend info once per session (grey text)."""
    global _backend_printed
    if _backend_printed:
        return
    _backend_printed = True

    import sys

    backend = "loopflow_engine (rust)" if _use_rust() else "python"
    if sys.stderr.isatty():
        sys.stderr.write(f"\033[2m[git: {backend}]\033[0m\n")
    else:
        sys.stderr.write(f"[git: {backend}]\n")


# -----------------------------------------------------------------------------
# Rust implementations (via PyO3)
# -----------------------------------------------------------------------------


def _rebase_rust(worktree: Path, onto: str, base_commit: str | None = None) -> RebaseResult:
    engine = _loopflow_engine_module()
    try:
        result = engine.git.py_git_rebase(str(worktree), onto, base_commit)
        conflicts = None
        if result.conflicts:
            conflicts = [Path(p) for p in result.conflicts]
        return RebaseResult(
            success=result.success,
            conflicts=conflicts,
            new_head=result.new_head,
        )
    except Exception as e:
        raise GitError(str(e)) from None


def _push_rust(worktree: Path, force_with_lease: bool = False) -> None:
    engine = _loopflow_engine_module()
    try:
        engine.git.py_git_push(str(worktree), force_with_lease)
    except Exception as e:
        raise GitError(str(e)) from None


def _push_with_upstream_rust(worktree: Path, remote: str, branch: str) -> None:
    engine = _loopflow_engine_module()
    try:
        engine.git.py_git_push_with_upstream(str(worktree), remote, branch)
    except Exception as e:
        raise GitError(str(e)) from None


def _create_branch_rust(worktree: Path, name: str) -> BranchInfo:
    engine = _loopflow_engine_module()
    try:
        result = engine.git.py_git_create_branch(str(worktree), name)
        return BranchInfo(
            old_branch=result.old_branch,
            old_head=result.old_head,
            new_branch=result.new_branch,
        )
    except Exception as e:
        raise GitError(str(e)) from None


def _land_rust(worktree: Path, strategy: str, main_branch: str = "main") -> LandResult:
    engine = _loopflow_engine_module()
    try:
        result = engine.git.py_git_land(str(worktree), strategy, main_branch)
        return LandResult(
            merged_commit=result.merged_commit,
            branch_deleted=result.branch_deleted,
        )
    except Exception as e:
        raise GitError(str(e)) from None


def _get_default_branch_rust(repo: Path) -> str:
    engine = _loopflow_engine_module()
    try:
        return engine.git.py_git_get_default_branch(str(repo))
    except Exception as e:
        raise GitError(str(e)) from None


def _is_clean_rust(repo: Path) -> bool:
    engine = _loopflow_engine_module()
    try:
        return engine.git.py_git_is_clean(str(repo))
    except Exception as e:
        raise GitError(str(e)) from None


def _stage_all_rust(repo: Path) -> None:
    engine = _loopflow_engine_module()
    try:
        engine.git.py_git_stage_all(str(repo))
    except Exception as e:
        raise GitError(str(e)) from None


def _commit_rust(repo: Path, message: str) -> None:
    engine = _loopflow_engine_module()
    try:
        engine.git.py_git_commit(str(repo), message)
    except Exception as e:
        raise GitError(str(e)) from None


def _delete_remote_branch_rust(repo: Path, remote: str, branch: str) -> None:
    engine = _loopflow_engine_module()
    try:
        engine.git.py_git_delete_remote_branch(str(repo), remote, branch)
    except Exception as e:
        raise GitError(str(e)) from None


def _delete_local_branch_rust(repo: Path, branch: str) -> None:
    engine = _loopflow_engine_module()
    try:
        engine.git.py_git_delete_local_branch(str(repo), branch)
    except Exception as e:
        raise GitError(str(e)) from None


def _pr_exists_rust(repo: Path) -> bool:
    engine = _loopflow_engine_module()
    try:
        return engine.git.py_git_pr_exists(str(repo))
    except Exception as e:
        raise GitError(str(e)) from None


def _pr_create_draft_rust(repo: Path) -> str:
    engine = _loopflow_engine_module()
    try:
        return engine.git.py_git_pr_create_draft(str(repo))
    except Exception as e:
        raise GitError(str(e)) from None


def _pr_merge_squash_auto_rust(repo: Path) -> None:
    engine = _loopflow_engine_module()
    try:
        engine.git.py_git_pr_merge_squash_auto(str(repo))
    except Exception as e:
        raise GitError(str(e)) from None


def _sync_main_rust(repo: Path, main_branch: str) -> bool:
    engine = _loopflow_engine_module()
    try:
        return engine.git.py_git_sync_main(str(repo), main_branch)
    except Exception as e:
        raise GitError(str(e)) from None


def _worktree_remove_rust(repo: Path, path: Path) -> None:
    engine = _loopflow_engine_module()
    try:
        engine.git.py_git_worktree_remove(str(repo), str(path))
    except Exception as e:
        raise GitError(str(e)) from None


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


def _get_default_branch_python(repo_root: Path) -> str:
    result = subprocess.run(
        ["git", "symbolic-ref", "--quiet", "--short", "refs/remotes/origin/HEAD"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0 and result.stdout.strip():
        return result.stdout.strip().split("/", 1)[-1]
    return "main"


def _is_clean_python(repo_root: Path) -> bool:
    result = subprocess.run(
        ["git", "status", "--porcelain"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0 and not result.stdout.strip()


def _stage_all_python(repo_root: Path) -> None:
    result = subprocess.run(["git", "add", "-A"], cwd=repo_root, capture_output=True, text=True)
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "git add failed")


def _commit_python(repo_root: Path, message: str) -> None:
    result = subprocess.run(
        ["git", "commit", "-m", message],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "git commit failed")


def _delete_remote_branch_python(repo_root: Path, remote: str, branch: str) -> None:
    result = subprocess.run(
        ["git", "push", remote, "--delete", branch],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "delete remote branch failed")


def _delete_local_branch_python(repo_root: Path, branch: str) -> None:
    result = subprocess.run(
        ["git", "branch", "-D", branch],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "delete local branch failed")


def _pr_exists_python(repo_root: Path) -> bool:
    result = subprocess.run(
        ["gh", "pr", "view", "--json", "state", "-q", ".state"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0 and bool(result.stdout.strip())


def _pr_create_draft_python(repo_root: Path) -> str:
    result = subprocess.run(
        ["gh", "pr", "create", "--draft", "--fill"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "create draft PR failed")
    return result.stdout.strip()


def _pr_merge_squash_auto_python(repo_root: Path) -> None:
    result = subprocess.run(
        ["gh", "pr", "merge", "--squash", "--auto"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "enable auto-merge failed")


def _sync_main_python(repo_root: Path, main_branch: str) -> bool:
    fetch_result = subprocess.run(
        ["git", "fetch", "origin", main_branch],
        cwd=repo_root,
        capture_output=True,
    )
    if fetch_result.returncode != 0:
        return False

    current_branch = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo_root,
        capture_output=True,
        text=True,
    ).stdout.strip()
    if current_branch != main_branch:
        return True

    if not _is_clean_python(repo_root):
        return False

    result = subprocess.run(
        ["git", "reset", "--hard", f"origin/{main_branch}"],
        cwd=repo_root,
        capture_output=True,
    )
    return result.returncode == 0


def _worktree_remove_python(repo_root: Path, path: Path) -> None:
    result = subprocess.run(
        ["git", "worktree", "remove", "--force", str(path)],
        cwd=repo_root,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "worktree remove failed")


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


def push_with_upstream(worktree: Path, remote: str, branch: str) -> None:
    _print_backend_once()
    if _use_rust():
        return _push_with_upstream_rust(worktree, remote, branch)
    result = subprocess.run(
        ["git", "push", "-u", remote, branch],
        cwd=worktree,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise GitError(result.stderr.strip() or "Push with upstream failed")


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


def get_default_branch(repo_root: Path) -> str:
    _print_backend_once()
    if _use_rust():
        return _get_default_branch_rust(repo_root)
    return _get_default_branch_python(repo_root)


def is_clean(repo_root: Path) -> bool:
    _print_backend_once()
    if _use_rust():
        return _is_clean_rust(repo_root)
    return _is_clean_python(repo_root)


def stage_all(repo_root: Path) -> None:
    _print_backend_once()
    if _use_rust():
        return _stage_all_rust(repo_root)
    return _stage_all_python(repo_root)


def commit(repo_root: Path, message: str) -> None:
    _print_backend_once()
    if _use_rust():
        return _commit_rust(repo_root, message)
    return _commit_python(repo_root, message)


def delete_remote_branch(repo_root: Path, remote: str, branch: str) -> None:
    _print_backend_once()
    if _use_rust():
        return _delete_remote_branch_rust(repo_root, remote, branch)
    return _delete_remote_branch_python(repo_root, remote, branch)


def delete_local_branch(repo_root: Path, branch: str) -> None:
    _print_backend_once()
    if _use_rust():
        return _delete_local_branch_rust(repo_root, branch)
    return _delete_local_branch_python(repo_root, branch)


def pr_exists(repo_root: Path) -> bool:
    _print_backend_once()
    if _use_rust():
        return _pr_exists_rust(repo_root)
    return _pr_exists_python(repo_root)


def pr_create_draft(repo_root: Path) -> str:
    _print_backend_once()
    if _use_rust():
        return _pr_create_draft_rust(repo_root)
    return _pr_create_draft_python(repo_root)


def pr_merge_squash_auto(repo_root: Path) -> None:
    _print_backend_once()
    if _use_rust():
        return _pr_merge_squash_auto_rust(repo_root)
    return _pr_merge_squash_auto_python(repo_root)


def sync_main(repo_root: Path, main_branch: str) -> bool:
    _print_backend_once()
    if _use_rust():
        return _sync_main_rust(repo_root, main_branch)
    return _sync_main_python(repo_root, main_branch)


def worktree_remove(repo_root: Path, path: Path) -> None:
    _print_backend_once()
    if _use_rust():
        return _worktree_remove_rust(repo_root, path)
    return _worktree_remove_python(repo_root, path)
