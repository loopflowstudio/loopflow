"""Tests for loopflow.context module."""

import pytest

from loopflow.context import (
    find_worktree_root,
    build_prompt,
    gather_task,
    gather_prompt_components,
    format_prompt,
    PromptComponents,
    list_builtin_tasks,
    list_user_tasks,
    list_all_tasks,
    _get_builtin_task,
)


@pytest.fixture
def temp_repo(tmp_path):
    """Create a repo with full loopflow structure."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Test Project\n")
    (tmp_path / "STYLE.md").write_text("# Style Guide\n")
    design_dir = tmp_path / ".design"
    design_dir.mkdir()
    (design_dir / "plan.md").write_text("# Plan\n\nDo the thing.\n")

    lf = tmp_path / ".lf"
    lf.mkdir()
    (lf / "implement.md").write_text("Implement the feature.\n")

    return tmp_path


def test_find_worktree_root_from_subdirectory(temp_repo):
    """Can find worktree root from any subdirectory."""
    subdir = temp_repo / "src" / "utils"
    subdir.mkdir(parents=True)

    assert find_worktree_root(temp_repo) == temp_repo
    assert find_worktree_root(subdir) == temp_repo


def test_find_worktree_root_returns_none_outside_repo(tmp_path):
    """Returns None when not in a git repository."""
    assert find_worktree_root(tmp_path) is None


def test_build_prompt_assembles_full_context(temp_repo):
    """Prompt includes root .md files and task with delimiters."""
    result = build_prompt(temp_repo, "implement")

    # Root docs with preamble outside
    assert "Follow STYLE" in result
    assert "<lf:docs>" in result
    assert "<lf:plan>" in result
    assert "Do the thing." in result
    assert "<lf:README>" in result
    assert "# Test Project" in result
    assert "<lf:STYLE>" in result
    assert "# Style Guide" in result
    assert "</lf:docs>" in result

    # Task with preamble outside
    assert "The task" in result
    assert "<lf:task:implement>" in result
    assert "Implement the feature." in result


def test_build_prompt_handles_missing_task(temp_repo):
    """Missing task shows helpful message instead of crashing."""
    result = build_prompt(temp_repo, "nonexistent")

    assert "No task file found" in result


def test_build_prompt_includes_context_files(temp_repo):
    """Context files passed via -c appear in output."""
    (temp_repo / "main.py").write_text("print('hello')\n")

    result = build_prompt(temp_repo, "implement", context=["main.py"])

    assert "Reference files" in result
    assert "<lf:files>" in result
    assert '<lf:file path="main.py">' in result
    assert "print('hello')" in result


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


def test_gather_task_prefers_claude_commands(temp_repo):
    """Task file in .claude/commands/ is preferred over .lf/."""
    claude_dir = temp_repo / ".claude" / "commands"
    claude_dir.mkdir(parents=True)
    (claude_dir / "test.md").write_text("Task from .claude/commands/\n")

    lf = temp_repo / ".lf"
    (lf / "test.md").write_text("Task from .lf/\n")

    result = gather_task(temp_repo, "test")
    assert result.content == "Task from .claude/commands/\n"


def test_gather_task_finds_md_in_lf(temp_repo):
    """Task file with .md extension in .lf/ works."""
    lf = temp_repo / ".lf"
    (lf / "test.md").write_text("Task from .lf/ md file\n")

    result = gather_task(temp_repo, "test")
    assert result.content == "Task from .lf/ md file\n"


def test_gather_task_ignores_non_md_extensions(temp_repo):
    """Task files with non-.md extensions are not found."""
    lf = temp_repo / ".lf"
    (lf / "test.lf").write_text("Task from .lf file\n")
    (lf / "test.txt").write_text("Task from .txt file\n")

    result = gather_task(temp_repo, "test")
    assert result is None  # Only .md is supported


def test_gather_task_returns_none_when_missing(temp_repo):
    """gather_task returns None when no matching file exists."""
    result = gather_task(temp_repo, "nonexistent")
    assert result is None


def test_gather_prompt_components_returns_dataclass(temp_repo):
    """gather_prompt_components returns PromptComponents with all fields."""
    components = gather_prompt_components(temp_repo, "implement")

    assert isinstance(components, PromptComponents)
    assert components.repo_root == temp_repo
    assert len(components.docs) == 3  # .design/plan + README, STYLE
    assert components.loopflow_doc is not None  # bundled system doc
    assert components.task == ("implement", "Implement the feature.\n")


def test_gather_prompt_components_includes_context(temp_repo):
    """gather_prompt_components captures context files in diff_files (merged)."""
    (temp_repo / "main.py").write_text("print('hello')")

    components = gather_prompt_components(temp_repo, "implement", context=["main.py"])

    # Context files are merged into diff_files (deduped at load time)
    main_files = [(p, c) for p, c in components.diff_files if p.name == "main.py"]
    assert len(main_files) == 1
    path, content = main_files[0]
    assert "print('hello')" in content


def test_gather_prompt_components_inline_task(temp_repo):
    """gather_prompt_components handles inline task."""
    components = gather_prompt_components(temp_repo, task=None, inline="fix the bug")

    assert components.task == ("inline", "fix the bug")


def test_gather_prompt_components_missing_task(temp_repo):
    """gather_prompt_components handles missing task file."""
    components = gather_prompt_components(temp_repo, "nonexistent")

    assert components.task is not None
    name, content = components.task
    assert name == "nonexistent"
    assert "No task file found" in content


def test_format_prompt_from_components(temp_repo):
    """format_prompt produces same output as build_prompt."""
    components = gather_prompt_components(temp_repo, "implement")
    formatted = format_prompt(components)
    direct = build_prompt(temp_repo, "implement")

    assert formatted == direct


def test_format_prompt_with_all_components(temp_repo):
    """format_prompt includes all component types."""
    (temp_repo / "main.py").write_text("print('hello')")

    components = gather_prompt_components(
        temp_repo, "implement", context=["main.py"]
    )
    formatted = format_prompt(components)

    assert "<lf:docs>" in formatted
    assert "<lf:task:implement>" in formatted
    assert "<lf:files>" in formatted


def test_gather_prompt_components_deduplicates_diff_and_context(temp_repo, monkeypatch):
    """Files in both diff_files and context are only loaded once."""
    (temp_repo / "shared.py").write_text("# shared file")
    (temp_repo / "context_only.py").write_text("# context only")
    (temp_repo / "diff_only.py").write_text("# diff only")

    # Mock gather_diff_files to return shared.py and diff_only.py
    monkeypatch.setattr(
        "loopflow.context.gather_diff_files",
        lambda repo_root: ["shared.py", "diff_only.py"],
    )

    components = gather_prompt_components(
        temp_repo,
        "implement",
        context=["shared.py", "context_only.py"],  # shared.py overlaps with diff
        include_diff_files=True,
    )

    # All three files should appear, each exactly once
    paths = [p.name for p, _ in components.diff_files]
    assert "shared.py" in paths
    assert "context_only.py" in paths
    assert "diff_only.py" in paths
    # Deduplication: shared.py appears only once despite being in both lists
    assert paths.count("shared.py") == 1
    assert paths.count("context_only.py") == 1
    assert paths.count("diff_only.py") == 1


# =============================================================================
# Builtin task tests
# =============================================================================


def test_list_builtin_tasks_returns_known_builtins():
    """Builtin tasks list includes expected tasks."""
    builtins = list_builtin_tasks()
    assert "design" in builtins
    assert "implement" in builtins
    assert "review" in builtins
    assert "reduce" in builtins
    assert "expand" in builtins
    assert "explore" in builtins


def test_get_builtin_task_returns_path_for_known_builtin():
    """_get_builtin_task returns path for existing builtin."""
    path = _get_builtin_task("design")
    assert path is not None
    assert path.exists()
    assert path.name == "design.md"


def test_get_builtin_task_returns_none_for_unknown():
    """_get_builtin_task returns None for non-existent task."""
    path = _get_builtin_task("nonexistent_task_xyz")
    assert path is None


def test_gather_task_falls_back_to_builtin(tmp_path):
    """gather_task returns builtin when no user task exists."""
    # Create empty repo
    (tmp_path / ".git").mkdir()

    # No .lf/ or .claude/commands/ - should fall back to builtin
    result = gather_task(tmp_path, "design")
    assert result is not None
    assert result.name == "design"
    assert "implementation spec" in result.content.lower()


def test_gather_task_user_overrides_builtin(tmp_path):
    """User task file takes precedence over builtin."""
    (tmp_path / ".git").mkdir()
    lf = tmp_path / ".lf"
    lf.mkdir()
    (lf / "design.md").write_text("My custom design task\n")

    result = gather_task(tmp_path, "design")
    assert result is not None
    assert "My custom design task" in result.content


def test_list_user_tasks_returns_user_tasks(tmp_path):
    """list_user_tasks returns tasks from .lf/ and .claude/commands/."""
    (tmp_path / ".git").mkdir()

    # .lf/ tasks
    lf = tmp_path / ".lf"
    lf.mkdir()
    (lf / "custom.md").write_text("custom task")
    (lf / "another.md").write_text("another task")
    (lf / "config.yaml").write_text("not a task")

    # .claude/commands/ tasks
    claude = tmp_path / ".claude" / "commands"
    claude.mkdir(parents=True)
    (claude / "third.md").write_text("third task")

    tasks = list_user_tasks(tmp_path)
    assert "custom" in tasks
    assert "another" in tasks
    assert "third" in tasks
    assert "config" not in tasks  # config.yaml should be excluded


def test_list_all_tasks_separates_user_and_builtin(tmp_path):
    """list_all_tasks returns user tasks and builtin-only tasks separately."""
    (tmp_path / ".git").mkdir()

    # Override one builtin
    lf = tmp_path / ".lf"
    lf.mkdir()
    (lf / "design.md").write_text("custom design")
    (lf / "custom.md").write_text("custom task")

    user_tasks, builtin_only = list_all_tasks(tmp_path)

    # design is overridden, so it's in user_tasks
    assert "design" in user_tasks
    assert "custom" in user_tasks

    # Other builtins like implement, review should be in builtin_only
    assert "implement" in builtin_only
    assert "review" in builtin_only

    # design should NOT be in builtin_only (it's overridden)
    assert "design" not in builtin_only


def test_list_all_tasks_without_repo():
    """list_all_tasks works without a repo root (returns only builtins)."""
    user_tasks, builtin_only = list_all_tasks(None)

    assert user_tasks == []
    assert "design" in builtin_only
    assert "implement" in builtin_only


def test_gather_task_works_without_repo():
    """gather_task returns builtin tasks even without repo_root."""
    result = gather_task(None, "design")
    assert result is not None
    assert result.name == "design"
    assert "implementation spec" in result.content.lower()


def test_gather_task_returns_none_for_unknown_without_repo():
    """gather_task returns None for unknown tasks without repo_root."""
    result = gather_task(None, "nonexistent_task_xyz")
    assert result is None


# =============================================================================
# Summaries tests
# =============================================================================


def test_trigger_background_refresh_creates_log_file(tmp_path, monkeypatch):
    """Background refresh writes output to log file instead of DEVNULL."""
    from loopflow.context import _trigger_background_refresh
    import subprocess

    (tmp_path / ".git").mkdir()
    summaries_dir = tmp_path / ".lf" / "summaries"

    # Mock Popen to avoid actually running the subprocess
    popen_calls = []

    class MockPopen:
        pid = 12345

        def __init__(self, *args, **kwargs):
            popen_calls.append((args, kwargs))

    monkeypatch.setattr(subprocess, "Popen", MockPopen)

    _trigger_background_refresh(tmp_path)

    # Should have created summaries dir
    assert summaries_dir.exists()

    # Should have written lock file
    lock_file = summaries_dir / ".refresh.lock"
    assert lock_file.exists()
    assert lock_file.read_text() == "12345"

    # Should have been called with log file, not DEVNULL
    assert len(popen_calls) == 1
    _, kwargs = popen_calls[0]
    assert "stdout" in kwargs
    # stdout should be a file object (not DEVNULL)
    assert kwargs["stdout"] is not subprocess.DEVNULL
