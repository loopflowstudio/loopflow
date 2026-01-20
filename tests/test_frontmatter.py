"""Tests for frontmatter parsing and config resolution."""

from loopflow.lf.config import Config
from loopflow.lf.frontmatter import (
    TaskConfig,
    get_defaults,
    parse_task_file,
    resolve_task_config,
)


def test_parse_task_file_no_frontmatter():
    content = "Just plain markdown content"
    result = parse_task_file("test", content)
    assert result.name == "test"
    assert result.content == content
    assert result.config.interactive is None


def test_parse_task_file_with_interactive():
    content = """---
interactive: true
---
Task content here"""
    result = parse_task_file("test", content)
    assert result.name == "test"
    assert result.content == "Task content here"
    assert result.config.interactive is True


def test_parse_task_file_with_include_list():
    content = """---
include:
  - tests/**
  - src/utils.py
---
Content"""
    result = parse_task_file("test", content)
    assert result.config.include == ["tests/**", "src/utils.py"]


def test_parse_task_file_with_inline_list():
    content = """---
include: [tests/**, src/utils.py]
---
Content"""
    result = parse_task_file("test", content)
    assert result.config.include == ["tests/**", "src/utils.py"]


def test_parse_task_file_with_model():
    content = """---
model: codex:o3
---
Content"""
    result = parse_task_file("test", content)
    assert result.config.model == "codex:o3"


def test_parse_task_file_preserves_content():
    content = """---
interactive: false
---
# Heading

Some content with **markdown**.

```python
def foo():
    pass
```"""
    result = parse_task_file("test", content)
    assert "# Heading" in result.content
    assert "def foo():" in result.content


def test_get_defaults():
    defaults = get_defaults()
    assert defaults.interactive is False
    assert defaults.exclude == ["tests/**"]
    assert defaults.include is None
    assert defaults.model is None


def test_resolve_task_config_cli_interactive_wins():
    resolved = resolve_task_config(
        task_name="test",
        global_config=None,
        frontmatter=TaskConfig(interactive=False),
        cli_interactive=True,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is True


def test_resolve_task_config_cli_auto_wins():
    resolved = resolve_task_config(
        task_name="test",
        global_config=None,
        frontmatter=TaskConfig(interactive=True),
        cli_interactive=None,
        cli_auto=True,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is False


def test_resolve_task_config_frontmatter_over_global():
    config = Config(interactive=["test"])
    resolved = resolve_task_config(
        task_name="test",
        global_config=config,
        frontmatter=TaskConfig(interactive=False),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is False


def test_resolve_task_config_global_interactive_list():
    config = Config(interactive=["design", "iterate"])
    resolved = resolve_task_config(
        task_name="design",
        global_config=config,
        frontmatter=TaskConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.interactive is True


def test_resolve_task_config_model_priority():
    config = Config(agent_model="claude:sonnet")
    resolved = resolve_task_config(
        task_name="test",
        global_config=config,
        frontmatter=TaskConfig(model="codex:o3"),
        cli_interactive=None,
        cli_auto=None,
        cli_model="gemini:2.5-pro",
        cli_context=None,
    )
    assert resolved.model == "gemini:2.5-pro"


def test_resolve_task_config_model_frontmatter():
    config = Config(agent_model="claude:opus")
    resolved = resolve_task_config(
        task_name="test",
        global_config=config,
        frontmatter=TaskConfig(model="codex:o3"),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert resolved.model == "codex:o3"


def test_resolve_task_config_include_removes_from_exclude():
    resolved = resolve_task_config(
        task_name="test",
        global_config=None,
        frontmatter=TaskConfig(include=["tests/**"]),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert "tests/**" in resolved.include
    assert "tests/**" not in resolved.exclude


def test_resolve_task_config_context_extends():
    config = Config(context=["README.md"])
    resolved = resolve_task_config(
        task_name="test",
        global_config=config,
        frontmatter=TaskConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=["src/main.py"],
    )
    assert resolved.context == ["README.md", "src/main.py"]


def test_resolve_task_config_legacy_include_tests_for():
    config = Config(include_tests_for=["polish", "implement"])
    resolved = resolve_task_config(
        task_name="polish",
        global_config=config,
        frontmatter=TaskConfig(),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    assert "tests/**" in resolved.include


def test_resolve_task_config_frontmatter_include_over_legacy():
    config = Config(include_tests_for=["polish"])
    resolved = resolve_task_config(
        task_name="polish",
        global_config=config,
        frontmatter=TaskConfig(include=["docs/**"]),
        cli_interactive=None,
        cli_auto=None,
        cli_model=None,
        cli_context=None,
    )
    # Frontmatter include takes precedence over legacy include_tests_for
    assert resolved.include == ["docs/**"]
