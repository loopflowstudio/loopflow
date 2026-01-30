# Rust Roadmap: lf Client Refactor (Stage 4)

Make `lf` and `lf ops` protocol clients that target lf-core.

## Goal
Keep the CLI UX but move execution to the protocol-first engine, enabling remote control and managed clusters.

## Scope
- Client config for target engine (local/remote)
- Authn for API keys and tokens
- Mapping existing commands to protocol calls
- Event streaming to terminal
- Local standalone mode without requiring `lfd`
- Local mode uses direct `lf` ↔ `lf-core` integration
- Remote mode switches `lf` engine to `lfd` that exposes the same subset of `lf-core` APIs used by `lf`
- `lf ops` commands (rebase, next, land) call lf-core git module

## lfops → lf ops transition

Consolidate `lfops` binary into `lf ops` subcommand. Orthogonal to Rust but natural to do at this stage.

**Current state:**
```bash
lf debug           # prompt/agent commands
lfops pr           # separate binary for git workflow
lfops land
lfops rebase
```

**Target state:**
```bash
lf debug           # prompt/agent commands
lf ops pr          # subcommand, same binary
lf ops land
lf ops rebase
```

**Why now:**
- One CLI to learn, one binary to install
- Consistent `lf <domain> <command>` pattern
- Natural breakpoint when moving to lf-core anyway
- Aliases can preserve `lfops` for muscle memory

**Migration:**
1. Add `lf ops` subcommand that delegates to existing lfops code
2. Deprecate `lfops` binary with warning pointing to `lf ops`
3. Remove `lfops` binary after transition period

## lf ops architecture

When `lf` moves to lf-core, `lf ops` moves with it. Both become thin Python CLIs over Rust.

```
lf-core (Rust)
├── git::rebase(worktree, onto, base_commit)
├── git::create_branch(worktree, name)
├── git::push_force_with_lease(worktree)
└── git::land(worktree, strategy)

lf (Python CLI)                  lf ops (Python CLI)
├── run → lf-core                ├── rebase → lf-core
├── flow → lf-core               ├── next → lf-core
└── ...                          └── land → lf-core
```

`lf ops` is stateless - it calls lf-core with simple defaults (e.g., `rebase(worktree, "origin/main", None)`).

`lfd` also calls lf-core but adds wave state (e.g., `rebase(worktree, "origin/main", wave.base_commit)`).

## Non-goals
- Removing Python immediately
- Rewriting all UX flows
- Changing git workflow semantics
- Wave state in lf-core (stays in lfd)

## UX principles
- `lf` behaves the same whether local or remote.
- Clear, actionable errors on auth or protocol mismatch.
- Local mode remains the default for dev.
- Users can run `lf` without installing or running `lfd`.
- Local mode should not require a daemon process.
- Remote mode should be a pure engine switch, not a UX switch.

## Success criteria
- `lf run` works identically against local and remote.
- Concerto and `lf` can connect to the same daemon.
- Users can opt into remote with a single config change.
- Hosted `lfd` can be targeted from a local `lf` without special flags.
- Local `lf` works out of the box with no daemon running.
- Remote `lf` uses the same engine API surface as local `lf` (subset parity).

---

## Ops Architecture

`lf ops` and `lfd` are siblings, not layers. Both call lf-core for git operations. The difference is state:

- `lf ops` — stateless, no daemon required
- `lfd` — adds wave state (base_branch, base_commit) for stacking workflows

### Architecture

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

### Key distinction

| | `lf ops` | `lfd` |
|---|----------|-------|
| Daemon required | No | Yes |
| State | None | Wave DB (SQLite/Postgres) |
| Scope | This worktree | Named wave |
| Rebase | Simple onto main | Squash-aware via base_commit |
| Next | New branch, done | New branch + record stacking |

Siblings, not layers. Shared engine, different state models.

### lf-core git module

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

### Stacking workflow

The `next`/`rebase` pair enables stacking:

1. `lfd next` — create branch from HEAD, record base_branch + base_commit
2. Work on stacked branch, create PR
3. Base PR lands (squash-merged to main)
4. `lfd rebase` — uses base_commit for `git rebase --onto origin/main <base_commit>`

Without daemon state, `lf ops rebase` does simple `git rebase origin/main`. The squash-aware logic requires the recorded base_commit.

---

## Open questions
- How should credentials be stored (keychain vs file)?
- Do we need offline mode with cached flows?
- FFI vs CLI for Python → Rust calls?
- Should `lf ops` become `lf git` or stay as `lf ops`?
- Agent-assisted conflict resolution: lf ops only, or lfd too?
