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
    assert len(groups) == len(timings.EXPERIENCES) + 2
    parent = next(
        g for g in groups if g["name"] == "open_task" and g["build"]["revision"] == "parent"
    )
    assert parent["samples"] == 2
    assert len(parent["attempts"]) == 3
    assert parent["acknowledged"]["median_ms"] == 50
    assert parent["usable"]["median_ms"] == 200
    assert parent["usable"]["p95_ms"] is None
    assert parent["outcomes"] == {"completed": 2, "unfinished": 1}
    unverified = next(g for g in groups if g["measurement"] == "unverified")
    assert unverified["outcomes"] == {"unmeasured": 1}
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


def test_capture_bounds_stay_separate_from_presentation_callbacks(tmp_path: Path) -> None:
    journal = tmp_path / "interactions"
    journal.mkdir()
    _write(
        journal / "2026-11-16.ndjson",
        _journey("find_work", "presented", "candidate")
        + _journey("find_work", "captured", "candidate", evidence="captured_frame"),
    )
    groups = timings.summarize(tmp_path)["experiences"]["groups"]
    assert {g["measurement"] for g in groups} == {"capture_upper_bound", "presented_frame"}
    assert all(g["samples"] == 1 for g in groups)


def test_dirty_builds_do_not_pool_between_launches(tmp_path: Path) -> None:
    journal = tmp_path / "interactions"
    journal.mkdir()
    _write(
        journal / "2026-11-16.ndjson",
        _journey("find_work", "one", "candidate-dirty")
        + _journey("find_work", "two", "candidate-dirty"),
    )
    groups = timings.summarize(tmp_path)["experiences"]["groups"]
    assert len(groups) == 2
    assert all(not g["comparable_build"] for g in groups)


def test_live_repositories_do_not_pool_their_journeys(tmp_path: Path) -> None:
    journal = tmp_path / "interactions"
    journal.mkdir()
    rows = []
    for repository in ("loopflow", "manabot", "kata"):
        journey = _journey("find_work", repository, "candidate", evidence="captured_frame")
        for row in journey:
            row["context"] = {**row["context"], "repository": repository}
        rows += journey
    _write(journal / "2026-11-16.ndjson", rows)
    groups = timings.summarize(tmp_path)["experiences"]["groups"]
    assert {g["repository"] for g in groups} == {"loopflow", "manabot", "kata"}
    assert all(g["samples"] == 1 for g in groups)


def test_workflow_stalls_include_queued_time_and_exclude_other_launches(tmp_path: Path) -> None:
    directory = tmp_path / "interactions"
    directory.mkdir()
    journey = _journey("start_task", "start", "candidate", evidence="captured_frame")
    for event in journey:
        event["build"]["launch"] = "current"
        event["at"] = 101 + event["ms"] / 1000
    # Input was recorded after queueing; terminal elapsed time still starts at 101.
    journey[0]["at"] = 101.15
    same = {"event": "main_thread_stall", "build": journey[0]["build"]}
    _write(
        directory / "day.ndjson",
        journey
        + [
            {**same, "at": 101.1, "ms": 100},
            {**same, "at": 101.3, "ms": 250},
            {**same, "at": 100, "ms": 500},
            {**same, "build": {**same["build"], "launch": "other"}, "at": 101.1, "ms": 100},
        ],
    )
    report = timings.summarize(tmp_path)
    group = report["experiences"]["groups"][0]
    assert group["stall_count"] == 2
    assert report["experiences"]["stall_threshold_counts"] == {"50": 4, "100": 4, "250": 2}
    document = timings.render_html(report)
    headline = document.split('<section class="below-fold">')[0]
    assert "Main-thread stalls" in headline
    assert "Complete → row gone" in headline
    assert "Unmeasured · no completed observations yet" in headline


def test_process_start_qualifies_launch_but_not_a_task_click(tmp_path: Path) -> None:
    journal = tmp_path / "interactions"
    journal.mkdir()
    records = []
    for name in ("open_app", "open_task"):
        journey = _journey(name, name, "candidate")
        journey[0]["evidence"] = "process_start"
        journey[0]["inputSource"] = "processStart"
        records.extend(journey)
    _write(journal / "2026-11-16.ndjson", records)
    groups = timings.summarize(tmp_path)["experiences"]["groups"]
    assert next(g for g in groups if g["name"] == "open_app")["samples"] == 1
    assert next(g for g in groups if g["name"] == "open_task")["samples"] == 0


def test_repository_trailing_separator_does_not_duplicate_bars(tmp_path: Path) -> None:
    journal = tmp_path / "interactions"
    journal.mkdir()
    records = []
    for identity, repository in (("one", "/src/loopflow"), ("two", "/src/loopflow/")):
        for row in _journey("open_task", identity, "candidate"):
            row["context"] = {**row["context"], "repository": repository}
            records.append(row)
    _write(journal / "2026-11-16.ndjson", records)
    report = timings.summarize(tmp_path)
    assert len(report["experiences"]["groups"]) == 1
    assert report["experiences"]["groups"][0]["samples"] == 2


def test_rejected_endpoint_stays_visible_without_becoming_a_latency(tmp_path: Path) -> None:
    journal = tmp_path / "interactions"
    journal.mkdir()
    _write(journal / "2026-11-16.ndjson", _journey("open_app", "invalid-focus", "candidate"))
    _write(
        tmp_path / "interaction-validations.ndjson",
        [
            {
                "observation": "invalid-focus",
                "verdict": "rejected",
                "reason": "Menu activation ended a background launch",
            }
        ],
    )
    group = timings.summarize(tmp_path)["experiences"]["groups"][0]
    assert group["samples"] == 0
    assert group["outcomes"] == {"rejected": 1}
    assert group["attempts"][0]["journal_outcome"] == "completed"
    assert "Menu activation" in group["attempts"][0]["validation"]["reason"]


def test_start_nests_machine_steps_and_splits_by_machine_kind(tmp_path: Path) -> None:
    journal = tmp_path / "interactions"
    journal.mkdir()
    rows = []
    starts = {
        "local": "8A0C2C47-6D0B-4F5E-9C55-1F7C0B7A2E01",
        "remote": "8A0C2C47-6D0B-4F5E-9C55-1F7C0B7A2E02",
    }
    for machine, observation in starts.items():
        journey = _journey("start_task", observation, "candidate", evidence="captured_frame")
        for row in journey:
            row["context"] = {**row["context"], "repository": "loopflow", "machine": machine}
        rows += journey
    _write(journal / "2026-11-16.ndjson", rows)
    local = starts["local"].lower()
    log = tmp_path / "lf.log"
    log.write_text(
        "2026-11-16T00:00:00Z  INFO server_request{trace_id=t}:"
        f'task_creation{{request_id="{local}"}}:creation_step{{step="file_task"}}: '
        "loopflow_execution::engine::creation: "
        "Creation step finished elapsed_ms=41.5 pid=1 lf=0.16.0 rev=abc\n"
        f'2026-11-16T00:00:01Z  INFO conversation_setup{{request_id="{local}" step="checkout"}}: '
        "loopflow_execution::ops::human_session::setup: Setup step finished "
        "elapsed_ms=180.25 pid=2 lf=0.16.0 rev=abc\n"
        "2026-11-16T00:00:01Z  WARN loopflow::server::service: unrelated pid=3 lf=0.16.0 rev=abc\n",
        encoding="utf-8",
    )
    groups = timings.summarize(tmp_path, log)["experiences"]["groups"]
    by_machine = {g["machine"]: g for g in groups}
    assert set(by_machine) == {"local", "remote"}
    assert by_machine["local"]["attempts"][0]["machine_steps"] == [
        {"source": "server", "step": "file_task", "ms": 41.5},
        {"source": "conversation", "step": "checkout", "ms": 180.25},
    ]
    assert by_machine["local"]["machine_steps"]["conversation checkout"]["median_ms"] == 180.2
    # A remote Machine's log lives on that Machine.
    assert by_machine["remote"]["attempts"][0]["machine_steps"] == []
    report = timings.summarize(tmp_path, log)
    text = timings.render(report)
    assert "/ remote Machine" in text
    assert "server file_task" in text
    assert "Loopflow · remote" in timings.render_html(report)
