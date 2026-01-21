"""Tests for frontmatter parsing and config resolution."""

from loopflow.lf.config import Config
from loopflow.lf.frontmatter import (
    StepConfig,
    get_step_defaults,
    parse_step_file,
    resolve_step_config,
)


def test_parse_step_file_no_frontmatter():
    content = "Just plain markdown content"
    result = parse_step_file("test", content)
    assert result.name == "test"
    assert result.content == content
    assert result.config.interactive is None


def test_parse_step_file_with_interactive():
    content = """---
interactive: true
---
Step content here"""
    result = parse_step_file("test", content)
    assert result.name == "test"
    assert result.content == "Step content here"
    assert result.config.interactive is True


def test_parse_step_file_with_include_list():
    content = """---
include:
  - tests/**
  - src/utils.py
---
Content"""
    result = parse_step_file("test", content)
    assert result.config.include == ["tests/**", "src/utils.py"]


def test_parse_step_file_with_inline_list():
    content = """---
include: [tests/**, src/utils.py]
---
Content"""
    result = parse_step_file("test", content)
    assert result.config.include == ["tests/**", "src/utils.py"]


def test_parse_step_file_with_model():
    content = """---
model: codex:o3
---
Content"""
    result = parse_step_file("test", content)
    assert result.config.model == "codex:o3"


def test_parse_step_file_preserves_content():
    content = """---
interactive: false
---
# Heading

Some content with **markdown**.

```python
def foo():
    pass
```"""
    result = parse_step_file("test", content)
    assert "# Heading" in result.content
    assert "def foo():" in result.content


def test_get_step_defaults():
    defaults = get_step_defaults()
    assert defaults.interactive is False
    assert defaults.exclude == ["tests/**"]
    assert defaults.include is None
    assert defaults.model is None


def test_resolve_step_config_cli_interactive_wins():
    resolved = resolve_step_config(
        step_name="test",
        global_config=None,
        frontmatter=StepConfig(interactive=False),
        cli_interactive=True,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is True


def test_resolve_step_config_cli_auto_wins():
    resolved = resolve_step_config(
        step_name="test",
        global_config=None,
        frontmatter=StepConfig(interactive=True),
        cli_interactive=None,
        cli_auto=True,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is False


def test_resolve_step_config_frontmatter_over_global():
    config = Config(interactive=["test"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(interactive=False),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is False


def test_resolve_step_config_global_interactive_list():
    config = Config(interactive=["design", "iterate"])
    resolved = resolve_step_config(
        step_name="design",
        global_config=config,
        frontmatter=StepConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is True


def test_resolve_step_config_model_priority():
    config = Config(agent_model="claude:sonnet")
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(model="codex:o3"),
        cli_interactive=None,
        cli_auto=None,
        cli_model="gemini:2.5-pro",
        cli_context=None,
    )
    assert resolved.model == "gemini:2.5-pro"


def test_resolve_step_config_model_frontmatter():
    config = Config(agent_model="claude:opus")
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(model="codex:o3"),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.model == "codex:o3"


def test_resolve_step_config_include_removes_from_exclude():
    resolved = resolve_step_config(
        step_name="test",
        global_config=None,
        frontmatter=StepConfig(include=["tests/**"]),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert "tests/**" in resolved.include
    assert "tests/**" not in resolved.exclude


def test_resolve_step_config_context_extends():
    config = Config(context=["README.md"])
    resolved = resolve_step_config(
        step_name="test",
        global_config=config,
        frontmatter=StepConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=["src/main.py"],
    )
    assert resolved.context == ["README.md", "src/main.py"]


def test_resolve_step_config_legacy_include_tests_for():
    config = Config(include_tests_for=["polish", "implement"])
    resolved = resolve_step_config(
        step_name="polish",
        global_config=config,
        frontmatter=StepConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert "tests/**" in resolved.include


def test_resolve_step_config_frontmatter_include_over_legacy():
    config = Config(include_tests_for=["polish"])
    resolved = resolve_step_config(
        step_name="polish",
        global_config=config,
        frontmatter=StepConfig(include=["docs/**"]),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    # Frontmatter include takes precedence over legacy include_tests_for
    assert resolved.include == ["docs/**"]


def test_resolve_step_config_frontmatter_interactive_true():
    """External skills set interactive=True in frontmatter at load time."""
    resolved = resolve_step_config(
        step_name="sp:brainstorm",
        global_config=None,
        frontmatter=StepConfig(interactive=True),  # Set by gather_step for external skills
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is True


def test_resolve_step_config_cli_auto_overrides_frontmatter():
    """CLI -a overrides frontmatter interactive=True."""
    resolved = resolve_step_config(
        step_name="sp:brainstorm",
        global_config=None,
        frontmatter=StepConfig(interactive=True),  # External skill default
        cli_interactive=None,
        cli_auto=True,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is False


def test_resolve_step_config_frontmatter_interactive_false():
    """Skill with explicit interactive=False in frontmatter stays auto."""
    resolved = resolve_step_config(
        step_name="sp:brainstorm",
        global_config=None,
        frontmatter=StepConfig(interactive=False),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is False


def test_resolve_step_config_no_frontmatter_defaults_auto():
    """Regular steps without frontmatter default to auto mode."""
    resolved = resolve_step_config(
        step_name="review",
        global_config=None,
        frontmatter=StepConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is False
