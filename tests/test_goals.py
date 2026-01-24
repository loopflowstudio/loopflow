"""Tests for goal loading and prompt integration."""

import pytest

from loopflow.lf.config import Config
from loopflow.lf.context import format_prompt, gather_prompt_components
from loopflow.lf.frontmatter import StepConfig, parse_step_file, resolve_step_config
from loopflow.lf.goals import Goal, load_goal, parse_goal_arg


@pytest.fixture
def temp_repo(tmp_path):
    """Create a repo with goals directory."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Test Project\n")

    lf = tmp_path / ".lf"
    lf.mkdir()
    steps = lf / "steps"
    steps.mkdir()
    (steps / "implement.md").write_text("Implement the feature.\n")

    goals = lf / "goals"
    goals.mkdir()
    (goals / "architect.md").write_text(
        "Bring an architect's perspective.\n\nFocus on system design."
    )
    (goals / "concise.md").write_text("Be concise. One sentence where possible.")

    return tmp_path


# load_goal tests


def test_load_goal_returns_goal_dataclass(temp_repo):
    goal = load_goal(temp_repo, "architect")
    assert isinstance(goal, Goal)
    assert goal.name == "architect"
    assert "architect's perspective" in goal.content


def test_load_goal_strips_whitespace(temp_repo):
    goal = load_goal(temp_repo, "concise")
    assert goal.content == "Be concise. One sentence where possible."


def test_load_goal_returns_none_when_not_found(temp_repo):
    goal = load_goal(temp_repo, "nonexistent")
    assert goal is None


def test_load_goal_returns_none_when_no_goals_exist(tmp_path):
    (tmp_path / ".git").mkdir()
    (tmp_path / ".lf").mkdir()
    goal = load_goal(tmp_path, "test")
    assert goal is None


# parse_goal_arg tests


def test_parse_goal_arg_single():
    assert parse_goal_arg("architect") == ["architect"]


def test_parse_goal_arg_multiple():
    assert parse_goal_arg("architect,concise") == ["architect", "concise"]


def test_parse_goal_arg_strips_whitespace():
    assert parse_goal_arg(" architect , concise ") == ["architect", "concise"]


def test_parse_goal_arg_empty_string():
    assert parse_goal_arg("") == []


def test_parse_goal_arg_none():
    assert parse_goal_arg(None) == []


def test_parse_goal_arg_filters_empty_items():
    assert parse_goal_arg("architect,,concise") == ["architect", "concise"]


# Frontmatter parsing tests


def test_parse_step_file_with_goal_string():
    content = """---
goal: architect
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.goal == ["architect"]


def test_parse_step_file_with_goal_list():
    content = """---
goal: [architect, concise]
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.goal == ["architect", "concise"]


def test_parse_step_file_with_goal_multiline_list():
    content = """---
goal:
  - architect
  - concise
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.goal == ["architect", "concise"]


def test_parse_step_file_no_goal():
    content = """---
interactive: true
---
Task content"""
    result = parse_step_file("test", content)
    assert result.config.goal is None


# Config goal resolution tests


def test_resolve_step_config_cli_goal_wins():
    config = Config(goal=["architect"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(goal=["concise"]),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
        cli_goal=["reviewer"],
    )
    assert resolved.goal == ["reviewer"]


def test_resolve_step_config_frontmatter_goal_over_global():
    config = Config(goal=["architect"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(goal=["concise"]),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.goal == ["concise"]


def test_resolve_step_config_global_goal():
    config = Config(goal=["architect"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.goal == ["architect"]


def test_resolve_step_config_no_goal():
    resolved = resolve_step_config(
        step_name="test",
        global_config=None,
        frontmatter=StepConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.goal == []


# Prompt formatting tests


def test_format_prompt_single_goal(temp_repo):
    components = gather_prompt_components(temp_repo, "implement", goals=["architect"])
    formatted = format_prompt(components)

    assert "<lf:goal:architect>" in formatted
    assert "architect's perspective" in formatted
    assert "</lf:goal:architect>" in formatted
    # Single goal should not have wrapper
    assert "<lf:goals>" not in formatted


def test_format_prompt_multiple_goals(temp_repo):
    components = gather_prompt_components(temp_repo, "implement", goals=["architect", "concise"])
    formatted = format_prompt(components)

    assert "<lf:goals>" in formatted
    assert "<lf:goal:architect>" in formatted
    assert "<lf:goal:concise>" in formatted
    assert "</lf:goals>" in formatted


def test_format_prompt_goal_before_task(temp_repo):
    components = gather_prompt_components(temp_repo, "implement", goals=["architect"])
    formatted = format_prompt(components)

    goal_pos = formatted.find("<lf:goal:architect>")
    step_pos = formatted.find("<lf:step:implement>")
    assert goal_pos < step_pos, "Goal should appear before step"


def test_format_prompt_no_goals(temp_repo):
    components = gather_prompt_components(temp_repo, "implement")
    formatted = format_prompt(components)

    assert "<lf:goal" not in formatted
    assert "<lf:goals>" not in formatted
