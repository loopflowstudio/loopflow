"""Tests for loopflow.context module."""

from pathlib import Path
from unittest.mock import patch

import pytest

from loopflow.lf.context import (
    ContextConfig,
    PromptComponents,
    _get_builtin_step,
    build_prompt,
    find_worktree_root,
    format_prompt,
    gather_prompt_components,
    gather_step,
    list_all_steps,
    list_builtin_steps,
    list_user_steps,
    trim_prompt_components,
)
from loopflow.lf.tokens import count_tokens


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
    assert "The step" in result
    assert "<lf:step:implement>" in result
    assert "Implement the feature." in result


def test_build_prompt_handles_missing_task(temp_repo):
    """Missing task shows helpful message instead of crashing."""
    result = build_prompt(temp_repo, "nonexistent")

    assert "No step file found" in result


def test_build_prompt_with_voices(temp_repo):
    """build_prompt passes voices through to formatting."""
    voices_dir = temp_repo / ".lf" / "voices"
    voices_dir.mkdir(parents=True, exist_ok=True)
    (voices_dir / "concise.md").write_text("Be concise.")

    result = build_prompt(temp_repo, "implement", voices=["concise"])

    assert "<lf:voice:concise>" in result
    assert "Be concise." in result


def test_build_prompt_includes_context_files(temp_repo):
    """Context files passed via pathset appear in output."""
    (temp_repo / "main.py").write_text("print('hello')\n")

    result = build_prompt(
        temp_repo, "implement", context_config=ContextConfig(pathset=["main.py"])
    )

    assert "Reference files" in result
    assert "<lf:files>" in result
    assert '<lf:file path="main.py">' in result
    assert "print('hello')" in result


def test_build_prompt_inline_instead_of_task(temp_repo):
    """Inline prompt replaces task file lookup."""
    result = build_prompt(temp_repo, step=None, inline="fix the bug in main.py")

    assert "The step" in result
    assert "<lf:step>" in result
    assert "fix the bug in main.py" in result
    assert "</lf:step>" in result
    # Should not have task name in delimiters
    assert "<lf:step:implement>" not in result


def test_build_prompt_inline_with_context(temp_repo):
    """Inline prompt works with context files."""
    (temp_repo / "main.py").write_text("print('hello')\n")

    result = build_prompt(
        temp_repo,
        step=None,
        inline="add tests",
        context_config=ContextConfig(pathset=["main.py"]),
    )

    assert "<lf:step>" in result
    assert "add tests" in result
    assert "<lf:files>" in result
    assert "print('hello')" in result


def test_gather_step_prefers_claude_commands(temp_repo):
    """Task file in .claude/commands/ is preferred over .lf/."""
    claude_dir = temp_repo / ".claude" / "commands"
    claude_dir.mkdir(parents=True)
    (claude_dir / "test.md").write_text("Task from .claude/commands/\n")

    lf = temp_repo / ".lf"
    (lf / "test.md").write_text("Task from .lf/\n")

    result = gather_step(temp_repo, "test")
    assert result.content == "Task from .claude/commands/\n"


def test_gather_step_finds_md_in_lf(temp_repo):
    """Task file with .md extension in .lf/ works."""
    lf = temp_repo / ".lf"
    (lf / "test.md").write_text("Task from .lf/ md file\n")

    result = gather_step(temp_repo, "test")
    assert result.content == "Task from .lf/ md file\n"


def test_gather_step_ignores_non_md_extensions(temp_repo):
    """Task files with non-.md extensions are not found."""
    lf = temp_repo / ".lf"
    (lf / "test.lf").write_text("Task from .lf file\n")
    (lf / "test.txt").write_text("Task from .txt file\n")

    result = gather_step(temp_repo, "test")
    assert result is None  # Only .md is supported


def test_gather_step_returns_none_when_missing(temp_repo):
    """gather_step returns None when no matching file exists."""
    result = gather_step(temp_repo, "nonexistent")
    assert result is None


def test_gather_prompt_components_returns_dataclass(temp_repo):
    """gather_prompt_components returns PromptComponents with all fields."""
    components = gather_prompt_components(temp_repo, "implement")

    assert isinstance(components, PromptComponents)
    assert components.repo_root == temp_repo
    assert len(components.docs) == 3  # .design/plan + README, STYLE
    assert components.loopflow_doc is not None  # bundled system doc
    assert components.step == ("implement", "Implement the feature.\n")


def test_gather_prompt_components_includes_context(temp_repo):
    """gather_prompt_components captures pathset files in diff_files (merged)."""
    (temp_repo / "main.py").write_text("print('hello')")

    components = gather_prompt_components(
        temp_repo, "implement", context_config=ContextConfig(pathset=["main.py"])
    )

    # Pathset files are merged into diff_files (deduped at load time)
    main_files = [(p, c) for p, c in components.diff_files if p.name == "main.py"]
    assert len(main_files) == 1
    path, content = main_files[0]
    assert "print('hello')" in content


def test_gather_prompt_components_inline_task(temp_repo):
    """gather_prompt_components handles inline task."""
    components = gather_prompt_components(temp_repo, step=None, inline="fix the bug")

    assert components.step == ("inline", "fix the bug")


def test_gather_prompt_components_missing_task(temp_repo):
    """gather_prompt_components handles missing task file."""
    components = gather_prompt_components(temp_repo, "nonexistent")

    assert components.step is not None
    name, content = components.step
    assert name == "nonexistent"
    assert "No step file found" in content


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
        temp_repo, "implement", context_config=ContextConfig(pathset=["main.py"])
    )
    formatted = format_prompt(components)

    assert "<lf:docs>" in formatted
    assert "<lf:step:implement>" in formatted
    assert "<lf:files>" in formatted


def test_gather_prompt_components_deduplicates_diff_and_context(temp_repo, monkeypatch):
    """Files in both diff_files and context are only loaded once."""
    (temp_repo / "shared.py").write_text("# shared file")
    (temp_repo / "context_only.py").write_text("# context only")
    (temp_repo / "diff_only.py").write_text("# diff only")

    # Mock gather_diff_files to return shared.py and diff_only.py
    monkeypatch.setattr(
        "loopflow.lf.context.gather_diff_files",
        lambda repo_root: ["shared.py", "diff_only.py"],
    )

    components = gather_prompt_components(
        temp_repo,
        "implement",
        context_config=ContextConfig(
            pathset=["shared.py", "context_only.py"],  # shared.py overlaps with diff
            diff_files=True,
        ),
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


def test_list_builtin_steps_returns_known_builtins():
    """Builtin tasks list includes expected tasks."""
    builtins = list_builtin_steps()
    assert "design" in builtins
    assert "implement" in builtins
    assert "review" in builtins
    assert "reduce" in builtins
    assert "expand" in builtins
    assert "explore" in builtins


def test_get_builtin_step_returns_path_for_known_builtin():
    """_get_builtin_step returns path for existing builtin."""
    path = _get_builtin_step("design")
    assert path is not None
    assert path.exists()
    assert path.name == "design.md"


def test_get_builtin_step_returns_none_for_unknown():
    """_get_builtin_step returns None for non-existent task."""
    path = _get_builtin_step("nonexistent_task_xyz")
    assert path is None


def test_gather_step_falls_back_to_builtin(tmp_path):
    """gather_step returns builtin when no user task exists."""
    # Create empty repo
    (tmp_path / ".git").mkdir()

    # No .lf/ or .claude/commands/ - should fall back to builtin
    result = gather_step(tmp_path, "design")
    assert result is not None
    assert result.name == "design"
    assert "implementation spec" in result.content.lower()


def test_gather_step_user_overrides_builtin(tmp_path):
    """User task file takes precedence over builtin."""
    (tmp_path / ".git").mkdir()
    lf = tmp_path / ".lf"
    lf.mkdir()
    (lf / "design.md").write_text("My custom design task\n")

    result = gather_step(tmp_path, "design")
    assert result is not None
    assert "My custom design task" in result.content


def test_list_user_steps_returns_user_tasks(tmp_path):
    """list_user_steps returns tasks from .lf/ and .claude/commands/."""
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

    tasks = list_user_steps(tmp_path)
    assert "custom" in tasks
    assert "another" in tasks
    assert "third" in tasks
    assert "config" not in tasks  # config.yaml should be excluded


@patch("loopflow.lf.skills._RAMS_PATH", Path("/nonexistent/rams.md"))
@patch("loopflow.lf.skills._SUPERPOWERS_PATHS", [])
@patch("loopflow.lf.context._GLOBAL_STEP_PATHS", [])
def test_list_all_steps_separates_user_and_builtin(tmp_path):
    """list_all_steps returns user tasks, global tasks, builtin-only tasks, and external skills."""
    (tmp_path / ".git").mkdir()

    # Override one builtin
    lf = tmp_path / ".lf"
    lf.mkdir()
    (lf / "design.md").write_text("custom design")
    (lf / "custom.md").write_text("custom task")

    user_tasks, global_tasks, builtin_only, external_skills = list_all_steps(tmp_path)

    # design is overridden, so it's in user_tasks
    assert "design" in user_tasks
    assert "custom" in user_tasks

    # global_tasks is empty when _GLOBAL_STEP_PATHS is mocked to []
    assert global_tasks == []

    # Other builtins like implement, review should be in builtin_only
    assert "implement" in builtin_only
    assert "review" in builtin_only

    # design should NOT be in builtin_only (it's overridden)
    assert "design" not in builtin_only

    # external_skills is empty without skill_sources config
    assert external_skills == []


@patch("loopflow.lf.skills._RAMS_PATH", Path("/nonexistent/rams.md"))
@patch("loopflow.lf.skills._SUPERPOWERS_PATHS", [])
@patch("loopflow.lf.context._GLOBAL_STEP_PATHS", [])
def test_list_all_steps_without_repo():
    """list_all_steps works without a repo root (returns only builtins)."""
    user_tasks, global_tasks, builtin_only, external_skills = list_all_steps(None)

    assert user_tasks == []
    assert global_tasks == []
    assert "design" in builtin_only
    assert "implement" in builtin_only
    assert external_skills == []


def test_gather_step_works_without_repo():
    """gather_step returns builtin tasks even without repo_root."""
    result = gather_step(None, "design")
    assert result is not None
    assert result.name == "design"
    assert "implementation spec" in result.content.lower()


def test_gather_step_returns_none_for_unknown_without_repo():
    """gather_step returns None for unknown tasks without repo_root."""
    result = gather_step(None, "nonexistent_task_xyz")
    assert result is None


# =============================================================================
# Summaries tests
# =============================================================================


def test_trigger_background_refresh_creates_log_file(tmp_path, monkeypatch):
    """Background refresh writes output to log file instead of DEVNULL."""
    import subprocess

    from loopflow.lf.context import _trigger_background_refresh

    (tmp_path / ".git").mkdir()

    # Mock Path.home() to use tmp_path so we can check the files
    lf_dir = tmp_path / ".lf"
    monkeypatch.setattr("loopflow.lf.context.Path.home", lambda: tmp_path)

    # Mock Popen to avoid actually running the subprocess
    popen_calls = []

    class MockPopen:
        pid = 12345

        def __init__(self, *args, **kwargs):
            popen_calls.append((args, kwargs))

    monkeypatch.setattr(subprocess, "Popen", MockPopen)

    _trigger_background_refresh(tmp_path)

    # Should have created ~/.lf dir
    assert lf_dir.exists()

    # Should have written lock file
    lock_file = lf_dir / ".refresh.lock"
    assert lock_file.exists()
    assert lock_file.read_text() == "12345"

    # Should have been called with log file, not DEVNULL
    assert len(popen_calls) == 1
    _, kwargs = popen_calls[0]
    assert "stdout" in kwargs
    # stdout should be a file object (not DEVNULL)
    assert kwargs["stdout"] is not subprocess.DEVNULL


# =============================================================================
# Task argument substitution tests
# =============================================================================


def test_task_args_replace_template_variables(tmp_path):
    """Task args replace {{key}} template variables in task content."""
    _setup_task_template(tmp_path)
    task_args = [
        "name_a=impl-claude",
        "name_b=impl-codex",
        "diff_a=+ added feature",
        "diff_b=- removed feature",
    ]

    components = gather_prompt_components(
        tmp_path,
        step="compare",
        step_args=task_args,
    )

    assert components.step is not None
    _, content = components.step

    assert "impl-claude" in content
    assert "impl-codex" in content
    assert "+ added feature" in content
    assert "- removed feature" in content
    assert "{{" not in content  # No template vars left


def test_task_args_in_formatted_prompt(tmp_path):
    """Task args work through full prompt formatting pipeline."""
    _setup_task_template(tmp_path)
    task_args = [
        "name_a=version-1",
        "name_b=version-2",
        "diff_a=diff content a",
        "diff_b=diff content b",
    ]

    components = gather_prompt_components(
        tmp_path,
        step="compare",
        step_args=task_args,
    )
    prompt = format_prompt(components)

    assert "version-1" in prompt
    assert "version-2" in prompt
    assert "diff content a" in prompt
    assert "diff content b" in prompt


def test_task_args_no_args_leaves_templates(tmp_path):
    """Without task_args, template variables remain unchanged."""
    _setup_task_template(tmp_path)
    components = gather_prompt_components(
        tmp_path,
        step="compare",
    )

    assert components.step is not None
    _, content = components.step

    # Template variables still present
    assert "{{name_a}}" in content
    assert "{{name_b}}" in content


def test_task_args_partial_substitution(tmp_path):
    """Task args only replace specified variables, leave others."""
    _setup_task_template(tmp_path)
    task_args = ["name_a=version-1"]

    components = gather_prompt_components(
        tmp_path,
        step="compare",
        step_args=task_args,
    )

    assert components.step is not None
    _, content = components.step

    assert "version-1" in content
    assert "{{name_a}}" not in content
    # Others still templated
    assert "{{name_b}}" in content
    assert "{{diff_a}}" in content


def test_task_args_with_equals_in_value(tmp_path):
    """Task args handle values containing '=' character."""
    _setup_task_template(tmp_path)
    task_args = ["diff_a=x=1, y=2"]

    components = gather_prompt_components(
        tmp_path,
        step="compare",
        step_args=task_args,
    )

    assert components.step is not None
    _, content = components.step

    # Should preserve the = in the value
    assert "x=1, y=2" in content


def test_task_args_with_multiline_value(tmp_path):
    """Task args support multiline values."""
    _setup_task_template(tmp_path)
    diff_value = """+ def new_function():
+     return 42
- old_code()"""

    task_args = [f"diff_a={diff_value}"]

    components = gather_prompt_components(
        tmp_path,
        step="compare",
        step_args=task_args,
    )

    assert components.step is not None
    _, content = components.step

    assert "def new_function():" in content
    assert "return 42" in content
    assert "old_code()" in content


def _setup_task_template(tmp_path):
    """Helper to set up a repo with a task template."""
    (tmp_path / ".git").mkdir(exist_ok=True)
    (tmp_path / "README.md").write_text("# Test\n")
    lf = tmp_path / ".lf"
    lf.mkdir(exist_ok=True)
    (lf / "compare.md").write_text(
        "Compare {{name_a}} and {{name_b}}.\n\nDiff A:\n{{diff_a}}\n\nDiff B:\n{{diff_b}}\n"
    )


# =============================================================================
# Internal docs (.docs/) tests
# =============================================================================


def test_internal_docs_auto_included(tmp_path):
    """.docs/ markdown files are auto-included in context."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Test\n")

    # Create .docs/ with some markdown files
    internal_docs = tmp_path / ".docs"
    internal_docs.mkdir()
    (internal_docs / "architecture.md").write_text("# Architecture\n\nHow it works.\n")
    (internal_docs / "decisions").mkdir()
    (internal_docs / "decisions" / "adr-001.md").write_text("# ADR 001\n\nWe chose X.\n")

    components = gather_prompt_components(tmp_path, "implement")
    prompt = format_prompt(components)

    # .docs/ files should be included
    assert "# Architecture" in prompt
    assert "How it works." in prompt
    assert "# ADR 001" in prompt
    assert "We chose X." in prompt


def test_internal_docs_empty_when_missing(tmp_path):
    """.docs/ missing doesn't cause errors."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Test\n")

    components = gather_prompt_components(tmp_path, step=None, inline="test")
    prompt = format_prompt(components)

    # Should still work, just no .docs content
    assert "# Test" in prompt


def test_public_docs_not_auto_included(tmp_path):
    """docs/ (public) is NOT auto-included in context."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Test\n")

    # Create docs/ with public documentation
    public_docs = tmp_path / "docs"
    public_docs.mkdir()
    (public_docs / "getting-started.md").write_text("# Getting Started\n\nFor users.\n")
    (public_docs / "api.md").write_text("# API Reference\n\nEndpoints.\n")

    components = gather_prompt_components(tmp_path, "implement")
    prompt = format_prompt(components)

    # docs/ should NOT be auto-included
    assert "# Getting Started" not in prompt
    assert "For users." not in prompt
    assert "# API Reference" not in prompt


def test_internal_docs_before_root_docs(tmp_path):
    """.docs/ appears before repo root .md files in context."""
    (tmp_path / ".git").mkdir()
    (tmp_path / "README.md").write_text("# Root README\n")

    design_dir = tmp_path / ".design"
    design_dir.mkdir()
    (design_dir / "plan.md").write_text("# Design Plan\n")

    internal_docs = tmp_path / ".docs"
    internal_docs.mkdir()
    (internal_docs / "context.md").write_text("# Internal Context\n")

    components = gather_prompt_components(tmp_path, step=None, inline="test")

    # Check order: .design/, then .docs/, then root docs
    doc_paths = [str(p) for p, _ in components.docs]
    design_idx = next(i for i, p in enumerate(doc_paths) if ".design" in p)
    internal_idx = next(i for i, p in enumerate(doc_paths) if ".docs" in p)
    readme_idx = next(i for i, p in enumerate(doc_paths) if "README" in p)

    assert design_idx < internal_idx < readme_idx


def test_trim_prompt_components_drops_oversize_diff_files(tmp_path):
    """Drop diff_files entirely when they exceed the token budget."""
    big_text = "hello " * 2000
    diff_tokens = count_tokens(big_text)
    components = PromptComponents(
        run_mode=None,
        docs=[],
        diff=None,
        diff_files=[(tmp_path / "big.py", big_text)],
        step=("implement", "do it"),
        repo_root=tmp_path,
        clipboard=None,
        loopflow_doc=None,
        voices=None,
        image_files=None,
        summaries=None,
    )

    trimmed, dropped = trim_prompt_components(components, diff_tokens - 1)

    assert trimmed.diff_files == []
    assert any(item.kind == "diff_files" for item in dropped)


def test_trim_prompt_components_keeps_small_diff_files(tmp_path):
    """Keep diff_files if they fit and drop largest other components first."""
    big_doc = "alpha " * 400
    small_doc = "beta " * 10
    diff_content = "code " * 5
    step_content = "step " * 5

    big_tokens = count_tokens(big_doc)
    total_tokens = (
        big_tokens
        + count_tokens(small_doc)
        + count_tokens(diff_content)
        + count_tokens(step_content)
    )
    max_tokens = total_tokens - (big_tokens // 2)

    components = PromptComponents(
        run_mode=None,
        docs=[(tmp_path / "big.md", big_doc), (tmp_path / "small.md", small_doc)],
        diff=None,
        diff_files=[(tmp_path / "main.py", diff_content)],
        step=("implement", step_content),
        repo_root=tmp_path,
        clipboard=None,
        loopflow_doc=None,
        voices=None,
        image_files=None,
        summaries=None,
    )

    trimmed, dropped = trim_prompt_components(components, max_tokens)

    assert trimmed.diff_files
    assert (tmp_path / "big.md") not in {path for path, _content in trimmed.docs}
    assert dropped
    assert dropped[0].kind == "docs"
