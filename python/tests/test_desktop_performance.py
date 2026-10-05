from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

from scripts import desktop_performance as performance


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


def _soak_events() -> list[dict]:
    events = _events()
    events[0]["soak_seconds"] = 3600
    events += [{"event": "soak_begin", "seconds": 3600}]
    events += [
        {"event": "soak_round", "round": i, "time": i + 10, "preserved": True} for i in range(4)
    ]
    events += [{"event": "soak_end", "elapsed_seconds": 3600, "preserved": True}]
    return events


def test_soak_reports_missing_resources_without_inventing_measurements(tmp_path: Path) -> None:
    result = _report(tmp_path, _soak_events())
    assert result["status"] == "complete"
    assert result["soak"]["resources"] is None
    assert result["soak"]["memory_after_four_rounds_mib"] is None


@pytest.mark.parametrize("failure", ["interrupted", "short", "lost_draft", "no_rounds"])
def test_soak_cannot_pass_without_duration_and_preservation(tmp_path: Path, failure: str) -> None:
    events = _soak_events()
    if failure == "interrupted":
        events.pop()
    elif failure == "short":
        events[-1]["elapsed_seconds"] = 3599
    elif failure == "lost_draft":
        events[-2]["preserved"] = False
    else:
        events = [event for event in events if event["event"] != "soak_round"]
    result = _report(tmp_path, events)
    assert result["status"] == "incomplete"
    assert result["soak"]["status"] == "incomplete"


def test_four_round_memory_uses_the_round_boundary(tmp_path: Path) -> None:
    directory = tmp_path / "soak-resources"
    directory.mkdir()
    (directory / "rss.jsonl").write_text(
        "\n".join(
            json.dumps(row)
            for row in [
                {"t": 9, "rss_kib": 1024},
                {"t": 13, "rss_kib": 3072},
                {"t": 99, "rss_kib": 9000},
            ]
        )
    )
    result = _report(tmp_path, _soak_events())
    assert result["soak"]["memory_after_four_rounds_mib"] == 2


def test_cli_volume_keeps_partial_counts_and_missing_measurements(tmp_path: Path) -> None:
    assert performance._cli_volume(tmp_path)["observed_totals"]["statements"] is None
    start = {
        "event": "start",
        "pid": 7,
        "time": 10,
        "elapsed_ms": 0,
        "connections": 0,
        "statements": 0,
        "rows": 0,
    }
    sample = {
        **start,
        "event": "sample",
        "time": 11,
        "elapsed_ms": 1000,
        "connections": 2,
        "statements": 12,
        "rows": 41,
    }
    path = tmp_path / "lf-7-owned.jsonl"
    path.write_text("\n".join(json.dumps(e) for e in [start, sample]) + '\n{"event":')
    partial = performance._cli_volume(tmp_path)
    assert partial["status"] == "partial"
    assert partial["processes_ended"] == 0
    assert partial["observed_totals"]["statements"] == 12
    assert partial["errors"]
    path.write_text("\n".join(json.dumps(e) for e in [start, sample, {**sample, "event": "end"}]))
    complete = performance._cli_volume(tmp_path)
    assert complete["status"] == "complete"
    assert complete["processes_started"] == complete["processes_ended"] == 1
    assert complete["observed_totals"]["statements"] == 12  # cumulative, not 24


@pytest.mark.parametrize("change", [{"statements": 1}, {"pid": 8}, {"elapsed_ms": -1}])
def test_cli_volume_rejects_counter_reset_or_identity_change(tmp_path: Path, change: dict) -> None:
    start = {
        "event": "start",
        "pid": 7,
        "time": 10,
        "elapsed_ms": 0,
        "connections": 1,
        "statements": 12,
        "rows": 41,
    }
    end = {**start, "event": "end", **change}
    (tmp_path / "lf-7-owned.jsonl").write_text("\n".join(json.dumps(e) for e in [start, end]))
    result = performance._cli_volume(tmp_path)
    assert result["status"] == "partial"
    assert result["errors"]
    assert result["processes_ended"] == 0


def test_changed_cli_binary_invalidates_completed_journey(tmp_path: Path) -> None:
    _report(tmp_path, _events())
    metadata = json.loads((tmp_path / "run.json").read_text())
    metadata.update(cli_sha256="before", cli_sha256_after="after")
    (tmp_path / "run.json").write_text(json.dumps(metadata))
    assert performance._report(tmp_path, None)["status"] == "incomplete"


def test_native_runner_stops_owned_child_after_unexpected_journal_error(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    # The child announces a malformed soak start, then waits for its owner to stop it.
    child = """
import os
import signal
import time
from pathlib import Path

journal = Path(os.environ["LF_DESKTOP_PERF_OUTPUT"])
def stopped(*_):
    journal.with_suffix(".stopped").touch()
    raise SystemExit(0)
signal.signal(signal.SIGTERM, stopped)
journal.write_text('{"event":"soak_begin"}\\n')
time.sleep(60)
"""
    monkeypatch.setattr(performance, "COMMAND", [sys.executable, "-c", child])
    monkeypatch.setattr(
        performance, "_prepare_native_fixture", lambda output, cli: output / "fixture"
    )

    with pytest.raises(KeyError, match="pid"):
        performance._run_native(tmp_path, samples=1, soak_seconds=1, cli=tmp_path / "unused-cli")

    assert (tmp_path / "attempts.stopped").exists()
