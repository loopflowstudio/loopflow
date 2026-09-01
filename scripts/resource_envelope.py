#!/usr/bin/env python3
"""Measure and safely recover Loopflow's local development resources."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Optional

REPO_ROOT = Path(__file__).resolve().parent.parent
POLICY_PATH = REPO_ROOT / "performance" / "budgets.json"
BUILD_RELATIVE_PATHS = (
    Path("target"),
    Path(".build"),
    Path("swift/.build"),
    Path("website/node_modules"),
)
GATE_RELATIVE_PATH = Path(".lf/tmp/gate")


@dataclass(frozen=True)
class ResourcePolicy:
    minimum_free_disk_bytes: int
    maximum_worktree_build_bytes: int
    maximum_aggregate_build_bytes: int
    maximum_run_record_bytes: int
    maximum_uv_cache_bytes: int
    maximum_cargo_cache_bytes: int
    maximum_gate_artifact_bytes: int
    maximum_recovery_roots: int
    maximum_recovery_bytes: int
    gate_artifact_retention_hours: int
    max_parallel_jobs: int
    process_nice: int
    host_security_cpu_percent: float
    host_security_samples: int
    sample_interval_seconds: float


@dataclass(frozen=True)
class Worktree:
    path: Path
    branch: str


@dataclass(frozen=True)
class ResourceSource:
    id: str
    kind: str
    owner: str
    root: Path
    paths: tuple[Path, ...]
    bytes: int
    budget_bytes: int
    disposable: bool
    active: bool
    action: str

    def as_record(self) -> dict[str, object]:
        return {
            "id": self.id,
            "kind": self.kind,
            "owner": self.owner,
            "root": str(self.root),
            "paths": [str(path) for path in self.paths],
            "bytes": self.bytes,
            "budget_bytes": self.budget_bytes,
            "disposable": self.disposable,
            "active": self.active,
            "action": self.action,
        }


@dataclass(frozen=True)
class ResourceIssue:
    code: str
    owner: str
    detail: str
    action: str
    recoverable: bool


@dataclass(frozen=True)
class ResourceSnapshot:
    filesystem: str
    total_disk_bytes: int
    free_disk_bytes: int
    minimum_free_disk_bytes: int
    max_parallel_jobs: int
    process_nice: int
    sources: tuple[ResourceSource, ...]
    issues: tuple[ResourceIssue, ...]
    warnings: tuple[str, ...]

    @property
    def ok(self) -> bool:
        return not self.issues

    def as_record(self) -> dict[str, object]:
        return {
            "ok": self.ok,
            "filesystem": self.filesystem,
            "total_disk_bytes": self.total_disk_bytes,
            "free_disk_bytes": self.free_disk_bytes,
            "minimum_free_disk_bytes": self.minimum_free_disk_bytes,
            "max_parallel_jobs": self.max_parallel_jobs,
            "process_nice": self.process_nice,
            "sources": [source.as_record() for source in self.sources],
            "issues": [asdict(issue) for issue in self.issues],
            "warnings": list(self.warnings),
        }


@dataclass(frozen=True)
class RecoveryAction:
    source: str
    owner: str
    removed_bytes: int
    removed_paths: tuple[str, ...]
    status: str
    detail: str


@dataclass(frozen=True)
class ResourceReport:
    policy_path: str
    before: ResourceSnapshot
    after: ResourceSnapshot
    recovery: tuple[RecoveryAction, ...]

    @property
    def ok(self) -> bool:
        return self.after.ok

    def as_record(self) -> dict[str, object]:
        return {
            "schema": 1,
            "ok": self.ok,
            "policy_path": self.policy_path,
            "before": self.before.as_record(),
            "after": self.after.as_record(),
            "recovery": [asdict(action) for action in self.recovery],
        }


def load_policy(path: Path = POLICY_PATH) -> ResourcePolicy:
    payload = json.loads(path.read_text(encoding="utf-8"))
    envelope = payload["resource_envelope"]
    return ResourcePolicy(**envelope)


def collect_snapshot(repo: Path, policy: ResourcePolicy) -> ResourceSnapshot:
    repo = repo.resolve()
    disk = shutil.disk_usage(repo)
    worktrees, discovery_error = _discover_worktrees(repo)
    active_paths, activity_warning = _running_cwds()
    warnings = [warning for warning in (activity_warning,) if warning]
    sources: list[ResourceSource] = []
    measurement_issues: list[ResourceIssue] = []

    if discovery_error is not None:
        measurement_issues.append(
            ResourceIssue(
                code="worktree-discovery",
                owner=str(repo),
                detail=discovery_error,
                action=(
                    "run `git worktree list --porcelain` from this checkout and repair "
                    "its metadata"
                ),
                recoverable=False,
            )
        )
        worktrees = [Worktree(repo, _branch(repo))]

    for worktree in worktrees:
        active = active_paths is None or any(
            _is_within(cwd, worktree.path) for cwd in active_paths
        )
        build_paths = tuple(worktree.path / relative for relative in BUILD_RELATIVE_PATHS)
        sources.append(
            _measure_source(
                id=f"build:{worktree.branch}",
                kind="build",
                owner=worktree.branch,
                root=worktree.path,
                paths=build_paths,
                budget=policy.maximum_worktree_build_bytes,
                disposable=True,
                active=active,
                action=(
                    "run `uv run python scripts/resource_envelope.py --recover`; "
                    "recovery removes only build roots from inactive worktrees"
                ),
                issues=measurement_issues,
            )
        )
        gate_path = worktree.path / GATE_RELATIVE_PATH
        sources.append(
            _measure_source(
                id=f"gate:{worktree.branch}",
                kind="gate",
                owner=worktree.branch,
                root=worktree.path,
                paths=(gate_path,),
                budget=policy.maximum_gate_artifact_bytes,
                disposable=True,
                active=active,
                action=(
                    "run `uv run python scripts/resource_envelope.py --recover`; "
                    "only gate runs older than "
                    f"{policy.gate_artifact_retention_hours}h are eligible"
                ),
                issues=measurement_issues,
            )
        )

    authority_home = _authority_home()
    run_root = authority_home / "runs"
    sources.append(
        _measure_source(
            id="runs:home",
            kind="runs",
            owner="Loopflow Home",
            root=authority_home,
            paths=(run_root,),
            budget=policy.maximum_run_record_bytes,
            disposable=False,
            active=True,
            action=(
                f"inspect {run_root}; Run records are durable local evidence and are never "
                "auto-deleted"
            ),
            issues=measurement_issues,
        )
    )
    uv_cache = _uv_cache_dir()
    sources.append(
        _measure_source(
            id="cache:uv",
            kind="cache",
            owner="uv",
            root=uv_cache,
            paths=(uv_cache,),
            budget=policy.maximum_uv_cache_bytes,
            disposable=True,
            active=False,
            action="run `uv cache prune`; recovery uses that supported cache boundary",
            issues=measurement_issues,
        )
    )
    cargo_home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo")).expanduser()
    sources.append(
        _measure_source(
            id="cache:cargo",
            kind="cache",
            owner="Cargo",
            root=cargo_home,
            paths=(cargo_home / "registry", cargo_home / "git"),
            budget=policy.maximum_cargo_cache_bytes,
            disposable=False,
            active=True,
            action=(
                "inspect the named Cargo registry/git roots; shared cache eviction is not "
                "automatic while builds may be using it"
            ),
            issues=measurement_issues,
        )
    )

    issues = _assess_sources(disk.free, policy, sources)
    issues.extend(measurement_issues)
    issues.sort(key=lambda issue: issue.code)
    return ResourceSnapshot(
        filesystem=str(repo),
        total_disk_bytes=disk.total,
        free_disk_bytes=disk.free,
        minimum_free_disk_bytes=policy.minimum_free_disk_bytes,
        max_parallel_jobs=policy.max_parallel_jobs,
        process_nice=policy.process_nice,
        sources=tuple(sorted(sources, key=lambda source: source.id)),
        issues=tuple(issues),
        warnings=tuple(warnings),
    )


def recover_resources(
    repo: Path,
    policy: ResourcePolicy,
    snapshot: ResourceSnapshot,
    now: Optional[float] = None,
) -> tuple[RecoveryAction, ...]:
    now = time.time() if now is None else now
    actions: list[RecoveryAction] = []
    roots_used = 0
    bytes_used = 0
    issue_codes = {issue.code for issue in snapshot.issues}
    disk_pressure = "disk:free" in issue_codes

    gate_sources = sorted(
        (source for source in snapshot.sources if source.kind == "gate" and source.bytes),
        key=lambda source: -source.bytes,
    )
    for source in gate_sources:
        if source.id not in issue_codes and not disk_pressure:
            continue
        if roots_used >= policy.maximum_recovery_roots:
            break
        if bytes_used + source.bytes > policy.maximum_recovery_bytes:
            continue
        action = _prune_old_gate_artifacts(source, policy, now)
        actions.append(action)
        roots_used += 1
        bytes_used += action.removed_bytes

    uv_source = next((source for source in snapshot.sources if source.id == "cache:uv"), None)
    if (
        uv_source is not None
        and ("cache:uv" in issue_codes or disk_pressure)
        and roots_used < policy.maximum_recovery_roots
        and bytes_used + uv_source.bytes <= policy.maximum_recovery_bytes
    ):
        uv_action = _prune_uv_cache()
        actions.append(uv_action)
        roots_used += 1
        bytes_used += uv_action.removed_bytes

    aggregate_build = sum(source.bytes for source in snapshot.sources if source.kind == "build")
    needed = max(
        0,
        policy.minimum_free_disk_bytes - snapshot.free_disk_bytes,
        aggregate_build - policy.maximum_aggregate_build_bytes,
    )
    needed = max(0, needed - bytes_used)
    required_builds = {
        source.id
        for source in snapshot.sources
        if source.kind == "build" and source.id in issue_codes
    }
    candidates = sorted(
        (
            source
            for source in snapshot.sources
            if source.kind == "build" and source.disposable and not source.active and source.bytes
        ),
        key=lambda source: (source.id not in required_builds, -source.bytes),
    )
    for source in candidates:
        if roots_used >= policy.maximum_recovery_roots:
            break
        if bytes_used >= policy.maximum_recovery_bytes:
            break
        if source.id not in required_builds and needed <= 0:
            break
        if bytes_used + source.bytes > policy.maximum_recovery_bytes:
            continue
        action = _remove_build_artifacts(source)
        actions.append(action)
        roots_used += 1
        bytes_used += action.removed_bytes
        needed = max(0, needed - action.removed_bytes)

    return tuple(actions)


def inspect_resources(repo: Path, policy: ResourcePolicy, recover: bool) -> ResourceReport:
    before = collect_snapshot(repo, policy)
    recovery = recover_resources(repo, policy, before) if recover and not before.ok else ()
    after = collect_snapshot(repo, policy) if recovery else before
    return ResourceReport(str(POLICY_PATH), before, after, tuple(recovery))


def _discover_worktrees(repo: Path) -> tuple[list[Worktree], Optional[str]]:
    result = subprocess.run(
        ["git", "worktree", "list", "--porcelain"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or f"git exited {result.returncode}"
        return [], f"cannot attribute build roots to worktrees: {detail}"

    worktrees: list[Worktree] = []
    path: Optional[Path] = None
    branch = "detached"
    for line in [*result.stdout.splitlines(), ""]:
        if line.startswith("worktree "):
            path = Path(line.removeprefix("worktree ")).resolve()
            branch = "detached"
        elif line.startswith("branch refs/heads/"):
            branch = line.removeprefix("branch refs/heads/")
        elif not line and path is not None:
            if branch == "detached":
                branch = f"detached:{path.name}"
            worktrees.append(Worktree(path, branch))
            path = None
    return worktrees, None


def _branch(repo: Path) -> str:
    result = subprocess.run(
        ["git", "branch", "--show-current"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    return result.stdout.strip() or "current"


def _running_cwds() -> tuple[Optional[set[Path]], Optional[str]]:
    try:
        result = subprocess.run(
            ["lsof", "-d", "cwd", "-Fn"],
            capture_output=True,
            text=True,
            timeout=15,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired) as exc:
        return None, f"cannot prove inactive worktrees ({exc}); recovery will retain all builds"
    if result.returncode != 0:
        return None, (
            "cannot prove inactive worktrees "
            f"(lsof exited {result.returncode}); recovery will retain all builds"
        )
    paths = {
        Path(line[1:]).resolve()
        for line in result.stdout.splitlines()
        if line.startswith("n/")
    }
    return paths, None


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False


def _authority_home() -> Path:
    for name in ("LF_CONTROL_HOME", "LF_HOME"):
        value = os.environ.get(name)
        if value:
            return Path(value).expanduser()
    return Path.home() / ".lf"


def _uv_cache_dir() -> Path:
    override = os.environ.get("UV_CACHE_DIR")
    if override:
        return Path(override).expanduser()
    result = subprocess.run(["uv", "cache", "dir"], capture_output=True, text=True)
    if result.returncode == 0 and result.stdout.strip():
        return Path(result.stdout.strip()).expanduser()
    return Path(os.environ.get("XDG_CACHE_HOME", Path.home() / ".cache")) / "uv"


def _allocated_bytes(path: Path) -> int:
    if not path.exists() and not path.is_symlink():
        return 0
    result = subprocess.run(["du", "-sk", str(path)], capture_output=True, text=True)
    if result.returncode == 0:
        first = result.stdout.split(maxsplit=1)
        if first:
            return int(first[0]) * 1024
    detail = result.stderr.strip() or f"du exited {result.returncode}"
    raise OSError(f"cannot measure {path}: {detail}")


def _measure_source(
    *,
    id: str,
    kind: str,
    owner: str,
    root: Path,
    paths: tuple[Path, ...],
    budget: int,
    disposable: bool,
    active: bool,
    action: str,
    issues: list[ResourceIssue],
) -> ResourceSource:
    measured = 0
    for path in paths:
        try:
            measured += _allocated_bytes(path)
        except (OSError, ValueError) as exc:
            issues.append(
                ResourceIssue(
                    code=f"measurement:{id}",
                    owner=owner,
                    detail=str(exc),
                    action=f"make {path} readable and rerun the resource check",
                    recoverable=False,
                )
            )
    return ResourceSource(
        id=id,
        kind=kind,
        owner=owner,
        root=root,
        paths=paths,
        bytes=measured,
        budget_bytes=budget,
        disposable=disposable,
        active=active,
        action=action,
    )


def _assess_sources(
    free_disk_bytes: int,
    policy: ResourcePolicy,
    sources: list[ResourceSource],
) -> list[ResourceIssue]:
    issues = []
    if free_disk_bytes < policy.minimum_free_disk_bytes:
        issues.append(
            ResourceIssue(
                code="disk:free",
                owner="host filesystem",
                detail=(
                    f"{_format_bytes(free_disk_bytes)} free is below the "
                    f"{_format_bytes(policy.minimum_free_disk_bytes)} verification floor"
                ),
                action=(
                    "run `uv run python scripts/resource_envelope.py --recover`; "
                    "verification will not start until the safety floor is restored"
                ),
                recoverable=True,
            )
        )
    for source in sources:
        if source.bytes <= source.budget_bytes:
            continue
        issues.append(
            ResourceIssue(
                code=source.id,
                owner=source.owner,
                detail=(
                    f"{source.kind} uses {_format_bytes(source.bytes)} / "
                    f"{_format_bytes(source.budget_bytes)} budget"
                ),
                action=source.action,
                recoverable=source.disposable and not source.active,
            )
        )
    build_bytes = sum(source.bytes for source in sources if source.kind == "build")
    if build_bytes > policy.maximum_aggregate_build_bytes:
        issues.append(
            ResourceIssue(
                code="build:aggregate",
                owner="Loopflow worktrees",
                detail=(
                    f"build roots use {_format_bytes(build_bytes)} / "
                    f"{_format_bytes(policy.maximum_aggregate_build_bytes)} aggregate budget"
                ),
                action=(
                    "run `uv run python scripts/resource_envelope.py --recover`; "
                    "the largest inactive worktree builds are cleaned first"
                ),
                recoverable=any(
                    source.kind == "build" and source.disposable and not source.active
                    for source in sources
                ),
            )
        )
    return issues


def _remove_build_artifacts(source: ResourceSource) -> RecoveryAction:
    removed_paths = []
    removed_bytes = 0
    allowed = {source.root / relative for relative in BUILD_RELATIVE_PATHS}
    for path in source.paths:
        if path not in allowed or path == source.root:
            return RecoveryAction(
                source.id,
                source.owner,
                0,
                (),
                "refused",
                f"{path} is outside the build-artifact allowlist",
            )
        if not path.exists() and not path.is_symlink():
            continue
        measured = _allocated_bytes(path)
        if path.is_dir() and not path.is_symlink():
            shutil.rmtree(path)
        else:
            path.unlink()
        removed_paths.append(str(path))
        removed_bytes += measured
    return RecoveryAction(
        source.id,
        source.owner,
        removed_bytes,
        tuple(removed_paths),
        "removed",
        "removed allowlisted build artifacts; worktree and source retained",
    )


def _prune_old_gate_artifacts(
    source: ResourceSource,
    policy: ResourcePolicy,
    now: float,
) -> RecoveryAction:
    gate_root = source.root / GATE_RELATIVE_PATH
    if source.paths != (gate_root,):
        return RecoveryAction(
            source.id,
            source.owner,
            0,
            (),
            "refused",
            "gate artifact root is outside the allowlist",
        )
    cutoff = now - policy.gate_artifact_retention_hours * 3600
    removed_paths = []
    removed_bytes = 0
    if gate_root.is_dir():
        for child in gate_root.iterdir():
            if child.stat().st_mtime >= cutoff:
                continue
            measured = _allocated_bytes(child)
            if child.is_dir() and not child.is_symlink():
                shutil.rmtree(child)
            else:
                child.unlink()
            removed_paths.append(str(child))
            removed_bytes += measured
    return RecoveryAction(
        source.id,
        source.owner,
        removed_bytes,
        tuple(removed_paths),
        "removed",
        f"removed only gate artifacts older than {policy.gate_artifact_retention_hours}h",
    )


def _prune_uv_cache() -> RecoveryAction:
    before = _allocated_bytes(_uv_cache_dir())
    result = subprocess.run(["uv", "cache", "prune"], capture_output=True, text=True)
    after = _allocated_bytes(_uv_cache_dir())
    detail = result.stderr.strip() or result.stdout.strip()
    return RecoveryAction(
        "cache:uv",
        "uv",
        max(0, before - after),
        (),
        "pruned" if result.returncode == 0 else "failed",
        detail or f"uv cache prune exited {result.returncode}",
    )


def _format_bytes(value: int) -> str:
    units = ("B", "KiB", "MiB", "GiB", "TiB")
    amount = float(value)
    unit = units[0]
    for unit in units:
        if amount < 1024 or unit == units[-1]:
            break
        amount /= 1024
    return f"{amount:.1f} {unit}"


def _print_report(report: ResourceReport) -> None:
    snapshot = report.after
    status = "PASS" if report.ok else "FAIL"
    print(
        f"Resource envelope: {status} · {_format_bytes(snapshot.free_disk_bytes)} free / "
        f"{_format_bytes(snapshot.minimum_free_disk_bytes)} floor · "
        f"{snapshot.max_parallel_jobs} workers · nice +{snapshot.process_nice}"
    )
    for source in snapshot.sources:
        if source.bytes == 0 and source.kind in {"build", "gate"}:
            continue
        mark = "FAIL" if source.bytes > source.budget_bytes else "ok  "
        active = " · active" if source.active else ""
        print(
            f"{mark}  {source.kind:<6} {_format_bytes(source.bytes):>10} / "
            f"{_format_bytes(source.budget_bytes):>10}  {source.owner}{active}"
        )
    for action in report.recovery:
        print(
            f"recover  {action.source} · {_format_bytes(action.removed_bytes)} · "
            f"{action.status}: {action.detail}"
        )
    for issue in snapshot.issues:
        print(f"\n{issue.code}: {issue.detail}\n  next: {issue.action}")
    for warning in snapshot.warnings:
        print(f"\nwarning: {warning}")


def _parse_args(argv: Optional[list[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Measure Loopflow build, cache, Run record, disk, and CPU budgets."
    )
    parser.add_argument("--json", action="store_true", help="emit the complete report as JSON")
    parser.add_argument(
        "--recover",
        action="store_true",
        help="clean only allowlisted inactive/disposable pressure, then remeasure",
    )
    parser.add_argument("--repo", type=Path, default=REPO_ROOT, help="Loopflow checkout to inspect")
    parser.add_argument("--policy", type=Path, default=POLICY_PATH, help="budget policy JSON")
    return parser.parse_args(argv)


def main(argv: Optional[list[str]] = None) -> int:
    args = _parse_args(argv)
    policy = load_policy(args.policy)
    report = inspect_resources(args.repo, policy, args.recover)
    if args.json:
        json.dump(report.as_record(), sys.stdout, indent=2, sort_keys=True)
        sys.stdout.write("\n")
    else:
        _print_report(report)
    return 0 if report.ok else 1


if __name__ == "__main__":
    sys.exit(main())
