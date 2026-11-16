"""Benchmark reports require complete passing cohorts and exclude seed costs."""

import importlib.util
import json
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture
def probe(monkeypatch):
    directory = ROOT / "scripts/benchmarks/crate-rebuild"
    monkeypatch.syspath_prepend(str(directory))
    spec = importlib.util.spec_from_file_location("crate_test_loop", directory / "test_loop.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


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
