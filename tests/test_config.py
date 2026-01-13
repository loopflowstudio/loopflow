"""Tests for loopflow.config module."""

import pytest

from loopflow.config import load_config, PipelineConfig, ConfigError


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
    """yolo flag is loaded."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("yolo: true\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.yolo is True


def test_load_config_skip_permissions_defaults_false(temp_repo):
    """yolo defaults to False when not set."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("pipelines:\n  foo:\n    tasks:\n      - task1\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.yolo is False


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
    config_yaml.write_text("yolo: false\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.context == []


def test_load_config_exclude_as_string(temp_repo):
    """exclude as space-separated string is split into list."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text('exclude: "*.log build/"\n')

    config = load_config(temp_repo)

    assert config is not None
    assert config.exclude == ["*.log", "build/"]


def test_load_config_exclude_as_list(temp_repo):
    """exclude as YAML list works."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("exclude:\n  - '*.log'\n  - build/\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.exclude == ["*.log", "build/"]


def test_load_config_exclude_defaults_empty(temp_repo):
    """exclude defaults to empty list."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("yolo: false\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.exclude == []


def test_load_config_ide_defaults(temp_repo):
    """ide settings default to warp=True, cursor=True, workspace=None."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("yolo: false\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.ide.warp is True
    assert config.ide.cursor is True
    assert config.ide.workspace is None


def test_load_config_ide_settings(temp_repo):
    """ide settings are loaded from config."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
ide:
  warp: false
  cursor: true
  workspace: myproject.code-workspace
""")

    config = load_config(temp_repo)

    assert config is not None
    assert config.ide.warp is False
    assert config.ide.cursor is True
    assert config.ide.workspace == "myproject.code-workspace"


def test_load_config_ide_partial(temp_repo):
    """ide settings can be partially specified."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
ide:
  cursor: false
""")

    config = load_config(temp_repo)

    assert config is not None
    assert config.ide.warp is True  # default
    assert config.ide.cursor is False
    assert config.ide.workspace is None  # default


def test_load_config_raises_on_invalid_yaml(temp_repo):
    """Invalid YAML raises ConfigError."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("foo: [invalid\n")

    with pytest.raises(ConfigError, match="Invalid YAML"):
        load_config(temp_repo)


def test_load_config_raises_on_invalid_schema(temp_repo):
    """Invalid config schema raises ConfigError."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
pipelines:
  ship:
    wrong_field: value
""")

    with pytest.raises(ConfigError, match="Invalid config"):
        load_config(temp_repo)


def test_load_config_agent_model_defaults(temp_repo):
    """agent_model defaults to 'claude:opus' when not set."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("push: false\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.agent_model == "claude:opus"


def test_load_config_agent_model_setting(temp_repo):
    """agent_model is loaded from config."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("agent_model: codex:o3\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.agent_model == "codex:o3"
