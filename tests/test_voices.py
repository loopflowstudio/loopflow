"""Tests for voice loading and prompt integration."""

import pytest

from loopflow.lf.config import Config
from loopflow.lf.context import format_prompt, gather_prompt_components
from loopflow.lf.frontmatter import StepConfig, parse_step_file, resolve_step_config
from loopflow.lf.voices import Voice, load_voice, parse_voice_arg


@pytest.fixture
def temp_repo(tmp_path):
    """Create a repo with voices directory."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Test Project\n")

    lf = tmp_path / ".lf"
    lf.mkdir()
    steps = lf / "steps"
    steps.mkdir()
    (steps / "implement.md").write_text("Implement the feature.\n")

    voices = lf / "voices"
    voices.mkdir()
    (voices / "architect.md").write_text(
        "Bring an architect's perspective.\n\nFocus on system design."
    )
    (voices / "concise.md").write_text("Be concise. One sentence where possible.")

    return tmp_path


# load_voice tests


def test_load_voice_returns_voice_dataclass(temp_repo):
    voice = load_voice(temp_repo, "architect")
    assert isinstance(voice, Voice)
    assert voice.name == "architect"
    assert "architect's perspective" in voice.content


def test_load_voice_strips_whitespace(temp_repo):
    voice = load_voice(temp_repo, "concise")
    assert voice.content == "Be concise. One sentence where possible."


def test_load_voice_returns_none_when_not_found(temp_repo):
    voice = load_voice(temp_repo, "nonexistent")
    assert voice is None


def test_load_voice_returns_none_when_no_voices_exist(tmp_path):
    (tmp_path / ".git").mkdir()
    (tmp_path / ".lf").mkdir()
    voice = load_voice(tmp_path, "test")
    assert voice is None


# parse_voice_arg tests


def test_parse_voice_arg_single():
    assert parse_voice_arg("architect") == ["architect"]


def test_parse_voice_arg_multiple():
    assert parse_voice_arg("architect,concise") == ["architect", "concise"]


def test_parse_voice_arg_strips_whitespace():
    assert parse_voice_arg(" architect , concise ") == ["architect", "concise"]


def test_parse_voice_arg_empty_string():
    assert parse_voice_arg("") == []


def test_parse_voice_arg_none():
    assert parse_voice_arg(None) == []


def test_parse_voice_arg_filters_empty_items():
    assert parse_voice_arg("architect,,concise") == ["architect", "concise"]


# Frontmatter parsing tests


def test_parse_step_file_with_voice_string():
    content = """---
voice: architect
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.voice == ["architect"]


def test_parse_step_file_with_voice_list():
    content = """---
voice: [architect, concise]
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.voice == ["architect", "concise"]


def test_parse_step_file_with_voice_multiline_list():
    content = """---
voice:
  - architect
  - concise
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.voice == ["architect", "concise"]


def test_parse_step_file_no_voice():
    content = """---
interactive: true
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.voice is None


# Config voice resolution tests


def test_resolve_step_config_cli_voice_wins():
    config = Config(voice=["architect"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(voice=["concise"]),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
        cli_voice=["reviewer"],
    )
    assert resolved.voice == ["reviewer"]


def test_resolve_step_config_frontmatter_voice_over_global():
    config = Config(voice=["architect"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(voice=["concise"]),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.voice == ["concise"]


def test_resolve_step_config_global_voice():
    config = Config(voice=["architect"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.voice == ["architect"]


def test_resolve_step_config_no_voice():
    resolved = resolve_step_config(
        step_name="test",
        global_config=None,
        frontmatter=StepConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.voice == []


# Prompt formatting tests


def test_format_prompt_single_voice(temp_repo):
    components = gather_prompt_components(temp_repo, "implement", voices=["architect"])
    formatted = format_prompt(components)

    assert "<lf:voice:architect>" in formatted
    assert "architect's perspective" in formatted
    assert "</lf:voice:architect>" in formatted
    # Single voice should not have wrapper
    assert "<lf:voices>" not in formatted


def test_format_prompt_multiple_voices(temp_repo):
    components = gather_prompt_components(temp_repo, "implement", voices=["architect", "concise"])
    formatted = format_prompt(components)

    assert "<lf:voices>" in formatted
    assert "<lf:voice:architect>" in formatted
    assert "<lf:voice:concise>" in formatted
    assert "</lf:voices>" in formatted


def test_format_prompt_voice_before_task(temp_repo):
    components = gather_prompt_components(temp_repo, "implement", voices=["architect"])
    formatted = format_prompt(components)

    voice_pos = formatted.find("<lf:voice:architect>")
    step_pos = formatted.find("<lf:step:implement>")
    assert voice_pos < step_pos, "Voice should appear before step"


def test_format_prompt_no_voices(temp_repo):
    components = gather_prompt_components(temp_repo, "implement")
    formatted = format_prompt(components)

    assert "<lf:voice" not in formatted
    assert "<lf:voices>" not in formatted
