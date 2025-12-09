# Loopflow Style Guide

This is the governing document of the loopflow codebase. Humans and LLMs alike are expected to follow it.

## Quick Reference

- Use `uv run` or activate `.venv` before any Python command
- Prefix private functions with `_`
- Return `None` for "not found"; raise exceptions for "shouldn't happen"
- No `Args:`/`Returns:` docstrings—if types are clear, skip the docstring
- Delete tests that require elaborate mocking
- Design docs go in `dd/`; delete them when the feature ships

## File-Type Guidelines

When editing `*.py` files:
- Put imports at the top, not inline
- Use type hints on all public functions
- One-line docstring if any; skip if the name and types are clear

When editing `*_test.py` or `test_*.py` files:
- Keep tests short and focused on one behavior
- Prefer simple assertions over elaborate mocking
- Delete flaky tests rather than adding retries

When writing CLI code with Typer:
- Prefer lowercase short flags (`-p`, `-c`), support uppercase as aliases
- Pass args through to underlying tools rather than re-implementing
- Default to sensible behavior (e.g., whole repo as context)

When editing `README.md` files:
- Start with Usage and example commands
- Don't duplicate what's in the source code
- Write for users, not maintainers

When editing `dd/*.md` design docs:
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

Put imports at the top of the file. Declare dependencies in `pyproject.toml` and assume they're available.

Keep one implementation. Avoid `v2_`, `_old`, `_new`, `_backup` prefixes and suffixes—look up old versions in git. If you're tempted to keep both old and new code around, delete the old version and commit. You can always get it back from git if needed.

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

Start projects with design docs in `dd/`. Delete the design doc when implementation is complete—by then, the code and its README should speak for themselves.

# Testing

Test user behavior, not implementation details. A good test proves that something users care about actually works. Most tests don't meet that bar. Delete them.

Aim for a mix:
- **Smoke tests**: Does the system run without crashing?
- **Edge case tests**: What happens at boundaries?
- **Value tests**: Does this feature do what users expect?

Use mocks to avoid slow or flaky dependencies. But if a test requires elaborate mocking, it's probably testing implementation rather than behavior—throw it out and write something simpler.

# Pre-Commit Checklist

Before committing, verify:
- [ ] No new `Args:`/`Returns:` docstrings on functions with clear types
- [ ] No inline imports; all imports at top of file
- [ ] No `v2_`, `_old`, `_new`, `_backup` etc.; keep one implementation, use git for history
- [ ] Tests are simple; delete any that need elaborate mocking
- [ ] Tests test user behavior, not implementation details
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
