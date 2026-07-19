import importlib.util
import json
import sqlite3
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/test_time.py"

_spec = importlib.util.spec_from_file_location("test_time_report", SCRIPT)
assert _spec is not None and _spec.loader is not None
test_time = importlib.util.module_from_spec(_spec)
sys.modules["test_time_report"] = test_time
_spec.loader.exec_module(test_time)


def _record(timestamp: str, line: dict) -> str:
    return json.dumps(
        {
            "schema_version": 1,
            "seq": 1,
            "ts": timestamp,
            "stream": "stdout",
            "line": json.dumps(line),
        }
    )


def test_classifier_distinguishes_proof_scope_from_search_text():
    assert test_time._classify("cargo nextest run --workspace") == "rust_full"
    assert test_time._classify("cargo test -p loopflow one_behavior") == "rust_focused"
    assert test_time._classify("rg -n 'cargo test' TESTING.md") is None
    assert (
        test_time._classify("/bin/zsh -lc 'cd rust/loopflow && cargo test -p loopflow probe'")
        == "rust_focused"
    )


def test_parallel_commands_count_once():
    intervals = [
        test_time.Interval(0, 10_000, "rust_focused"),
        test_time.Interval(2_000, 7_000, "rust_focused"),
        test_time.Interval(12_000, 15_000, "check_build"),
    ]
    assert test_time._merge_millis(intervals) == 13_000


def test_legacy_codex_and_claude_capture_command_intervals():
    codex = [
        _record(
            "2026-07-17T12:00:00Z",
            {
                "type": "item.started",
                "item": {
                    "id": "cmd-1",
                    "type": "command_execution",
                    "command": "cargo test -p loopflow probe",
                },
            },
        ),
        _record(
            "2026-07-17T12:00:05Z",
            {
                "type": "item.completed",
                "item": {"id": "cmd-1", "type": "command_execution"},
            },
        ),
    ]
    claude = [
        _record(
            "2026-07-17T12:00:00Z",
            {
                "type": "assistant",
                "message": {
                    "content": [
                        {
                            "type": "tool_use",
                            "name": "Bash",
                            "id": "tool-1",
                            "input": {"command": "cargo clippy --all-targets"},
                        }
                    ]
                },
            },
        ),
        _record(
            "2026-07-17T12:00:03Z",
            {
                "type": "user",
                "message": {
                    "content": [{"type": "tool_result", "tool_use_id": "tool-1"}]
                },
            },
        ),
    ]

    start = test_time._timestamp_ms("2026-07-17T12:00:00Z")
    assert test_time._raw_intervals(codex, "codex") == [
        test_time.Interval(start, start + 5000, "rust_focused")
    ]
    assert test_time._raw_intervals(claude, "claude") == [
        test_time.Interval(start, start + 3000, "check_build")
    ]


def test_report_aggregates_without_exposing_trace_content(tmp_path):
    db = tmp_path / "loopflow.db"
    traces = tmp_path / "traces"
    conversation = traces / "run/process/launch/conversation.jsonl"
    conversation.parent.mkdir(parents=True)
    started = int(datetime.now(timezone.utc).timestamp()) - 10
    started_at = datetime.fromtimestamp(started, timezone.utc).isoformat().replace("+00:00", "Z")
    ended_at = datetime.fromtimestamp(started + 5, timezone.utc).isoformat().replace("+00:00", "Z")
    events = [
        {
            "schema_version": 1,
            "seq": 1,
            "ts": started_at,
            "payload": {
                "type": "conversation",
                "event": {
                    "type": "item_started",
                    "item": {
                        "type": "command",
                        "id": "one",
                        "command": ["cargo", "test", "private-target-name"],
                    },
                },
            },
        },
        {
            "schema_version": 1,
            "seq": 2,
            "ts": ended_at,
            "payload": {
                "type": "conversation",
                "event": {
                    "type": "item_completed",
                    "item": {
                        "type": "command",
                        "id": "one",
                        "command": ["cargo", "test", "private-target-name"],
                        "duration_ms": 5000,
                    },
                },
            },
        },
    ]
    conversation.write_text("\n".join(json.dumps(event) for event in events))
    connection = sqlite3.connect(db)
    connection.execute(
        """
        CREATE TABLE agent_launches (
            started_at INTEGER, ended_at INTEGER, repo TEXT, worktree TEXT,
            skill TEXT, provider TEXT, conversation_path TEXT,
            provider_events_path TEXT
        )
        """
    )
    connection.execute(
        "INSERT INTO agent_launches VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (
            started,
            started + 5,
            "/repo",
            "/repo/task",
            "implement",
            "codex",
            "run/process/launch/conversation.jsonl",
            None,
        ),
    )
    connection.commit()
    connection.close()

    report = test_time._build_report(db, traces, 7, None, None)
    rendered = test_time._render_report(report, 7, "all repositories")

    assert report.activity_millis == 5000
    assert "rust · focused" in rendered
    assert "private-target-name" not in rendered
    assert "command text, prompts, and output are never printed" in rendered.lower()
