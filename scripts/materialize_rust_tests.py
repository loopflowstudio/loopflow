#!/usr/bin/env python3
"""Run Rust tests against CI's materialized draft-migration graph.

The active checkout may contain committed, staged, unstaged, and untracked
work. Mirror that exact tree into a disposable Git worktree, materialize draft
migrations there, and run the requested command without dirtying the checkout.
"""

from __future__ import annotations

import argparse
import json
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
    parser.add_argument(
        "--prepare-cli", action="store_true", help="build the exact materialized fixture CLI once"
    )
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args(argv)
    if args.command[:1] == ["--"]:
        args.command = args.command[1:]
    if not args.command:
        parser.error("provide a command after --")
    return args


def _cargo_build_options(command: list[str]) -> list[str]:
    # Forward build selection, not test selection or harness arguments.
    parser = argparse.ArgumentParser(add_help=False, allow_abbrev=False)
    for name in ("config", "target", "target-dir"):
        parser.add_argument(f"--{name}", action="append", default=[])
    parser.add_argument("--features", "-F", action="append", default=[])
    profile_flag = "--cargo-profile" if "nextest" in command else "--profile"
    parser.add_argument(profile_flag, dest="profile", action="append", default=[])
    parser.add_argument("--jobs", "-j", "--build-jobs", dest="jobs")
    for name in ("release", "all-features", "no-default-features", "frozen"):
        parser.add_argument(f"--{name}", action="store_true")
    arguments = command[: command.index("--")] if "--" in command else command
    options, _ = parser.parse_known_args(arguments)
    forwarded = ["--jobs", options.jobs or "4"]
    if not options.profile and not options.release:
        forwarded.extend(["--profile", "test"])
    for name in ("config", "target", "target-dir", "profile", "features"):
        for value in getattr(options, name.replace("-", "_")):
            forwarded.extend([f"--{name}", value])
    for name in ("release", "all-features", "no-default-features", "frozen"):
        if getattr(options, name.replace("-", "_")):
            forwarded.append(f"--{name}")
    return forwarded


def _verification_target(value: str) -> str:
    return str((REPO_ROOT / value).resolve() / "materialized-tests")


def _prepare_cli(worktree: Path, environment: dict[str, str], command: list[str]) -> str:
    toolchain = (
        command[1:2]
        if command[:1] == ["cargo"] and command[1:2] and command[1].startswith("+")
        else []
    )
    prepared = subprocess.run(
        [
            "cargo",
            *toolchain,
            "build",
            # Materialization can advance workspace versions in the disposable lockfile.
            "--offline",
            "-p",
            "loopflow",
            "--bin",
            "lf",
            # Keep verification's dependency graph instead of rebuilding the ordinary CLI graph.
            "--tests",
            "--message-format=json",
            *_cargo_build_options(command),
        ],
        cwd=worktree,
        env=environment,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        text=True,
        check=True,
    )
    executables = set()
    for line in prepared.stdout.splitlines():
        artifact = json.loads(line)
        if (
            artifact.get("reason") == "compiler-artifact"
            and artifact["target"]["name"] == "lf"
            and "bin" in artifact["target"]["kind"]
            and not artifact["profile"]["test"]
            and artifact.get("executable")
        ):
            executables.add(artifact["executable"])
    if len(executables) != 1:
        raise RuntimeError(f"expected one prepared lf executable, found {len(executables)}")
    executable = Path(executables.pop())
    if not executable.is_file() or not os.access(executable, os.X_OK):
        raise RuntimeError(f"prepared lf executable is unavailable: {executable}")
    return str(executable)


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

            environment = {
                name: value for name, value in os.environ.items() if not name.startswith("LF_")
            }
            environment["LF_HOME"] = str(worktree / ".lf-test-home")
            environment["CARGO_TARGET_DIR"] = _verification_target(
                environment.get("CARGO_TARGET_DIR", "target")
            )
            # Cargo's command-line target overrides its environment selection.
            for index, argument in enumerate(args.command):
                if argument == "--":
                    break
                if argument == "--target-dir":
                    args.command[index + 1] = _verification_target(args.command[index + 1])
                elif argument.startswith("--target-dir="):
                    args.command[index] = "--target-dir=" + _verification_target(
                        argument.split("=", 1)[1]
                    )
            if args.prepare_cli:
                environment.pop("LOOPFLOW_TEST_LF", None)
                environment["LOOPFLOW_BUILD_PROVENANCE"] = "development"
                environment["LOOPFLOW_MIGRATION_AUTHORITY"] = "validation_only"
                environment.pop("LOOPFLOW_RELEASE_TAG", None)
                environment["LOOPFLOW_TEST_LF"] = _prepare_cli(worktree, environment, args.command)
            return subprocess.run(
                args.command, cwd=worktree, env=environment, stdin=subprocess.DEVNULL
            ).returncode
    except (OSError, RuntimeError, ValueError, subprocess.CalledProcessError) as exc:
        print(f"materialized Rust test setup failed: {exc}", file=sys.stderr)
        return 1
    finally:
        signal.signal(signal.SIGTERM, previous_sigterm)


if __name__ == "__main__":
    raise SystemExit(main())
