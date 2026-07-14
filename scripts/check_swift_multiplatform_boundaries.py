#!/usr/bin/env python3
"""Check Swift multiplatform boundaries for this branch."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent

MAC_ONLY_IMPORTS = {
    "AppKit",
    "Cocoa",
    "Carbon",
    "ApplicationServices",
    "IOKit",
    "Metal",
    "GhosttyKit",
}

# The shared library must build on every platform; the per-platform app targets
# are single-platform shells where `#if` and platform-only imports are expected.
SHARED_PREFIX = "swift/Loopflow/"
PLATFORM_PREFIXES = ("swift/LoopflowMac/",)


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
    raise RuntimeError("cannot find main branch as 'main' or 'origin/main'")


def _run_git_diff(*paths: str) -> str:
    base = _resolve_main_ref()
    cmd = ["git", "diff", "--no-color", "--unified=0", base, "--", *paths]
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
    # A file-header comment block may precede the gate and a trailing comment may
    # follow #endif; neither changes that the file is wholly platform-gated.
    code = [
        stripped
        for line in raw_lines
        if (stripped := line.strip()) and not stripped.startswith("//")
    ]
    if len(code) < 2:
        return False

    return code[0].startswith("#if ") and code[-1] == "#endif"


def _check_shared_imports() -> list[str]:
    violations: list[str] = []
    # Diff the whole swift/ tree (not just the shared dir) so git can pair a
    # moved file with its old path — a narrow pathspec hides the rename and
    # reports every line of a moved file as newly added.
    diff_text = _run_git_diff("swift")

    for file_path, line in _parse_added_lines(diff_text):
        if not file_path.startswith(SHARED_PREFIX) or not file_path.endswith(".swift"):
            continue
        # A wholly platform-gated file compiles empty off-platform, so its
        # platform-only imports are safe.
        if _is_whole_file_platform_gate(file_path):
            continue
        match = re.match(r"\s*import\s+([A-Za-z0-9_]+)", line)
        if not match:
            continue
        module = match.group(1)
        if module in MAC_ONLY_IMPORTS:
            violations.append(f"{file_path}: added macOS-only import `{module}`")

    return violations


def _platform_if_has_fallback(path: str, if_line: str) -> bool:
    """A shared-code `#if os(...)` is multiplatform-safe when balanced by an
    `#else`/`#elseif`, so both platforms compile. Locate the block opened by
    `if_line` and report whether it carries a fallback at the same nesting depth.
    A fallback-less split (the real hazard: a symbol undefined off-platform)
    returns False.
    """
    file_path = REPO_ROOT / path
    if not file_path.exists():
        return False

    target = if_line.strip()
    depth = 0
    for line in file_path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if depth == 0:
            if stripped == target:
                depth = 1
            continue
        if stripped.startswith("#if"):
            depth += 1
        elif stripped.startswith("#endif"):
            depth -= 1
            if depth == 0:
                return False
        elif (stripped.startswith("#else") or stripped.startswith("#elseif")) and depth == 1:
            return True
    return False


def _check_new_if_boundaries() -> list[str]:
    violations: list[str] = []
    diff_text = _run_git_diff("swift")

    for file_path, line in _parse_added_lines(diff_text):
        if not file_path.endswith(".swift"):
            continue
        if not re.match(r"\s*#if\b", line):
            continue

        # Single-platform app shells: `#if` is expected.
        if file_path.startswith(PLATFORM_PREFIXES):
            continue

        if file_path.startswith(SHARED_PREFIX):
            # canImport is a capability check, not a platform split — allow it.
            if re.match(r"\s*#if\s+canImport\(", line):
                continue
            # Any `#if` balanced by an `#else`/`#elseif` compiles on every target
            # (covers os() splits and build-config gates like `#if SWIFT_PACKAGE`);
            # so does a wholly platform-gated file (empty off-platform).
            if _platform_if_has_fallback(file_path, line):
                continue
            if _is_whole_file_platform_gate(file_path):
                continue
            violations.append(f"{file_path}: new `#if` in shared code")
            continue

        if _is_whole_file_platform_gate(file_path):
            continue

        violations.append(f"{file_path}: new non-shell `#if`")

    return violations


def main() -> int:
    violations = [*_check_shared_imports(), *_check_new_if_boundaries()]

    if not violations:
        print("Swift multiplatform boundary checks passed.")
        return 0

    print("Swift multiplatform boundary checks failed:\n")
    for violation in violations:
        print(f"- {violation}")
    print("\nAllowed: single-platform app shells, or whole-file platform gates.")
    return 1


if __name__ == "__main__":
    sys.exit(main())
