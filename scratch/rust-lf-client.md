# lf Client Refactor (Stage 4)

Move git operations from Python lf ops to Rust lf-core. Python CLI becomes a thin wrapper over Rust.

## Problem

The Python lf ops code duplicates logic that will exist in the Rust daemon. Both need rebasing, branch creation, and landing. Today:

- `lfops` is a separate binary from `lf`
- Git operations are scattered across `_helpers.py`, `land.py`, `next.py`, `rebase.py`
- No shared code with the Rust daemon

This creates maintenance burden and blocks the Rust daemon from using the same git logic for stacking workflows.

## Approach

Extend `rust/loopflow-engine/src/git.rs` with the operations needed by `lf ops`. Expose via PyO3 bindings. Python commands become thin wrappers that call Rust.

### Git module expansion

The current `git.rs` has:
- `rebase(worktree, onto, base_commit)` — already squash-aware
- `create_branch(worktree, name)` — records old state
- `push(worktree, force_with_lease)` — basic push
- `land(worktree, strategy, main_branch)` — local merge only

Add:
- `get_default_branch(repo)` — detect main/master via origin/HEAD
- `is_clean(repo)` — check git status --porcelain
- `stage_all(repo)` — git add -A
- `commit(repo, message)` — git commit -m
- `push_with_upstream(repo, remote, branch)` — push -u (already exists)
- `delete_remote_branch(repo, remote, branch)` — push origin --delete
- `delete_local_branch(repo, branch)` — branch -D
- `pr_exists(repo)` — check if PR exists for current branch (shell to gh)
- `pr_create_draft(repo)` — create draft PR (shell to gh)
- `pr_merge_squash_auto(repo)` — enable auto-merge (shell to gh)
- `sync_main(repo, main_branch)` — fetch + reset if checked out
- `worktree_remove(repo, path)` — git worktree remove --force

### PyO3 bindings

Expose new functions in `python.rs`:

```python
from loopflow_engine import git

# Current
git.rebase("/path/to/worktree", "origin/main", None)
git.create_branch("/path/to/worktree", "feature")
git.push("/path/to/worktree", force_with_lease=True)
git.land("/path/to/worktree", "squash_merge", "main")

# New
git.get_default_branch("/path/to/repo")  # -> "main"
git.is_clean("/path/to/repo")            # -> bool
git.stage_all("/path/to/repo")
git.commit("/path/to/repo", "message")
git.delete_remote_branch("/path/to/repo", "origin", "feature")
git.sync_main("/path/to/repo", "main")   # fetch + maybe reset
```

### Python migration path

Each lf ops command calls Rust for git operations:

| Command | Python keeps | Rust handles |
|---------|-------------|--------------|
| `pr` | CLI output, gh calls | stage, commit, push, is_clean |
| `land` | strategy selection, scratch/ cleanup | rebase, push, land, delete branch |
| `next` | branch naming, wave metadata | is_ancestor, checkout, create_branch, push |
| `rebase` | conflict UX, agent fallback | rebase, push |
| `commit` | message generation (via agent) | stage, commit, push |

The Python layer handles:
- User-facing output (typer.echo)
- Agent invocation for message generation
- GitHub CLI calls (gh) — these stay in Python for now
- Wave metadata updates (daemon integration)

### lfops removal

Use `lf ops` only:

```bash
lf ops pr
lf ops land
lf ops rebase
```

Migration:
1. Add `lf ops` subcommand
2. Remove `lfops` binary (no backwards compatibility)

### gh CLI integration

GitHub operations stay as subprocess calls to `gh`:
- `gh pr view` — check PR status
- `gh pr create` — create PR
- `gh pr merge --squash --auto` — enable auto-merge

Wrapping `gh` in Rust adds complexity without benefit. The `gh` CLI handles auth, caching, and rate limiting. Keep it.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Rewrite lfops in pure Rust | Single language, no FFI | Python still needed for agent invocation; not worth the migration cost |
| Keep Python git operations | No Rust dependency | Daemon needs same operations in Rust anyway; blocks stacking workflow |
| Use git2-rs instead of shelling out | No subprocess overhead | Adds 500KB to binary; git CLI is fast enough; git2 API is complex |
| Wrap gh in Rust too | Single implementation | gh handles auth well; no benefit to reimplementing |

## Key decisions

**Shell to git, not git2-rs.** The git CLI is stable, fast, and handles edge cases we'd have to reimplement. git2-rs adds binary size and complexity for no user benefit. Per the roadmap principle: "Shell to lf" for execution, we extend this to "shell to git" for git operations.

**Coherent Rust lf ops API.** The Rust layer defines the lf ops surface. It may shell out to `gh` directly as needed to keep the API cohesive. Shelling out to Python is a fallback, not the default.

**Stateless engine, stateful daemon.** `loopflow-engine` stays stateless; `lfd` supplies wave state (e.g., base_commit for squash-aware rebase). Both `lf ops` and `lfd` call the same Rust functions with explicit inputs.

**Rust lf ops first.** After this change, `lf ops` should default to the Rust implementation (gated by `internal.rust` config). Rust `lfd` is not required yet, but the API should be shaped for eventual daemon reuse.

**Thin Python wrapper.** Python lf ops becomes a 50-line wrapper per command. All git logic lives in Rust. This makes the Python code obvious and the Rust code testable.

## Scope

- In scope: Git operations in Rust, PyO3 bindings, lf ops wiring, lfops removal
- Directional: move the entire `lf` CLI toward Rust over time; this change should keep that path open (ops first, other commands later).
- Non-zero in this diff: add a Rust entrypoint used by `lf` for at least one non-ops surface (e.g., `lf --version`/`lf info`), gated by `internal.rust`. This establishes a concrete end-to-end path beyond ops without pulling full CLI behavior into Rust yet.
- Out of scope: gh wrapper in Rust, agent message generation in Rust, daemon integration

## Done when

```bash
# All tests pass
cargo test -p loopflow-engine

# Python tests pass
uv run pytest tests/test_git.py

# Commands work
lf ops pr       # stages, commits, pushes, creates PR
lf ops land     # rebases, lands via PR or local merge
lf ops next     # lands current, creates stacked branch
lf ops rebase   # rebases with squash-aware logic

# Rust CLI path beyond ops
lf --version    # served by Rust when internal.rust is enabled

# Deprecation works
# No lfops binary
```

## Implementation order

1. Extend `git.rs` with missing functions (is_clean, stage_all, commit, delete branches, sync_main)
2. Add PyO3 bindings in `python.rs`
3. Migrate `_helpers.py` to use Rust functions
4. Migrate `rebase.py` (simplest command)
5. Migrate `land.py` (most complex)
6. Migrate `next.py` and `commit.py`
7. Add `lf ops` subcommand
8. Remove `lfops` binary
