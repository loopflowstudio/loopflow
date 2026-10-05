from __future__ import annotations

import importlib.util
import json
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts/benchmarks/desktop-performance/timings.py"
spec = importlib.util.spec_from_file_location("timings", SCRIPT)
timings = importlib.util.module_from_spec(spec)
spec.loader.exec_module(timings)


def _write(path: Path, records: list[dict], junk: str = "") -> None:
    path.write_text(
        junk + "".join(json.dumps(record) + "\n" for record in records), encoding="utf-8"
    )


def test_report_separates_saved_from_fresh_launches_and_counts_failures(tmp_path: Path) -> None:
    launches = []
    for index in range(20):
        launch = {"launch": f"saved-{index}", "app": "1.2.3"}
        launches += [
            {**launch, "event": "launch", "ms": 40},
            {**launch, "event": "restored", "ms": 60, "cache": "hit"},
            {**launch, "event": "first_frame", "ms": 300 + index},
            {**launch, "event": "usable", "ms": 500 + index, "source": "saved"},
            {**launch, "event": "fresh", "ms": 14000},
        ]
    launches += [
        {"launch": "first", "app": "1.2.3", "event": "launch", "ms": 45},
        {"launch": "first", "app": "1.2.3", "event": "restored", "ms": 70, "cache": "miss"},
        {
            "launch": "first",
            "app": "1.2.3",
            "event": "refresh_failed",
            "ms": 9000,
            "part": "planning",
        },
        {"launch": "newer", "app": "1.2.4", "event": "launch", "ms": 50},
    ]
    _write(tmp_path / "launches.ndjson", launches, junk='{"truncated\n')
    _write(
        tmp_path / "reads.ndjson",
        [
            {"event": "read", "verb": "roadmap", "ms": 14000, "ok": True, "app": "1.2.3"},
            {"event": "read", "verb": "roadmap", "ms": 90000, "ok": False, "app": "1.2.3"},
            {"event": "refresh", "part": "sessions", "ms": 12000, "ok": True, "app": "1.2.3"},
        ],
    )

    report = timings.summarize(tmp_path)

    version = report["versions"]["1.2.3"]
    assert version["launches"] == 21
    assert version["cache"] == {"hit": 20, "miss": 1}
    assert version["metrics"]["usable_saved"] == {
        "samples": 20,
        "median_ms": 509.5,
        "p95_ms": 518,
        "max_ms": 519,
        "budget_ms": 1000,
    }
    assert version["metrics"]["usable_fresh"]["samples"] == 0
    assert version["never_usable"] == 1
    assert version["never_fresh"] == 1
    assert version["refresh_failed_before_fresh"] == {"planning": 1}
    assert version["reads"]["roadmap"] == {
        "samples": 1,
        "median_ms": 14000,
        "p95_ms": None,
        "max_ms": 14000,
        "failed": 1,
    }
    assert version["refreshes"]["sessions"]["samples"] == 1
    assert report["versions"]["1.2.4"]["launches"] == 1
    assert "usable_saved" in timings.render(report)


def test_report_without_recordings_says_so(tmp_path: Path) -> None:
    assert "No launches recorded yet" in timings.render(timings.summarize(tmp_path))
