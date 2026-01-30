# lf Client Refactor (Stage 4)

One CLI, one engine. Move git operations to Rust lf-core and consolidate `lfops` into `lf ops`.

## Problem

Three problems converging:

1. **Two binaries to install**: `lf` for prompts, `lfops` for git workflow. Users learn two tools, muscle memory splits.

2. **Python git operations in a Rust world**: Stages 2-3 establish lf-core as the engine. Git operations remain in Python, creating a split-brain architecture.

3. **Overlapping commands**: `lfops rebase` and `lfd rebase` do similar things differently. `lfops next` and `lfd next` too. Code duplicates, behaviors drift.

## Approach

### Single CLI with subcommand

Consolidate `lfops` into `lf ops`:

```bash
# Before                          # After
lfops pr                          lf ops pr
lfops land                        lf ops land
lfops rebase                      lf ops rebase
lfops wt create auth              lf ops wt create auth
```

`lf` becomes the only user-facing binary. `lfd` remains the daemon (background service, not daily-driver CLI).

### Git module in lf-core

Move git operations from Python to Rust:

```rust
// rust/lf-core/src/git.rs (expanded from existing status/diff)

pub fn rebase(worktree: &Path, onto: &str, base_commit: Option<&str>) -> Result<RebaseResult, GitError>;
pub fn create_branch(worktree: &Path, name: &str) -> Result<BranchInfo, GitError>;
pub fn push(worktree: &Path, force_with_lease: bool) -> Result<(), GitError>;
pub fn land(worktree: &Path, strategy: LandStrategy) -> Result<LandResult, GitError>;
```

Python `lf ops` calls Rust via subprocess (initially) or FFI (later). Same pattern as daemon → engine.

### Siblings, not layers

`lf ops` and `lfd` both call lf-core. The difference is state:

| | `lf ops` | `lfd` |
|---|----------|-------|
| Daemon required | No | Yes |
| State | None | Wave DB |
| Scope | This worktree | Named wave |
| Rebase | Simple onto main | Squash-aware via base_commit |
| Next | New branch, done | New branch + record stacking |

Both use the same underlying git operations. `lf ops` passes `None` for base_commit. `lfd` passes the recorded base_commit from wave state.

This isn't duplication—it's the same operation with different context.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep lfops separate | Two tools to learn, two to install | Fragments the UX; users already confused |
| Move ops to lfd | Couples ops to daemon lifecycle | Want `lf ops pr` to work without daemon running |
| Python-only (no Rust git) | Simpler migration | Perpetuates split-brain; misses Stage 4 goal |
| Full Rust CLI | Maximum consistency | Too much migration at once; Python frontend is fine |

## Key decisions

### 1. Subprocess before FFI

Call lf-core via CLI subprocess initially:

```bash
lf-core rebase --worktree /path --onto origin/main
```

FFI (PyO3) can come later. Subprocess is simpler, debuggable, and sufficient for git operations that already shell out to `git` anyway.

**Wave principle followed**: "Protocol first"—the CLI interface *is* the protocol for this stage.

### 2. No command aliasing at wave level

`lfd rebase` and `lf ops rebase` are different commands that happen to share implementation:

- `lfd rebase` requires a wave name, uses wave state
- `lf ops rebase` works in current worktree, stateless

Don't try to make them interchangeable. Different UX, same engine.

### 3. Gradual migration with deprecation

```
Phase 1: Add `lf ops` subcommand, delegates to existing lfops code
Phase 2: lfops binary prints deprecation warning
Phase 3: Move git operations to Rust, Python calls subprocess
Phase 4: Remove lfops binary
```

Each phase ships independently. Users get 2-3 months of deprecation warnings.

### 4. gh stays in Python

The `gh` CLI integration (PR creation, merge queue) stays in Python. It's already subprocess calls. No value in Rust-ifying `gh api` wrappers.

What moves to Rust: `git rebase`, `git push`, branch management, worktree operations.

**Wave principle followed**: "UX invariants"—PR workflows feel identical, just faster.

## Scope

### In scope

- `lf ops` subcommand with all current lfops commands
- Rust git module: rebase, push, branch operations
- Subprocess interface for Python → Rust calls
- Deprecation path for `lfops` binary
- `lfd` updated to call lf-core for git operations

### Out of scope

- FFI bindings (future optimization)
- `gh` CLI integration (stays Python)
- Remote `lf` mode (separate design—covered by local/remote engine switching)
- Wave state management changes (stays in lfd)
- Agent-assisted conflict resolution (future enhancement)

## Implementation sketch

### lf-core git module

Expand existing `git.rs`:

```rust
// rust/lf-core/src/git.rs

#[derive(Debug, Clone, PartialEq)]
pub struct RebaseResult {
    pub success: bool,
    pub conflicts: Option<Vec<PathBuf>>,
    pub new_head: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BranchInfo {
    pub old_branch: String,
    pub old_head: String,
    pub new_branch: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LandStrategy {
    SquashMerge,
    LocalMerge,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LandResult {
    pub merged_commit: String,
    pub branch_deleted: bool,
}

pub fn rebase(
    worktree: &Path,
    onto: &str,
    base_commit: Option<&str>,
) -> Result<RebaseResult, GitError> {
    // If base_commit provided: git rebase --onto <onto> <base_commit>
    // Otherwise: git rebase <onto>
    // On conflict: abort and return conflicts list
}

pub fn create_branch(worktree: &Path, name: &str) -> Result<BranchInfo, GitError> {
    // Record current branch/head
    // git checkout -b <name>
    // Return old state for undo/stacking
}

pub fn push(worktree: &Path, force_with_lease: bool) -> Result<(), GitError> {
    // git push (with --force-with-lease if requested)
    // Handle non-fast-forward gracefully
}

pub fn land(
    worktree: &Path,
    strategy: LandStrategy,
    main_branch: &str,
) -> Result<LandResult, GitError> {
    // SquashMerge: reset --soft, commit, checkout main, merge
    // LocalMerge: checkout main, merge --no-ff
}
```

### CLI binary

New binary in `rust/lf-core/src/bin/lf-core.rs`:

```rust
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lf-core")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Rebase {
        #[arg(long)]
        worktree: PathBuf,
        #[arg(long)]
        onto: String,
        #[arg(long)]
        base_commit: Option<String>,
    },
    Push {
        #[arg(long)]
        worktree: PathBuf,
        #[arg(long)]
        force_with_lease: bool,
    },
    Branch {
        #[arg(long)]
        worktree: PathBuf,
        #[arg(long)]
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Commands::Rebase { worktree, onto, base_commit } => {
            lf_core::git::rebase(&worktree, &onto, base_commit.as_deref())
                .map(|r| serde_json::to_string(&r).unwrap())
        }
        // ... etc
    };
    match result {
        Ok(json) => println!("{}", json),
        Err(e) => {
            eprintln!("{}", serde_json::to_string(&e).unwrap());
            std::process::exit(1);
        }
    }
}
```

### Python integration

```python
# src/loopflow/lf/ops/git.py

import subprocess
import json
from pathlib import Path
from dataclasses import dataclass

@dataclass
class RebaseResult:
    success: bool
    conflicts: list[Path] | None
    new_head: str | None

def rebase(worktree: Path, onto: str, base_commit: str | None = None) -> RebaseResult:
    args = ["lf-core", "rebase", "--worktree", str(worktree), "--onto", onto]
    if base_commit:
        args.extend(["--base-commit", base_commit])
    result = subprocess.run(args, capture_output=True, text=True)
    if result.returncode != 0:
        raise GitError(json.loads(result.stderr))
    data = json.loads(result.stdout)
    return RebaseResult(**data)
```

### lf ops subcommand

```python
# src/loopflow/lf/cli.py

ops_app = typer.Typer(help="Git workflow operations")
app.add_typer(ops_app, name="ops")

# Import and register existing lfops commands
from loopflow.lfops import pr, land, rebase, next_, wt, commit, abandon, sync

pr.register_commands(ops_app)
land.register_commands(ops_app)
rebase.register_commands(ops_app)
# ... etc
```

### lfops deprecation

```python
# src/loopflow/lfops/__init__.py

def main():
    import sys
    import typer

    # Print deprecation warning to stderr
    typer.echo(
        "Warning: 'lfops' is deprecated. Use 'lf ops' instead.\n"
        f"  Example: lf ops {' '.join(sys.argv[1:])}\n",
        err=True
    )

    # Still run the command
    from loopflow.lfops.commands import app
    app()
```

## Done when

```bash
# lf ops works for all current lfops commands
lf ops pr
lf ops land
lf ops rebase
lf ops wt create feature

# lfops still works but prints deprecation
lfops pr  # "Warning: lfops is deprecated. Use 'lf ops pr' instead."

# Rust git operations pass tests
cargo test --package lf-core git_rebase
cargo test --package lf-core git_push
cargo test --package lf-core git_create_branch

# Python calls Rust subprocess
lf ops rebase  # internally: subprocess.run(["lf-core", "rebase", ...])

# lfd uses shared git module
lfd rebase my-wave  # internally: lf_core::git::rebase with wave.base_commit

# Parity test suite
pytest tests/test_ops_parity.py  # Same operations via old and new paths
```

## Open questions

Resolved during design:

- ~~FFI vs CLI for Python → Rust calls?~~ **CLI subprocess first.** Simpler, sufficient, can optimize later.
- ~~Should `lf ops` become `lf git`?~~ **No, keep `ops`.** "Git" is implementation detail; "ops" is the user mental model.

Remaining:

- **Credential handling for remote lf mode**: Keychain vs config file? Defer to remote mode design.
- **Agent-assisted conflict resolution**: `lf ops rebase --assist` launches agent on conflict. Implementation TBD.
- **Worktree command promotion**: Keep `lf ops wt` or eventually `lf wt`? Monitor usage.
