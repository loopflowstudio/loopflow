"""Resource pressure is attributed and recovery never crosses into durable work."""

import importlib.util
import sys
from pathlib import Path

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
        "maximum_worktree_build_bytes": 100,
        "maximum_aggregate_build_bytes": 150,
        "maximum_run_record_bytes": 100,
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


def test_snapshot_measures_home_run_records_and_names_retention(
    tmp_path: Path, monkeypatch
) -> None:
    repo = tmp_path / "repo"
    repo.mkdir()
    home = tmp_path / "lf-home"
    run_dir = home / "runs" / "12" / "run_1234"
    run_dir.mkdir(parents=True)
    (run_dir / "manifest.json").write_text('{"id":"run_1234"}\n')
    (run_dir / "events.jsonl").write_text('{"type":"usage"}\n')
    monkeypatch.delenv("LF_CONTROL_HOME", raising=False)
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
        _policy(maximum_run_record_bytes=1),
    )

    source = next(source for source in snapshot.sources if source.id == "runs:home")
    issue = next(issue for issue in snapshot.issues if issue.code == "runs:home")
    assert source.kind == "runs"
    assert source.paths == (home / "runs",)
    assert source.bytes > 0
    assert source.disposable is False
    assert issue.owner == "Loopflow Home"
    assert issue.recoverable is False
    assert str(home / "runs") in issue.action
    assert "reconcile" not in issue.action


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

    policy = _policy(maximum_aggregate_build_bytes=1)
    sources = [
        _source(active, id="build:active", owner="active", active=True),
        _source(inactive, id="build:inactive", owner="inactive"),
        resources.ResourceSource(
            id="runs:home",
            kind="runs",
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

    actions = resources.recover_resources(tmp_path, policy, _snapshot(policy, sources))

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
    policy = _policy(maximum_aggregate_build_bytes=1, maximum_recovery_roots=1)
    sources = [
        _source(root, id=f"build:{root.name}", owner=root.name) for root in roots
    ]

    actions = resources.recover_resources(tmp_path, policy, _snapshot(policy, sources))

    assert len(actions) == 1
    assert sum((root / "target").exists() for root in roots) == 1


def test_disk_pressure_prunes_uv_through_its_supported_boundary(
    tmp_path: Path, monkeypatch
) -> None:
    cache = tmp_path / "uv-cache"
    cache.mkdir()
    (cache / "archive").write_bytes(b"x" * 4096)
    policy = _policy(minimum_free_disk_bytes=1_000)
    source = resources.ResourceSource(
        id="cache:uv",
        kind="cache",
        owner="uv",
        root=cache,
        paths=(cache,),
        bytes=resources._allocated_bytes(cache),
        budget_bytes=1_000_000,
        disposable=True,
        active=False,
        action="uv cache prune",
    )
    called = []

    def _prune() -> "resources.RecoveryAction":
        called.append(True)
        return resources.RecoveryAction(
            "cache:uv", "uv", source.bytes, (), "pruned", "supported boundary"
        )

    monkeypatch.setattr(resources, "_prune_uv_cache", _prune)

    actions = resources.recover_resources(
        tmp_path,
        policy,
        _snapshot(policy, [source], free=100),
    )

    assert called == [True]
    assert actions[0].source == "cache:uv"


def test_active_over_budget_build_is_named_but_not_auto_recoverable(tmp_path: Path) -> None:
    root = tmp_path / "active"
    (root / "target").mkdir(parents=True)
    (root / "target/artifact").write_bytes(b"x" * 4096)
    policy = _policy(maximum_worktree_build_bytes=1, maximum_aggregate_build_bytes=1_000_000)
    source = _source(root, id="build:active", active=True, budget=1)

    issues = resources._assess_sources(1_000_000, policy, [source])

    assert len(issues) == 1
    assert issues[0].code == "build:active"
    assert issues[0].recoverable is False
