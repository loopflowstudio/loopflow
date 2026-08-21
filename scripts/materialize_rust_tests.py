#!/usr/bin/env python3
"""Run Rust tests against CI's materialized draft-migration graph.

The active checkout may contain committed, staged, unstaged, and untracked
work. Mirror that exact tree into a disposable Git worktree, materialize draft
migrations there, and run the requested command without dirtying the checkout.
"""

from __future__ import annotations

import argparse
import os
import shutil
import signal
import subprocess
import sys
import tempfile
from contextlib import contextmanager
from pathlib import Path
from types import FrameType
from typing import Iterator

REPO_ROOT = Path(__file__).resolve().parent.parent
AMBIENT_WORK_AUTHORITY = (
    "LF_AGENT_INVOCATION_ID",
    "LF_CONTROL_BIN",
    "LF_PROCESS_ID",
    "LF_RUN_CONTEXT",
    "LF_RUN_ID",
    "LF_RUN_LEASE",
    "LF_TRACE_ID",
    "LF_WAVE_ID",
    "LF_ACCOUNT_LEASE",
    "LF_WORKTREE_WRITER_ID",
)


def _run_git(repo_root: Path, *arguments: str, **kwargs: object) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *arguments], cwd=repo_root, **kwargs)


def _copy_untracked_files(repo_root: Path, worktree: Path) -> None:
    listed = _run_git(
        repo_root,
        "ls-files",
        "--others",
        "--exclude-standard",
        "-z",
        check=True,
        capture_output=True,
    )
    for raw_path in (path for path in listed.stdout.split(b"\0") if path):
        relative = Path(os.fsdecode(raw_path))
        source = repo_root / relative
        destination = worktree / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        if source.is_symlink():
            destination.symlink_to(os.readlink(source))
        else:
            shutil.copy2(source, destination)


def _remove_worktree(repo_root: Path, worktree: Path, temp_root: Path) -> None:
    removed = _run_git(
        repo_root,
        "worktree",
        "remove",
        "--force",
        str(worktree),
        capture_output=True,
        text=True,
    )
    if removed.returncode != 0:
        shutil.rmtree(worktree, ignore_errors=True)
        _run_git(repo_root, "worktree", "prune", check=True)
    shutil.rmtree(temp_root, ignore_errors=True)

    registered = _run_git(
        repo_root,
        "worktree",
        "list",
        "--porcelain",
        check=True,
        capture_output=True,
        text=True,
    )
    if str(worktree) in registered.stdout:
        raise RuntimeError(f"failed to remove disposable worktree {worktree}")


@contextmanager
def _exact_tree_worktree(repo_root: Path) -> Iterator[Path]:
    temp_root = Path(tempfile.mkdtemp(prefix="loopflow-materialized-"))
    worktree = temp_root / "worktree"
    try:
        _run_git(
            repo_root,
            "worktree",
            "add",
            "--detach",
            str(worktree),
            "HEAD",
            check=True,
        )

        patch = _run_git(
            repo_root,
            "diff",
            "--binary",
            "HEAD",
            check=True,
            capture_output=True,
        ).stdout
        if patch:
            subprocess.run(
                ["git", "apply", "--binary", "--whitespace=nowarn", "-"],
                cwd=worktree,
                input=patch,
                check=True,
            )
        _copy_untracked_files(repo_root, worktree)
        yield worktree
    finally:
        _remove_worktree(repo_root, worktree, temp_root)


def _workspace_version(worktree: Path) -> str:
    in_workspace_package = False
    for line in (worktree / "Cargo.toml").read_text().splitlines():
        stripped = line.strip()
        if stripped == "[workspace.package]":
            in_workspace_package = True
            continue
        if stripped.startswith("["):
            in_workspace_package = False
        if not in_workspace_package or not stripped.startswith("version"):
            continue
        key, separator, value = stripped.partition("=")
        if separator and key.strip() == "version":
            return value.strip().strip('"')
    raise ValueError("Cargo.toml has no [workspace.package] version")


def _terminate(signum: int, _frame: FrameType | None) -> None:
    raise SystemExit(128 + signum)


def _parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    if args.command[:1] == ["--"]:
        args.command = args.command[1:]
    if not args.command:
        parser.error("provide a command after --")
    return args


def main(argv: list[str] | None = None) -> int:
    args = _parse_args(argv)
    previous_sigterm = signal.signal(signal.SIGTERM, _terminate)
    try:
        with _exact_tree_worktree(REPO_ROOT) as worktree:
            version = _workspace_version(worktree)
            materialized = subprocess.run(
                [
                    sys.executable,
                    "scripts/canonicalize_migrations.py",
                    version,
                    "--materialize-for-tests",
                ],
                cwd=worktree,
            )
            if materialized.returncode != 0:
                return materialized.returncode

            environment = os.environ.copy()
            for name in AMBIENT_WORK_AUTHORITY:
                environment.pop(name, None)
            environment["LF_CONTROL_HOME"] = str(worktree / ".lf-test-control")
            environment.pop("LF_CONTROL_DB_PATH", None)
            environment.setdefault("CARGO_TARGET_DIR", str(REPO_ROOT / "target"))
            return subprocess.run(args.command, cwd=worktree, env=environment).returncode
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as exc:
        print(f"materialized Rust test setup failed: {exc}", file=sys.stderr)
        return 1
    finally:
        signal.signal(signal.SIGTERM, previous_sigterm)


if __name__ == "__main__":
    raise SystemExit(main())
