# Ops Architecture

Design reference for the relationship between `lf ops` and `lfd`.

The `lf ops` refactor is part of Rust Stage 4 - see `roadmap/rust/04-lf-client.md`.

## Core decision

`lf ops` and `lfd` are siblings, not layers. Both call lf-core for git operations. The difference is state:

- `lf ops` — stateless, no daemon required
- `lfd` — adds wave state (base_branch, base_commit) for stacking workflows

## Architecture

```
lf-core (Rust)
├── git::rebase(worktree, onto, base_commit)
├── git::create_branch(worktree, name) → BranchInfo
├── git::push_force_with_lease(worktree)
├── git::land(worktree, strategy)
└── git::pr_create(worktree, title, body)

lf ops (Python CLI)              lfd (Rust daemon)
├── rebase → lf-core(None)       ├── rebase → lf-core(wave.base_commit)
├── next → lf-core               ├── next → lf-core + update wave state
└── land → lf-core               └── land → lf-core + update wave state
```

## Key distinction

| | `lf ops` | `lfd` |
|---|----------|-------|
| Daemon required | No | Yes |
| State | None | Wave DB (SQLite/Postgres) |
| Scope | This worktree | Named wave |
| Rebase | Simple onto main | Squash-aware via base_commit |
| Next | New branch, done | New branch + record stacking |

Siblings, not layers. Shared engine, different state models.

## Scope

- Move git operations from Python lfops to Rust lf-core
- Expose operations via FFI or CLI for Python frontend
- lfd calls lf-core directly (in-process Rust)
- lf ops becomes `lf ops` subcommand (not separate binary)

## Non-goals

- Changing git workflow semantics
- Removing Python CLI immediately
- Wave state in lf-core (stays in lfd)

## lf-core git module

```rust
// rust/lf-core/src/git.rs

pub struct BranchInfo {
    pub old_branch: String,
    pub old_head: String,
    pub new_branch: String,
}

pub fn rebase(
    worktree: &Path,
    onto: &str,
    base_commit: Option<&str>,
) -> Result<(), GitError>;

pub fn create_stacked_branch(
    worktree: &Path,
    new_branch: &str,
) -> Result<BranchInfo, GitError>;

pub fn push_force_with_lease(worktree: &Path) -> Result<(), GitError>;

pub fn land(
    worktree: &Path,
    strategy: LandStrategy,
) -> Result<LandResult, GitError>;
```

## Stacking workflow

The `next`/`rebase` pair enables stacking:

1. `lfd next` — create branch from HEAD, record base_branch + base_commit
2. Work on stacked branch, create PR
3. Base PR lands (squash-merged to main)
4. `lfd rebase` — uses base_commit for `git rebase --onto origin/main <base_commit>`

Without daemon state, `lf ops rebase` does simple `git rebase origin/main`. The squash-aware logic requires the recorded base_commit.

## Open questions

- FFI vs CLI for Python → Rust calls?
- Should `lf ops` become `lf git` or stay as `lf ops`?
- Agent-assisted conflict resolution: lf ops only, or lfd too?
