"""Tests for loopflow.context module."""

import pytest

from loopflow.context import find_repo_root, build_prompt, gather_task


@pytest.fixture
def temp_repo(tmp_path):
    """Create a repo with full loopflow structure."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Test Project\n")
    (tmp_path / "STYLE.md").write_text("# Style Guide\n")
    (tmp_path / "VOICE.md").write_text("# Voice\n\nBe excellent.\n")

    lf = tmp_path / ".lf"
    lf.mkdir()
    (lf / "implement.lf").write_text("Implement the feature.\n")

    return tmp_path


def test_find_repo_root_from_subdirectory(temp_repo):
    """Can find repo root from any subdirectory."""
    subdir = temp_repo / "src" / "utils"
    subdir.mkdir(parents=True)

    assert find_repo_root(temp_repo) == temp_repo
    assert find_repo_root(subdir) == temp_repo


def test_find_repo_root_returns_none_outside_repo(tmp_path):
    """Returns None when not in a git repository."""
    assert find_repo_root(tmp_path) is None


def test_build_prompt_assembles_full_context(temp_repo):
    """Prompt includes root .md files and task with delimiters."""
    result = build_prompt(temp_repo, "implement")

    # Root docs with preamble outside
    assert "Follow VOICE and STYLE" in result
    assert "<lf:docs>" in result
    assert "<lf:README>" in result
    assert "# Test Project" in result
    assert "<lf:STYLE>" in result
    assert "# Style Guide" in result
    assert "<lf:VOICE>" in result
    assert "Be excellent." in result
    assert "</lf:docs>" in result

    # Task with preamble outside
    assert "The task" in result
    assert "<lf:task:implement>" in result
    assert "Implement the feature." in result


def test_build_prompt_handles_missing_task(temp_repo):
    """Missing task shows helpful message instead of crashing."""
    result = build_prompt(temp_repo, "nonexistent")

    assert "No task file found" in result


def test_build_prompt_includes_arg(temp_repo):
    """Task arg appears as primary input before task definition."""
    (temp_repo / "design.md").write_text("# Design\n\nBuild a widget.\n")

    result = build_prompt(temp_repo, "implement", arg="design.md")

    assert "Task input" in result
    assert '<lf:arg path="design.md">' in result
    assert "Build a widget." in result
    # Arg should come before task
    assert result.index("<lf:arg") < result.index("<lf:task")


def test_build_prompt_includes_context_files(temp_repo):
    """Context files passed via -c appear in output."""
    (temp_repo / "main.py").write_text("print('hello')\n")

    result = build_prompt(temp_repo, "implement", context=["main.py"])

    assert "Reference files" in result
    assert "<lf:files>" in result
    assert '<lf:file path="main.py">' in result
    assert "print('hello')" in result


def test_build_prompt_arg_and_context_separate(temp_repo):
    """Arg and context are distinct sections."""
    (temp_repo / "design.md").write_text("# Design\n")
    (temp_repo / "main.py").write_text("print('hello')\n")

    result = build_prompt(temp_repo, "implement", arg="design.md", context=["main.py"])

    assert "<lf:arg" in result
    assert "<lf:files>" in result
    # Both present, in different sections
    assert "Task input" in result
    assert "Reference files" in result


def test_build_prompt_inline_instead_of_task(temp_repo):
    """Inline prompt replaces task file lookup."""
    result = build_prompt(temp_repo, task=None, inline="fix the bug in main.py")

    assert "The task" in result
    assert "<lf:task>" in result
    assert "fix the bug in main.py" in result
    assert "</lf:task>" in result
    # Should not have task name in delimiters
    assert "<lf:task:implement>" not in result


def test_build_prompt_inline_with_context(temp_repo):
    """Inline prompt works with context files."""
    (temp_repo / "main.py").write_text("print('hello')\n")

    result = build_prompt(temp_repo, task=None, inline="add tests", context=["main.py"])

    assert "<lf:task>" in result
    assert "add tests" in result
    assert "<lf:files>" in result
    assert "print('hello')" in result


def test_gather_task_prefers_lf_extension(temp_repo):
    """Task file with .lf extension is preferred."""
    lf = temp_repo / ".lf"
    (lf / "test.lf").write_text("Task from .lf file\n")
    (lf / "test.md").write_text("Task from .md file\n")
    (lf / "test.txt").write_text("Task from .txt file\n")

    result = gather_task(temp_repo, "test")
    assert result == "Task from .lf file\n"


def test_gather_task_prefers_md_over_other_extensions(temp_repo):
    """Task file with .md extension preferred over others."""
    lf = temp_repo / ".lf"
    (lf / "test.md").write_text("Task from .md file\n")
    (lf / "test.txt").write_text("Task from .txt file\n")

    result = gather_task(temp_repo, "test")
    assert result == "Task from .md file\n"


def test_gather_task_accepts_other_extensions(temp_repo):
    """Task file with other extension works when .lf/.md absent."""
    lf = temp_repo / ".lf"
    (lf / "test.txt").write_text("Task from .txt file\n")

    result = gather_task(temp_repo, "test")
    assert result == "Task from .txt file\n"


def test_gather_task_accepts_bare_name(temp_repo):
    """Task file with no extension works as fallback."""
    lf = temp_repo / ".lf"
    (lf / "test").write_text("Task from bare file\n")

    result = gather_task(temp_repo, "test")
    assert result == "Task from bare file\n"


def test_gather_task_returns_none_when_missing(temp_repo):
    """gather_task returns None when no matching file exists."""
    result = gather_task(temp_repo, "nonexistent")
    assert result is None
