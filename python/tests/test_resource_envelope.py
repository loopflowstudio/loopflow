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


@pytest.fixture
def build_owner(monkeypatch) -> None:
    receipt = (ROOT / "tests/fixtures/dto/build_cleanup_report.json").read_text()
    run = resources.subprocess.run

    def cleanup_or_run(command, *args, **kwargs):
        if command[0] == "fixture-lf":
            return SimpleNamespace(returncode=0, stdout=receipt)
        return run(command, *args, **kwargs)

    monkeypatch.setenv("LF_BIN", "fixture-lf")
    monkeypatch.setattr(resources.subprocess, "run", cleanup_or_run)


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
    warning = next(warning for warning in snapshot.warnings if "Loopflow Machine" in warning)
    assert str(home / "runs") in warning
    assert "never auto-deleted" in warning


@pytest.mark.parametrize("old_entry", [None, "file", "empty-directory"])
def test_recent_gate_roots_leave_recovery_capacity_for_stale_builds(
    tmp_path: Path, old_entry: str | None, build_owner
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
    actions = resources.recover_resources(policy, _snapshot(policy, sources, free=500))

    if old_entry is None:
        assert actions[0].removed_paths == ("/completed/target",)
        assert sum(action.removed_bytes for action in actions) == 4096
    else:
        assert len(actions) == 1
        assert actions[0].source == "gate:worker-0"
    assert all((root / resources.GATE_RELATIVE_PATH / "recent.log").exists() for root in gates)
    if old_entry is not None:
        assert not expired.exists()


def test_concurrent_recovery_skips_busy_cleaner_then_reports_unavailable_build_owner(
    tmp_path: Path, monkeypatch
) -> None:
    monkeypatch.setattr(resources, "Path", lambda path: tmp_path if path == "/tmp" else Path(path))
    (tmp_path / "target").mkdir()
    artifact = tmp_path / "target/artifact"
    artifact.write_bytes(b"x" * 4096)
    policy = _policy()
    source = _source(tmp_path, id="build:stale")
    monkeypatch.setattr(resources, "collect_snapshot", lambda *_: _snapshot(policy, [source]))
    monkeypatch.setenv("LF_BIN", str(tmp_path / "missing-lf"))
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
    assert report.recovery[0].status == "retained"
    assert artifact.exists()


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
    tmp_path: Path, monkeypatch, busy: bool, build_owner
) -> None:
    cache = tmp_path / "uv-cache"
    archive = cache / "archive-v0" / "unused"
    archive.mkdir(parents=True)
    (archive / "module.py").write_bytes(b"x" * 4096)
    project = tmp_path / "project"
    project.mkdir()
    (project / "pyproject.toml").write_text(
        '[project]\nname = "cache-lock-fixture"\nversion = "0.0.0"\n'
    )
    for key in tuple(os.environ):
        if key.startswith(("UV_", "_UV_", "LF_", "LOOPFLOW_")) or key == "VIRTUAL_ENV":
            monkeypatch.delenv(key)
    monkeypatch.setenv("UV_CACHE_DIR", str(cache))
    monkeypatch.setenv("LF_BIN", "fixture-lf")
    monkeypatch.setenv("UV_OFFLINE", "1")
    monkeypatch.setenv("UV_PYTHON_DOWNLOADS", "never")
    monkeypatch.setenv("UV_NO_CONFIG", "1")
    policy = _policy(minimum_free_disk_bytes=1_000)
    sources = [
        _source(cache, id="cache:uv", kind="cache", owner="uv", budget=1_000_000),
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

        assert actions[0].source == "cache:uv"
        assert actions[0].status == ("failed" if busy else "pruned")
        assert archive.exists() is busy
        assert actions[1].removed_paths == ("/completed/target",)
        assert actions[1].removed_bytes == 4096
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


def test_inventory_warns_for_oversized_builds_and_gates_on_disk_pressure(
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

    disk.free = 50
    monkeypatch.setattr(resources, "_running_cwds", lambda: ({repo}, None))
    report = resources.inspect_resources(repo, policy, recover=False)
    assert not report.ok
    issue = next(issue for issue in report.after.issues if issue.code == "disk:free")
    assert f"landing: {sibling}" in issue.action


def test_recovery_consumes_rust_receipt_above_pressure_threshold(
    tmp_path: Path, build_owner
) -> None:
    target = tmp_path / "target"
    target.mkdir()
    artifact = target / "artifact"
    artifact.write_text("retained by this isolated receipt-consumer test")
    policy = _policy()
    actions = resources.recover_resources(policy, _snapshot(policy, []))
    assert sum(action.removed_bytes for action in actions) == 4096
    assert actions[0].removed_paths == ("/completed/target",)
    assert actions[1].status == "retained"
    assert artifact.exists()  # Python has no build-deletion path.


def test_unavailable_build_owner_retains_output(tmp_path: Path, monkeypatch) -> None:
    target = tmp_path / "target"
    target.mkdir()
    (target / "source").write_text("preserve")
    monkeypatch.setenv("LF_BIN", "fixture-lf")

    def run(*_args, **_kwargs):
        raise FileNotFoundError("missing")

    monkeypatch.setattr(resources.subprocess, "run", run)
    actions = resources.recover_resources(_policy(), _snapshot(_policy(), []))
    assert actions[0].status == "retained"
    assert actions[0].removed_bytes == 0
    assert (target / "source").read_text() == "preserve"


@pytest.mark.parametrize("failure", ["exit", "timeout", "invalid-json"])
def test_lost_cleanup_receipt_does_not_claim_output_was_retained(
    tmp_path: Path, monkeypatch, failure: str
) -> None:
    artifact = tmp_path / "artifact"
    artifact.write_text("generated output")
    monkeypatch.setenv("LF_BIN", "fixture-lf")

    def run(*_args, **_kwargs):
        artifact.unlink()
        if failure == "timeout":
            raise subprocess.TimeoutExpired("lf", 120)
        return SimpleNamespace(returncode=1 if failure == "exit" else 0, stdout="invalid")

    monkeypatch.setattr(resources.subprocess, "run", run)
    actions = resources.recover_resources(_policy(), _snapshot(_policy(), []))
    assert not artifact.exists()
    assert actions[0].status == "failed"
    assert "result unknown" in actions[0].detail


def test_inventory_timeout_is_unknown_and_never_authorizes_build_deletion(
    tmp_path: Path, monkeypatch
) -> None:
    target = tmp_path / "target"
    target.mkdir()
    (target / "artifact").write_text("retain")
    monkeypatch.setattr(
        resources.subprocess,
        "run",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(subprocess.TimeoutExpired("du", 5)),
    )
    with pytest.raises(OSError, match="inventory deadline"):
        resources._allocated_bytes(target)
    assert (target / "artifact").exists()
