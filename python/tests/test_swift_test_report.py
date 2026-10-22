import json
from pathlib import Path

from scripts.swift_test_report import report


def _event(kind: str, test: str, at: float) -> str:
    return json.dumps(
        {"kind": "event", "payload": {"kind": kind, "testID": test, "instant": {"absolute": at}}}
    )


def test_names_unfinished_and_slowest_tests(tmp_path: Path) -> None:
    events = tmp_path / "events.jsonl"
    events.write_text(
        "\n".join(
            [
                json.dumps(
                    {"kind": "test", "payload": {"id": "Suite/fast()", "displayName": "Fast"}}
                ),
                json.dumps(
                    {"kind": "test", "payload": {"id": "Suite/slow()", "displayName": "Slow"}}
                ),
                json.dumps(
                    {
                        "kind": "test",
                        "payload": {"id": "World/build()", "displayName": "Builds a world"},
                    }
                ),
                _event("testStarted", "Suite/fast()", 10.0),
                _event("testEnded", "Suite/fast()", 10.5),
                _event("testStarted", "Suite/slow()", 11.0),
                _event("testEnded", "Suite/slow()", 41.0),
                _event("testStarted", "World/build()", 50.0),
                "not json",
            ]
        )
    )
    text = report(events)
    assert "`Builds a world` (World): started 40s into the run" in text
    assert text.index("30.0s `Slow`") < text.index("0.5s `Fast`")


def test_a_clean_run_says_every_test_finished(tmp_path: Path) -> None:
    events = tmp_path / "events.jsonl"
    events.write_text(
        "\n".join([_event("testStarted", "A/a()", 1.0), _event("testEnded", "A/a()", 2.0)])
    )
    assert "Every started test finished." in report(events)
