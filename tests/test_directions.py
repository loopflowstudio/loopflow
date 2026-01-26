"""Tests for direction loading and prompt integration."""

import pytest

from loopflow.lf.config import Config
from loopflow.lf.context import format_prompt, gather_prompt_components
from loopflow.lf.frontmatter import StepConfig, parse_step_file, resolve_step_config
from loopflow.lf.directions import Direction, load_direction, parse_direction_arg, resolve_directions


@pytest.fixture
def temp_repo(tmp_path):
    """Create a repo with directions directory."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Test Project\n")

    lf = tmp_path / ".lf"
    lf.mkdir()
    steps = lf / "steps"
    steps.mkdir()
    (steps / "implement.md").write_text("Implement the feature.\n")

    directions = lf / "directions"
    directions.mkdir()
    (directions / "architect.md").write_text(
        "Bring an architect's perspective.\n\nFocus on system design."
    )
    (directions / "concise.md").write_text("Be concise. One sentence where possible.")

    return tmp_path


# load_direction tests


def test_load_direction_returns_direction_dataclass(temp_repo):
    direction = load_direction(temp_repo, "architect")
    assert isinstance(direction, Direction)
    assert direction.name == "architect"
    assert "architect's perspective" in direction.content


def test_load_direction_strips_whitespace(temp_repo):
    direction = load_direction(temp_repo, "concise")
    assert direction.content == "Be concise. One sentence where possible."


def test_load_direction_returns_none_when_not_found(temp_repo):
    direction = load_direction(temp_repo, "nonexistent")
    assert direction is None


def test_load_direction_returns_none_when_no_directions_exist(tmp_path):
    (tmp_path / ".git").mkdir()
    (tmp_path / ".lf").mkdir()
    direction = load_direction(tmp_path, "test")
    assert direction is None


# parse_direction_arg tests


def test_parse_direction_arg_single():
    assert parse_direction_arg("architect") == ["architect"]


def test_parse_direction_arg_multiple():
    assert parse_direction_arg("architect,concise") == ["architect", "concise"]


def test_parse_direction_arg_strips_whitespace():
    assert parse_direction_arg(" architect , concise ") == ["architect", "concise"]


def test_parse_direction_arg_empty_string():
    assert parse_direction_arg("") == []


def test_parse_direction_arg_none():
    assert parse_direction_arg(None) == []


def test_parse_direction_arg_filters_empty_items():
    assert parse_direction_arg("architect,,concise") == ["architect", "concise"]


# Frontmatter parsing tests


def test_parse_step_file_with_direction_string():
    content = """---
direction: architect
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.direction == ["architect"]


def test_parse_step_file_with_direction_list():
    content = """---
direction: [architect, concise]
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.direction == ["architect", "concise"]


def test_parse_step_file_with_direction_multiline_list():
    content = """---
direction:
  - architect
  - concise
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.direction == ["architect", "concise"]


def test_parse_step_file_no_direction():
    content = """---
interactive: true
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.direction is None


# Config direction resolution tests


def test_resolve_step_config_cli_direction_wins():
    config = Config(direction=["architect"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(direction=["concise"]),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
        cli_direction=["reviewer"],
    )
    assert resolved.direction == ["reviewer"]


def test_resolve_step_config_frontmatter_direction_over_global():
    config = Config(direction=["architect"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(direction=["concise"]),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.direction == ["concise"]


def test_resolve_step_config_global_direction():
    config = Config(direction=["architect"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.direction == ["architect"]


def test_resolve_step_config_no_direction():
    resolved = resolve_step_config(
        step_name="test",
        global_config=None,
        frontmatter=StepConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.direction == []


# Prompt formatting tests


def test_format_prompt_single_direction(temp_repo):
    direction = resolve_directions(temp_repo, ["architect"])
    components = gather_prompt_components(temp_repo, "implement", direction=direction)
    formatted = format_prompt(components)

    assert "<lf:direction:architect>" in formatted
    assert "architect's perspective" in formatted
    assert "</lf:direction:architect>" in formatted
    # Single direction should not have wrapper
    assert "<lf:directions>" not in formatted


def test_format_prompt_multiple_directions(temp_repo):
    direction = resolve_directions(temp_repo, ["architect", "concise"])
    components = gather_prompt_components(temp_repo, "implement", direction=direction)
    formatted = format_prompt(components)

    assert "<lf:directions>" in formatted
    assert "<lf:direction:architect>" in formatted
    assert "<lf:direction:concise>" in formatted
    assert "</lf:directions>" in formatted


def test_format_prompt_direction_before_task(temp_repo):
    direction = resolve_directions(temp_repo, ["architect"])
    components = gather_prompt_components(temp_repo, "implement", direction=direction)
    formatted = format_prompt(components)

    direction_pos = formatted.find("<lf:direction:architect>")
    step_pos = formatted.find("<lf:step:implement>")
    assert direction_pos < step_pos, "Direction should appear before step"


def test_format_prompt_no_directions(temp_repo):
    components = gather_prompt_components(temp_repo, "implement")
    formatted = format_prompt(components)

    assert "<lf:direction" not in formatted
    assert "<lf:directions>" not in formatted
