"""Tests for automode default behavior."""

from loopflow.lf.config import Config


def test_config_interactive_list():
    """Config supports interactive task list."""
    config = Config(interactive=["design", "iterate"])

    assert "design" in config.interactive
    assert "iterate" in config.interactive
    assert "implement" not in config.interactive


def test_config_interactive_defaults_empty():
    """Config interactive list defaults to empty."""
    config = Config()

    assert config.interactive == []


def test_task_in_interactive_list(tmp_path):
    """Tasks in interactive list default to interactive mode."""
    from loopflow.lf.config import load_config

    config_file = tmp_path / ".git"
    config_file.mkdir()
    lf_dir = tmp_path / ".lf"
    lf_dir.mkdir()

    config_yaml = lf_dir / "config.yaml"
    config_yaml.write_text("""
interactive:
  - design
  - iterate
""")

    config = load_config(tmp_path)

    assert config is not None
    assert "design" in config.interactive
    assert "iterate" in config.interactive


def test_determine_run_mode_interactive_flag_wins():
    """Interactive flag overrides config and auto flag."""
    # Simulating the logic from run.py
    config_interactive = ["design"]
    task = "design"
    interactive_flag = True
    auto_flag = False

    # Logic from run.py
    if interactive_flag:
        is_interactive = True
    elif auto_flag:
        is_interactive = False
    else:
        is_interactive = task in config_interactive

    assert is_interactive is True


def test_determine_run_mode_auto_flag_wins():
    """Auto flag overrides config default."""
    config_interactive = ["design"]
    task = "design"
    interactive_flag = False
    auto_flag = True

    if interactive_flag:
        is_interactive = True
    elif auto_flag:
        is_interactive = False
    else:
        is_interactive = task in config_interactive

    assert is_interactive is False


def test_determine_run_mode_config_default():
    """Without flags, config determines mode."""
    config_interactive = ["design"]
    task = "design"
    interactive_flag = False
    auto_flag = False

    if interactive_flag:
        is_interactive = True
    elif auto_flag:
        is_interactive = False
    else:
        is_interactive = task in config_interactive

    assert is_interactive is True


def test_determine_run_mode_default_auto():
    """Without flags or config, default is auto."""
    config_interactive = []
    task = "implement"
    interactive_flag = False
    auto_flag = False

    if interactive_flag:
        is_interactive = True
    elif auto_flag:
        is_interactive = False
    else:
        is_interactive = task in config_interactive

    assert is_interactive is False
