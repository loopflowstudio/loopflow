"""Tests for loopflow.config module."""

import pytest

from loopflow.lf.config import ConfigError, FlowConfig, load_config, parse_model


@pytest.fixture
def temp_repo(tmp_path):
    """Create a minimal repo."""
    (tmp_path / ".git").mkdir()
    (tmp_path / ".lf").mkdir()
    return tmp_path


def test_load_config_returns_none_when_missing(temp_repo):
    """No config file means None."""
    assert load_config(temp_repo) is None


def test_load_config_parses_flows(temp_repo):
    """Flows are loaded from config.yaml."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
flows:
  ship:
    steps:
      - implement
      - review
      - commit
  quick:
    steps:
      - implement
""")

    config = load_config(temp_repo)

    assert config is not None
    assert "ship" in config.flows
    assert config.flows["ship"].steps == ["implement", "review", "commit"]
    assert config.flows["quick"].steps == ["implement"]


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
    config_yaml.write_text("flows:\n  foo:\n    steps:\n      - task1\n")

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
    config_yaml.write_text("flows:\n  foo:\n    steps:\n      - task1\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.push is False
    assert config.pr is False


def test_load_config_flow_push_pr_override(temp_repo):
    """Flow-specific push/pr settings override globals."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
push: false
pr: false
flows:
  ship:
    steps:
      - implement
      - review
    pr: true
""")

    config = load_config(temp_repo)

    assert config is not None
    assert config.push is False
    assert config.pr is False
    assert config.flows["ship"].pr is True
    assert config.flows["ship"].push is None


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
flows:
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


def test_parse_model_with_variant():
    """parse_model extracts backend and variant."""
    assert parse_model("claude:opus") == ("claude", "opus")
    assert parse_model("codex:o3") == ("codex", "o3")
    assert parse_model("gemini:2.5-pro") == ("gemini", "2.5-pro")


def test_parse_model_default_variants():
    """parse_model applies smart defaults when no variant specified."""
    assert parse_model("claude") == ("claude", "opus")
    assert parse_model("codex") == ("codex", None)  # let Codex CLI pick
    assert parse_model("gemini") == ("gemini", "2.5-pro")


def test_parse_model_unknown_backend():
    """parse_model returns None variant for unknown backends."""
    assert parse_model("unknown") == ("unknown", None)


def test_load_config_voice_as_string(temp_repo):
    """voice as string is converted to single-item list."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("voice: architect\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.voice == ["architect"]


def test_load_config_voice_as_list(temp_repo):
    """voice as YAML list works."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("voice:\n  - architect\n  - concise\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.voice == ["architect", "concise"]


def test_load_config_voice_defaults_none(temp_repo):
    """voice defaults to None when not set."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("yolo: false\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.voice is None


def test_load_config_voice_empty_string(temp_repo):
    """voice as empty string is converted to None."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("voice: ''\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.voice is None


# =============================================================================
# Interactive mode configuration tests
# =============================================================================


def test_config_interactive_list():
    """Config supports interactive task list."""
    from loopflow.lf.config import Config

    config = Config(interactive=["design", "iterate"])

    assert "design" in config.interactive
    assert "iterate" in config.interactive
    assert "implement" not in config.interactive


def test_config_interactive_defaults_empty():
    """Config interactive list defaults to empty."""
    from loopflow.lf.config import Config

    config = Config()

    assert config.interactive == []


def test_load_config_interactive_list(temp_repo):
    """interactive list is loaded from config.yaml."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
interactive:
  - design
  - iterate
""")

    config = load_config(temp_repo)

    assert config is not None
    assert "design" in config.interactive
    assert "iterate" in config.interactive


# =============================================================================
# FlowConfig tests
# =============================================================================


def test_flow_config():
    """FlowConfig holds name and step list."""
    f = FlowConfig(name="ship", steps=["implement", "review"])

    assert f.name == "ship"
    assert f.steps == ["implement", "review"]


def test_flow_config_with_push_pr():
    """FlowConfig can override push/pr settings."""
    f = FlowConfig(name="ship", steps=["implement"], push=True, pr=True)

    assert f.push is True
    assert f.pr is True


def test_flow_config_defaults_none():
    """FlowConfig push/pr default to None."""
    f = FlowConfig(name="ship", steps=["implement"])

    assert f.push is None
    assert f.pr is None


# =============================================================================
# ignore alias tests
# =============================================================================


def test_load_config_ignore_as_list(temp_repo):
    """ignore as YAML list works and merges into exclude."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("ignore:\n  - '*.log'\n  - build/\n")

    config = load_config(temp_repo)

    assert config is not None
    assert "*.log" in config.exclude
    assert "build/" in config.exclude
    assert config.ignore == []  # cleared after merge


def test_load_config_ignore_as_string(temp_repo):
    """ignore as space-separated string is split and merged into exclude."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text('ignore: "*.log build/"\n')

    config = load_config(temp_repo)

    assert config is not None
    assert "*.log" in config.exclude
    assert "build/" in config.exclude
    assert config.ignore == []


def test_load_config_ignore_merges_with_exclude(temp_repo):
    """ignore and exclude merge together."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
exclude:
  - '*.log'
ignore:
  - uv.lock
  - dist/
""")

    config = load_config(temp_repo)

    assert config is not None
    assert "*.log" in config.exclude
    assert "uv.lock" in config.exclude
    assert "dist/" in config.exclude
    assert config.ignore == []


def test_load_config_ignore_deduplicates(temp_repo):
    """Duplicate patterns in ignore and exclude are deduplicated."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
exclude:
  - '*.log'
ignore:
  - '*.log'
  - dist/
""")

    config = load_config(temp_repo)

    assert config is not None
    assert config.exclude.count("*.log") == 1
    assert "dist/" in config.exclude


def test_load_config_ignore_empty(temp_repo):
    """Empty ignore is a no-op."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
exclude:
  - '*.log'
ignore: []
""")

    config = load_config(temp_repo)

    assert config is not None
    assert config.exclude == ["*.log"]
    assert config.ignore == []
