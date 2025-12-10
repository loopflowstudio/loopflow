# Loopflow Style Guide

This is the governing document of the loopflow codebase. Humans and LLMs alike are expected to follow it.

## Quick Reference

- Use `uv run` or activate `.venv` before any Python command
- Prefix private functions with `_`
- Return `None` for "not found"; raise exceptions for "shouldn't happen"
- No `Args:`/`Returns:` docstrings—if types are clear, skip the docstring
- Mock side effects, but don't test mock wiring
- Design docs go in `<branch>.md` at repo root; delete them when the feature ships

## File-Type Guidelines

When editing `*.py` files:
- Put imports at the top, not inline
- Use type hints on all public functions
- One-line docstring if any; skip if the name and types are clear

When editing `*_test.py` or `test_*.py` files:
- Keep tests short and focused on one behavior
- Mock side effects (network, subprocess), but assert on results, not mock calls
- Delete flaky tests rather than adding retries

When writing CLI code with Typer:
- Prefer lowercase short flags (`-p`, `-c`), support uppercase as aliases
- Pass args through to underlying tools rather than re-implementing
- Default to sensible behavior (e.g., whole repo as context)

When editing `README.md` files:
- Start with Usage and example commands
- Don't duplicate what's in the source code
- Write for users, not maintainers
- Update when adding or changing user-facing features

When editing `<branch>.md` design docs:
- Focus on what's left to build, not what's done
- Delete the doc when the feature ships

# Goals

## Clarity

Design around data structures and public APIs. Aim for a 1:1 mapping between real-world concepts and their representation in code.

Write code that demonstrates its own correctness. If a feature exists, write a test that proves it works. Assume you won't finish everything you start—make it easy to see what's done and what's broken.

## Simplicity

Every line of code must earn its place. Readable code is not terse code; don't sacrifice clarity for brevity. But recognize that lines can be net-negative:

* Unused code
* Comments that restate the obvious
* Checks for impossible conditions

Start with minimal data structures and APIs. If the core is right, trimming excess at the edges is straightforward.

# Development Environment

Use `uv` for all package management. Never use pip directly.

```bash
uv sync                       # Install dependencies
uv run pytest tests/          # Run tests
uv run lf agent --help        # Run commands

# Or activate the venv
source .venv/bin/activate
pytest tests/
```

# Code Organization

Follow PEP8. Consistency with existing code matters more than any specific rule.

Keep `__init__.py` files empty. They exist only to mark directories as packages.

Keep information in one place. Version numbers, configuration, documentation—each piece of information should have a single source of truth. Don't duplicate versions in `__init__.py` and `pyproject.toml`. Don't copy FAQs into multiple READMEs. If something needs to appear in multiple places, generate it or reference the source.

Put imports at the top of the file. Declare dependencies in `pyproject.toml` and assume they're available.

Keep one implementation. Avoid `v2_`, `_old`, `_new`, `_backup` prefixes and suffixes—look up old versions in git. If you're tempted to keep both old and new code around, delete the old version and commit. You can always get it back from git if needed.

Don't maintain backwards compatibility unless explicitly required. If a config format or API changes, migrate everything to the new format—don't write code that handles both old and new. Backwards compatibility is for production databases and published APIs with external users, not internal config files. Unless the design doc specifies a migration path, assume we don't want compatibility shims.

Use header comments to group related code sections.

## Naming

Use verb-first names for action functions: `find_prompt()`, `load_config()`, `create_worktree()`.

Prefix private functions with underscore: `_should_ignore()`, `_load_file()`.

Name things after what they are, not what they're for: `Document`, `FileEdit`, `Target`—not `DocumentHelper`, `EditResult`, `OutputHandler`.

## Type Hints

Use `Optional[X]` only when a value is truly optional (caller can omit it):

```python
# Good: caller can omit repo_root
def find_prompt(name: str, repo_root: Optional[Path] = None) -> Path: ...

# Bad: None means "not found"—that's an error, not an option
def get_user() -> Optional[User]: ...
```

## Error Handling

Errors are for users; exceptions are for programmers.

Return errors when the caller should handle them—invalid input, missing files, failed requests. Raise exceptions for bugs: violated invariants, impossible states, programming mistakes.

```python
# Error: caller decides what to do
def find_config(path: Path) -> Optional[Config]:
    if not path.exists():
        return None
    return load(path)

# Exception: this shouldn't happen
def get_target(name: str) -> Target:
    if name not in TARGETS:
        raise ValueError(f"Unknown target: {name}")
    return TARGETS[name]
```

When in doubt: if you'd write an `assert`, raise an exception instead—it's easier for callers to catch.

# Documentation

The best documentation is simple code. Descriptive names, type hints, and clear APIs often suffice.

The worst documentation is wrong documentation. If it can drift from the code, it will. Update docs when you change code—or delete them.

Put documentation next to code. A few paragraphs at the top of a key file beats a separate doc that nobody maintains.

Skip obvious docstrings:

```python
# Bad
def open_warp(path: Path) -> None:
    """
    Open Warp terminal at the given path.

    Args:
        path: The path to open Warp at

    Returns:
        None
    """
    subprocess.run(["open", f"warp://action/new_window?path={path}"])

# Good
def open_warp(path: Path) -> None:
    """Open Warp terminal at path."""
    subprocess.run(["open", f"warp://action/new_window?path={path}"])
```

Give each module a `README.md` for users. Use inline comments for maintainers. Don't duplicate what's in the code.

Start features with design docs in `<branch>.md` at repo root. Delete the design doc when implementation is complete—by then, the code and its README should speak for themselves.

# Testing

Test user behavior, not implementation details. A good test proves that something users care about actually works. Most tests don't meet that bar. Delete them.

Aim for a mix:
- **Smoke tests**: Does the system run without crashing?
- **Edge case tests**: What happens at boundaries?
- **Value tests**: Does this feature do what users expect?

## When to Mock

Mock to isolate your code from things that shouldn't be part of unit tests:
- **External systems**: Network calls, databases, file systems (when testing logic, not I/O)
- **Side effects**: Sending emails, writing logs, spawning processes
- **Slow operations**: Anything that would make tests take seconds instead of milliseconds

Don't mock to verify internal wiring. If a test's assertions are just "did we call the mock with the right args?"—that's testing implementation, not behavior. The test will break when you refactor, even if the feature still works.

```python
# Bad: testing that we called the mock correctly
def test_send_notification():
    with patch("app.email.send") as mock_send:
        notify_user(user)
        mock_send.assert_called_once_with(user.email, ANY)

# Good: mock the side effect, test the behavior
def test_notify_user_returns_success():
    with patch("app.email.send"):  # prevent actual email
        result = notify_user(user)
        assert result.success

# Better: if possible, test without mocking
def test_notification_message_format():
    msg = build_notification(user)
    assert user.name in msg.body
```

If a test requires elaborate mock setup, it's usually a sign that either:
1. The code under test does too much (refactor it)
2. You're testing implementation rather than behavior (test something else)
3. This should be an integration test, not a unit test (move it)

# Pre-Commit Checklist

Before committing, verify:
- [ ] No new `Args:`/`Returns:` docstrings on functions with clear types
- [ ] No inline imports; all imports at top of file
- [ ] No `v2_`, `_old`, `_new`, `_backup` etc.; keep one implementation, use git for history
- [ ] Mocks prevent side effects, not verify internal wiring
- [ ] Tests assert on results, not mock calls
- [ ] README changes don't duplicate source code
- [ ] Existing READMEs updated if behavior changed

# Git

Commit messages are documentation. Explain what changed and why, not line-by-line what you did:

```
# Bad
Add open_warp function
Add open_cursor function
Update cli.py to call new functions
Fix import statement

# Good
lf ide: create worktrees as siblings to main repo

Run `lf ide feature-name` to create a worktree at ../feature-name
and open Warp + Cursor. Reuses existing worktree if branch exists.
```

Keep messages short—one sentence to one paragraph.
