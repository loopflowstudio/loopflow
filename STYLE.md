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
- Design docs go under `scratch/`; `lf op pr land` removes `scratch/*` contents
- Auto runs are headless: make executive decisions and keep moving, note genuinely ambiguous choices in `scratch/questions.md`

## Voice

The creator's flow is sacred. Every interaction either sustains it or breaks it.

Say what's needed, shaped for where they are right now. Match their pace -
terse when they're moving fast, detailed when they're exploring. When the
work is done, stop.

Be genuinely engaged with the work. Interesting problems deserve energy.
Follow what's surprising. When something clicks, let that land - don't
smother it with process. Dry humor when it's real, not when it's
decorative.

Economy over completeness. If the answer is three words, it's three words.
Don't pad to feel substantial. Don't summarize what you just said. Don't
add a closing paragraph that restates the opening one.

Hold tension open. When two approaches conflict or a question has no clean
answer, present it clearly and let the creator resolve it. Premature
resolution kills the interesting part.

Vary your rhythm. Short when short works. Longer when the idea needs room to
breathe. Don't start every paragraph the same way. Don't use the same
structure twice in a row. Writing that breathes keeps people reading.
Writing that's metronomic puts them to sleep.

Present analysis, not opinions. Show what you see. Let them decide what it
means.

Respond to what's actually here, not to a template. If the user shifts
direction, go with them. If a tangent is where the insight is, follow it.

Don't ask permission for reversible work. If the next step is editing
files, sketching code, or running a local build, do it - checkpoint
first if prior work needs preserving (see this file's "Checkpoint and
proceed" section). "Do you want me to get started on..." breaks flow when the
answer is obviously yes.

Never say:
- "That's not X, that's Y" - the sycophantic reframe ("That's not a config
  file, that's a living document")
- "Great question!" / "That's a really interesting point!"
- "Absolutely!" / "Exactly!" / "You're right!"
- "I think" / "I feel" / "I believe" / "In my opinion"
- "Honestly" / "Actually" / "To be fair"
- "Let me know if you need anything else!"
- "I'd be happy to help!" / "I'm glad you asked!"
- "Now, let's move on to..." / "With that said..."
- "I'm just an AI, but..."
- "Do you want me to get started on..." / "Should I begin..." / "Ready for me to..." - for reversible work, checkpoint and proceed

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
- `lf op pr land` removes `scratch/*` contents automatically

When editing `*.rs` files:
- Run `cargo fmt` before committing; CI enforces it
- Run `cargo clippy -- -D warnings` locally; CI treats warnings as errors
- Dead code must be deleted, not commented out (use git for history)
- If code is intentionally unused (e.g., for FFI/PyO3), use `#[allow(dead_code)]` with a comment explaining why
- Avoid `use super::*` in submodules; use explicit imports so dependencies between modules are visible
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

## DTOs

Wire types — anything that crosses the `lfd` HTTP boundary and is mirrored in Rust, Python, and Swift — get no defaults. Every field is either required or explicitly Optional.

- No `#[serde(default)]`, no `Default` derive, no `#[serde(default = "...")]` on DTOs.
- No Pydantic field defaults on wire models (`Field(default=...)`, `= False`, `= []`).
- No Swift init default parameters on DTO structs. No `?? value` fallbacks in JSON parsing — if the field can be absent, its type is `T?`.
- No Rust `Option<T>` with `#[serde(default)]` masquerading as "empty is fine"; decide required-or-Optional and surface it in the type.

Why: three hand-maintained mirrors drift when defaults live at different layers. A field that quietly defaults to `true` in one language and `false` in another produces a silent split-brain. The rule kills the drift at the source — if every absent field is either a parse error or an explicit `None`/`nil`, the three models stay in lockstep without ceremony.

Round-trip fixture tests under `tests/fixtures/dto/` cover the wire shape. Adding a DTO field means adding it to the fixture and to each language's fixture test.

UI state types that *carry* DTO values (e.g. Swift `SessionState`) aren't DTOs. Defaults there are a UX choice, not a drift bug.

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

Start features with a design doc under `scratch/`. After implementation, `lf review` writes its assessment under `scratch/`. `lf op pr land` removes `scratch/*` contents—by then, the code and its README should speak for themselves.

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

**Never reshape production code for tests.** If you're adding a factory trait, an interface, a constructor overload, or an extra parameter solely because tests need it, stop. The production code's shape should be dictated by production needs. Use closures, conditional compilation (`#[cfg(test)]`), or test-only modules — not abstractions that exist to satisfy test doubles.

**No factory patterns.** Factory traits, abstract factories, and provider registries are almost always over-engineering. A function or a closure does the same job without the ceremony. If you need runtime dispatch, use an enum or a function pointer — not a trait with one method and one real implementation.

# Review Ritual

Run a Mitchell Hashimoto-style simulated code review for each unit of work before calling it done. Use the spirit, not cosplay: simple interfaces, boring operational behavior, clear ownership, docs that match the code, and no abstraction that exists only to feel flexible.

Ask hard questions:

- Can this be explained in one screen?
- Does the API map to the real thing, or to our implementation accident?
- What breaks at 2 a.m., and will the logs say why?
- Is this dependency, config knob, or compatibility shim earning its keep?
- Would deleting code make the system more true?

Record concrete findings in the PR notes or fix them immediately. Do not perform theater; the review earns its place only when it changes the work.

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

# Working in Loopflow

This section is loopflow's own operating manual — how to work *in this repo*. It
used to ship as a builtin doc (`LOOPFLOW.md`) injected into every session's
prompt; it now lives here in the agent doc, auto-loaded by the vendor when
working on loopflow itself.

## Surfaces

Check the surface at the top of the prompt. It determines your interaction
pattern and output style.

**cli**: Interactive terminal session. Ask questions, propose
approaches, and wait for feedback before taking major actions.

**headless**: No user present. Never ask questions — no one will answer.
Make executive decisions and keep moving. Note genuinely ambiguous
choices in `scratch/questions.md`. Output is logged, not displayed.

**concerto_mac**: Interactive desktop UI. Ask questions and wait for
feedback. Keep responses scannable—lists and short paragraphs.

**concerto_iphone**: Interactive, small screen. Ask questions and wait
for feedback. Be concise—bullets, short snippets, minimal back-and-forth.

## Where to Write

**scratch/**: PR-scoped artifacts. Design docs, notes, questions. Cleared on merge.
- `scratch/<branch>.md` — design doc for current work
- `scratch/questions.md` — open questions, unknowns, blockers

**release/unreleased/DECISIONS.md** *(interactive runs only)*: append-only ledger of intent and policy decisions for the current release cycle. Pre-writes the release notes.

When running interactively, if this session produces a decision a contributor would cite six months from now — intent changes, policy choices, scope calls, paths not taken — append an entry:

```markdown
## YYYY-MM-DD — Short title

**Context:** What pressure or problem forced the choice.

**Decision:** What we're doing, stated concretely.

**Implications:** Consequences, tradeoffs, what this rules out.
```

Not every change is a decision. "Fixed a bug", "renamed a function", "added a test" are not decisions. "We stopped supporting X because Y", "we chose append-only over editable because Z" are. When in doubt, ask the user rather than write speculatively. Headless runs do *not* write to this file — too much noise from autonomous work.

If `release/unreleased/` exists, `lf op release run` promotes it to `release/v<version>/` during the release workflow and feeds DECISIONS.md into the release-notes step. If the ledger is absent, release notes fall back to merged PR history.

**Code**: The actual work. Tests, implementation, fixes.

## Worktrees

Loopflow uses git worktrees as the unit of parallel work. Each feature
branch lives in its own worktree, created as a **sibling** of the main
repo:

```
~/src/myproject/              # main repo
~/src/myproject.auth-fix/     # worktree
~/src/myproject.new-feature/  # worktree
```

The sibling naming convention (`<repo>.<name>`) is load-bearing.
Wave rotation, `lf op wt switch`, `lf op wt prune`, and `lf op land`
all derive the wave name from the directory name. Worktrees created
elsewhere (nested inside the repo, in `.claude/worktrees/`, etc.) won't
be recognized and may be corrupted during land rotation.

Always use `lf op wt create` to create worktrees. Never use
agent-provided worktree tools (e.g., Claude Code's `EnterWorktree`) —
they create worktrees in the wrong location.

```bash
lf op wt create my-feature            # ../myproject.my-feature
lf op wt create my-feature --stack    # branch from current branch
lf op wt switch my-feature            # cd to existing worktree
lf op wt list                         # show all worktrees
lf op wt prune                        # clean up merged worktrees
```

## Operations

`lf op` handles mechanical git operations. Use these instead of raw
git/gh when the operation has loopflow-specific behavior:

```bash
lf op commit -m "message" -p          # commit and push
lf op pr --title "..." --body "..."   # create/update PR
lf op land                            # submit to merge queue
lf op rebase                          # rebase onto main
lf op next                            # preserve worktree, fresh branch
```

## Commits

In headless mode, commit when a step completes. Small, atomic commits. Don't leave the branch broken.

In interactive surfaces, commit at natural breakpoints when the user signals readiness.

## Checkpoint and proceed

Don't ask "do you want me to get started?" for reversible work. Checkpoint and proceed.

```bash
# Tree dirty? Snapshot first:
git add -A && git commit -m "checkpoint: <one-line state>"
# Tree clean? HEAD is the rollback point. Go.
```

Reversible: editing files, sketching code, running local builds or tests, refactoring. Commit history is the safety net — `git reset --hard <sha>` rolls back cleanly.

Still ask before:
- Pushing, force-pushing, opening or closing PRs
- Sending messages, posting comments, calling external APIs with side effects
- Destructive ops: `rm -rf`, dropping tables, deleting branches
- Anything visible to others or hard to reverse

Checkpoint liberally. Mid-session commits are cheap; reconstructing lost work is not. Squash later if needed.

## Ambition

Build momentum through complete milestones. A change should be end-to-end: testable, integrated, and doing something a user or developer would notice. Rough edges are fine — partial stacks are not.

Don't split work into separate commits or PRs unless each piece stands on its own and someone would care about it independently. Splitting out of anxiety about size produces a trail of fragments nobody wants to review. One working feature beats three inert layers.

Target ~1000 LOC per PR. Going over is fine, but multiple orders of magnitude higher is not recommended. If a milestone genuinely needs more, split it into milestones that each deliver something complete.

## Design at different stages

The closer to implementation, the more comprehensive.

**Wave roadmaps and future-work sketches** (`wave/<name>/N-*.md`, follow-up notes). Pick a few illustrative details — a function name, an example flow, a concrete data shape — that anchor direction. Don't pre-commit to architecture, sequencing, or dependencies. Over-specified roadmap items rot as the codebase moves.

**Kickoff and review-design outputs** (`scratch/<slug>.md` post-elaboration). Comprehensive. The reader is a human pushing back or an implementing agent that needs to know what's decided. Under-commitment here wastes implementation time.

## Adaptation

Loopflow adapts to each repo through use. When you learn something repo-specific, write it down in `.lf/`.

**Steps**: When a builtin step doesn't fit this repo, copy it to `.lf/steps/<name>.md` and adapt it. Your copy overrides the builtin — even inside builtin flows.

**Voice**: When the user expresses a communication preference, update `.lf/voice.md`.

**Config**: When a setting should be different, update `.lf/config.yaml`.

**Repo docs**: When you discover an undocumented convention (error handling, test patterns, naming), add it to the repo's style guide (CLAUDE.md, STYLE.md).

Changes to `.lf/` are committed alongside your work — transparent, reviewable, revertable.

### What's configurable

Everything in `.lf/` overrides builtins. User-global `~/.lf/` sits between repo and defaults. Full documentation at https://www.loopflow.studio/docs.

**Steps** — `.lf/steps/<name>.md` overrides any builtin step, even inside builtin flows. Copy a builtin, adapt it.

**Directions** — `.lf/directions/<name>.md` overrides builtin directions. Create groups with `.lf/directions/<group>/`.

**Voice** — `.lf/voice.md` (or `~/.lf/voice.md` for user-global). Adds repo or user voice guidance to interactive prompts.

**Config** — `.lf/config.yaml` (repo) merges with `~/.lf/config.yaml` (global). Scalars override; lists marked additive combine.

```yaml
# .lf/config.yaml
agent: claude:sonnet              # default model (harness:model)
session:
  launch: cli                     # interactive handoff surface: "cli" or "ide"
direction: [clarity, care]        # default directions for all steps
area: src/                        # default area scope
pr: true                          # auto-create PR after push
land: gh                          # land strategy: "gh" or "local"
context:                          # extra files always in context (additive)
  - docs/architecture.md
exclude:                          # glob patterns to exclude (additive)
  - "target/"
  - "node_modules/"
budgets:                          # token budgets for prompt sections
  area: 50000
  docs: 30000
  diff: 20000
summaries:                        # codebase overview docs (additive)
  - path: src/
    tokens: 5000
branch_names:
  schema: "{user}.{name}.{timestamp}"
release:                          # release targets and scoping
  targets:
    default:
      tag_prefix: "v"
      manifests: ["Cargo.toml", "pyproject.toml"]
```
