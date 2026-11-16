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
            {**launch, "event": "exit", "at": "2026-10-29T16:00:00Z", "reason": "quit"},
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
            {"event": "read", "verb": "wave show", "ms": 14000, "ok": True, "app": "1.2.3"},
            {"event": "read", "verb": "wave show", "ms": 90000, "ok": False, "app": "1.2.3"},
            {"event": "refresh", "part": "sessions", "ms": 12000, "ok": True, "app": "1.2.3"},
            {"event": "frame_wait", "part": "planning", "ms": 7000, "ok": True, "app": "1.2.3"},
            {"event": "frame_apply", "part": "planning", "ms": 300, "ok": True, "app": "1.2.3"},
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
    assert version["reads"]["wave show"] == {
        "samples": 1,
        "median_ms": 14000,
        "p95_ms": None,
        "max_ms": 14000,
        "failed": 1,
    }
    assert version["refreshes"]["sessions"]["samples"] == 1
    assert version["frame_waits"]["planning"]["median_ms"] == 7000
    assert version["frame_applies"]["planning"]["median_ms"] == 300
    assert report["versions"]["1.2.4"]["launches"] == 1
    assert "usable_saved" in timings.render(report)


def test_report_without_recordings_says_so(tmp_path: Path) -> None:
    assert "No launches recorded yet" in timings.render(timings.summarize(tmp_path))


def test_live_interactions_keep_superseded_and_unfinished_out_of_latency(tmp_path: Path) -> None:
    records = []
    for index in range(20):
        base = {"app": "new", "name": "navigation", "scenario": "task", "observation": str(index)}
        records += [
            {**base, "phase": "begin", "ms": 0},
            {**base, "phase": "ready", "ms": index + 1},
        ]
    base = {"app": "new", "name": "navigation", "scenario": "task"}
    records += [
        {**base, "observation": "pending", "phase": "begin", "ms": 0},
        {**base, "observation": "replaced", "phase": "begin", "ms": 0},
        {**base, "observation": "replaced", "phase": "superseded", "ms": 9000},
    ]
    _write(tmp_path / "interactions.ndjson", records)
    report = timings.summarize(tmp_path)
    assert report["versions"]["new"]["interactions"]["navigation / task"] == {
        "samples": 20,
        "median_ms": 10.5,
        "p95_ms": 19,
        "max_ms": 20,
        "superseded": 1,
        "unfinished": 1,
    }
    assert "superseded 1; unfinished 1" in timings.render(report)


def test_html_shows_real_values_missing_tails_and_escapes_labels(tmp_path: Path) -> None:
    _write(
        tmp_path / "reads.ndjson",
        [
            {"app": '<script>"', "event": "read", "verb": "task show", "ok": True, "ms": 123.4},
        ],
    )
    document = timings.render_html(timings.summarize(tmp_path))
    assert "123.4" in document
    assert "Unmeasured" in document
    assert "&lt;script&gt;&quot;" in document
    assert 'value="<script>' not in document
    assert "Host load was not recorded" in document
    headline = document.split('<details id="diagnostics">')[0]
    assert "123.4" not in headline
    assert "Open the app" in headline
    assert "Get an answer" in headline
    assert "Unmeasured" in headline
    headline = document.split('<details id="diagnostics">')[0]
    assert "123.4" not in headline
    assert "Get an answer" in headline
    assert "no complete journey observations" in timings.render(timings.summarize(tmp_path))


def _trial(
    path: Path,
    elapsed: float | None,
    *,
    case: str = "cached",
    revision: str = "abc123",
    status: str = "observed; native captures require review",
) -> Path:
    path.write_text(
        json.dumps(
            {
                "metadata": {
                    "provider": "codex",
                    "provider_version": "pinned",
                    "case": case,
                    "conditions": "live store, busy host",
                    "app_revision": revision,
                    "helper_revision": "def456",
                    "machine": "local",
                    "data": "live",
                    "variant": "before",
                },
                "status": status,
                "click_to_observed_response_ms": elapsed,
            }
        )
    )
    return path


def test_all_in_trials_keep_complete_cases_separate_and_retain_failed_attempts(
    tmp_path: Path,
) -> None:
    trials = [_trial(tmp_path / f"{i}.json", 100 + i) for i in range(20)]
    trials += [
        _trial(tmp_path / "failed.json", 9000, status="failed"),
        _trial(tmp_path / "interrupted.json", None, status="unproved"),
        _trial(tmp_path / "resume.json", 500, case="resume"),
        _trial(tmp_path / "after.json", 50, revision="changed"),
    ]
    alias = tmp_path / "alias.json"
    alias.symlink_to(trials[0])
    report = timings.summarize(tmp_path, trials + [trials[0], alias])
    groups = report["experiences"]["groups"]
    assert len(groups) == 3
    assert groups[0]["samples"] == 20
    assert len(groups[0]["attempts"]) == 22
    assert groups[0]["unmeasured"] == 2
    assert groups[0]["median_ms"] == 109.5
    assert groups[0]["p95_ms"] == 118
    assert groups[1]["samples"] == 1
    assert groups[1]["p95_ms"] is None
    document = timings.render_html(report)
    assert "captures require review" in document
    assert "20 / 22 attempts" in document
    assert "2 unmeasured" in document
    assert "changed" in document
    assert trials[0].as_uri() in document


def test_invalid_trial_receipts_are_named_without_manufacturing_latency(tmp_path: Path) -> None:
    malformed = tmp_path / "malformed.json"
    malformed.write_text('{"metadata":')
    invalid = _trial(tmp_path / "invalid.json", float("nan"))
    missing = tmp_path / "missing.json"
    report = timings.summarize(tmp_path, [malformed, invalid, missing])
    assert report["experiences"]["groups"] == []
    assert len(report["experiences"]["errors"]) == 3
    assert str(missing) in timings.render(report)
    assert "nonnegative elapsed time" in timings.render_html(report)
