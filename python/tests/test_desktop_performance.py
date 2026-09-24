from __future__ import annotations

import json
from pathlib import Path

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
