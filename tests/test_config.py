"""Tests for loopflow.config module."""

import pytest

from loopflow.config import load_config
from loopflow.pipeline import Pipeline


@pytest.fixture
def temp_repo(tmp_path):
    """Create a minimal repo."""
    (tmp_path / ".git").mkdir()
    (tmp_path / ".lf").mkdir()
    return tmp_path


def test_load_config_returns_none_when_missing(temp_repo):
    """No config file means None."""
    assert load_config(temp_repo) is None


def test_load_config_parses_pipelines(temp_repo):
    """Pipelines are loaded from config.yaml."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
pipelines:
  ship:
    tasks:
      - implement
      - review
      - commit
  quick:
    tasks:
      - implement
""")

    config = load_config(temp_repo)

    assert config is not None
    assert "ship" in config.pipelines
    assert config.pipelines["ship"].tasks == ["implement", "review", "commit"]
    assert config.pipelines["quick"].tasks == ["implement"]


def test_load_config_empty_file(temp_repo):
    """Empty config file returns None."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("")

    assert load_config(temp_repo) is None


def test_load_config_skip_permissions_flag(temp_repo):
    """dangerously_skip_permissions flag is loaded."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("dangerously_skip_permissions: true\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.dangerously_skip_permissions is True


def test_load_config_skip_permissions_defaults_false(temp_repo):
    """dangerously_skip_permissions defaults to False when not set."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("pipelines:\n  foo:\n    tasks:\n      - task1\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.dangerously_skip_permissions is False


def test_load_config_push_pr_flags(temp_repo):
    """push and pr flags are loaded from config."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
push: true
pr: false
""")

    config = load_config(temp_repo)

    assert config is not None
    assert config.push is True
    assert config.pr is False


def test_load_config_push_pr_defaults_false(temp_repo):
    """push and pr default to False when not set."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("pipelines:\n  foo:\n    tasks:\n      - task1\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.push is False
    assert config.pr is False


def test_load_config_pipeline_push_pr_override(temp_repo):
    """Pipeline-specific push/pr settings override globals."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
push: false
pr: false
pipelines:
  ship:
    tasks:
      - implement
      - review
    pr: true
""")

    config = load_config(temp_repo)

    assert config is not None
    assert config.push is False
    assert config.pr is False
    assert config.pipelines["ship"].pr is True
    assert config.pipelines["ship"].push is None


def test_load_config_context_as_string(temp_repo):
    """context as space-separated string is split into list."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text('context: ". src/foo tests/"\n')

    config = load_config(temp_repo)

    assert config is not None
    assert config.context == [".", "src/foo", "tests/"]


def test_load_config_context_as_list(temp_repo):
    """context as YAML list works."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("context:\n  - src\n  - tests\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.context == ["src", "tests"]


def test_load_config_context_defaults_empty(temp_repo):
    """context defaults to empty list."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("dangerously_skip_permissions: false\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.context == []
