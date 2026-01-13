# Style Guide

This is the governing document for this codebase. Humans and LLMs alike are expected to follow it.

## Quick Reference

- Prefix private functions with `_`
- Return `None` for "not found"; raise exceptions for "shouldn't happen"
- No `Args:`/`Returns:` docstrings—if types are clear, skip the docstring
- Mock side effects, but don't test mock wiring

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

# Code Organization

Consistency with existing code matters more than any specific rule.

Keep information in one place. Version numbers, configuration, documentation—each piece of information should have a single source of truth. Don't duplicate versions in multiple files. If something needs to appear in multiple places, generate it or reference the source.

Put imports at the top of the file, not inline.

Keep one implementation. Avoid `v2_`, `_old`, `_new`, `_backup` prefixes and suffixes—look up old versions in git. If you're tempted to keep both old and new code around, delete the old version and commit. You can always get it back from git if needed.

Don't maintain backwards compatibility unless explicitly required. If a config format or API changes, migrate everything to the new format—don't write code that handles both old and new. Backwards compatibility is for production databases and published APIs with external users, not internal config files.

## Naming

Use verb-first names for action functions: `find_user()`, `load_config()`, `create_session()`.

Prefix private functions with underscore: `_validate()`, `_parse_line()`.

Name things after what they are, not what they're for: `Document`, `Session`, `Target`—not `DocumentHelper`, `SessionManager`, `OutputHandler`.

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

Skip obvious docstrings. If the function name and types tell the whole story, don't repeat it in prose.

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

# Git

Commit messages are documentation. Explain what changed and why, not line-by-line what you did.

Keep messages short—one sentence to one paragraph.

Do not add AI attribution footers like "Generated with Claude Code" or "Co-Authored-By: Claude" to commits. The git history should read the same whether written by a human or AI.
