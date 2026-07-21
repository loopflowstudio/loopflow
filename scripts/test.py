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
import hashlib
import json
import os
import platform
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
from typing import Callable, Optional

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
GATE_EVIDENCE_SCHEMA = 2


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


def _rust_commands(_changed: list[str]) -> list[Command]:
    if shutil.which("cargo-nextest"):
        test_argv = ["cargo", "nextest", "run", "--all"]
    else:
        test_argv = ["cargo", "test", "--all"]
    return [
        Command(["cargo", "fmt", "--all", "--", "--check"], REPO_ROOT, "rustfmt"),
        Command(
            ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"],
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
        ),
        Command(
            ["uv", "run", "python", "scripts/check_swift_multiplatform_boundaries.py"],
            REPO_ROOT,
            "swift-boundaries",
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

    def as_record(self) -> dict[str, object]:
        return {
            "suite": self.suite,
            "phase": self.phase,
            "budget_s": self.budget_s,
            "elapsed_s": round(self.elapsed_s, 3),
            "status": self.status,
            "over_budget": self.over_budget,
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
        }
        return record


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
) -> Optional[_GateRecorder]:
    try:
        run = _new_gate_run(kind, plans, tree_fingerprint, plan_fingerprint)
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


def _kill_group(proc: "subprocess.Popen[str]") -> None:
    """SIGTERM the phase's whole process group, SIGKILL after a grace period.

    A hung `xcodebuild` spawns a test-host child; killing only the parent leaves
    the runner alive and the terminal wedged, so we signal the group.
    """
    try:
        pgid = os.getpgid(proc.pid)
    except ProcessLookupError:
        return
    try:
        os.killpg(pgid, signal.SIGTERM)
    except ProcessLookupError:
        return
    try:
        proc.wait(timeout=KILL_GRACE_S)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(pgid, signal.SIGKILL)
        except ProcessLookupError:
            pass


def _run_command(cmd: Command, artifact_dir: Path, suite: str) -> PhaseOutcome:
    """Run one command bounded by its budget, streaming output live and teeing
    it to a per-phase log."""
    budget = _budget_for(cmd.label)
    artifact_dir.mkdir(parents=True, exist_ok=True)
    log_path = artifact_dir / f"{cmd.label}.log"
    started = time.monotonic()
    with open(log_path, "w") as logf:
        try:
            proc = subprocess.Popen(
                cmd.argv,
                cwd=cmd.cwd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                start_new_session=True,  # own process group for a clean group-kill
                bufsize=1,
                text=True,
            )
        except FileNotFoundError:
            elapsed = time.monotonic() - started
            return PhaseOutcome(
                suite=suite,
                phase=cmd.label,
                budget_s=budget,
                elapsed_s=elapsed,
                status="missing_tool",
                over_budget=False,
                failure=(
                    f"MISSING TOOL: '{cmd.argv[0]}' is not installed on this host. "
                    f"Install it or run the {cmd.label} phase where it exists."
                ),
            )

        def _pump() -> None:
            assert proc.stdout is not None
            for line in proc.stdout:
                sys.stdout.write(line)
                sys.stdout.flush()
                logf.write(line)

        pump = threading.Thread(target=_pump, daemon=True)
        pump.start()
        try:
            proc.wait(timeout=budget)
        except subprocess.TimeoutExpired:
            _kill_group(proc)
            pump.join(timeout=5)
            elapsed = time.monotonic() - started
            return PhaseOutcome(
                suite=suite,
                phase=cmd.label,
                budget_s=budget,
                elapsed_s=elapsed,
                status="timed_out",
                over_budget=True,
                failure=(
                    f"TIMEOUT: phase '{cmd.label}' exceeded its {budget}s budget "
                    f"(ran {elapsed:.0f}s) and was killed. No phase hangs the gate. "
                    f"Log: {log_path}"
                ),
            )
        pump.join(timeout=5)

    elapsed = time.monotonic() - started
    over_budget = elapsed > budget
    if proc.returncode != 0:
        if proc.returncode < 0:
            reason = f"killed by signal {-proc.returncode}"
        else:
            reason = f"exit {proc.returncode}"
        return PhaseOutcome(
            suite=suite,
            phase=cmd.label,
            budget_s=budget,
            elapsed_s=elapsed,
            status="failed",
            over_budget=over_budget,
            failure=f"FAILED ({cmd.label}, {reason}). Log: {log_path}",
        )
    failure = None
    if over_budget:
        failure = f"OVER BUDGET: phase '{cmd.label}' ran {elapsed:.1f}s / {budget}s budget"
    return PhaseOutcome(
        suite=suite,
        phase=cmd.label,
        budget_s=budget,
        elapsed_s=elapsed,
        status="passed",
        over_budget=over_budget,
        failure=failure,
    )


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

    for index, cmd in enumerate(plan.commands):
        print(f"\n$ {_fmt_cmd(cmd)}  (budget {_budget_for(cmd.label)}s)", flush=True)
        outcome = _run_command(cmd, artifact_dir, plan.suite.name)
        if outcome.failure is not None:
            if plan.suite.classify is not None:
                log_path = artifact_dir / f"{cmd.label}.log"
                log_text = log_path.read_text() if log_path.exists() else ""
                refined = plan.suite.classify(log_text)
                if refined is not None:
                    outcome.failure = refined
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

    try:
        tree_fingerprint = _tree_fingerprint()
    except (OSError, subprocess.SubprocessError) as exc:
        tree_fingerprint = None
        print(f"MEASUREMENT WARNING: cannot fingerprint the working tree: {exc}", file=sys.stderr)
    plan_fingerprint = _plan_fingerprint(plans)

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

    recorder = _start_recorder(kind, plans, tree_fingerprint, plan_fingerprint)

    def _checkpoint_phase(outcome: PhaseOutcome) -> None:
        if recorder is None:
            return
        recorder.run.update_phase(outcome)
        recorder.checkpoint()

    outcomes: dict[str, SuiteOutcome] = {}
    for plan in plans:
        if plan.run:
            outcomes[plan.suite.name] = _run_suite(plan, artifact_root, _checkpoint_phase)

    failed = [(name, outcome) for name, outcome in outcomes.items() if not outcome.ok]
    if recorder is not None:
        recorder.run.status = "failed" if failed else "passed"
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
                    "missing_tool": "MISSING",
                    "not_run": "NOT RUN",
                }[phase.status]
                elapsed = "not run" if phase.status == "not_run" else f"{phase.elapsed_s:.1f}s"
                over = "  OVER BUDGET" if phase.over_budget else ""
                print(
                    f"  {status:<7} {phase.suite}/{phase.phase:<20} "
                    f"{elapsed} / {phase.budget_s}s budget{over}"
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
    if not outcomes:
        print("No suites ran (nothing changed). Use --all to force the full matrix.")
        return 0
    if failed:
        for name, outcome in failed:
            print(f"[{name}] {outcome.failure}")
        names = ", ".join(name for name, _ in failed)
        print(
            f"\nResult: FAIL ({len(passed)} passed, {len(failed)} failed: {names}) "
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
