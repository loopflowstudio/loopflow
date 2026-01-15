"""Tests for event normalization and formatting functions."""

import json

from loopflow.launcher import normalize_claude_event, normalize_codex_event
from loopflow.lfd.collector import _format_stream_line


def test_normalize_claude_event_single_block():
    """normalize_claude_event handles single tool_use block."""
    event = {
        "type": "assistant",
        "message": {
            "content": [
                {"type": "tool_use", "name": "Read", "input": {"path": "foo.py"}}
            ]
        }
    }

    result = normalize_claude_event(event)

    assert len(result) == 1
    assert result[0]["type"] == "tool_use"
    assert result[0]["tool"] == "Read"


def test_normalize_claude_event_multiple_blocks():
    """normalize_claude_event handles multiple blocks in one event."""
    event = {
        "type": "assistant",
        "message": {
            "content": [
                {"type": "tool_use", "name": "Read", "input": {"path": "foo.py"}},
                {"type": "text", "text": "I'll read this file..."},
                {"type": "tool_use", "name": "Edit", "input": {"path": "bar.py"}},
            ]
        }
    }

    result = normalize_claude_event(event)

    assert len(result) == 3
    assert result[0]["type"] == "tool_use"
    assert result[0]["tool"] == "Read"
    assert result[1]["type"] == "text"
    assert result[1]["content"] == "I'll read this file..."
    assert result[2]["type"] == "tool_use"
    assert result[2]["tool"] == "Edit"


def test_normalize_claude_event_empty_text_skipped():
    """normalize_claude_event skips empty text blocks."""
    event = {
        "type": "assistant",
        "message": {
            "content": [
                {"type": "text", "text": ""},
                {"type": "tool_use", "name": "Read", "input": {}},
            ]
        }
    }

    result = normalize_claude_event(event)

    assert len(result) == 1
    assert result[0]["type"] == "tool_use"


def test_normalize_claude_event_result_type():
    """normalize_claude_event handles result events."""
    event = {
        "type": "result",
        "subtype": "success",
    }

    result = normalize_claude_event(event)

    assert len(result) == 1
    assert result[0]["type"] == "result"
    assert result[0]["status"] == "success"


def test_normalize_claude_event_unknown_type():
    """normalize_claude_event returns empty list for unknown types."""
    event = {"type": "unknown"}

    result = normalize_claude_event(event)

    assert len(result) == 0


def test_normalize_codex_event():
    """normalize_codex_event wraps event in list."""
    event = {"type": "tool_use", "tool": "Read"}

    result = normalize_codex_event(event)

    assert len(result) == 1
    assert result[0] == event


def test_normalize_codex_event_empty():
    """normalize_codex_event returns empty list for None."""
    result = normalize_codex_event(None)

    assert len(result) == 0


# Tests for _format_stream_line


def test_format_stream_line_invalid_json():
    """_format_stream_line returns raw line for invalid JSON."""
    result = _format_stream_line("not json at all")

    assert result == ["not json at all"]


def test_format_stream_line_filters_system_events():
    """_format_stream_line filters out system events."""
    result = _format_stream_line('{"type": "system", "data": "init"}')

    assert result == []


def test_format_stream_line_filters_user_events():
    """_format_stream_line filters out user events."""
    result = _format_stream_line('{"type": "user", "message": "prompt"}')

    assert result == []


def test_format_stream_line_assistant_tool_use():
    """_format_stream_line formats assistant tool_use blocks."""
    event = {
        "type": "assistant",
        "message": {
            "content": [
                {"type": "tool_use", "name": "Read", "input": {"path": "foo.py"}}
            ]
        }
    }
    result = _format_stream_line(json.dumps(event))

    assert result == ["→ Read: foo.py"]


def test_format_stream_line_assistant_tool_use_no_path():
    """_format_stream_line handles tool_use without path."""
    event = {
        "type": "assistant",
        "message": {
            "content": [
                {"type": "tool_use", "name": "Bash", "input": {"command": "ls"}}
            ]
        }
    }
    result = _format_stream_line(json.dumps(event))

    assert result == ["→ Bash"]


def test_format_stream_line_assistant_text():
    """_format_stream_line formats assistant text blocks."""
    event = {
        "type": "assistant",
        "message": {
            "content": [
                {"type": "text", "text": "I'll help you with that."}
            ]
        }
    }
    result = _format_stream_line(json.dumps(event))

    assert result == ["I'll help you with that."]


def test_format_stream_line_assistant_empty_text_skipped():
    """_format_stream_line skips empty text blocks."""
    event = {
        "type": "assistant",
        "message": {
            "content": [
                {"type": "text", "text": ""}
            ]
        }
    }
    result = _format_stream_line(json.dumps(event))

    assert result == []


def test_format_stream_line_assistant_multiple_blocks():
    """_format_stream_line handles multiple content blocks."""
    event = {
        "type": "assistant",
        "message": {
            "content": [
                {"type": "text", "text": "Reading the file..."},
                {"type": "tool_use", "name": "Read", "input": {"path": "test.py"}},
            ]
        }
    }
    result = _format_stream_line(json.dumps(event))

    assert result == ["Reading the file...", "→ Read: test.py"]


def test_format_stream_line_result_success():
    """_format_stream_line formats success result."""
    event = {"type": "result", "subtype": "success"}
    result = _format_stream_line(json.dumps(event))

    assert result == ["✓ Done"]


def test_format_stream_line_result_failure():
    """_format_stream_line formats failure result."""
    event = {"type": "result", "subtype": "error"}
    result = _format_stream_line(json.dumps(event))

    assert result == ["✗ Failed"]


def test_format_stream_line_result_with_status_field():
    """_format_stream_line handles result with status field instead of subtype."""
    event = {"type": "result", "status": "success"}
    result = _format_stream_line(json.dumps(event))

    assert result == ["✓ Done"]


def test_format_stream_line_codex_tool_use():
    """_format_stream_line handles Codex-style tool_use events."""
    event = {"type": "tool_use", "tool": "Read", "input": {"path": "bar.py"}}
    result = _format_stream_line(json.dumps(event))

    assert result == ["→ Read: bar.py"]


def test_format_stream_line_codex_text():
    """_format_stream_line handles Codex-style text events."""
    event = {"type": "text", "content": "Processing..."}
    result = _format_stream_line(json.dumps(event))

    assert result == ["Processing..."]


def test_format_stream_line_unknown_type_filtered():
    """_format_stream_line filters unknown event types."""
    event = {"type": "unknown_event", "data": "something"}
    result = _format_stream_line(json.dumps(event))

    assert result == []
