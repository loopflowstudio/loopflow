"""Subprocess wrapper for lf-core git operations."""

import json
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Any


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


def _run_lf_core(args: list[str]) -> Any:
    try:
        result = subprocess.run(args, capture_output=True, text=True)
    except FileNotFoundError as err:
        raise GitError(f"lf-core not found: {err}") from None
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


def rebase(worktree: Path, onto: str, base_commit: str | None = None) -> RebaseResult:
    args = ["lf-core", "rebase", "--worktree", str(worktree), "--onto", onto]
    if base_commit:
        args.extend(["--base-commit", base_commit])
    data = _run_lf_core(args)
    conflicts = None
    if data.get("conflicts"):
        conflicts = [Path(path) for path in data["conflicts"]]
    return RebaseResult(
        success=data["success"],
        conflicts=conflicts,
        new_head=data.get("new_head"),
    )


def create_branch(worktree: Path, name: str) -> BranchInfo:
    data = _run_lf_core(["lf-core", "branch", "--worktree", str(worktree), "--name", name])
    return BranchInfo(**data)


def push(worktree: Path, force_with_lease: bool = False) -> None:
    args = ["lf-core", "push", "--worktree", str(worktree)]
    if force_with_lease:
        args.append("--force-with-lease")
    _run_lf_core(args)


def land(worktree: Path, strategy: str, main_branch: str = "main") -> LandResult:
    args = [
        "lf-core",
        "land",
        "--worktree",
        str(worktree),
        "--strategy",
        strategy,
        "--main-branch",
        main_branch,
    ]
    data = _run_lf_core(args)
    return LandResult(**data)
