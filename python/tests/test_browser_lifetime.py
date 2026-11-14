from __future__ import annotations

import datetime as dt
import json
import runpy
from pathlib import Path

import pytest

PROBE = runpy.run_path(
    str(Path(__file__).resolve().parents[2] / "scripts/benchmarks/browser-lifetime/run.py")
)


def _completed() -> list[dict]:
    return [
        {"event": "environment", "display_session": True},
        {"event": "display-ready"},
        {"event": "presentation", "visible": True, "width": 800, "height": 600},
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
        events.pop(3)
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


def test_occluded_or_zero_size_pages_are_not_visible_window_proofs() -> None:
    for field, value in (("visible", False), ("width", 0), ("height", 0)):
        events = _completed()
        events[2][field] = value
        assert PROBE["_result"](0, events)["outcome"] == "incomplete"
    assert PROBE["_result"](2, [{"event": "display-unavailable"}])["outcome"] == "unavailable"
    assert PROBE["_result"](-11, [{"event": "display-unavailable"}])["outcome"] == "crash"


REPORTER = runpy.run_path(
    str(Path(__file__).resolve().parents[2] / "scripts/benchmarks/browser-lifetime/reporter/run.py")
)


def test_os_redacted_crash_paths_match_the_owned_executable_uuid(tmp_path: Path) -> None:
    report = tmp_path / "diagnostic.ips"
    executable = tmp_path / "Diagnostic"
    start = dt.datetime(2026, 11, 14, 17, 0, tzinfo=dt.UTC)
    body = {
        "pid": 42,
        "procName": "Diagnostic",
        "procPath": "/Users/USER/*/Diagnostic",
        "captureTime": "2026-11-14 09:00:01.0000 -0800",
        "usedImages": [{"name": "Diagnostic", "uuid": "1234"}],
    }

    def matches() -> bool:
        report.write_text("{}\n" + json.dumps(body))
        return REPORTER["_ips"](
            report, 42, executable, "1234", start, start + dt.timedelta(seconds=2)
        )

    assert matches()
    for key, value in (
        ("pid", 99),
        ("procName", "Other"),
        ("usedImages", []),
        ("usedImages", [{"name": "Diagnostic", "uuid": "other"}]),
        ("usedImages", [{"name": "Other", "uuid": "1234"}]),
        ("captureTime", "2026-11-14 08:59:59.0000 -0800"),
    ):
        original = body[key]
        body[key] = value
        assert not matches()
        body[key] = original
    report.write_text("{}\n{torn")
    assert not REPORTER["_ips"](report, 42, executable, "1234", start, start)
