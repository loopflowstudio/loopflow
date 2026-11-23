"""Benchmark reports require complete passing cohorts and exclude seed costs."""

import gzip
import importlib
import json
import os
import subprocess
from pathlib import Path
from types import SimpleNamespace

import pytest

ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture
def probe(monkeypatch):
    directory = ROOT / "scripts/benchmarks/crate-rebuild"
    monkeypatch.syspath_prepend(str(directory))
    return importlib.import_module("test_loop")


def test_measurement_worktree_starts_cold_after_the_checkout_hook(probe, tmp_path, monkeypatch):
    repo = tmp_path / "repo"
    subprocess.run(["git", "init", "--quiet", repo], check=True)
    subprocess.run(
        [
            "git",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=f@example.com",
            "commit",
            "--allow-empty",
            "-qm",
            "init",
        ],
        cwd=repo,
        check=True,
    )
    hook = repo / ".git/hooks/post-checkout"
    hook.write_text("#!/bin/sh\nmkdir -p target/debug/deps && touch hook-ran\n")
    hook.chmod(0o755)
    monkeypatch.setattr(probe, "ROOT", repo)
    tree = tmp_path / "measurement"
    probe._add_cold_worktree(tree, "HEAD", os.environ.copy())
    assert (tree / "hook-ran").exists()
    assert not (tree / "target").exists()


def test_consecutive_edits_replace_the_committed_message_in_each_crate(probe, tmp_path):
    tree = tmp_path / "source"
    subprocess.run(["git", "clone", "--quiet", "--shared", "--no-checkout", ROOT, tree], check=True)
    subprocess.run(
        [
            "git",
            "restore",
            "--source=HEAD",
            "--worktree",
            "rust/loopflow/src",
            "rust/loopflow-execution/src",
        ],
        cwd=tree,
        check=True,
    )
    for owner in ("app", "execution"):
        path = tree / probe._edit(tree, owner, "first measurement")
        first = path.read_text()
        probe._edit(tree, owner, "second measurement")
        assert "first measurement" in first
        assert path.read_text() == first.replace("first measurement", "second measurement")


def _receipts(probe):
    rows = []
    for cohort in range(1, 4):
        for mode in ("cache", "incremental"):
            for case in probe.CASES + (["execution_new"] if mode == "cache" else []):
                rows.append(
                    {
                        "cohort": cohort,
                        "mode": mode,
                        "case": case,
                        "exit_code": 0,
                        "tests_passed": 1,
                        "workspace_compilers": [{"kind": "test"}],
                        **dict.fromkeys(
                            [
                                "wall_seconds",
                                "cpu_seconds",
                                "rss_bytes",
                                "tree_rss_bytes",
                                "daemon_cpu_seconds",
                            ],
                            cohort,
                        ),
                    }
                )
    return rows


def test_complete_cohorts_exclude_seed_and_report_range(probe):
    rows = _receipts(probe)
    probe.validate(rows, 3)
    rows.append({**rows[0], "case": "seed_app", "cohort": 0, "wall_seconds": 999})
    summary = probe.summarize(rows)
    assert len(summary) == 13
    assert all(row["n"] == 3 for row in summary)
    assert summary[0]["wall_seconds"] == {"median": 2, "min": 1, "max": 3}
    assert json.loads(json.dumps(summary)) == summary


@pytest.mark.parametrize("change", ["missing", "zero_tests", "duplicate", "no_test_link"])
def test_incomplete_or_nonpassing_samples_cannot_publish(probe, change):
    rows = _receipts(probe)
    if change == "missing":
        rows.pop()
    elif change == "duplicate":
        rows.append(rows[0])
    elif change == "zero_tests":
        rows[0]["tests_passed"] = 0
    else:
        rows[0]["workspace_compilers"] = []
    with pytest.raises(ValueError):
        probe.validate(rows, 3)


@pytest.mark.parametrize(
    "winners,default", [(set(), False), ({"app"}, False), ({"app", "execution"}, True)]
)
def test_report_selects_incremental_only_when_both_edit_totals_win(
    probe, tmp_path, winners, default
):
    report = importlib.import_module("report")
    rows = _receipts(probe)
    for row in rows:
        owner = "app" if row["case"] in probe.CASES[:3] else "execution"
        if owner in winners and row["mode"] == "incremental":
            row["wall_seconds"] *= 0.5
    rows[0]["stats_before"] = {
        "stats": {"compile_requests": 2},
        "basedirs": ["/measurement/a/", "/unrelated-checkout/"],
    }
    source = tmp_path / "source"
    source.mkdir()
    (source / "receipts.json").write_text(json.dumps(rows))
    (source / "configuration.json").write_text(
        json.dumps(
            {
                "cohorts": 3,
                "revision": "fixture",
                "rustc": "fixture rustc",
                "trees": {"a": {"path": "/measurement/a"}},
            }
        )
    )
    output = tmp_path / "results"
    report._test_loop(source, output)
    result = json.loads((output / "loo-921.json").read_text())
    assert result["default_incremental"] is default
    assert result["concurrent_worktree_capacity"] is None
    assert "10 GiB cap" in result["shared_cache_limit"]
    assert len(result["receipts"]) == 39
    raw = json.loads(gzip.decompress((output / "loo-921-raw.json.gz").read_bytes()))
    assert raw["receipts"][0]["stats_before"] == {
        "stats": {"compile_requests": 2},
        "basedir_count": 2,
        "measurement_basedirs": ["/measurement/a/"],
    }
    assert "A Rust edit through a passing test" in (output / "loo-921.html").read_text()
    assert result["shared_cache_limit"] in (output / "loo-921.html").read_text()


def test_resume_complete_measurement_reuses_receipts_without_a_target(
    probe, tmp_path, monkeypatch, capsys
):
    output = tmp_path / "measurement"
    output.mkdir()
    (output / "configuration.json").write_text(json.dumps({"cohorts": 3}))
    (output / "receipts.json").write_text(json.dumps(_receipts(probe)))
    (output / "passed.json").write_text('{"passed": true}')
    monkeypatch.setattr("sys.argv", ["test_loop.py", "--output", str(output), "--resume"])
    probe.main()
    assert "already complete" in capsys.readouterr().out
    assert set(p.name for p in output.iterdir()) == {
        "configuration.json",
        "receipts.json",
        "passed.json",
    }


def test_calibration_cannot_replace_a_missing_warm_cohort(probe):
    rows = _receipts(probe)
    for row in rows:
        row["cohort"] += 1
    rows.append({**rows[0], "cohort": 1, "exit_code": 101, "tests_passed": 0})
    probe.validate(rows, 4, [1])
    with pytest.raises(ValueError):
        probe.validate(rows, 3, [1])


def test_low_disk_stops_before_cache_or_worktree_setup(probe, tmp_path, monkeypatch):
    monkeypatch.setenv("HOME", str(tmp_path / "home"))
    monkeypatch.setenv("CARGO_HOME", str(tmp_path / "cargo"))
    monkeypatch.setattr(probe.shutil, "disk_usage", lambda path: SimpleNamespace(free=0))
    output = tmp_path / "experiment"
    monkeypatch.setattr("sys.argv", ["test_loop.py", "--output", str(output)])
    with pytest.raises(SystemExit, match="repository reserve"):
        probe.main()
    assert not list(output.iterdir())


@pytest.mark.parametrize("complete", [True, False])
def test_resume_after_worktree_loss_uses_only_complete_receipts(
    probe, tmp_path, monkeypatch, complete
):
    state = tmp_path / "configuration.json"
    original = json.dumps({"cohorts": 3, "temporary": str(tmp_path / "gone")})
    state.write_text(original)
    rows = _receipts(probe)
    (tmp_path / "receipts.json").write_text(json.dumps(rows if complete else rows[:-1]))
    monkeypatch.setattr("sys.argv", ["test_loop.py", "--output", str(tmp_path), "--resume"])
    if complete:
        probe.main()
    else:
        with pytest.raises(SystemExit, match="cache and worktrees missing"):
            probe.main()
    assert (tmp_path / "passed.json").exists() == complete
    assert state.read_text() == original
