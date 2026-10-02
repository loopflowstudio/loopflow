"""Resource pressure is attributed and recovery never crosses into durable work."""

import fcntl
import importlib.util
import os
import select
import signal
import subprocess
import sys
import threading
import time
from pathlib import Path
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/resource_envelope.py"

_spec = importlib.util.spec_from_file_location("resource_envelope", SCRIPT)
assert _spec is not None and _spec.loader is not None
resources = importlib.util.module_from_spec(_spec)
sys.modules["resource_envelope"] = resources
_spec.loader.exec_module(resources)


def _policy(**overrides) -> "resources.ResourcePolicy":
    values = {
        "minimum_free_disk_bytes": 100,
        "cleanup_target_free_disk_bytes": 1_000,
        "build_cache_retention_hours": 0,
        "worktree_build_cleanup_bytes": 100,
        "maximum_session_capture_bytes": 100,
        "maximum_uv_cache_bytes": 100,
        "maximum_cargo_cache_bytes": 100,
        "maximum_gate_artifact_bytes": 100,
        "maximum_recovery_roots": 8,
        "maximum_recovery_bytes": 10_000_000,
        "gate_artifact_retention_hours": 168,
        "max_parallel_jobs": 4,
        "process_nice": 10,
        "host_security_cpu_percent": 200.0,
        "host_security_samples": 3,
        "sample_interval_seconds": 5.0,
    }
    values.update(overrides)
    return resources.ResourcePolicy(**values)


def _source(
    root: Path,
    *,
    id: str,
    kind: str = "build",
    owner: str = "feature",
    active: bool = False,
    disposable: bool = True,
    budget: int = 100,
) -> "resources.ResourceSource":
    paths = (root / "target",) if kind == "build" else (root,)
    if kind == "gate":
        paths = (root / resources.GATE_RELATIVE_PATH,)
    return resources.ResourceSource(
        id=id,
        kind=kind,
        owner=owner,
        root=root,
        paths=paths,
        bytes=sum(resources._allocated_bytes(path) for path in paths),
        budget_bytes=budget,
        disposable=disposable,
        active=active,
        action=f"repair {owner}",
    )


def _snapshot(
    policy: "resources.ResourcePolicy",
    sources: list["resources.ResourceSource"],
    free: int = 1_000_000,
) -> "resources.ResourceSnapshot":
    return resources.ResourceSnapshot(
        filesystem="fixture",
        total_disk_bytes=2_000_000,
        free_disk_bytes=free,
        minimum_free_disk_bytes=policy.minimum_free_disk_bytes,
        max_parallel_jobs=policy.max_parallel_jobs,
        process_nice=policy.process_nice,
        sources=tuple(sources),
        issues=tuple(resources._assess_sources(free, policy, sources)),
        warnings=(),
    )


def test_snapshot_measures_home_session_captures_and_names_retention(
    tmp_path: Path, monkeypatch
) -> None:
    repo = tmp_path / "repo"
    repo.mkdir()
    home = tmp_path / "lf-home"
    run_dir = home / "runs" / "12" / "run_1234"
    run_dir.mkdir(parents=True)
    (run_dir / "manifest.json").write_text('{"id":"run_1234"}\n')
    (run_dir / "events.jsonl").write_text('{"type":"usage"}\n')
    monkeypatch.setenv("LF_HOME", str(home))
    monkeypatch.setenv("UV_CACHE_DIR", str(tmp_path / "uv-cache"))
    monkeypatch.setenv("CARGO_HOME", str(tmp_path / "cargo-home"))
    monkeypatch.setattr(
        resources,
        "_discover_worktrees",
        lambda _repo: ([resources.Worktree(repo, "feature")], None),
    )
    monkeypatch.setattr(resources, "_running_cwds", lambda: ({repo}, None))

    snapshot = resources.collect_snapshot(
        repo,
        _policy(maximum_session_capture_bytes=1),
    )

    source = next(source for source in snapshot.sources if source.id == "session-captures:home")
    assert source.kind == "session-captures"
    assert source.paths == (home / "runs",)
    assert source.bytes > 0
    assert source.disposable is False
    assert snapshot.ok
    warning = next(warning for warning in snapshot.warnings if "Loopflow Home" in warning)
    assert str(home / "runs") in warning
    assert "never auto-deleted" in warning


def test_recovery_removes_only_inactive_allowlisted_builds(tmp_path: Path) -> None:
    active = tmp_path / "active"
    inactive = tmp_path / "inactive"
    durable = tmp_path / "home"
    for root in (active, inactive):
        (root / "target").mkdir(parents=True)
        (root / "target/artifact").write_bytes(b"x" * 4096)
        (root / "source.rs").write_text("fn main() {}\n")
        (root / ".git").write_text("gitdir: retained\n")
    run_dir = durable / "runs" / "12" / "run_1234"
    run_dir.mkdir(parents=True)
    (run_dir / "events.jsonl").write_text("durable\n")
    (durable / "loopflow.db").write_bytes(b"sqlite")

    policy = _policy()
    sources = [
        _source(active, id="build:active", owner="active", active=True),
        _source(inactive, id="build:inactive", owner="inactive"),
        resources.ResourceSource(
            id="session-captures:home",
            kind="session-captures",
            owner="Loopflow Home",
            root=durable,
            paths=(durable / "runs",),
            bytes=resources._allocated_bytes(durable / "runs"),
            budget_bytes=1,
            disposable=False,
            active=True,
            action="retain",
        ),
    ]

    actions = resources.recover_resources(policy, _snapshot(policy, sources))

    assert [action.source for action in actions] == ["build:inactive"]
    assert not (inactive / "target").exists()
    assert (active / "target/artifact").exists()
    assert (active / "source.rs").exists()
    assert (inactive / "source.rs").exists()
    assert (inactive / ".git").exists()
    assert (run_dir / "events.jsonl").exists()
    assert (durable / "loopflow.db").exists()


def test_recovery_root_limit_is_a_hard_bound(tmp_path: Path) -> None:
    roots = [tmp_path / "one", tmp_path / "two"]
    for root in roots:
        (root / "target").mkdir(parents=True)
        (root / "target/artifact").write_bytes(b"x" * 4096)
    policy = _policy(maximum_recovery_roots=1)
    sources = [_source(root, id=f"build:{root.name}", owner=root.name) for root in roots]

    actions = resources.recover_resources(policy, _snapshot(policy, sources))

    assert len(actions) == 1
    assert sum((root / "target").exists() for root in roots) == 1


@pytest.mark.parametrize("old_entry", [None, "file", "empty-directory"])
def test_recent_gate_roots_leave_recovery_capacity_for_stale_builds(
    tmp_path: Path, old_entry: str | None
) -> None:
    policy = _policy(maximum_recovery_roots=1, build_cache_retention_hours=24)
    gates = [tmp_path / f"worker-{index}" for index in range(8)]
    for root in gates:
        gate = root / resources.GATE_RELATIVE_PATH
        gate.mkdir(parents=True)
        (gate / "recent.log").write_text("recent verification evidence")
    if old_entry is not None:
        expired = gates[0] / resources.GATE_RELATIVE_PATH / "expired"
        if old_entry == "file":
            expired.write_text("expired output")
        else:
            expired.mkdir()
        old = time.time() - 8 * 24 * 3600
        os.utime(expired, (old, old))
    sources = [_source(root, id=f"gate:{root.name}", kind="gate") for root in gates]
    if old_entry is None:
        assert not resources.recover_resources(policy, _snapshot(policy, sources, free=500))

    stale = tmp_path / "stale"
    (stale / "target").mkdir(parents=True)
    artifact = stale / "target/artifact"
    artifact.write_bytes(b"x" * 4096)
    old = time.time() - 48 * 3600
    for path in (artifact, stale / "target"):
        os.utime(path, (old, old))
    sources.append(_source(stale, id="build:stale"))

    actions = resources.recover_resources(policy, _snapshot(policy, sources, free=500))

    assert len(actions) == 1
    assert artifact.exists() == (old_entry is not None)
    assert actions[0].source == ("build:stale" if old_entry is None else "gate:worker-0")
    assert all((root / resources.GATE_RELATIVE_PATH / "recent.log").exists() for root in gates)
    if old_entry is not None:
        assert not expired.exists()


def test_cleanup_reclaims_stale_builds_before_emergency_reserve(tmp_path: Path) -> None:
    policy = _policy(build_cache_retention_hours=24)
    roots = [tmp_path / name for name in ("stale", "recent", "active")]
    old = time.time() - 48 * 3600
    for root in roots:
        target = root / "target"
        target.mkdir(parents=True)
        artifact = target / "artifact"
        artifact.write_bytes(b"x" * 4096)
        os.utime(artifact, (old, old))
        os.utime(target, (old, old))
    # Updating an existing file does not update its parent's modification time.
    (roots[1] / "target/artifact").write_bytes(b"warm")
    sources = [
        _source(root, id=f"build:{root.name}", active=root.name == "active", budget=10**6)
        for root in roots
    ]
    assert not resources.recover_resources(policy, _snapshot(policy, sources))
    snapshot = _snapshot(policy, sources, free=500)
    assert snapshot.ok  # Above the emergency reserve, below the cleanup target.

    actions = resources.recover_resources(policy, snapshot)

    assert [action.source for action in actions] == ["build:stale"]
    assert not (roots[0] / "target").exists()
    assert (roots[1] / "target/artifact").read_bytes() == b"warm"
    assert (roots[2] / "target/artifact").exists()


def test_concurrent_recovery_skips_busy_cleaner_then_can_reclaim(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(resources, "Path", lambda path: tmp_path if path == "/tmp" else Path(path))
    (tmp_path / "target").mkdir()
    artifact = tmp_path / "target/artifact"
    artifact.write_bytes(b"x" * 4096)
    policy = _policy()
    source = _source(tmp_path, id="build:stale")
    monkeypatch.setattr(resources, "collect_snapshot", lambda *_: _snapshot(policy, [source]))
    lock = resources._lock_recovery()
    assert lock is not None
    try:
        report = resources.inspect_resources(tmp_path, policy, recover=True)
        assert report.ok
        assert not report.recovery
        assert artifact.exists()
    finally:
        lock.close()

    report = resources.inspect_resources(tmp_path, policy, recover=True)
    assert report.recovery[0].status == "removed"
    assert not artifact.exists()


def test_uv_cleanup_preserves_busy_cache_then_prunes_when_idle(tmp_path: Path, monkeypatch) -> None:
    cache = tmp_path / "uv-cache"
    unused = cache / "archive-v0" / "unused"
    unused.mkdir(parents=True)
    payload = unused / "payload"
    payload.write_bytes(b"unused cache entry" * 4096)
    monkeypatch.setenv("UV_CACHE_DIR", str(cache))
    monkeypatch.delenv("UV_LOCK_TIMEOUT", raising=False)

    with (cache / ".lock").open("a+") as lock:
        fcntl.flock(lock, fcntl.LOCK_SH)
        # Bound the regression: a waiting cleaner would prune after this unlock.
        release = threading.Timer(3, lambda: fcntl.flock(lock, fcntl.LOCK_UN))
        release.start()
        try:
            busy = resources._prune_uv_cache()
        finally:
            release.cancel()
            release.join()
        assert busy.status == "failed"
        assert busy.removed_bytes == 0
        assert payload.exists()
        assert "lock" in busy.detail.lower()

    idle = resources._prune_uv_cache()
    assert idle.status == "pruned"
    assert idle.removed_bytes > 0
    assert not payload.exists()


@pytest.mark.parametrize("busy", [False, True], ids=["unlocked", "busy"])
def test_disk_pressure_recovers_builds_even_when_uv_cache_is_busy(
    tmp_path: Path, monkeypatch, busy: bool
) -> None:
    cache = tmp_path / "uv-cache"
    archive = cache / "archive-v0" / "unused"
    archive.mkdir(parents=True)
    (archive / "module.py").write_bytes(b"x" * 4096)
    project = tmp_path / "project"
    (project / "target").mkdir(parents=True)
    (project / "target/artifact").write_bytes(b"x" * 4096)
    (project / "pyproject.toml").write_text(
        '[project]\nname = "cache-lock-fixture"\nversion = "0.0.0"\n'
    )
    for key in tuple(os.environ):
        if key.startswith(("UV_", "_UV_", "LF_", "LOOPFLOW_")) or key == "VIRTUAL_ENV":
            monkeypatch.delenv(key)
    monkeypatch.setenv("UV_CACHE_DIR", str(cache))
    monkeypatch.setenv("UV_OFFLINE", "1")
    monkeypatch.setenv("UV_PYTHON_DOWNLOADS", "never")
    monkeypatch.setenv("UV_NO_CONFIG", "1")
    policy = _policy(minimum_free_disk_bytes=1_000)
    sources = [
        _source(cache, id="cache:uv", kind="cache", owner="uv", budget=1_000_000),
        _source(project, id="build:fixture"),
    ]
    holder = None
    try:
        if busy:
            holder = subprocess.Popen(
                [
                    "uv",
                    "run",
                    "--project",
                    str(project),
                    "--python",
                    sys.executable,
                    "python",
                    "-c",
                    "import select, sys; print('ready', flush=True); "
                    "select.select([sys.stdin], [], [], 10)",
                ],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                start_new_session=True,
            )
            assert select.select([holder.stdout], [], [], 15)[0], "uv fixture did not start"
            assert holder.stdout.readline().strip() == "ready"

        actions = resources.recover_resources(policy, _snapshot(policy, sources, free=100))

        assert [action.source for action in actions] == ["cache:uv", "build:fixture"]
        assert actions[0].status == ("failed" if busy else "pruned")
        assert archive.exists() is busy
        assert not (project / "target").exists()
        assert (project / "pyproject.toml").exists()
        if busy:
            assert "lock" in actions[0].detail.lower()
            assert actions[0].removed_bytes == 0
            assert holder.poll() is None
        else:
            assert actions[0].removed_bytes > 0
    finally:
        if holder is not None:
            try:
                holder.communicate(input="", timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(holder.pid, signal.SIGKILL)
                holder.communicate()

    if busy:
        action = resources._prune_uv_cache()
        assert action.status == "pruned"
        assert not archive.exists()


def test_oversized_active_builds_survive_recovery_and_real_disk_pressure_stops(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    repo, sibling = tmp_path / "current", tmp_path / "landing"
    for root in (repo, sibling):
        (root / "target").mkdir(parents=True)
        (root / "source.rs").write_text("fn main() {}\n")
    (sibling / "target/artifact").write_bytes(b"x" * 4096)
    monkeypatch.setenv("LF_HOME", str(tmp_path / "home"))
    monkeypatch.setenv("UV_CACHE_DIR", str(tmp_path / "uv"))
    monkeypatch.setenv("CARGO_HOME", str(tmp_path / "cargo"))
    monkeypatch.setattr(
        resources,
        "_discover_worktrees",
        lambda _: (
            [resources.Worktree(repo, "current"), resources.Worktree(sibling, "landing")],
            None,
        ),
    )
    monkeypatch.setattr(resources, "_running_cwds", lambda: ({repo, sibling}, None))
    allocated_bytes = resources._allocated_bytes
    monkeypatch.setattr(
        resources,
        "_allocated_bytes",
        lambda path: 200 * 2**30 if path == sibling / "target" else allocated_bytes(path),
    )
    disk = SimpleNamespace(total=1024 * 2**30, free=128 * 2**30)
    monkeypatch.setattr(resources.shutil, "disk_usage", lambda _: disk)
    policy = _policy(worktree_build_cleanup_bytes=1)

    report = resources.inspect_resources(repo, policy, recover=False)
    assert report.ok
    assert not report.recovery
    resources._print_report(report)
    output = capsys.readouterr().out
    assert "Resource envelope: PASS" in output
    assert f"warning: landing ({sibling})" in output
    assert "200.0 GiB" in output
    assert (sibling / "target/artifact").exists()

    # Explicit recovery retains active builds, including the current checkout.
    (repo / "target/artifact").write_bytes(b"x" * 4096)
    report = resources.inspect_resources(repo, policy, recover=True)
    assert report.ok
    assert not report.recovery
    assert (repo / "target/artifact").exists()
    assert (repo / "source.rs").exists()
    assert (sibling / "target/artifact").exists()

    disk.free = 50
    monkeypatch.setattr(resources, "_running_cwds", lambda: ({repo}, None))
    report = resources.inspect_resources(repo, policy, recover=False)
    assert not report.ok
    issue = next(issue for issue in report.after.issues if issue.code == "disk:free")
    assert f"landing: {sibling}" in issue.action
