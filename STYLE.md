# Loopflow Style Guide

This is the governing document of the loopflow codebase. Humans and LLMs alike are expected to follow it.

## Quick Reference

**Python:**
- Use `uv run` or activate `.venv` before any Python command
- Prefix private functions with `_`
- Return `None` for "not found"; raise exceptions for "shouldn't happen"
- No `Args:`/`Returns:` docstrings—if types are clear, skip the docstring

**Rust:**
- Run `cargo fmt` and `cargo clippy` before committing
- Return `Option<T>` for "not found"; return `Result<T, E>` for failures
- Use `expect("reason")` over `unwrap()` outside tests
- Derive `Debug` on all public types

**Both:**
- Mock side effects, but don't test mock wiring or reshape production code for tests
- Design docs go under `scratch/`; `lf ops pr land` removes `scratch/*` contents
- Auto runs are headless: make best-effort assumptions and append open questions to `scratch/questions.md`

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
- Examples first, explanation after—show `lf debug -c`, then say what it does
- Action-focused tables: "What it does" not "What it is"
- Terse prose around code blocks—the code speaks
- One good example beats three similar ones
- No preamble: "Assembles context" not "Loopflow assembles context for you"
- Write for users, not maintainers
- Update when adding or changing user-facing features

When editing docs in `scratch/`:
- Focus on what's left to build, not what's done
- `lf review` writes its assessment under `scratch/`
- `lf ops pr land` removes `scratch/*` contents automatically

When editing `*.rs` files:
- Run `cargo fmt` before committing; CI enforces it
- Run `cargo clippy -- -D warnings` locally; CI treats warnings as errors
- Dead code must be deleted, not commented out (use git for history)
- If code is intentionally unused (e.g., for FFI/PyO3), use `#[allow(dead_code)]` with a comment explaining why
- Derive `Debug` on all public types; add `Clone`, `PartialEq`, `Default` where sensible
- Use `thiserror` for library error types callers need to match on
- Use `expect("why this is safe")` over `unwrap()` outside tests
- Conversion methods: `as_` (cheap/borrowed), `to_` (allocates), `into_` (consumes self)
- No `get_` prefix on getters: `fn name(&self)` not `fn get_name(&self)`
- Return `Option<T>` for "not found", `Result<T, E>` for "something went wrong"
- Newtypes for domain concepts: `struct RunId(String)` not `type RunId = String`
- Every `unsafe` block requires a `// SAFETY:` comment explaining invariants
- When a name conflicts with a keyword: use `r#type` or `type_`, not `typ`
- Use `#[non_exhaustive]` on public enums that may grow
- Never use `()` as an error type
- For public APIs, include `# Panics`/`# Errors`/`# Safety` doc sections where non-obvious

When editing Rust tests:
- `unwrap()` is fine in tests
- Use `#[test]` for unit tests in the same file
- Integration tests go in `tests/` directory
- Mock via closures or `#[cfg(test)]`, not factory traits or extra abstractions

# Development Environment

Use `uv` for Python package management. Never use pip directly.

```bash
# Python
uv sync                       # Install dependencies
uv run pytest python/tests/   # Run Python tests
uv run lf agent --help        # Run commands

# Or activate the venv
source .venv/bin/activate
pytest python/tests/

# Rust
cargo build                   # Build all crates
cargo test                    # Run all Rust tests
cargo fmt                     # Format code
cargo clippy -- -D warnings   # Lint (warnings = errors)
```

See TESTING.md for the full test suite (Python, Swift, Rust, Concerto UI). CI runs all.

# Code Organization

Follow PEP8. Consistency with existing code matters more than any specific rule.

Keep `__init__.py` files empty. They exist only to mark directories as packages. Don't use them for re-exports—import from the actual module (`from loopflow.lfd.runs.loop import create_loop`) not from package-level re-exports (`from loopflow.lfd.runs import create_loop`). A docstring describing the package contents is fine.

Keep information in one place. Version numbers, configuration, documentation—each piece of information should have a single source of truth. Don't duplicate versions in `__init__.py` and `pyproject.toml`. Don't copy FAQs into multiple READMEs. If something needs to appear in multiple places, generate it or reference the source.

Put imports at the top of the file. Declare dependencies in `pyproject.toml` and assume they're available.

Prefer explicit imports over magic. Don't inject names into module namespaces to save one import line—it breaks linters, IDE autocomplete, and confuses readers. If a module needs `Flow`, it should `from loopflow.lf.flows import Flow`. Standard Python beats clever patterns that fight tooling.

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

## User-Facing Documentation

User docs follow the same principles as prompts (see PROMPT_STYLE.md):

**Direct and imperative.** State what something does, not what it is. "Runs a prompt with assembled context" beats "A step is a markdown file containing instructions."

**Examples carry the weight.** Code blocks are the primary content. Prose exists to connect them. If you can cut a paragraph and the examples still make sense, cut it.

**Tables for reference, not education.** Tables work for quick lookup once you understand the concepts. Lead with examples that teach.

**No throat-clearing.** Cut "In order to...", "You can use...", "This allows you to...". Just show it.

```markdown
# Bad
In order to run a step with clipboard content, you can use the -c flag.
This allows you to paste an error and have the agent fix it.

# Good
lf debug -c    # paste an error, watch it fix
```

# Testing

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

**Never reshape production code for tests.** If you're adding a factory trait, an interface, a constructor overload, or an extra parameter solely because tests need it, stop. The production code's shape should be dictated by production needs. Use closures, conditional compilation (`#[cfg(test)]`), or test-only modules — not abstractions that exist to satisfy test doubles.

**No factory patterns.** Factory traits, abstract factories, and provider registries are almost always over-engineering. A function or a closure does the same job without the ceremony. If you need runtime dispatch, use an enum or a function pointer — not a trait with one method and one real implementation.

# Pre-Commit Checklist

Before committing, verify:

**Python:**
- [ ] No new `Args:`/`Returns:` docstrings on functions with clear types
- [ ] No inline imports; all imports at top of file

**Rust:**
- [ ] `cargo fmt` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Public types derive `Debug`
- [ ] No `unwrap()` outside tests; use `expect("reason")`

**Both:**
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

Do not add AI attribution footers like "Generated with Claude Code" or "Co-Authored-By: Claude" to commits. The git history should read the same whether written by a human or AI.
