#!/usr/bin/env python3
"""Changed-aware test runner.

Runs only the CI suites the branch actually touches, so the iterative
`lf gate` loop (rebase -> gate -> bugfix -> gate) doesn't pay for the whole
matrix every pass. Stdlib only.

    uv run python scripts/test.py            # run suites the branch touched
    uv run python scripts/test.py --all      # run every suite
    uv run python scripts/test.py --list     # print the plan, run nothing

Suites mirror the jobs in .github/workflows/ci.yml. Slow suites (loopflow,
e2e) stay off in changed-mode unless forced with --all or their own flag,
since they dominate wall-clock time.
"""

from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Optional

REPO_ROOT = Path(__file__).resolve().parent.parent


# --- Changed files -------------------------------------------------------


def _ref_exists(ref: str) -> bool:
    result = subprocess.run(
        ["git", "rev-parse", "--verify", "--quiet", ref],
        cwd=REPO_ROOT,
        capture_output=True,
    )
    return result.returncode == 0


def _resolve_base(base_arg: Optional[str]) -> str:
    if base_arg:
        return base_arg
    for candidate in ("origin/main", "main"):
        if _ref_exists(candidate):
            return candidate
    return "main"


def changed_files(base: str) -> list[str]:
    """Paths (repo-relative) changed on this branch vs the merge-base with base.

    Includes committed, staged, unstaged, and untracked files, so the dev
    loop sees work-in-progress the same way a reviewer eventually will.
    """
    merge_base = subprocess.run(
        ["git", "merge-base", "HEAD", base],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )
    diff_ref = merge_base.stdout.strip() if merge_base.returncode == 0 else base

    diff = subprocess.run(
        ["git", "diff", "--name-only", diff_ref],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    untracked = subprocess.run(
        ["git", "ls-files", "--others", "--exclude-standard"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    paths = set(diff.stdout.split()) | set(untracked.stdout.split())
    return sorted(p for p in paths if p)


def _touches(changed: list[str], *prefixes: str) -> bool:
    return any(p.startswith(prefix) for p in changed for prefix in prefixes)


def _touches_exact(changed: list[str], *names: str) -> bool:
    names_set = set(names)
    return any(p in names_set for p in changed)


def _toplevel_py(changed: list[str]) -> bool:
    return any("/" not in p and p.endswith(".py") for p in changed)


# --- Suites --------------------------------------------------------------


@dataclass
class Command:
    argv: list[str]
    cwd: Path
    label: str


@dataclass
class Suite:
    name: str
    slow: bool
    trigger_desc: str
    match: Callable[[list[str]], bool]
    build: Callable[[list[str]], list[Command]]


def _rust_commands(_changed: list[str]) -> list[Command]:
    if shutil.which("cargo-nextest"):
        argv = ["cargo", "nextest", "run", "--all"]
    else:
        argv = ["cargo", "test", "--all"]
    return [Command(argv, REPO_ROOT, "rust")]


def _python_commands(changed: list[str]) -> list[Command]:
    test_files = [
        p
        for p in changed
        if p.startswith("python/tests/") and Path(p).name.startswith("test_") and p.endswith(".py")
    ]
    touches_source = (
        any(p.startswith("python/") and p not in test_files for p in changed)
        or _toplevel_py(changed)
        or _touches_exact(changed, "pyproject.toml", "uv.lock")
    )
    if test_files and not touches_source:
        argv = ["uv", "run", "pytest", *test_files]
    else:
        argv = ["uv", "run", "pytest", "python/tests/"]
    return [Command(argv, REPO_ROOT, "python")]


def _website_commands(_changed: list[str]) -> list[Command]:
    return [
        Command(
            ["uv", "run", "python", "dev.py", "test"],
            REPO_ROOT / "website",
            "website",
        )
    ]


def _swift_commands(_changed: list[str]) -> list[Command]:
    return [
        Command(
            ["swift", "test", "--package-path", "swift", "-Xswiftc", "-gnone"],
            REPO_ROOT,
            "swift",
        )
    ]


def _loopflow_commands(_changed: list[str]) -> list[Command]:
    swift_dir = REPO_ROOT / "swift"
    return [
        Command(["xcodegen", "generate"], swift_dir, "xcodegen"),
        Command(
            [
                "xcodebuild",
                "build-for-testing",
                "-project",
                "LoopflowSwift.xcodeproj",
                "-scheme",
                "LoopflowMac",
                "-destination",
                "platform=macOS",
                "-derivedDataPath",
                "DerivedData",
                "CODE_SIGNING_ALLOWED=NO",
                "CODE_SIGNING_REQUIRED=NO",
            ],
            swift_dir,
            "xcodebuild",
        ),
    ]


def _e2e_commands(_changed: list[str]) -> list[Command]:
    return [
        Command(["tests/e2e/test_smoke.sh"], REPO_ROOT, "e2e-smoke"),
    ]


# Ordered fast -> slow. Slow suites are gated behind --all / their own flag.
SUITES: list[Suite] = [
    Suite(
        name="python",
        slow=False,
        trigger_desc="python/ or top-level *.py",
        match=lambda c: (
            _touches(c, "python/")
            or _toplevel_py(c)
            or _touches_exact(c, "pyproject.toml", "uv.lock")
        ),
        build=_python_commands,
    ),
    Suite(
        name="rust",
        slow=False,
        trigger_desc="rust/ or Cargo.toml/lock",
        match=lambda c: _touches(c, "rust/") or _touches_exact(c, "Cargo.toml", "Cargo.lock"),
        build=_rust_commands,
    ),
    Suite(
        name="website",
        slow=False,
        trigger_desc="website/ or docs/",
        match=lambda c: _touches(c, "website/", "docs/"),
        build=_website_commands,
    ),
    Suite(
        name="swift",
        slow=False,
        trigger_desc="swift/",
        match=lambda c: _touches(c, "swift/"),
        build=_swift_commands,
    ),
    Suite(
        name="e2e",
        slow=True,
        trigger_desc="lfd http or lfdb or tests/e2e/",
        match=lambda c: _touches(
            c,
            "rust/loopflow/src/lfd/http",
            "rust/loopflow/src/lfdb",
            "tests/e2e/",
        ),
        build=_e2e_commands,
    ),
    Suite(
        name="loopflow",
        slow=True,
        trigger_desc="Loopflow app/UI (swift/Loopflow, project.yml)",
        match=lambda c: (
            _touches(c, "swift/LoopflowMac/", "swift/LoopflowUITests/")
            or _touches_exact(c, "swift/project.yml")
        ),
        build=_loopflow_commands,
    ),
]


# --- Planning ------------------------------------------------------------


@dataclass
class Plan:
    suite: Suite
    run: bool
    reason: str
    commands: list[Command]


def build_plan(changed: list[str], run_all: bool, forced: set[str]) -> list[Plan]:
    plans: list[Plan] = []
    for suite in SUITES:
        matched = suite.match(changed)
        if run_all:
            plans.append(Plan(suite, True, "all suites (--all)", suite.build(changed)))
            continue
        if suite.name in forced:
            plans.append(Plan(suite, True, f"forced (--{suite.name})", suite.build(changed)))
            continue
        if not matched:
            plans.append(Plan(suite, False, f"skipped (no {suite.trigger_desc} changes)", []))
            continue
        if suite.slow:
            plans.append(
                Plan(
                    suite,
                    False,
                    f"skipped (slow; use --all or --{suite.name})",
                    [],
                )
            )
            continue
        plans.append(Plan(suite, True, "matched (changed paths)", suite.build(changed)))
    return plans


def _fmt_cmd(cmd: Command) -> str:
    rel = cmd.cwd.relative_to(REPO_ROOT) if cmd.cwd != REPO_ROOT else Path(".")
    prefix = "" if rel == Path(".") else f"cd {rel} && "
    return prefix + " ".join(cmd.argv)


def print_plan(plans: list[Plan], changed: list[str]) -> None:
    print(f"Changed files: {len(changed)}")
    for path in changed:
        print(f"  {path}")
    print()
    print("Plan:")
    for plan in plans:
        mark = "RUN " if plan.run else "SKIP"
        print(f"  {mark} {plan.suite.name:<9} {plan.reason}")
        if plan.run:
            for cmd in plan.commands:
                print(f"         $ {_fmt_cmd(cmd)}")


# --- Running -------------------------------------------------------------


def _run_suite(plan: Plan) -> bool:
    bar = "=" * 72
    print(f"\n{bar}")
    print(f" SUITE: {plan.suite.name}  ({plan.reason})")
    print(bar, flush=True)
    for cmd in plan.commands:
        print(f"$ {_fmt_cmd(cmd)}", flush=True)
        result = subprocess.run(cmd.argv, cwd=cmd.cwd)
        if result.returncode != 0:
            print(
                f"\n[{plan.suite.name}] command failed ({cmd.label}, exit {result.returncode})",
                flush=True,
            )
            return False
    return True


def run_plans(plans: list[Plan]) -> int:
    results: dict[str, bool] = {}
    for plan in plans:
        if plan.run:
            results[plan.suite.name] = _run_suite(plan)

    print("\n" + "=" * 72)
    print(" SUMMARY")
    print("=" * 72)
    for plan in plans:
        if plan.run:
            status = "PASS" if results[plan.suite.name] else "FAIL"
        else:
            status = "----"
        detail = plan.reason if not plan.run else ""
        print(f"  {status}  {plan.suite.name:<9} {detail}")

    passed = [name for name, ok in results.items() if ok]
    failed = [name for name, ok in results.items() if not ok]
    print()
    if not results:
        print("No suites ran (nothing changed). Use --all to force the full matrix.")
        return 0
    if failed:
        print(f"Result: FAIL ({len(passed)} passed, {len(failed)} failed: {', '.join(failed)})")
        return 1
    print(f"Result: PASS ({len(passed)} suites)")
    return 0


# --- CLI -----------------------------------------------------------------


def _parse_args(argv: Optional[list[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="scripts/test.py",
        description="Run only the CI suites the branch changed.",
    )
    parser.add_argument("--all", action="store_true", help="run every suite, slow ones included")
    parser.add_argument(
        "--base",
        metavar="REF",
        help="base ref to diff against (default: origin/main, then main)",
    )
    parser.add_argument(
        "--list",
        dest="list_only",
        action="store_true",
        help="print the plan and exit without running",
    )
    for suite in SUITES:
        parser.add_argument(
            f"--{suite.name}",
            action="store_true",
            help=f"force the {suite.name} suite on",
        )
    return parser.parse_args(argv)


def main(argv: Optional[list[str]] = None) -> int:
    args = _parse_args(argv)
    forced = {suite.name for suite in SUITES if getattr(args, suite.name)}

    base = _resolve_base(args.base)
    changed = changed_files(base)
    plans = build_plan(changed, args.all, forced)

    if args.list_only:
        print(f"(base: {base})")
        print_plan(plans, changed)
        return 0

    return run_plans(plans)


if __name__ == "__main__":
    sys.exit(main())
