from __future__ import annotations

import hashlib
import json
import os
import signal
import sqlite3
import subprocess
import sys
import time
from pathlib import Path

import pytest

from scripts import desktop_performance as performance


@pytest.mark.parametrize("termination", ["signal", "timeout"])
def test_interruption_stops_owned_benchmark_and_returns_outcome(
    tmp_path: Path, termination: str
) -> None:
    ready = tmp_path / "ready"
    child = (
        "import os, time; from pathlib import Path; "
        f"Path({str(ready)!r}).write_text(str(os.getpid())); time.sleep(60)"
    )
    script = (
        "import json, os, sys; from scripts import desktop_performance as p; "
        f"result = p._run_process([sys.executable, '-c', {child!r}], "
        f"os.environ.copy(), sys.stderr, {2 if termination == 'timeout' else 30}); "
        "print(json.dumps(result))"
    )
    runner = subprocess.Popen(
        [sys.executable, "-c", script],
        cwd=performance.REPO,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    child_pid = None
    try:
        deadline = time.monotonic() + 10
        while not ready.exists() and time.monotonic() < deadline:
            time.sleep(0.01)
        assert ready.exists(), "Owned benchmark did not start"
        child_pid = int(ready.read_text())
        if termination == "signal":
            runner.send_signal(signal.SIGTERM)
        stdout, stderr = runner.communicate(timeout=10)
        assert runner.returncode == 0, stderr
        code, outcome = json.loads(stdout)
        assert code != 0
        assert outcome == ("interrupted" if termination == "signal" else "timeout")
        with pytest.raises(ProcessLookupError):
            os.kill(child_pid, 0)
    finally:
        if runner.poll() is None:
            runner.kill()
            runner.wait()
        if child_pid is not None:
            try:
                os.killpg(child_pid, signal.SIGKILL)
            except ProcessLookupError:
                pass


def test_snapshot_preserves_committed_wal_history_without_mutating_source(tmp_path: Path) -> None:
    source = tmp_path / "live.db"
    with sqlite3.connect(source) as database:
        database.execute("PRAGMA journal_mode=WAL")
        database.execute("CREATE TABLE history(id INTEGER PRIMARY KEY, text TEXT)")
        database.executemany(
            "INSERT INTO history(text) VALUES (?)", [(f"event {i}",) for i in range(1000)]
        )
        database.commit()
        output = tmp_path / "snapshot"
        performance._snapshot(source, output)
        database.execute("INSERT INTO history(text) VALUES ('later')")
        database.commit()
        with sqlite3.connect(output / "loopflow.db") as copy:
            assert copy.execute("SELECT count(*) FROM history").fetchone()[0] == 1000
            assert (
                copy.execute("SELECT text FROM history WHERE id=1000").fetchone()[0] == "event 999"
            )
        assert database.execute("SELECT count(*) FROM history").fetchone()[0] == 1001
    manifest = json.loads((output / "snapshot.json").read_text())
    assert manifest["counts"]["history"] == 1000
    assert manifest["sha256"] == hashlib.sha256((output / "loopflow.db").read_bytes()).hexdigest()
    assert (output.stat().st_mode & 0o777) == 0o700


def test_snapshot_comparison_rejects_different_population(tmp_path: Path) -> None:
    baseline = _report(tmp_path, _events())
    current = _report(tmp_path, _events())
    baseline["metadata"]["snapshot"] = {"sha256": "before"}
    current["metadata"]["snapshot"] = {"sha256": "different"}
    assert performance._comparison(current, baseline) == {
        "available": False,
        "reason": "Different snapshot",
    }


def test_comparison_retains_observed_timeouts(tmp_path: Path) -> None:
    events = _events()
    events[-1]["outcome"] = "timeout"
    baseline = _report(tmp_path, events)
    current = _report(tmp_path, _events())
    comparison = performance._comparison(current, baseline)
    assert comparison["available"]
    assert comparison["deltas"][1]["before_failure_rate"] == 1 / 20
    assert comparison["deltas"][1]["after_failure_rate"] == 0


def _events(samples: int = 21) -> list[dict]:
    events = [
        {
            "event": "plan",
            "population_version": "test-v1",
            "populations": {"small": 8},
            "samples": samples,
            "scenarios": ["full"],
            "endpoint": "ax",
            "poll_interval_ms": 5,
        }
    ]
    for index in range(samples):
        start = {
            "event": "begin",
            "id": str(index),
            "metric": "hierarchy_interaction_ms",
            "scenario": "full",
            "population": "small",
            "attempt": index,
            "state": "first_interaction" if index == 0 else "warm",
        }
        events.extend(
            [start, {**start, "event": "end", "duration_ms": index + 1, "outcome": "passed"}]
        )
    return events


def _report(tmp_path: Path, events: list[dict], *, stable: bool = True) -> dict:
    (tmp_path / "run.json").write_text(
        json.dumps(
            {
                "exit_code": 0,
                "source_before": "before",
                "source_after": "before" if stable else "after",
                "host": "host-a",
                "build_mode": "debug",
                "command": ["native-test"],
                "measurement_source": "same-runner",
            }
        )
    )
    (tmp_path / "attempts.jsonl").write_text("".join(json.dumps(event) + "\n" for event in events))
    return performance._report(tmp_path, None)


def test_interrupted_attempt_and_unstarted_journeys_remain_visible(tmp_path: Path) -> None:
    events = _events(3)
    result = _report(tmp_path, events[:4])  # One result, next attempt started, third never reached.
    assert result["status"] == "incomplete"
    assert result["not_started"] == 1
    assert [attempt["outcome"] for attempt in result["attempts"]] == ["passed", "interrupted"]
    assert result["groups"][1]["failure_rate"] == 1
    assert result["groups"][1]["p50_ms"] is None


def test_percentiles_separate_first_interaction_and_require_twenty_warm_samples(
    tmp_path: Path,
) -> None:
    result = _report(tmp_path, _events())
    assert result["status"] == "complete"
    first, warm = result["groups"]
    assert first["p50_ms"] == 1
    assert first["p95_ms"] is None
    assert warm["passed"] == 20
    assert warm["p50_ms"] == 11.5
    assert warm["p95_ms"] == 20
    result = _report(tmp_path, _events(20))
    assert result["groups"][1]["p95_ms"] is None


def test_comparison_rejects_source_drift_or_different_endpoints(tmp_path: Path) -> None:
    baseline = _report(tmp_path, _events())
    current = _report(tmp_path, _events(), stable=False)
    assert performance._comparison(current, baseline)["available"] is False
    current = _report(tmp_path, _events())
    assert performance._comparison(current, baseline)["deltas"][0]["p50_delta_ms"] == 0
    current["plan"]["endpoint"] = "model-assignment"
    assert performance._comparison(current, baseline) == {
        "available": False,
        "reason": "Different endpoint",
    }


@pytest.mark.parametrize("replacement", ["unplanned", "duplicate"])
def test_completion_requires_each_planned_observation(tmp_path: Path, replacement: str) -> None:
    events = _events(2)
    for event in events[-2:]:
        if replacement == "unplanned":
            event["scenario"] = "unplanned"
        else:
            event["attempt"] = 0
            event["state"] = "first_interaction"
    result = _report(tmp_path, events)
    assert result["status"] == "incomplete"
    assert result["not_started"] == 1
    assert result["journal_errors"]


@pytest.mark.parametrize("defect", ["duplicate_end", "orphan_end", "changed_subject"])
def test_ambiguous_results_cannot_complete_a_run(tmp_path: Path, defect: str) -> None:
    events = _events(1)
    if defect == "duplicate_end":
        events.append({**events[-1], "outcome": "failed"})
    elif defect == "orphan_end":
        events.append({**events[-1], "id": "orphan"})
    else:
        events[-1]["population"] = "different"
    result = _report(tmp_path, events)
    assert result["status"] == "incomplete"
    assert result["journal_errors"]
