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

    version = report["versions"]["1.2.3 (unattributed)"]
    assert version["launches"] == 21
    assert version["cache"] == {"hit": 20, "miss": 1}
    assert version["metrics"]["usable_saved"] == {
        "samples": 20,
        "median_ms": 509.5,
        "p95_ms": 518,
        "max_ms": 519,
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
    assert report["versions"]["1.2.4 (unattributed)"]["launches"] == 1
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
    assert report["versions"]["new (unattributed)"]["interactions"]["navigation / task"] == {
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
    assert "no interaction journal observations" in timings.render(timings.summarize(tmp_path))


def _journey(
    name: str,
    identity: str,
    revision: str,
    *,
    evidence: str = "presented_frame",
    outcome: str = "completed",
) -> list[dict]:
    base = {
        "schema": 1,
        "observation": identity,
        "name": name,
        "scenario": "cached",
        "build": {"app": "1.0", "revision": revision, "platform": "macOS", "launch": identity},
        "context": {"thermalState": 0, "liveSessions": 4},
        "work": {
            "cliReads": 1,
            "servedReads": 0,
            "projections": 2,
            "sqlStatements": 12,
            "sqlRows": 25,
        },
    }
    return [
        {**base, "phase": "input", "evidence": "input_event", "inputSource": "nsevent", "ms": 0},
        {**base, "phase": "acknowledged", "evidence": evidence, "ms": 50},
        {**base, "phase": "usable", "evidence": evidence, "ms": 200},
        {**base, "phase": "outcome", "evidence": "unavailable", "ms": 210, "outcome": outcome},
    ]


def test_all_experiences_come_from_journal_and_revisions_do_not_pool(tmp_path: Path) -> None:
    journal = tmp_path / "interactions"
    journal.mkdir()
    events = []
    for name, *_ in timings.EXPERIENCES:
        events += _journey(name, name, "parent")
    events += _journey("open_task", "candidate", "candidate")
    events += _journey("open_task", "same-parent-next-launch", "parent")
    events += _journey(
        "open_task", "state-only", "parent", evidence="app_state", outcome="unmeasured"
    )
    events += _journey("open_task", "unfinished", "parent")[:-1]
    _write(journal / "2026-11-16.ndjson", events)
    report = timings.summarize(tmp_path)
    groups = report["experiences"]["groups"]
    assert len(groups) == len(timings.EXPERIENCES) + 1
    parent = next(
        g for g in groups if g["name"] == "open_task" and g["build"]["revision"] == "parent"
    )
    assert parent["samples"] == 2
    assert len(parent["attempts"]) == 4
    assert parent["acknowledged"]["median_ms"] == 50
    assert parent["usable"]["median_ms"] == 200
    assert parent["usable"]["p95_ms"] is None
    assert parent["outcomes"] == {"completed": 2, "unmeasured": 1, "unfinished": 1}
    assert parent["attempts"][0]["work"]["sqlRows"] == 25
    document = timings.render_html(report)
    assert "Ack median ms" in document
    assert "Usable P95 ms" in document
    assert "candidate" in document and "parent" in document
    assert 'class="label"' not in document


def test_internal_journals_join_launch_revision_instead_of_version(tmp_path: Path) -> None:
    _write(
        tmp_path / "launches.ndjson",
        [
            {"app": "1.0", "launch": "a", "rev": "parent", "event": "launch", "ms": 1},
            {"app": "1.0", "launch": "a", "event": "usable", "source": "saved", "ms": 100},
            {"app": "1.0", "launch": "b", "rev": "candidate", "event": "launch", "ms": 1},
            {"app": "1.0", "launch": "b", "event": "usable", "source": "saved", "ms": 200},
        ],
    )
    _write(
        tmp_path / "reads.ndjson",
        [
            {
                "app": "1.0",
                "launch": "a",
                "event": "read",
                "verb": "task show",
                "ms": 10,
                "ok": True,
            },
            {
                "app": "1.0",
                "rev": "candidate",
                "event": "read",
                "verb": "task show",
                "ms": 20,
                "ok": True,
            },
        ],
    )
    builds = timings.summarize(tmp_path)["versions"]
    assert builds["1.0 @ parent"]["metrics"]["usable_saved"]["median_ms"] == 100
    assert builds["1.0 @ candidate"]["metrics"]["usable_saved"]["median_ms"] == 200
    assert builds["1.0 @ parent"]["reads"]["task show"]["median_ms"] == 10
    assert builds["1.0 @ candidate"]["reads"]["task show"]["median_ms"] == 20


def test_damaged_daily_rows_do_not_hide_other_observations(tmp_path: Path) -> None:
    journal = tmp_path / "interactions"
    journal.mkdir()
    _write(
        journal / "2026-11-16.ndjson",
        [
            [],
            {"schema": 1},
            {"event": "main_thread_stall", "ms": -1},
            {"event": "main_thread_stall", "ms": 90},
            *_journey("find_work", "good", "candidate"),
        ],
    )
    report = timings.summarize(tmp_path)
    assert len(report["experiences"]["errors"]) == 3
    assert report["experiences"]["groups"][0]["samples"] == 1
    assert report["experiences"]["stall_summary"]["median_ms"] == 90


def test_unknown_revision_cannot_pool_launches_or_prove_a_comparison(tmp_path: Path) -> None:
    journal = tmp_path / "interactions"
    journal.mkdir()
    _write(
        journal / "2026-11-16.ndjson",
        _journey("open_task", "one", "unknown") + _journey("open_task", "two", "unknown"),
    )
    groups = timings.summarize(tmp_path)["experiences"]["groups"]
    assert len(groups) == 2
    assert all(group["samples"] == 0 for group in groups)
