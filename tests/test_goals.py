"""Tests for loopflow.lf.goals module."""

from loopflow.lf.goals import (
    goal_exists,
    list_builtin_goals,
    list_goals,
    load_goal,
    load_goal_content,
)


def test_load_goal_user_defined(tmp_path):
    """load_goal finds user-defined goal in lf/goals/."""
    repo = tmp_path / "repo"
    goals_dir = repo / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "my-goal.md").write_text("""---
pipeline: ship
area: api
---

# My Goal

Goal content here.
""")

    goal = load_goal(repo, "my-goal")

    assert goal is not None
    assert goal.name == "my-goal"
    assert goal.pipeline == "ship"
    assert goal.area == ["api"]
    assert "# My Goal" in goal.content


def test_load_goal_falls_back_to_builtin(tmp_path):
    """load_goal returns builtin goal when user-defined doesn't exist."""
    repo = tmp_path / "repo"
    repo.mkdir()

    # adaptive is a builtin goal
    goal = load_goal(repo, "adaptive")

    assert goal is not None
    assert goal.name == "adaptive"
    # Note: builtin templates have quoted values which our simple parser preserves
    assert "ship" in goal.pipeline


def test_load_goal_user_overrides_builtin(tmp_path):
    """User-defined goal takes precedence over builtin."""
    repo = tmp_path / "repo"
    goals_dir = repo / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "adaptive.md").write_text("""---
pipeline: custom
---

# Custom Adaptive

My custom version.
""")

    goal = load_goal(repo, "adaptive")

    assert goal.pipeline == "custom"
    assert "Custom Adaptive" in goal.content


def test_load_goal_returns_none_for_missing(tmp_path):
    """load_goal returns None for non-existent goal."""
    repo = tmp_path / "repo"
    repo.mkdir()

    goal = load_goal(repo, "nonexistent-goal-name")

    assert goal is None


def test_load_goal_parses_area_as_list(tmp_path):
    """Goal area can be a comma-separated string parsed as list."""
    repo = tmp_path / "repo"
    goals_dir = repo / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "multi-area.md").write_text("""---
area: api, core, ui
---

# Multi Area
""")

    goal = load_goal(repo, "multi-area")

    assert goal.area == ["api", "core", "ui"]


def test_load_goal_parses_area_inline_list(tmp_path):
    """Goal area can be an inline YAML list."""
    repo = tmp_path / "repo"
    goals_dir = repo / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "inline-area.md").write_text("""---
area: [api, core]
---

# Inline Area
""")

    goal = load_goal(repo, "inline-area")

    assert goal.area == ["api", "core"]


def test_load_goal_content_backwards_compat(tmp_path):
    """load_goal_content returns just the content string."""
    repo = tmp_path / "repo"
    goals_dir = repo / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "simple.md").write_text("""---
pipeline: "ship"
---

# Simple Goal

Content only.
""")

    content = load_goal_content(repo, "simple")

    assert content is not None
    assert "# Simple Goal" in content
    assert "Content only." in content


def test_list_builtin_goals():
    """list_builtin_goals returns known builtin goals."""
    builtins = list_builtin_goals()

    assert "adaptive" in builtins
    assert "build" in builtins
    assert "roadmap" in builtins
    assert "simplify" in builtins


def test_list_goals_includes_builtins(tmp_path):
    """list_goals includes builtin goals even when no user goals exist."""
    repo = tmp_path / "repo"
    repo.mkdir()

    goals = list_goals(repo)

    assert "adaptive" in goals
    assert "build" in goals


def test_list_goals_includes_user_goals(tmp_path):
    """list_goals includes user-defined goals."""
    repo = tmp_path / "repo"
    goals_dir = repo / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "custom.md").write_text("# Custom")

    goals = list_goals(repo)

    assert "custom" in goals
    assert "adaptive" in goals  # builtins still included


def test_list_goals_deduplicates(tmp_path):
    """list_goals doesn't duplicate when user overrides builtin."""
    repo = tmp_path / "repo"
    goals_dir = repo / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "adaptive.md").write_text("# Override")

    goals = list_goals(repo)

    # Should only appear once
    assert goals.count("adaptive") == 1


def test_goal_exists_user_defined(tmp_path):
    """goal_exists returns True for user-defined goals."""
    repo = tmp_path / "repo"
    goals_dir = repo / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "my-goal.md").write_text("# My Goal")

    assert goal_exists(repo, "my-goal") is True
    assert goal_exists(repo, "nonexistent") is False


def test_goal_exists_builtin(tmp_path):
    """goal_exists returns True for builtin goals."""
    repo = tmp_path / "repo"
    repo.mkdir()

    assert goal_exists(repo, "adaptive") is True
    assert goal_exists(repo, "build") is True


def test_goal_exists_empty_name(tmp_path):
    """goal_exists returns False for empty name."""
    repo = tmp_path / "repo"
    repo.mkdir()

    assert goal_exists(repo, "") is False
    assert goal_exists(repo, None) is False


def test_load_goal_empty_name(tmp_path):
    """load_goal returns None for empty name."""
    repo = tmp_path / "repo"
    repo.mkdir()

    assert load_goal(repo, "") is None
    assert load_goal(repo, None) is None


def test_load_goal_default_pipeline(tmp_path):
    """Goal defaults to ship pipeline when not specified."""
    repo = tmp_path / "repo"
    goals_dir = repo / ".lf" / "goals"
    goals_dir.mkdir(parents=True)
    (goals_dir / "no-pipeline.md").write_text("# No Pipeline Specified")

    goal = load_goal(repo, "no-pipeline")

    assert goal.pipeline == "ship"
