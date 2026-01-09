"""Tests for task argument substitution."""

import pytest

from loopflow.context import gather_prompt_components, format_prompt


@pytest.fixture
def temp_repo(tmp_path):
    """Create a repo with task that uses template variables."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Test\n")

    lf = tmp_path / ".lf"
    lf.mkdir()
    (lf / "compare.md").write_text(
        "Compare {{name_a}} and {{name_b}}.\n\n"
        "Diff A:\n{{diff_a}}\n\n"
        "Diff B:\n{{diff_b}}\n"
    )

    return tmp_path


def test_task_args_replace_template_variables(temp_repo):
    """Task args replace {{key}} template variables in task content."""
    task_args = [
        "name_a=impl-claude",
        "name_b=impl-codex",
        "diff_a=+ added feature",
        "diff_b=- removed feature",
    ]

    components = gather_prompt_components(
        temp_repo,
        task="compare",
        task_args=task_args,
    )

    assert components.task is not None
    _, content = components.task

    assert "impl-claude" in content
    assert "impl-codex" in content
    assert "+ added feature" in content
    assert "- removed feature" in content
    assert "{{" not in content  # No template vars left


def test_task_args_in_formatted_prompt(temp_repo):
    """Task args work through full prompt formatting pipeline."""
    task_args = [
        "name_a=version-1",
        "name_b=version-2",
        "diff_a=diff content a",
        "diff_b=diff content b",
    ]

    components = gather_prompt_components(
        temp_repo,
        task="compare",
        task_args=task_args,
    )
    prompt = format_prompt(components)

    assert "version-1" in prompt
    assert "version-2" in prompt
    assert "diff content a" in prompt
    assert "diff content b" in prompt


def test_task_args_no_args_leaves_templates(temp_repo):
    """Without task_args, template variables remain unchanged."""
    components = gather_prompt_components(
        temp_repo,
        task="compare",
    )

    assert components.task is not None
    _, content = components.task

    # Template variables still present
    assert "{{name_a}}" in content
    assert "{{name_b}}" in content


def test_task_args_partial_substitution(temp_repo):
    """Task args only replace specified variables, leave others."""
    task_args = ["name_a=version-1"]

    components = gather_prompt_components(
        temp_repo,
        task="compare",
        task_args=task_args,
    )

    assert components.task is not None
    _, content = components.task

    assert "version-1" in content
    assert "{{name_a}}" not in content
    # Others still templated
    assert "{{name_b}}" in content
    assert "{{diff_a}}" in content


def test_task_args_with_equals_in_value(temp_repo):
    """Task args handle values containing '=' character."""
    task_args = ["diff_a=x=1, y=2"]

    components = gather_prompt_components(
        temp_repo,
        task="compare",
        task_args=task_args,
    )

    assert components.task is not None
    _, content = components.task

    # Should preserve the = in the value
    assert "x=1, y=2" in content


def test_task_args_with_multiline_value(temp_repo):
    """Task args support multiline values."""
    diff_value = """+ def new_function():
+     return 42
- old_code()"""

    task_args = [f"diff_a={diff_value}"]

    components = gather_prompt_components(
        temp_repo,
        task="compare",
        task_args=task_args,
    )

    assert components.task is not None
    _, content = components.task

    assert "def new_function():" in content
    assert "return 42" in content
    assert "old_code()" in content
