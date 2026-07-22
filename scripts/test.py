#!/usr/bin/env python3
"""Changed-aware test runner.

Runs only the CI suites the branch actually touches, so the iterative
`lf gate` loop (rebase -> gate -> bugfix -> gate) doesn't pay for the whole
matrix every pass. Stdlib only.

    uv run python scripts/test.py            # run suites the branch touched
    uv run python scripts/test.py --reuse-passing  # reuse this exact tree's pass
    uv run python scripts/test.py --all      # run every suite
    uv run python scripts/test.py --list     # print the plan, run nothing

Suites mirror the jobs in .github/workflows/ci.yml. Slow suites (loopflow,
e2e) stay off in changed-mode unless forced with --all or their own flag,
since they dominate wall-clock time.
"""

from __future__ import annotations

import argparse
import fcntl
import hashlib
import json
import os
import platform
import resource as process_resource
import secrets
import shutil
import signal
import subprocess
import sys
import threading
import time
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import IO, Callable, Optional

REPO_ROOT = Path(__file__).resolve().parent.parent
XCODE_LOCAL_SIGNING = [
    "CODE_SIGNING_ALLOWED=YES",
    "CODE_SIGNING_REQUIRED=YES",
    "CODE_SIGN_STYLE=Manual",
    "CODE_SIGN_IDENTITY=-",
    "DEVELOPMENT_TEAM=",
]
XCODE_DERIVED_DATA = ".build/xcode-derived-data"

# Failure artifacts (phase logs, xcresult bundles) land here so a worker can
# repair the first red signal without reopening Xcode. Ignored (.lf/tmp/*).
GATE_ARTIFACT_ROOT = REPO_ROOT / ".lf" / "tmp" / "gate"
GATE_EVIDENCE_SCHEMA = 3
RESOURCE_SCRIPT = REPO_ROOT / "scripts" / "resource_envelope.py"
RESOURCE_POLICY_PATH = REPO_ROOT / "performance" / "budgets.json"
HOST_SECURITY_PROCESSES = {"amfid", "syspolicyd", "taskgated", "trustd"}


def _load_resource_policy() -> dict[str, object]:
    payload = json.loads(RESOURCE_POLICY_PATH.read_text(encoding="utf-8"))
    envelope = payload.get("resource_envelope")
    if not isinstance(envelope, dict):
        raise ValueError(f"resource_envelope is missing from {RESOURCE_POLICY_PATH}")
    return envelope


RESOURCE_ENVELOPE = _load_resource_policy()
MAX_PARALLEL_JOBS = int(RESOURCE_ENVELOPE["max_parallel_jobs"])


def _run_artifact_root() -> Path:
    """This invocation's artifact directory, unique per process.

    Scoping it by pid keeps successive gate runs from colliding — notably the
    UI-host `.xcresult`, which `xcodebuild test` refuses to overwrite (it exits
    64 on an existing `-resultBundlePath`). A fresh pid-scoped path per run means
    `--ui-host` can run back-to-back, which the 5/5 host proof requires.
    """
    return GATE_ARTIFACT_ROOT / f"run-{os.getpid()}"


# Per-phase wall-clock limits in seconds, keyed by Command.label. These kill
# hangs; they are not performance targets. An unlisted label falls back to
# DEFAULT_BUDGET_S.
PHASE_BUDGETS: dict[str, int] = {
    "rustfmt": 120,
    "clippy": 900,
    "rust": 1200,
    "python": 600,
    "website": 900,
    "swift": 1200,
    "swift-boundaries": 120,
    "swift-build": 900,
    "swift-surface": 900,
    "xcodegen": 180,
    "xcodebuild": 1200,
    "e2e-smoke": 600,
    "ui-host": 1200,
}
DEFAULT_BUDGET_S = 900
# Grace between SIGTERM and SIGKILL when a phase overruns its budget.
KILL_GRACE_S = 10


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


def _scripts_py(changed: list[str]) -> bool:
    return any(p.startswith("scripts/") and p.endswith(".py") for p in changed)


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
    # What a PASS on this suite actually proves. Printed in the summary so the
    # gate never over-claims (e.g. loopflow compiles the app; it does not run
    # hosted UI). None => a pass means the suite's commands passed, nothing more.
    proves: Optional[str] = None
    # A required gate that runs only when named explicitly (never under --all),
    # because it needs a permissioned host. Absence of that host is a failure,
    # never a silent skip.
    host_gate: bool = False
    # Runs before the suite's commands. A returned string is an immediate,
    # actionable failure (e.g. wrong platform, simulated capability gap).
    precheck: Optional[Callable[[], Optional[str]]] = None
    # Refines a raw command failure using its captured log — e.g. recognising a
    # UI-runner bootstrap failure and naming the missing capability.
    classify: Optional[Callable[[str], Optional[str]]] = None
    # Name of a machine-wide flock held for the suite's whole run. Suites that
    # drive machine-global facilities (UI automation) must not interleave:
    # overlapping hosted runs are how testmanagerd leaks Automation Mode
    # clients (release/UI_HOST_GATE.md). None => no lock.
    machine_lock: Optional[str] = None
    # Runs after the suite's commands, pass or fail. The gate must leave the
    # machine as it found it; a returned string fails a passing suite and is
    # appended to an already-failing one.
    postcheck: Optional[Callable[[], Optional[str]]] = None


def _rust_commands(_changed: list[str]) -> list[Command]:
    if shutil.which("cargo-nextest"):
        test_argv = [
            "cargo",
            "nextest",
            "run",
            "--all",
            "--build-jobs",
            str(MAX_PARALLEL_JOBS),
            "--test-threads",
            str(MAX_PARALLEL_JOBS),
        ]
    else:
        test_argv = [
            "cargo",
            "test",
            "--all",
            "--jobs",
            str(MAX_PARALLEL_JOBS),
            "--",
            "--test-threads",
            str(MAX_PARALLEL_JOBS),
        ]
    return [
        Command(["cargo", "fmt", "--all", "--", "--check"], REPO_ROOT, "rustfmt"),
        Command(
            [
                "cargo",
                "clippy",
                "--all-targets",
                "--jobs",
                str(MAX_PARALLEL_JOBS),
                "--",
                "-D",
                "warnings",
            ],
            REPO_ROOT,
            "clippy",
        ),
        Command(
            [
                "uv",
                "run",
                "python",
                "scripts/materialize_rust_tests.py",
                "--",
                *test_argv,
            ],
            REPO_ROOT,
            "rust",
        ),
    ]


def _python_commands(changed: list[str]) -> list[Command]:
    test_files = [
        p
        for p in changed
        if p.startswith("python/tests/")
        and Path(p).name.startswith("test_")
        and p.endswith(".py")
        and (REPO_ROOT / p).is_file()
    ]
    touches_source = (
        any(p.startswith("python/") and p not in test_files for p in changed)
        or _toplevel_py(changed)
        or _scripts_py(changed)
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
            [
                "swift",
                "test",
                "--package-path",
                "swift",
                "--jobs",
                str(MAX_PARALLEL_JOBS),
                "-Xswiftc",
                "-gnone",
            ],
            REPO_ROOT,
            "swift",
        ),
        Command(
            ["uv", "run", "python", "scripts/check_swift_multiplatform_boundaries.py"],
            REPO_ROOT,
            "swift-boundaries",
        ),
        Command(
            [
                "swift",
                "build",
                "--package-path",
                "swift",
                "--product",
                "LoopflowMac",
                "--jobs",
                str(MAX_PARALLEL_JOBS),
            ],
            REPO_ROOT,
            "swift-build",
        ),
        Command(
            ["scripts/prove_wave_surface_states.sh"],
            REPO_ROOT,
            "swift-surface",
        ),
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
                XCODE_DERIVED_DATA,
                "-jobs",
                str(MAX_PARALLEL_JOBS),
                "-disableAutomaticPackageResolution",
                *XCODE_LOCAL_SIGNING,
            ],
            swift_dir,
            "xcodebuild",
        ),
    ]


def _e2e_commands(_changed: list[str]) -> list[Command]:
    return [
        Command(["tests/e2e/test_smoke.sh"], REPO_ROOT, "e2e-smoke"),
    ]


# --- Required-host UI gate ----------------------------------------------
#
# The ordinary gate compiles the app but never runs a hosted UI test: the test
# host launches the real app and needs macOS UI-automation permission, which a
# headless/unpermissioned host lacks. Rather than silently skip UI behaviour,
# the real run lives here as a separately named REQUIRED gate: it runs only when
# invoked explicitly (`--ui-host`), never under `--all`, and its absence is a
# failure that names the missing capability, not a silent pass.

# Markers in xcodebuild output that mean the test *runner* failed to bootstrap
# (a capability gap) rather than a test assertion failing (a real red).
_UI_BOOTSTRAP_MARKERS = (
    "Test runner never began executing",
    "Failed to background test runner",
    "Failed to load the test bundle",
    "UI Testing Failure - App accessibility isn't loaded",
    "The test runner exited with code",
    "Timed out waiting",
    # The runner launches but never connects because UI automation is denied —
    # the signature on macOS 26 / Xcode 26 is a hung control session, not an
    # Apple-events denial. Without these two, a permission gap on this host
    # misreports as a raw red test (observed: run hung ~710s, exit 65).
    "hung before establishing connection",
    "initiating control session with daemon",
    "not permitted to send Apple events",
    "not authorized to send Apple events",
    "AutomationPermission",
    "TCC",
)

_UI_CAPABILITY_HELP = (
    "MISSING CAPABILITY: macOS UI-automation (Automation / Accessibility) "
    "permission for the test runner.\n"
    "NEXT ACTION: on the maintained UI host, grant the test runner Automation "
    "and Accessibility permission (System Settings > Privacy & Security), then "
    "re-run `uv run python scripts/test.py --ui-host`. "
    "See release/UI_HOST_GATE.md."
)


def _ui_host_commands(_changed: list[str]) -> list[Command]:
    swift_dir = REPO_ROOT / "swift"
    # Land the result bundle beside this run's phase logs, under the pid-scoped
    # artifact dir (release/UI_HOST_GATE.md). A fixed
    # path in derived data collided across runs — xcodebuild exits 64 rather
    # than overwrite an existing bundle, so the second `--ui-host` run onward
    # died before launching a test. This path is fresh per invocation.
    xcresult = _run_artifact_root() / "ui-host" / "ui-host.xcresult"
    return [
        Command(["xcodegen", "generate"], swift_dir, "xcodegen"),
        Command(
            [
                "xcodebuild",
                "test",
                "-project",
                "LoopflowSwift.xcodeproj",
                "-scheme",
                "LoopflowMac",
                "-destination",
                "platform=macOS",
                "-derivedDataPath",
                XCODE_DERIVED_DATA,
                "-disableAutomaticPackageResolution",
                "-only-testing:LoopflowUITests",
                "-resultBundlePath",
                str(xcresult),
                "-jobs",
                str(MAX_PARALLEL_JOBS),
                "-parallel-testing-worker-count",
                str(MAX_PARALLEL_JOBS),
                *XCODE_LOCAL_SIGNING,
            ],
            swift_dir,
            "ui-host",
        ),
    ]


def _ui_host_precheck() -> Optional[str]:
    """Fail fast before launching Xcode when the host obviously can't host the
    UI run: wrong OS, or a simulated capability gap for the proof harness."""
    if os.environ.get("LF_UI_HOST_SIMULATE_NO_PERMISSION"):
        return _UI_CAPABILITY_HELP
    if platform.system() != "Darwin":
        return (
            "MISSING CAPABILITY: the required UI host gate needs macOS; this "
            f"host is {platform.system()}.\n"
            "NEXT ACTION: run `uv run python scripts/test.py --ui-host` on the "
            "maintained macOS UI host. See release/UI_HOST_GATE.md."
        )
    return None


def _ui_host_classify(log_text: str) -> Optional[str]:
    """Refine a ui-host failure: a runner-bootstrap failure is a capability gap,
    not a red test. Return None to keep the raw failure (a genuine assertion)."""
    if any(marker in log_text for marker in _UI_BOOTSTRAP_MARKERS):
        return _UI_CAPABILITY_HELP
    return None


# Deliberately /tmp, not the per-worktree artifact root: UI automation is a
# machine-global facility (testmanagerd, Automation Mode), so runs launched
# from different worktrees must contend on the same path. flock releases on
# process death; /tmp clears on reboot.
_MACHINE_LOCK_DIR = Path("/tmp")


def _acquire_machine_lock(name: str) -> tuple[Optional[IO[str]], Optional[str]]:
    """Take the named machine-wide lock without blocking.

    Returns (handle, None) when acquired — closing the handle releases it — or
    (None, actionable failure) when another run holds it.
    """
    path = _MACHINE_LOCK_DIR / f"lf-{name}.lock"
    handle = open(path, "a+")
    try:
        fcntl.flock(handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        handle.seek(0)
        holder = handle.read().strip() or "unknown pid"
        handle.close()
        return None, (
            f"MACHINE LOCK HELD: another '{name}' run (pid {holder}) is live on "
            f"this machine ({path}). Hosted UI runs must not interleave — "
            "overlapping sessions leak Automation Mode clients. "
            "NEXT ACTION: let it finish (`lf ps` shows live runs), then rerun."
        )
    handle.seek(0)
    handle.truncate()
    handle.write(f"{os.getpid()}\n")
    handle.flush()
    return handle, None


# How long Automation Mode may stay enabled after the last session before the
# gate calls it a leak. Disable normally lands within a second of the final
# client releasing; the margin absorbs testmanagerd bookkeeping lag.
_AUTOMATION_SETTLE_S = 15

_UI_AUTOMATION_LEAK = (
    "AUTOMATION MODE LEAKED: macOS still reports Automation Mode ENABLED after "
    "the ui-host run. A UI-test runner died without releasing its automation "
    "client, so the 'Automation Running' banner will squat on this host until "
    "the count is repaired — SIP blocks deleting the state file or restarting "
    "automationmode-writer, even as root. NEXT ACTION: `killall testmanagerd` "
    "(user-owned; resets the stale client count), then rerun "
    "`uv run python scripts/test.py --ui-host` so one session ends cleanly. "
    "See release/UI_HOST_GATE.md."
)


def _automation_mode_enabled() -> bool:
    """True when `automationmodetool` reports Automation Mode enabled. False on
    a disabled report, a missing tool, or an unrecognised answer — only a
    positive ENABLED is worth failing a gate over."""
    tool = shutil.which("automationmodetool")
    if tool is None:
        return False
    result = subprocess.run([tool], capture_output=True, text=True)
    return "automation mode is enabled" in (result.stdout + result.stderr).lower()


def _ui_host_postcheck() -> Optional[str]:
    """The gate must leave the machine as it found it.

    Automation Mode is machine-global state owned by testmanagerd; a client
    that dies unobserved keeps it enabled forever, which surfaces to the
    operator as an unkillable 'Automation Running' banner (the ⌥⌘. it
    advertises targets a process that no longer exists). Poll briefly so a
    normally-settling disable doesn't read as a leak.
    """
    if platform.system() != "Darwin":
        return None
    deadline = time.monotonic() + _AUTOMATION_SETTLE_S
    while _automation_mode_enabled():
        if time.monotonic() >= deadline:
            return _UI_AUTOMATION_LEAK
        time.sleep(3)
    return None


# Ordered fast -> slow. Slow suites are gated behind --all / their own flag.
SUITES: list[Suite] = [
    Suite(
        name="python",
        slow=False,
        trigger_desc="python/, scripts/*.py, or top-level *.py",
        match=lambda c: (
            _touches(c, "python/")
            or _toplevel_py(c)
            or _scripts_py(c)
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
        trigger_desc="swift/ or the headless surface proof",
        match=lambda c: (
            _touches(c, "swift/") or _touches_exact(c, "scripts/prove_wave_surface_states.sh")
        ),
        build=_swift_commands,
    ),
    Suite(
        name="e2e",
        slow=True,
        trigger_desc="store, worktrees, or tests/e2e/",
        match=lambda c: _touches(
            c,
            "rust/loopflow/src/store",
            "rust/loopflow/src/engine/worktrees",
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
        proves=(
            "app + UI-test runners COMPILE (build-for-testing). Hosted UI "
            "behaviour is NOT run here; the required `--ui-host` gate owns it."
        ),
    ),
    Suite(
        name="ui-host",
        slow=True,
        trigger_desc="never auto-selected; explicit --ui-host only",
        match=lambda _c: False,
        build=_ui_host_commands,
        proves="hosted LoopflowUITests actually EXECUTE on a permissioned host.",
        host_gate=True,
        precheck=_ui_host_precheck,
        classify=_ui_host_classify,
        machine_lock="ui-host",
        postcheck=_ui_host_postcheck,
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
        if suite.name in forced:
            plans.append(Plan(suite, True, f"forced (--{suite.name})", suite.build(changed)))
            continue
        if suite.host_gate:
            # Required host gate: never auto-run (not even under --all); it
            # needs a permissioned host and is named explicitly.
            plans.append(
                Plan(suite, False, f"required host gate (run --{suite.name} on its host)", [])
            )
            continue
        if run_all:
            plans.append(Plan(suite, True, "all suites (--all)", suite.build([])))
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


def _budget_for(label: str) -> int:
    return PHASE_BUDGETS.get(label, DEFAULT_BUDGET_S)


def _plan_budget(plan: Plan) -> int:
    return sum(_budget_for(cmd.label) for cmd in plan.commands)


def print_plan(plans: list[Plan], changed: list[str]) -> None:
    print(f"Changed files: {len(changed)}")
    for path in changed:
        print(f"  {path}")
    print()
    print("Plan:")
    for plan in plans:
        mark = "RUN " if plan.run else "SKIP"
        budget = f"  [budget {_plan_budget(plan)}s]" if plan.run else ""
        print(f"  {mark} {plan.suite.name:<9} {plan.reason}{budget}")
        if plan.suite.proves:
            print(f"         proves: {plan.suite.proves}")
        if plan.run:
            for cmd in plan.commands:
                print(f"         $ {_fmt_cmd(cmd)}  (budget {_budget_for(cmd.label)}s)")


# --- Resource envelope ---------------------------------------------------


def _run_resource_check(recover: bool) -> tuple[Optional[dict[str, object]], Optional[str]]:
    argv = [sys.executable, str(RESOURCE_SCRIPT), "--json", "--repo", str(REPO_ROOT)]
    if recover:
        argv.append("--recover")
    try:
        result = subprocess.run(argv, cwd=REPO_ROOT, capture_output=True, text=True, timeout=60)
    except (OSError, subprocess.TimeoutExpired) as exc:
        return None, f"RESOURCE MEASUREMENT FAILED: {exc}"
    try:
        report = json.loads(result.stdout)
    except json.JSONDecodeError:
        detail = result.stderr.strip() or result.stdout.strip() or f"exit {result.returncode}"
        return None, f"RESOURCE MEASUREMENT FAILED: {detail}"
    if not isinstance(report, dict):
        return None, "RESOURCE MEASUREMENT FAILED: report is not a JSON object"
    if report.get("ok") is True and result.returncode == 0:
        return report, None
    after = report.get("after")
    issues = after.get("issues") if isinstance(after, dict) else None
    if not isinstance(issues, list) or not issues:
        detail = result.stderr.strip() or f"resource check exited {result.returncode}"
        return report, f"RESOURCE PRESSURE: {detail}"
    lines = ["RESOURCE PRESSURE: verification did not start inside its envelope."]
    for issue in issues:
        if not isinstance(issue, dict):
            continue
        lines.append(
            f"- {issue.get('code', 'unknown')} ({issue.get('owner', 'unknown')}): "
            f"{issue.get('detail', 'no detail')}"
        )
        lines.append(f"  NEXT ACTION: {issue.get('action', 'inspect the named source')}")
    return report, "\n".join(lines)


def _resource_summary(report: dict[str, object], label: str) -> str:
    after = report.get("after")
    if not isinstance(after, dict):
        return f"Resource {label}: UNKNOWN"
    free = after.get("free_disk_bytes")
    floor = after.get("minimum_free_disk_bytes")
    jobs = after.get("max_parallel_jobs")
    nice = after.get("process_nice")
    if not isinstance(free, int) or not isinstance(floor, int):
        return f"Resource {label}: UNKNOWN"
    return (
        f"Resource {label}: PASS · {free / 2**30:.1f} GiB free / "
        f"{floor / 2**30:.1f} GiB floor · {jobs} workers · nice +{nice}"
    )


# --- Durable evidence ----------------------------------------------------


@dataclass
class PhaseOutcome:
    suite: str
    phase: str
    budget_s: int
    elapsed_s: float
    status: str
    over_budget: bool
    failure: Optional[str] = None
    failure_kind: Optional[str] = None
    cpu_s: Optional[float] = None
    minimum_free_disk_bytes: Optional[int] = None

    def as_record(self) -> dict[str, object]:
        return {
            "suite": self.suite,
            "phase": self.phase,
            "budget_s": self.budget_s,
            "elapsed_s": round(self.elapsed_s, 3),
            "status": self.status,
            "over_budget": self.over_budget,
            "failure_kind": self.failure_kind,
            "cpu_s": None if self.cpu_s is None else round(self.cpu_s, 3),
            "minimum_free_disk_bytes": self.minimum_free_disk_bytes,
        }


@dataclass
class GateRun:
    run_id: str
    kind: str
    branch: str
    head: str
    worktree: str
    tree_fingerprint: Optional[str]
    plan_fingerprint: str
    started_at: str
    finished_at: Optional[str]
    status: str
    phases: list[PhaseOutcome]
    resources: Optional[dict[str, object]] = None

    def update_phase(self, outcome: PhaseOutcome) -> None:
        for index, phase in enumerate(self.phases):
            if phase.suite == outcome.suite and phase.phase == outcome.phase:
                self.phases[index] = outcome
                return
        raise ValueError(f"phase {outcome.suite}/{outcome.phase} is not in the selected gate plan")

    def as_record(self) -> dict[str, object]:
        record: dict[str, object] = {
            "schema": GATE_EVIDENCE_SCHEMA,
            "run_id": self.run_id,
            "kind": self.kind,
            "branch": self.branch,
            "head": self.head,
            "worktree": self.worktree,
            "tree_fingerprint": self.tree_fingerprint,
            "plan_fingerprint": self.plan_fingerprint,
            "started_at": self.started_at,
            "finished_at": self.finished_at,
            "status": self.status,
            "phases": [phase.as_record() for phase in self.phases],
            "resources": self.resources,
        }
        return record


def _resource_receipt(
    preflight: Optional[dict[str, object]],
    postflight: Optional[dict[str, object]],
    phases: list[PhaseOutcome],
) -> Optional[dict[str, object]]:
    if preflight is None:
        return None
    before = preflight.get("after")
    after_report = postflight or preflight
    after = after_report.get("after")
    if not isinstance(before, dict) or not isinstance(after, dict):
        return None
    before_sources = before.get("sources")
    after_sources = after.get("sources")
    if not isinstance(before_sources, list) or not isinstance(after_sources, list):
        return None

    def _indexed(sources: list[object]) -> dict[str, dict[str, object]]:
        return {
            source["id"]: source
            for source in sources
            if isinstance(source, dict) and isinstance(source.get("id"), str)
        }

    before_by_id = _indexed(before_sources)
    after_by_id = _indexed(after_sources)
    sources = []
    for source_id in sorted(set(before_by_id) | set(after_by_id)):
        initial = before_by_id.get(source_id, {})
        final = after_by_id.get(source_id, {})
        before_bytes = initial.get("bytes")
        after_bytes = final.get("bytes")
        if not isinstance(before_bytes, int):
            before_bytes = None
        if not isinstance(after_bytes, int):
            after_bytes = None
        sources.append(
            {
                "id": source_id,
                "kind": final.get("kind", initial.get("kind")),
                "owner": final.get("owner", initial.get("owner")),
                "budget_bytes": final.get("budget_bytes", initial.get("budget_bytes")),
                "before_bytes": before_bytes,
                "after_bytes": after_bytes,
                "growth_bytes": (
                    after_bytes - before_bytes
                    if before_bytes is not None and after_bytes is not None
                    else None
                ),
            }
        )

    current = str(REPO_ROOT.resolve())
    current_build = next(
        (
            source
            for source in after_sources
            if isinstance(source, dict)
            and source.get("kind") == "build"
            and source.get("root") == current
        ),
        None,
    )
    build_disk_bytes = current_build.get("bytes") if isinstance(current_build, dict) else None
    aggregate_build_bytes = sum(
        source.get("bytes", 0)
        for source in after_sources
        if isinstance(source, dict)
        and source.get("kind") == "build"
        and isinstance(source.get("bytes"), int)
    )
    cpu_values = [phase.cpu_s for phase in phases if phase.cpu_s is not None]
    free_values = [
        value
        for value in (
            before.get("free_disk_bytes"),
            after.get("free_disk_bytes"),
            *(phase.minimum_free_disk_bytes for phase in phases),
        )
        if isinstance(value, int)
    ]
    return {
        "build_disk_bytes": build_disk_bytes,
        "aggregate_build_bytes": aggregate_build_bytes,
        "cpu_seconds": sum(cpu_values) if cpu_values else None,
        "minimum_free_disk_bytes": min(free_values) if free_values else None,
        "max_parallel_jobs": after.get("max_parallel_jobs"),
        "process_nice": after.get("process_nice"),
        "sources": sources,
        "recovery": preflight.get("recovery", []),
        "issues": after.get("issues", []),
    }


def _utc_now() -> datetime:
    return datetime.now(timezone.utc)


def _format_timestamp(value: datetime) -> str:
    return value.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def _git_value(*args: str) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip()


def _gate_evidence_root() -> Path:
    common_dir = _git_value("rev-parse", "--path-format=absolute", "--git-common-dir")
    return Path(common_dir) / "loopflow" / "pre-land" / "runs"


def _tree_fingerprint() -> str:
    """Hash the tracked and untracked worktree content that tests read."""
    digest = hashlib.sha256()
    listed = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=REPO_ROOT,
        check=True,
        capture_output=True,
    )
    for raw_path in sorted(path for path in listed.stdout.split(b"\0") if path):
        path = REPO_ROOT / os.fsdecode(raw_path)
        digest.update(b"path\0")
        digest.update(raw_path)
        digest.update(b"\0")
        if not path.exists() and not path.is_symlink():
            digest.update(b"deleted\0")
            continue
        file_stat = path.lstat()
        digest.update(str(file_stat.st_mode).encode())
        digest.update(b"\0")
        if path.is_symlink():
            digest.update(os.fsencode(os.readlink(path)))
        elif path.is_file():
            with path.open("rb") as handle:
                for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                    digest.update(chunk)
        elif path.is_dir():
            submodule_head = subprocess.run(
                ["git", "-C", str(path), "rev-parse", "HEAD"],
                check=False,
                capture_output=True,
            )
            digest.update(submodule_head.stdout)
    return digest.hexdigest()


def _plan_fingerprint(plans: list[Plan]) -> str:
    selected = []
    for plan in plans:
        if not plan.run:
            continue
        selected.append(
            {
                "suite": plan.suite.name,
                "commands": [
                    {
                        "label": cmd.label,
                        "argv": cmd.argv,
                        "cwd": str(cmd.cwd.relative_to(REPO_ROOT)),
                    }
                    for cmd in plan.commands
                ],
            }
        )
    payload = json.dumps(selected, separators=(",", ":"), sort_keys=True).encode()
    return hashlib.sha256(payload).hexdigest()


def _find_reusable_run(tree_fingerprint: str, plan_fingerprint: str) -> Optional[Path]:
    worktree = str(REPO_ROOT.resolve())
    evidence_dir = _gate_evidence_root() / "changed"
    for path in sorted(evidence_dir.glob("*.json"), reverse=True):
        try:
            record = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if not isinstance(record, dict):
            continue
        phases = record.get("phases")
        if (
            record.get("schema") == GATE_EVIDENCE_SCHEMA
            and record.get("status") == "passed"
            and record.get("worktree") == worktree
            and record.get("tree_fingerprint") == tree_fingerprint
            and record.get("plan_fingerprint") == plan_fingerprint
            and isinstance(phases, list)
            and phases
            and all(isinstance(phase, dict) and phase.get("status") == "passed" for phase in phases)
        ):
            return path
    return None


def _not_run_phase(suite: str, cmd: Command) -> PhaseOutcome:
    return PhaseOutcome(
        suite=suite,
        phase=cmd.label,
        budget_s=_budget_for(cmd.label),
        elapsed_s=0.0,
        status="not_run",
        over_budget=False,
    )


def _new_gate_run(
    kind: str,
    plans: list[Plan],
    tree_fingerprint: Optional[str],
    plan_fingerprint: str,
    now: Optional[datetime] = None,
    resources: Optional[dict[str, object]] = None,
) -> GateRun:
    started = now or _utc_now()
    run_id = f"{started.strftime('%Y%m%dT%H%M%SZ')}-{os.getpid()}-{secrets.token_hex(4)}"
    phases = [
        _not_run_phase(plan.suite.name, cmd) for plan in plans if plan.run for cmd in plan.commands
    ]
    return GateRun(
        run_id=run_id,
        kind=kind,
        branch=_git_value("rev-parse", "--abbrev-ref", "HEAD"),
        head=_git_value("rev-parse", "HEAD"),
        worktree=str(REPO_ROOT.resolve()),
        tree_fingerprint=tree_fingerprint,
        plan_fingerprint=plan_fingerprint,
        started_at=_format_timestamp(started),
        finished_at=None,
        status="running",
        phases=phases,
        resources=resources,
    )


def _write_run_record(path: Path, record: dict[str, object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temp_path = path.with_name(f".{path.name}.tmp")
    try:
        with temp_path.open("w", encoding="utf-8") as handle:
            json.dump(record, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temp_path, path)
    finally:
        try:
            temp_path.unlink(missing_ok=True)
        except OSError:
            pass


@dataclass
class _GateRecorder:
    run: GateRun
    path: Path
    enabled: bool = True

    def checkpoint(self) -> None:
        if not self.enabled:
            return
        try:
            _write_run_record(self.path, self.run.as_record())
        except OSError as exc:
            self.enabled = False
            print(
                f"MEASUREMENT WARNING: cannot persist gate evidence at {self.path}: {exc}",
                file=sys.stderr,
                flush=True,
            )


def _start_recorder(
    kind: str,
    plans: list[Plan],
    tree_fingerprint: Optional[str],
    plan_fingerprint: str,
    resources: Optional[dict[str, object]] = None,
) -> Optional[_GateRecorder]:
    try:
        run = _new_gate_run(
            kind,
            plans,
            tree_fingerprint,
            plan_fingerprint,
            resources=resources,
        )
        path = _gate_evidence_root() / kind / f"{run.run_id}.json"
    except (OSError, subprocess.SubprocessError) as exc:
        print(
            "MEASUREMENT WARNING: cannot resolve durable gate evidence under "
            f"<git-common-dir>/loopflow/pre-land/runs/{kind}: {exc}",
            file=sys.stderr,
            flush=True,
        )
        return None

    recorder = _GateRecorder(run=run, path=path)
    recorder.checkpoint()
    return recorder


# --- Running -------------------------------------------------------------


@dataclass
class SuiteOutcome:
    ok: bool
    elapsed_s: float
    phases: list[PhaseOutcome]
    # Actionable message when not ok (timeout, missing tool, classified gap).
    failure: Optional[str] = None


# The hosted UI-test runner is spawned by testmanagerd, NOT by xcodebuild, so
# it lives outside the phase's process group and killpg can never reach it. A
# lingering runner holds an Automation Mode client; when it later dies
# unobserved, testmanagerd's client count leaks and the machine wedges in
# 'Automation Running' (release/UI_HOST_GATE.md). Labels here get an explicit
# runner reap after the group kill; the ui-host postcheck then verifies the
# mode actually released.
_UI_RUNNER_LABELS = {"ui-host"}
_UI_RUNNER_PROCESS = "LoopflowUITests-Runner"


def _kill_group(proc: "subprocess.Popen[str]", label: str) -> None:
    """SIGTERM the phase's whole process group, SIGKILL after a grace period.

    The group covers xcodebuild and its direct helpers. It does NOT cover the
    hosted UI-test runner (testmanagerd's child), so UI phases also reap any
    leftover runner by name — SIGTERM, giving it a chance to release its
    Automation Mode client on the way out.
    """
    try:
        pgid = os.getpgid(proc.pid)
    except ProcessLookupError:
        pgid = None
    if pgid is not None:
        try:
            os.killpg(pgid, signal.SIGTERM)
            try:
                proc.wait(timeout=KILL_GRACE_S)
            except subprocess.TimeoutExpired:
                os.killpg(pgid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    if label in _UI_RUNNER_LABELS:
        subprocess.run(["pkill", "-x", _UI_RUNNER_PROCESS], capture_output=True)


def _command_exists(cmd: Command) -> bool:
    executable = Path(cmd.argv[0])
    if executable.is_absolute():
        return executable.exists()
    if executable.parent != Path("."):
        return (cmd.cwd / executable).exists()
    return shutil.which(cmd.argv[0]) is not None


def _child_cpu_seconds() -> float:
    usage = process_resource.getrusage(process_resource.RUSAGE_CHILDREN)
    return usage.ru_utime + usage.ru_stime


def _free_disk_bytes() -> int:
    return shutil.disk_usage(REPO_ROOT).free


def _host_security_pressure() -> Optional[tuple[str, float]]:
    if platform.system() != "Darwin":
        return None
    result = subprocess.run(
        ["ps", "-Ao", "pcpu=,comm="],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    hottest: Optional[tuple[str, float]] = None
    for line in result.stdout.splitlines():
        fields = line.strip().split(maxsplit=1)
        if len(fields) != 2:
            continue
        try:
            cpu = float(fields[0])
        except ValueError:
            continue
        name = Path(fields[1]).name
        if name not in HOST_SECURITY_PROCESSES:
            continue
        if hottest is None or cpu > hottest[1]:
            hottest = (name, cpu)
    return hottest


def _run_command(cmd: Command, artifact_dir: Path, suite: str) -> PhaseOutcome:
    """Run one command bounded by its budget, streaming output live and teeing
    it to a per-phase log."""
    budget = _budget_for(cmd.label)
    artifact_dir.mkdir(parents=True, exist_ok=True)
    log_path = artifact_dir / f"{cmd.label}.log"
    started = time.monotonic()
    cpu_started = _child_cpu_seconds()
    minimum_free = _free_disk_bytes()
    disk_floor = int(RESOURCE_ENVELOPE["minimum_free_disk_bytes"])
    sample_interval = float(RESOURCE_ENVELOPE["sample_interval_seconds"])
    security_limit = float(RESOURCE_ENVELOPE["host_security_cpu_percent"])
    security_samples = int(RESOURCE_ENVELOPE["host_security_samples"])

    def _finish(
        status: str,
        failure: Optional[str],
        failure_kind: Optional[str],
        over_budget: bool,
    ) -> PhaseOutcome:
        nonlocal minimum_free
        try:
            minimum_free = min(minimum_free, _free_disk_bytes())
        except OSError:
            pass
        return PhaseOutcome(
            suite=suite,
            phase=cmd.label,
            budget_s=budget,
            elapsed_s=time.monotonic() - started,
            status=status,
            over_budget=over_budget,
            failure=failure,
            failure_kind=failure_kind,
            cpu_s=max(0.0, _child_cpu_seconds() - cpu_started),
            minimum_free_disk_bytes=minimum_free,
        )

    if not _command_exists(cmd):
        return _finish(
            "missing_tool",
            (
                f"MISSING TOOL: '{cmd.argv[0]}' is not installed on this host. "
                f"Install it or run the {cmd.label} phase where it exists."
            ),
            "missing_tool",
            False,
        )

    run_argv = cmd.argv
    nice = shutil.which("nice")
    process_nice = int(RESOURCE_ENVELOPE["process_nice"])
    if nice is not None and process_nice > 0:
        run_argv = [nice, "-n", str(process_nice), *cmd.argv]

    with open(log_path, "w") as logf:
        try:
            proc = subprocess.Popen(
                run_argv,
                cwd=cmd.cwd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                start_new_session=True,  # own process group for a clean group-kill
                bufsize=1,
                text=True,
            )
        except FileNotFoundError:
            return _finish(
                "missing_tool",
                (
                    f"MISSING TOOL: '{cmd.argv[0]}' is not installed on this host. "
                    f"Install it or run the {cmd.label} phase where it exists."
                ),
                "missing_tool",
                False,
            )

        def _pump() -> None:
            assert proc.stdout is not None
            for line in proc.stdout:
                sys.stdout.write(line)
                sys.stdout.flush()
                logf.write(line)

        pump = threading.Thread(target=_pump, daemon=True)
        pump.start()
        pressure_count = 0
        pressure: Optional[tuple[str, float]] = None
        deadline = started + budget
        try:
            while proc.poll() is None:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    _kill_group(proc, cmd.label)
                    pump.join(timeout=5)
                    elapsed = time.monotonic() - started
                    return _finish(
                        "timed_out",
                        (
                            f"VERIFICATION BUDGET: phase '{cmd.label}' exceeded its {budget}s "
                            f"wall limit (ran {elapsed:.0f}s) and was killed with its process "
                            f"group. Product result: unproven. Log: {log_path}"
                        ),
                        "verification_budget",
                        True,
                    )
                try:
                    proc.wait(timeout=min(sample_interval, remaining))
                except subprocess.TimeoutExpired:
                    try:
                        free = _free_disk_bytes()
                        minimum_free = min(minimum_free, free)
                    except OSError:
                        free = disk_floor
                    if free < disk_floor:
                        _kill_group(proc, cmd.label)
                        pump.join(timeout=5)
                        return _finish(
                            "resource_exhausted",
                            (
                                f"RESOURCE PRESSURE: free disk crossed the "
                                f"{disk_floor / 2**30:.0f} GiB safety floor during "
                                f"'{cmd.label}', so its process group was killed. Product result: "
                                "unproven. NEXT ACTION: run `uv run python "
                                "scripts/resource_envelope.py --recover`, then rerun the gate. "
                                f"Log: {log_path}"
                            ),
                            "resource_pressure",
                            False,
                        )
                    pressure = _host_security_pressure()
                    if pressure is not None and pressure[1] >= security_limit:
                        pressure_count += 1
                    else:
                        pressure_count = 0
                    if pressure_count >= security_samples and pressure is not None:
                        _kill_group(proc, cmd.label)
                        pump.join(timeout=5)
                        return _finish(
                            "host_pressure",
                            (
                                f"HOST SECURITY PRESSURE: {pressure[0]} held {pressure[1]:.0f}% "
                                f"CPU for {pressure_count} samples while '{cmd.label}' ran, so the "
                                "verification group was stopped at low priority. Product result: "
                                "unproven. NEXT ACTION: let macOS verification drain, inspect "
                                "`ps -Ao pid,pcpu,comm | sort -k2 -nr | head`, then rerun. "
                                f"Log: {log_path}"
                            ),
                            "host_security_pressure",
                            False,
                        )
        except KeyboardInterrupt:
            _kill_group(proc, cmd.label)
            pump.join(timeout=5)
            raise
        pump.join(timeout=5)

    elapsed = time.monotonic() - started
    over_budget = elapsed > budget
    if proc.returncode != 0:
        if proc.returncode < 0:
            reason = f"killed by signal {-proc.returncode}"
        else:
            reason = f"exit {proc.returncode}"
        return _finish(
            "failed",
            f"PRODUCT FAILURE ({cmd.label}, {reason}). Log: {log_path}",
            "product",
            over_budget,
        )
    failure = None
    failure_kind = None
    if over_budget:
        failure = (
            f"VERIFICATION BUDGET: phase '{cmd.label}' ran "
            f"{elapsed:.1f}s / {budget}s wall limit"
        )
        failure_kind = "verification_budget"
    return _finish("passed", failure, failure_kind, over_budget)


def _run_suite(
    plan: Plan,
    artifact_root: Path,
    checkpoint_phase: Callable[[PhaseOutcome], None],
) -> SuiteOutcome:
    bar = "=" * 72
    print(f"\n{bar}")
    print(f" SUITE: {plan.suite.name}  ({plan.reason})  [budget {_plan_budget(plan)}s]")
    if plan.suite.proves:
        print(f" proves: {plan.suite.proves}")
    print(bar, flush=True)
    started = time.monotonic()
    artifact_dir = artifact_root / plan.suite.name
    phases = [_not_run_phase(plan.suite.name, cmd) for cmd in plan.commands]

    if plan.suite.precheck is not None:
        gap = plan.suite.precheck()
        if gap is not None:
            print(f"\n[{plan.suite.name}] {gap}", flush=True)
            return SuiteOutcome(False, time.monotonic() - started, phases, gap)

    lock = None
    if plan.suite.machine_lock is not None:
        lock, held = _acquire_machine_lock(plan.suite.machine_lock)
        if lock is None:
            assert held is not None
            print(f"\n[{plan.suite.name}] {held}", flush=True)
            return SuiteOutcome(False, time.monotonic() - started, phases, held)
    try:
        result = _run_suite_commands(plan, artifact_dir, phases, started, checkpoint_phase)

        # The postcheck runs whether the commands passed or failed — a failed
        # or killed run is exactly when machine state is most likely to have
        # leaked — and while the lock is still held, so a queued next run
        # cannot re-enable the state mid-check and fake a leak.
        if plan.suite.postcheck is not None:
            leak = plan.suite.postcheck()
            if leak is not None:
                print(f"\n[{plan.suite.name}] {leak}", flush=True)
                result.ok = False
                result.failure = (
                    leak if result.failure is None else f"{result.failure}\n{leak}"
                )
    finally:
        if lock is not None:
            lock.close()
    return result


def _run_suite_commands(
    plan: Plan,
    artifact_dir: Path,
    phases: list[PhaseOutcome],
    started: float,
    checkpoint_phase: Callable[[PhaseOutcome], None],
) -> SuiteOutcome:
    for index, cmd in enumerate(plan.commands):
        print(f"\n$ {_fmt_cmd(cmd)}  (budget {_budget_for(cmd.label)}s)", flush=True)
        outcome = _run_command(cmd, artifact_dir, plan.suite.name)
        if outcome.failure is not None:
            if plan.suite.classify is not None and outcome.failure_kind == "product":
                log_path = artifact_dir / f"{cmd.label}.log"
                log_text = log_path.read_text() if log_path.exists() else ""
                refined = plan.suite.classify(log_text)
                if refined is not None:
                    outcome.failure = refined
                    outcome.failure_kind = "missing_capability"
        phases[index] = outcome
        checkpoint_phase(outcome)
        if outcome.failure is not None:
            print(f"\n[{plan.suite.name}] {outcome.failure}", flush=True)
            return SuiteOutcome(False, time.monotonic() - started, phases, outcome.failure)
    return SuiteOutcome(True, time.monotonic() - started, phases)


def run_plans(
    plans: list[Plan],
    kind: str = "changed",
    reuse_passing: bool = False,
) -> int:
    artifact_root = _run_artifact_root()
    total_budget = sum(_plan_budget(p) for p in plans if p.run)
    running = [p.suite.name for p in plans if p.run]
    if running:
        print(f"Gate budget: {total_budget}s across {len(running)} suites ({', '.join(running)})")
        print(f"Artifacts on failure: {artifact_root}")

    preflight, preflight_failure = _run_resource_check(True)
    if preflight is not None and preflight_failure is None:
        print(_resource_summary(preflight, "preflight"))

    try:
        tree_fingerprint = _tree_fingerprint()
    except (OSError, subprocess.SubprocessError) as exc:
        tree_fingerprint = None
        print(f"MEASUREMENT WARNING: cannot fingerprint the working tree: {exc}", file=sys.stderr)
    plan_fingerprint = _plan_fingerprint(plans)

    initial_resources = _resource_receipt(preflight, None, [])
    if preflight_failure is not None:
        recorder = _start_recorder(
            kind,
            plans,
            tree_fingerprint,
            plan_fingerprint,
            resources=initial_resources,
        )
        if recorder is not None:
            recorder.run.status = "resource_blocked"
            recorder.run.finished_at = _format_timestamp(_utc_now())
            recorder.checkpoint()
        print(f"\n{preflight_failure}")
        print("\nResult: FAIL (resource preflight; product suites not run)")
        return 1

    if reuse_passing and kind == "changed" and running and tree_fingerprint is not None:
        try:
            reusable = _find_reusable_run(tree_fingerprint, plan_fingerprint)
        except (OSError, subprocess.SubprocessError) as exc:
            reusable = None
            print(f"MEASUREMENT WARNING: cannot read passing gate evidence: {exc}", file=sys.stderr)
        if reusable is not None:
            print(
                "Result: REUSED passing affected-suite evidence for the identical "
                f"tree and plan ({reusable.name})"
            )
            return 0

    recorder = _start_recorder(
        kind,
        plans,
        tree_fingerprint,
        plan_fingerprint,
        resources=initial_resources,
    )

    def _checkpoint_phase(outcome: PhaseOutcome) -> None:
        if recorder is None:
            return
        recorder.run.update_phase(outcome)
        recorder.run.resources = _resource_receipt(preflight, None, recorder.run.phases)
        recorder.checkpoint()

    outcomes: dict[str, SuiteOutcome] = {}
    for plan in plans:
        if plan.run:
            outcomes[plan.suite.name] = _run_suite(plan, artifact_root, _checkpoint_phase)

    failed = [(name, outcome) for name, outcome in outcomes.items() if not outcome.ok]
    phases = [phase for outcome in outcomes.values() for phase in outcome.phases]
    postflight, postflight_failure = _run_resource_check(False)
    resource_breach = postflight_failure if postflight is not None else None
    if postflight is not None and postflight_failure is None:
        print(_resource_summary(postflight, "postflight"))
    elif postflight is None and postflight_failure is not None:
        print(f"MEASUREMENT WARNING: {postflight_failure}", file=sys.stderr)
    if recorder is not None:
        recorder.run.resources = _resource_receipt(preflight, postflight, phases)
        recorder.run.status = "failed" if failed or resource_breach else "passed"
        recorder.run.finished_at = _format_timestamp(_utc_now())
        recorder.checkpoint()

    print("\n" + "=" * 72)
    print(" SUMMARY")
    print("=" * 72)
    for plan in plans:
        if plan.run:
            outcome = outcomes[plan.suite.name]
            for phase in outcome.phases:
                status = {
                    "passed": "PASS",
                    "failed": "FAIL",
                    "timed_out": "TIMEOUT",
                    "resource_exhausted": "RESOURCE",
                    "host_pressure": "HOST",
                    "missing_tool": "MISSING",
                    "not_run": "NOT RUN",
                }[phase.status]
                elapsed = "not run" if phase.status == "not_run" else f"{phase.elapsed_s:.1f}s"
                over = "  OVER BUDGET" if phase.over_budget else ""
                print(
                    f"  {status:<7} {phase.suite}/{phase.phase:<20} "
                    f"{elapsed} / {phase.budget_s}s budget · "
                    f"{phase.cpu_s or 0.0:.1f}s CPU{over}"
                )
            status = "PASS" if outcome.ok else "FAIL"
            detail = f"{outcome.elapsed_s:.0f}s / {_plan_budget(plan)}s budget"
        else:
            status = "----"
            detail = plan.reason
        print(f"  {status}  {plan.suite.name:<9} {detail}")

    passed = [name for name, o in outcomes.items() if o.ok]
    total_elapsed = sum(o.elapsed_s for o in outcomes.values())
    print()
    if not outcomes and resource_breach is None:
        print("No suites ran (nothing changed). Use --all to force the full matrix.")
        return 0
    if failed or resource_breach is not None:
        for name, outcome in failed:
            print(f"[{name}] {outcome.failure}")
        if resource_breach is not None:
            print(f"[resources] {resource_breach}")
        names = ", ".join(
            [
                *(name for name, _ in failed),
                *(("resources",) if resource_breach else ()),
            ]
        )
        failure_count = len(failed) + int(resource_breach is not None)
        print(
            f"\nResult: FAIL ({len(passed)} passed, {failure_count} failed: {names}) "
            f"in {total_elapsed:.0f}s / {total_budget}s budget"
        )
        return 1
    print(f"Result: PASS ({len(passed)} suites) in {total_elapsed:.0f}s / {total_budget}s budget")
    return 0


# --- CLI -----------------------------------------------------------------


def _parse_args(argv: Optional[list[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="scripts/test.py",
        description="Run only the CI suites the branch changed.",
    )
    parser.add_argument("--all", action="store_true", help="run every suite, slow ones included")
    parser.add_argument(
        "--reuse-passing",
        action="store_true",
        help="reuse a passing changed-mode run for the identical tree and plan",
    )
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
    args = parser.parse_args(argv)
    forced = {suite.name for suite in SUITES if getattr(args, suite.name.replace("-", "_"))}
    if args.reuse_passing and (args.all or "ui-host" in forced):
        parser.error("--reuse-passing cannot be combined with --all or --ui-host")
    return args


def main(argv: Optional[list[str]] = None) -> int:
    args = _parse_args(argv)
    forced = {suite.name for suite in SUITES if getattr(args, suite.name.replace("-", "_"))}

    base = _resolve_base(args.base)
    changed = changed_files(base)
    plans = build_plan(changed, args.all, forced)

    if args.list_only:
        print(f"(base: {base})")
        print_plan(plans, changed)
        return 0

    if args.all:
        kind = "full"
    elif "ui-host" in forced:
        kind = "required_host"
    else:
        kind = "changed"
    return run_plans(plans, kind=kind, reuse_passing=args.reuse_passing)


if __name__ == "__main__":
    sys.exit(main())
