#!/usr/bin/env python3
"""Check Swift multiplatform boundaries for this branch."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent


def _resolve_main_ref() -> str:
    for ref in ("main", "origin/main"):
        result = subprocess.run(
            ["git", "rev-parse", "--verify", ref],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        if result.returncode == 0:
            return ref
    return "main"


MAIN_REF = _resolve_main_ref()

MAC_ONLY_IMPORTS = {
    "AppKit",
    "Cocoa",
    "Carbon",
    "ApplicationServices",
    "IOKit",
    "Metal",
    "GhosttyKit",
}

APP_WIRING_FILES = {
    "swift/Concerto/ConcertoApp.swift",
}

PLATFORM_PREFIXES = (
    "swift/Concerto/Platform/",
)


def _run_git_diff(*paths: str) -> str:
    cmd = ["git", "diff", "--no-color", "--unified=0", f"{MAIN_REF}...HEAD", "--", *paths]
    result = subprocess.run(
        cmd,
        cwd=REPO_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "git diff failed")
    return result.stdout


def _parse_added_lines(diff_text: str) -> list[tuple[str, str]]:
    file_path = ""
    added: list[tuple[str, str]] = []

    for raw_line in diff_text.splitlines():
        if raw_line.startswith("+++ b/"):
            file_path = raw_line.removeprefix("+++ b/")
            continue
        if not raw_line.startswith("+") or raw_line.startswith("+++"):
            continue
        added.append((file_path, raw_line[1:]))

    return added


def _is_whole_file_platform_gate(path: str) -> bool:
    file_path = REPO_ROOT / path
    if not file_path.exists():
        return False

    raw_lines = file_path.read_text(encoding="utf-8").splitlines()
    non_empty = [line.strip() for line in raw_lines if line.strip()]
    if len(non_empty) < 2:
        return False

    if not non_empty[0].startswith("#if "):
        return False
    if non_empty[-1] != "#endif":
        return False

    return True


def _check_loopflowcore_imports() -> list[str]:
    violations: list[str] = []
    diff_text = _run_git_diff("swift/LoopflowCore")

    for file_path, line in _parse_added_lines(diff_text):
        if not file_path.startswith("swift/LoopflowCore/") or not file_path.endswith(".swift"):
            continue
        match = re.match(r"\s*import\s+([A-Za-z0-9_]+)", line)
        if not match:
            continue
        module = match.group(1)
        if module in MAC_ONLY_IMPORTS:
            violations.append(f"{file_path}: added macOS-only import `{module}`")

    return violations


def _check_new_if_boundaries() -> list[str]:
    violations: list[str] = []
    diff_text = _run_git_diff("swift")

    for file_path, line in _parse_added_lines(diff_text):
        if not file_path.endswith(".swift"):
            continue
        if not re.match(r"\s*#if\b", line):
            continue

        if file_path.startswith("swift/LoopflowCore/"):
            violations.append(f"{file_path}: new `#if` in LoopflowCore shared code")
            continue
        if file_path in APP_WIRING_FILES:
            continue
        if file_path.startswith(PLATFORM_PREFIXES):
            continue
        if _is_whole_file_platform_gate(file_path):
            continue

        violations.append(f"{file_path}: new non-shell `#if`")

    return violations


def main() -> int:
    violations = [*_check_loopflowcore_imports(), *_check_new_if_boundaries()]

    if not violations:
        print("Swift multiplatform boundary checks passed.")
        return 0

    print("Swift multiplatform boundary checks failed:\n")
    for violation in violations:
        print(f"- {violation}")
    print("\nAllowed: app wiring, Platform/ shell files, or whole-file platform gates.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
