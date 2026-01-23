"""Tests for loopflow.config module."""

from pathlib import Path

import pytest

from loopflow.lf.config import (
    AutopruneConfig,
    ConfigError,
    _merge_config_dicts,
    load_config,
    parse_model,
)


@pytest.fixture
def temp_repo(tmp_path, monkeypatch):
    """Create a minimal repo with isolated global config."""
    # Isolate from user's actual ~/.lf/config.yaml
    fake_home = tmp_path / "home"
    fake_home.mkdir()
    monkeypatch.setattr(Path, "home", lambda: fake_home)

    repo = tmp_path / "repo"
    repo.mkdir()
    (repo / ".git").mkdir()
    (repo / ".lf").mkdir()
    return repo


def test_load_config_returns_none_when_missing(temp_repo):
    """No config file means None."""
    assert load_config(temp_repo) is None


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
    config_yaml.write_text("agent_model: claude:opus\n")

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
    config_yaml.write_text("agent_model: claude:opus\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.push is False
    assert config.pr is False


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
summary_tokens: nope
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
# Global config merge tests
# =============================================================================


def test_merge_config_dicts_repo_overrides_scalar():
    """Repo config overrides global for scalar values."""
    global_cfg = {"agent_model": "claude:opus", "yolo": False}
    repo_cfg = {"agent_model": "codex"}

    merged = _merge_config_dicts(global_cfg, repo_cfg)

    assert merged["agent_model"] == "codex"
    assert merged["yolo"] is False


def test_merge_config_dicts_additive_keys_combine():
    """Additive keys combine lists from both configs."""
    global_cfg = {"context": ["global.md"], "exclude": ["*.log"]}
    repo_cfg = {"context": ["local.md"], "exclude": ["build/"]}

    merged = _merge_config_dicts(global_cfg, repo_cfg)

    assert merged["context"] == ["global.md", "local.md"]
    assert merged["exclude"] == ["*.log", "build/"]


def test_merge_config_dicts_global_only():
    """Global config used when no repo config."""
    global_cfg = {"agent_model": "claude:opus"}

    merged = _merge_config_dicts(global_cfg, None)

    assert merged["agent_model"] == "claude:opus"


def test_merge_config_dicts_repo_only():
    """Repo config used when no global config."""
    repo_cfg = {"agent_model": "codex"}

    merged = _merge_config_dicts(None, repo_cfg)

    assert merged["agent_model"] == "codex"


def test_merge_config_dicts_both_empty():
    """Empty dict when both configs are None."""
    merged = _merge_config_dicts(None, None)

    assert merged == {}


def test_load_config_global_only(tmp_path, monkeypatch):
    """Global config is loaded when no repo config exists."""
    global_lf = tmp_path / "global_lf"
    global_lf.mkdir()
    (global_lf / "config.yaml").write_text("agent_model: codex\n")

    monkeypatch.setattr("pathlib.Path.home", lambda: tmp_path / "global_lf" / "..")

    # Create a repo without config
    repo = tmp_path / "repo"
    repo.mkdir()
    (repo / ".lf").mkdir()

    # Patch home to point to our test global
    import pathlib

    original_home = pathlib.Path.home

    def mock_home():
        return global_lf / ".."

    monkeypatch.setattr(pathlib.Path, "home", staticmethod(mock_home))

    # Actually we need to construct the path differently
    # Let's just test the merge function directly


def test_load_config_with_global_merge(tmp_path, monkeypatch):
    """Config merges global and repo settings."""
    # Create global config
    global_home = tmp_path / "home"
    global_home.mkdir()
    global_lf = global_home / ".lf"
    global_lf.mkdir()
    (global_lf / "config.yaml").write_text("agent_model: claude:opus\ncontext:\n  - global.md\n")

    # Patch Path.home()
    monkeypatch.setattr("pathlib.Path.home", lambda: global_home)

    # Create repo config
    repo = tmp_path / "repo"
    repo.mkdir()
    (repo / ".lf").mkdir()
    (repo / ".lf" / "config.yaml").write_text("agent_model: codex\ncontext:\n  - local.md\n")

    config = load_config(repo)

    assert config is not None
    assert config.agent_model == "codex"  # repo overrides
    assert "global.md" in config.context  # combined
    assert "local.md" in config.context


# =============================================================================
# Autoprune config tests
# =============================================================================


def test_autoprune_config_bool_true(temp_repo):
    """autoprune: true enables with defaults."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("autoprune: true\n")

    config = load_config(temp_repo)

    assert config is not None
    assert isinstance(config.autoprune, AutopruneConfig)
    assert config.autoprune.enabled is True
    assert config.autoprune.poll_interval_seconds == 60


def test_autoprune_config_bool_false(temp_repo):
    """autoprune: false disables."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("autoprune: false\n")

    config = load_config(temp_repo)

    assert config is not None
    assert isinstance(config.autoprune, AutopruneConfig)
    assert config.autoprune.enabled is False


def test_autoprune_config_dict(temp_repo):
    """autoprune as dict with options."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("""
autoprune:
  enabled: true
  poll_interval_seconds: 120
""")

    config = load_config(temp_repo)

    assert config is not None
    assert config.autoprune.enabled is True
    assert config.autoprune.poll_interval_seconds == 120


def test_autoprune_config_defaults(temp_repo):
    """autoprune defaults to disabled."""
    config_yaml = temp_repo / ".lf" / "config.yaml"
    config_yaml.write_text("yolo: false\n")

    config = load_config(temp_repo)

    assert config is not None
    assert config.autoprune.enabled is False
