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
    - implement
    - review
    - commit
  quick:
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
    config_yaml.write_text("pipelines:\n  foo:\n    - task1\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.dangerously_skip_permissions is False
