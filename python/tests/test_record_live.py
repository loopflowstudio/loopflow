from __future__ import annotations

import importlib.util
import json
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts/benchmarks/desktop-performance/record_live.py"
spec = importlib.util.spec_from_file_location("record_live", SCRIPT)
record_live = importlib.util.module_from_spec(spec)
spec.loader.exec_module(record_live)


def _signpost(kind: str, name: str, ident: int, at: str, message: str) -> str:
    return json.dumps(
        {
            "eventType": "signpostEvent",
            "signpostType": kind,
            "signpostName": name,
            "signpostID": ident,
            "timestamp": at,
            "eventMessage": message,
            "subsystem": "studio.loopflow",
            "category": "perf",
        }
    )


HITCHES = """<?xml version="1.0"?><trace-query-result><node><schema name="hitches"/>
<row><start-time>322452541</start-time><duration id="2">25000000</duration></row>
<row><start-time>1689121166</start-time><duration>75000000</duration></row>
<row><start-time>2000000000</start-time><duration ref="2"/></row>
</node></trace-query-result>"""


def test_summarize_pairs_signposts_and_scores_hitches(tmp_path: Path) -> None:
    lines = [
        _signpost(
            "begin",
            "task_workspace_ready",
            1,
            "2026-09-26 10:00:00.000000-0700",
            "scenario=task id=a ",
        ),
        _signpost("end", "task_workspace_ready", 1, "2026-09-26 10:00:00.040000-0700", "ready"),
        _signpost(
            "begin",
            "task_workspace_ready",
            2,
            "2026-09-26 10:00:01.000000-0700",
            "scenario=task id=b ",
        ),
        _signpost("end", "task_workspace_ready", 2, "2026-09-26 10:00:01.120000-0700", "ready"),
        _signpost(
            "begin",
            "task_workspace_ready",
            3,
            "2026-09-26 10:00:02.000000-0700",
            "scenario=session id=c ",
        ),
        _signpost(
            "end", "task_workspace_ready", 3, "2026-09-26 10:00:02.001000-0700", "superseded"
        ),
        _signpost("begin", "lf", 4, "2026-09-26 10:00:03.000000-0700", "session list"),
        _signpost("end", "lf", 4, "2026-09-26 10:00:03.300000-0700", ""),
        json.dumps(
            {"eventType": "logEvent", "eventMessage": "metric=time_to_live_pane value_ms=88.5"}
        ),
    ]
    (tmp_path / "signposts.ndjson").write_text("\n".join(lines) + "\n")
    (tmp_path / "rss.jsonl").write_text(
        "\n".join(json.dumps({"t": i, "rss_kib": 200_000 + i * 1024}) for i in range(5))
    )
    (tmp_path / "hitches.xml").write_text(HITCHES)
    (tmp_path / "run.json").write_text(
        json.dumps(
            {
                "process": "Loopflow",
                "pid": 1,
                "seconds": 10,
                "started_at": "2026-09-26T10:00:00",
                "ended_at": "2026-09-26T10:00:10",
                "xctrace": "recorded",
                "host": {"macos": "26", "cpu": "test"},
            }
        )
    )

    report = record_live.summarize(tmp_path)

    by_key = {(row["name"], row["scenario"]): row for row in report["intervals"]}
    assert by_key[("task_workspace_ready", "task")]["n"] == 2
    assert by_key[("task_workspace_ready", "task")]["p50_ms"] == 80.0
    assert by_key[("task_workspace_ready", "task")]["p95_ms"] is None
    assert by_key[("task_workspace_ready", "task")]["superseded"] == 0
    assert by_key[("task_workspace_ready", "session")] == {
        "name": "task_workspace_ready",
        "scenario": "session",
        "n": 0,
        "p50_ms": None,
        "p95_ms": None,
        "max_ms": None,
        "superseded": 1,
    }
    assert by_key[("lf", "session list")]["max_ms"] == 300.0
    assert by_key[("time_to_live_pane", "log")]["p50_ms"] == 88.5
    assert report["hitches"] == {
        "recorded": True,
        "count": 3,
        "hitch_ms_per_second": 12.5,
        "worst_ms": 75.0,
        "hangs": None,
        "worst_hang_ms": None,
    }
    assert report["memory"]["growth_mib"] == 4.0
    assert report["cpu"]["samples"] == 0
    assert report["cpu"]["p50_percent"] is None
    assert (
        "| task_workspace_ready | task | 2 | 80.0 | None |" in (tmp_path / "report.md").read_text()
    )


@pytest.mark.parametrize("count", [19, 20])
def test_cpu_and_latency_percentiles_require_twenty_samples(tmp_path: Path, count: int) -> None:
    (tmp_path / "run.json").write_text(
        json.dumps(
            {
                "process": "Loopflow",
                "pid": 1,
                "seconds": count,
                "started_at": "2026-10-05T10:00:00",
                "xctrace": "skipped",
                "host": {"macos": "26", "cpu": "test"},
            }
        )
    )
    (tmp_path / "rss.jsonl").write_text(
        "\n".join(
            json.dumps({"t": i, "rss_kib": 200_000, "cpu_percent": float(i)}) for i in range(count)
        )
    )
    (tmp_path / "signposts.ndjson").write_text(
        "\n".join(
            _signpost(kind, "lf", i, f"2026-10-05 10:00:{i:02d}.{micros}-0700", message)
            for i in range(count)
            for kind, micros, message in [
                ("begin", "000000", "session list"),
                ("end", "100000", "ready"),
            ]
        )
    )

    report = record_live.summarize(tmp_path)

    assert report["cpu"] == {
        "samples": count,
        "p50_percent": (count - 1) / 2,
        "p95_percent": 18.0 if count == 20 else None,
        "max_percent": count - 1,
    }
    assert report["intervals"][0]["p95_ms"] == (100.0 if count == 20 else None)


@pytest.mark.parametrize("xml", [None, "", "<trace-query-result/>", "not XML"])
def test_missing_hitch_table_is_unmeasured(tmp_path: Path, xml: str | None) -> None:
    (tmp_path / "run.json").write_text(
        json.dumps(
            {
                "process": "Loopflow",
                "pid": 1,
                "seconds": 10,
                "started_at": "2026-09-26T10:00:00",
                "xctrace": "recorded",
                "host": {"macos": "26", "cpu": "test"},
            }
        )
    )
    if xml is not None:
        (tmp_path / "hitches.xml").write_text(xml)
    report = record_live.summarize(tmp_path)
    assert report["hitches"]["count"] is None
    assert report["hitches"]["hitch_ms_per_second"] is None
    assert "Hitches: not recorded." in (tmp_path / "report.md").read_text()


def test_empty_recorded_hitch_table_measures_zero(tmp_path: Path) -> None:
    (tmp_path / "run.json").write_text(
        json.dumps(
            {
                "process": "Loopflow",
                "pid": 1,
                "seconds": 10,
                "started_at": "2026-09-26T10:00:00",
                "xctrace": "recorded",
                "host": {"macos": "26", "cpu": "test"},
            }
        )
    )
    (tmp_path / "hitches.xml").write_text(
        '<trace-query-result><node><schema name="hitches"/></node></trace-query-result>'
    )
    report = record_live.summarize(tmp_path)
    assert report["hitches"]["recorded"] is True
    assert report["hitches"]["count"] == 0
    assert report["hitches"]["hitch_ms_per_second"] == 0.0


def test_idle_hitches_use_trace_clock_and_clip_phase_boundaries(tmp_path: Path) -> None:
    origin = 1791223200.0
    (tmp_path / "phases.jsonl").write_text(
        "\n".join(
            json.dumps(row)
            for row in [
                {"event": "soak_phase", "phase": "idle", "time": origin},
                {"event": "soak_phase", "phase": "navigation_typing", "time": origin + 2},
                {"event": "soak_end", "time": origin + 4},
            ]
        )
    )
    start = datetime.fromtimestamp(origin, timezone.utc)
    end = datetime.fromtimestamp(origin + 4, timezone.utc)
    (tmp_path / "trace-toc.xml").write_text(
        f'<trace-toc><run number="1"><info><run-info><start-date>{start.isoformat()}</start-date>'
        f"<end-date>{end.isoformat()}</end-date></run-info></info></run></trace-toc>"
    )
    rows = [
        {"start_ms": 1000, "duration_ms": 100},
        {"start_ms": 1900, "duration_ms": 200},
        {"start_ms": 2500, "duration_ms": 500},
    ]
    result = record_live._idle(tmp_path, rows, rows)
    assert result["requested_seconds"] == result["covered_seconds"] == 2
    assert result["hitch_ms_per_second"] == pytest.approx(100, abs=0.001)
    assert result["hangs"] == 2


def test_idle_clock_or_table_absence_never_means_zero_hitches(tmp_path: Path) -> None:
    (tmp_path / "phases.jsonl").write_text(
        "\n".join(
            json.dumps(row)
            for row in [
                {"event": "soak_phase", "phase": "idle", "time": 10},
                {"event": "soak_end", "time": 20},
            ]
        )
    )
    result = record_live._idle(tmp_path, [], [])
    assert result["requested_seconds"] == 10
    assert result["covered_seconds"] is None
    assert result["hitch_ms_per_second"] is None
    assert result["hangs"] is None


def test_recording_storage_limit_retains_failure_and_stops_owned_child(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    real_popen = subprocess.Popen
    free = iter([10 * 1024**3, 7 * 1024**3])
    monkeypatch.setattr(record_live.shutil, "which", lambda _: sys.executable)
    monkeypatch.setattr(
        record_live.shutil, "disk_usage", lambda _: shutil._ntuple_diskusage(0, 0, next(free))
    )

    def launch(*args, **kwargs):
        return real_popen([sys.executable, "-c", "import time; time.sleep(60)"], **kwargs)

    monkeypatch.setattr(record_live.subprocess, "Popen", launch)
    reason = record_live._xctrace(1, 3600, tmp_path / "hitches.trace", "fixture")
    receipt = json.loads((tmp_path / "trace-recording.json").read_text())
    assert reason == "recording stopped at storage limit"
    assert receipt["status"] == "failed"
    assert receipt["exit_code"] is not None
    assert receipt["maximum_volume_consumption_bytes"] == 2 * 1024**3
    assert (tmp_path / "trace-tmp").is_dir()
    assert (tmp_path / "xctrace.log").exists()


def test_recording_refuses_inadequate_headroom_with_receipt(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(record_live.shutil, "which", lambda _: sys.executable)
    monkeypatch.setattr(
        record_live.shutil, "disk_usage", lambda _: shutil._ntuple_diskusage(0, 0, 7 * 1024**3)
    )
    reason = record_live._xctrace(1, 3600, tmp_path / "hitches.trace", "fixture")
    receipt = json.loads((tmp_path / "trace-recording.json").read_text())
    assert reason == "insufficient recording headroom"
    assert receipt["status"] == "refused"
    assert "pid" not in receipt
