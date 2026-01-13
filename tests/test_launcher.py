"""Tests for loopflow.launcher module."""

from pathlib import Path

from loopflow.launcher import (
    build_claude_command,
    build_codex_command,
    build_codex_interactive_command,
    build_model_command,
    build_model_interactive_command,
    check_claude_available,
    _format_normalized_event,
)


def test_check_claude_available_returns_bool():
    """check_claude_available returns a boolean."""
    result = check_claude_available()
    assert isinstance(result, bool)


# Tests for build_claude_command


def test_build_claude_command_interactive():
    """build_claude_command returns basic command for interactive mode."""
    cmd = build_claude_command(auto=False, stream=False, skip_permissions=False)

    assert cmd == ["claude"]


def test_build_claude_command_auto():
    """build_claude_command adds print flags for auto mode."""
    cmd = build_claude_command(auto=True, stream=False, skip_permissions=False)

    assert cmd == ["claude", "--print", "--dangerously-skip-permissions"]


def test_build_claude_command_auto_stream():
    """build_claude_command adds stream-json flags for auto+stream."""
    cmd = build_claude_command(auto=True, stream=True, skip_permissions=False)

    assert cmd == [
        "claude",
        "--print",
        "--dangerously-skip-permissions",
        "--output-format",
        "stream-json",
        "--verbose",
    ]


def test_build_claude_command_skip_permissions():
    """build_claude_command adds skip-permissions for interactive mode."""
    cmd = build_claude_command(auto=False, stream=False, skip_permissions=True)

    assert cmd == ["claude", "--dangerously-skip-permissions"]


def test_build_claude_command_with_model_variant():
    """build_claude_command adds model flag when variant specified."""
    cmd = build_claude_command(
        auto=False, stream=False, skip_permissions=False, model_variant="opus"
    )

    assert cmd == ["claude", "--model", "opus"]


def test_build_claude_command_auto_with_model_variant():
    """build_claude_command includes model flag in auto mode."""
    cmd = build_claude_command(
        auto=True, stream=False, skip_permissions=False, model_variant="sonnet"
    )

    assert "--model" in cmd
    assert "sonnet" in cmd
    assert "--print" in cmd


# Tests for build_codex_command


def test_build_codex_command_basic():
    """build_codex_command returns exec subcommand."""
    cmd = build_codex_command(
        auto=False,
        stream=False,
        skip_permissions=False,
        sandbox_root=Path("/repo"),
        workdir=Path("/repo/worktree"),
    )

    assert cmd == [
        "codex",
        "exec",
        "-C",
        "/repo/worktree",
        "--sandbox",
        "workspace-write",
        "--add-dir",
        "/repo",
    ]


def test_build_codex_command_stream():
    """build_codex_command adds json flag for streaming."""
    cmd = build_codex_command(
        auto=False,
        stream=True,
        skip_permissions=False,
        sandbox_root=Path("/repo"),
        workdir=Path("/repo/worktree"),
    )

    assert cmd == [
        "codex",
        "exec",
        "-C",
        "/repo/worktree",
        "--json",
        "--sandbox",
        "workspace-write",
        "--add-dir",
        "/repo",
    ]


def test_build_codex_command_auto():
    """build_codex_command adds approval and sandbox flags for auto mode."""
    cmd = build_codex_command(
        auto=True,
        stream=False,
        skip_permissions=False,
        sandbox_root=Path("/repo"),
        workdir=Path("/repo/worktree"),
    )

    assert cmd[:2] == ["codex", "exec"]
    assert "-C" in cmd
    assert "/repo/worktree" in cmd
    assert "--sandbox" in cmd
    assert "workspace-write" in cmd
    assert "--add-dir" in cmd
    assert "/repo" in cmd
    if "--full-auto" in cmd:
        return
    assert "-c" in cmd
    assert 'approval_policy="on-request"' in cmd


def test_build_codex_command_skip_permissions():
    """build_codex_command adds approval flags for skip_permissions."""
    cmd = build_codex_command(
        auto=False,
        stream=False,
        skip_permissions=True,
        sandbox_root=Path("/repo"),
        workdir=Path("/repo/worktree"),
    )

    assert "-c" in cmd
    assert 'approval_policy="never"' in cmd
    assert "--sandbox" in cmd
    assert "workspace-write" in cmd
    assert "--add-dir" in cmd
    assert "/repo" in cmd


def test_build_codex_command_with_model_variant():
    """build_codex_command adds config flag for model variant."""
    cmd = build_codex_command(
        auto=False,
        stream=False,
        skip_permissions=False,
        model_variant="gpt-4",
        sandbox_root=Path("/repo"),
        workdir=Path("/repo/worktree"),
    )

    assert "-c" in cmd
    assert 'model="gpt-4"' in cmd


# Tests for build_codex_interactive_command


def test_build_codex_interactive_command_basic():
    """build_codex_interactive_command returns base codex command."""
    cmd = build_codex_interactive_command(
        skip_permissions=False,
        sandbox_root=Path("/repo"),
        workdir=Path("/repo/worktree"),
    )

    assert cmd == [
        "codex",
        "-C",
        "/repo/worktree",
        "--sandbox",
        "workspace-write",
        "--add-dir",
        "/repo",
    ]


def test_build_codex_interactive_command_skip_permissions():
    """build_codex_interactive_command adds approval flags."""
    cmd = build_codex_interactive_command(
        skip_permissions=True,
        sandbox_root=Path("/repo"),
        workdir=Path("/repo/worktree"),
    )

    assert "-a" in cmd
    assert "never" in cmd
    assert "-C" in cmd
    assert "/repo/worktree" in cmd
    assert "--sandbox" in cmd
    assert "workspace-write" in cmd
    assert "--add-dir" in cmd
    assert "/repo" in cmd


def test_build_codex_interactive_command_with_model():
    """build_codex_interactive_command adds model config."""
    cmd = build_codex_interactive_command(
        skip_permissions=False,
        model_variant="gpt-4o",
        sandbox_root=Path("/repo"),
        workdir=Path("/repo/worktree"),
    )

    assert "-c" in cmd
    assert 'model="gpt-4o"' in cmd


# Tests for build_model_command


def test_build_model_command_claude():
    """build_model_command delegates to build_claude_command for claude."""
    cmd = build_model_command("claude", auto=True, stream=True, skip_permissions=False)

    assert cmd[0] == "claude"
    assert "--print" in cmd


def test_build_model_command_codex():
    """build_model_command delegates to build_codex_command for codex."""
    cmd = build_model_command(
        "codex",
        auto=True,
        stream=True,
        skip_permissions=False,
        sandbox_root=Path("/repo"),
        workdir=Path("/repo/worktree"),
    )

    assert cmd[0] == "codex"
    assert "exec" in cmd


def test_build_model_command_passes_variant():
    """build_model_command passes model_variant through."""
    cmd = build_model_command(
        "claude", auto=False, stream=False, skip_permissions=False, model_variant="opus"
    )

    assert "--model" in cmd
    assert "opus" in cmd


# Tests for build_model_interactive_command


def test_build_model_interactive_command_claude():
    """build_model_interactive_command returns claude command."""
    cmd = build_model_interactive_command("claude", skip_permissions=False)

    assert cmd == ["claude"]


def test_build_model_interactive_command_codex():
    """build_model_interactive_command returns codex command."""
    cmd = build_model_interactive_command(
        "codex",
        skip_permissions=False,
        sandbox_root=Path("/repo"),
        workdir=Path("/repo/worktree"),
    )

    assert cmd == [
        "codex",
        "-C",
        "/repo/worktree",
        "--sandbox",
        "workspace-write",
        "--add-dir",
        "/repo",
    ]


def test_build_model_interactive_command_with_skip():
    """build_model_interactive_command passes skip_permissions."""
    cmd = build_model_interactive_command("claude", skip_permissions=True)

    assert "--dangerously-skip-permissions" in cmd


# Tests for _format_normalized_event


def test_format_normalized_event_tool_use():
    """_format_normalized_event formats tool_use with path."""
    event = {"type": "tool_use", "tool": "Read", "input": {"path": "foo.py"}}

    result = _format_normalized_event(event)

    assert result == "→ Read: foo.py"


def test_format_normalized_event_tool_use_no_path():
    """_format_normalized_event formats tool_use without path."""
    event = {"type": "tool_use", "tool": "Bash", "input": {"command": "ls"}}

    result = _format_normalized_event(event)

    assert result == "→ Bash"


def test_format_normalized_event_tool_use_no_input():
    """_format_normalized_event handles missing input."""
    event = {"type": "tool_use", "tool": "Unknown"}

    result = _format_normalized_event(event)

    assert result == "→ Unknown"


def test_format_normalized_event_text():
    """_format_normalized_event returns text content."""
    event = {"type": "text", "content": "Hello world"}

    result = _format_normalized_event(event)

    assert result == "Hello world"


def test_format_normalized_event_text_empty():
    """_format_normalized_event returns None for empty text."""
    event = {"type": "text", "content": ""}

    result = _format_normalized_event(event)

    assert result is None


def test_format_normalized_event_result_success():
    """_format_normalized_event formats success result."""
    event = {"type": "result", "status": "success"}

    result = _format_normalized_event(event)

    assert result == "\n✓ Done\n"


def test_format_normalized_event_result_error():
    """_format_normalized_event formats error result."""
    event = {"type": "result", "status": "error"}

    result = _format_normalized_event(event)

    assert result == "✗ Failed\n"


def test_format_normalized_event_result_unknown():
    """_format_normalized_event returns None for unknown result status."""
    event = {"type": "result", "status": "pending"}

    result = _format_normalized_event(event)

    assert result is None


def test_format_normalized_event_unknown_type():
    """_format_normalized_event returns None for unknown event types."""
    event = {"type": "unknown", "data": "something"}

    result = _format_normalized_event(event)

    assert result is None
