from __future__ import annotations

import runpy
from pathlib import Path

import pytest

PROBE = runpy.run_path(
    str(Path(__file__).resolve().parents[2] / "scripts/benchmarks/browser-lifetime/run.py")
)


def _completed() -> list[dict]:
    return [
        {"event": "environment", "display_session": True},
        {"event": "final-release", "page": "first"},
        {"event": "complete", "created": 1, "retained": 0, "retiring": 0},
    ]


def test_missing_completion_is_unknown_even_when_child_exits_zero() -> None:
    result = PROBE["_result"](0, _completed()[:-1])
    assert result["outcome"] == "incomplete"
    assert result["retained"] is None


@pytest.mark.parametrize(
    "problem", ["missing_release", "leaked_page", "no_display", "content_death", "wrong_undo"]
)
def test_failed_lifetime_evidence_never_passes(problem: str) -> None:
    events = _completed()
    if problem == "missing_release":
        events.pop(1)
    elif problem == "leaked_page":
        events[-1]["retained"] = 1
    elif problem == "no_display":
        events[0]["display_session"] = False
    elif problem == "content_death":
        events.append({"event": "web-content-terminated"})
    else:
        events.append({"event": "undo", "same_page": False})
    assert PROBE["_result"](0, events)["outcome"] == "incomplete"


def test_crash_timeout_and_completed_reclamation_are_distinct() -> None:
    assert PROBE["_result"](-11, _completed())["outcome"] == "crash"
    assert PROBE["_result"](-9, _completed(), timed_out=True)["outcome"] == "timeout"
    assert PROBE["_result"](0, _completed())["outcome"] == "pass"
