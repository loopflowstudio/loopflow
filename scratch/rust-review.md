# rust review

## What was implemented

Consolidated `lfops` into `lf ops` subcommand and moved git operations to Rust lf-core. Stage 4 of the Rust roadmap (lf client refactor).

**CLI consolidation:**
- Added `lf ops` subcommand exposing all `lfops` commands (pr, land, rebase, wt, commit, etc.)
- `lfops` binary prints deprecation warning pointing users to `lf ops`
- Updated all user-facing messages to reference `lf ops`

**Rust git module:**
- Extended `rust/lf-core/src/git.rs` with rebase, create_branch, push, and land operations
- Added `lf-core` CLI binary with clap for subprocess interface
- Outputs JSON for structured Python integration
- Tests for branch creation, linear rebase, and push with force-with-lease

**Python integration:**
- Added `src/loopflow/lf/ops/git.py` as subprocess wrapper for lf-core
- Updated `lfops rebase` to call Rust implementation

**Also added:**
- `AGENTS.md` for Codex/Gemini CLI compatibility (same content as CLAUDE.md)
- `scratch/questions.md` with open questions for later phases

## Key choices

1. **Subprocess before FFI.** Python calls lf-core via CLI rather than PyO3. Simpler to debug, sufficient for git operations.

2. **Siblings not layers.** `lf ops` and `lfd` both call lf-core. `lf ops` is stateless; `lfd` tracks wave state for stacking.

3. **Gradual migration.** `lfops` still works with deprecation warning. Users get transition period.

4. **gh stays in Python.** Only git operations moved to Rust. `gh` CLI integration remains Python.

## How it fits together

```
lf ops rebase
    → loopflow/lfops/rebase.py
        → loopflow/lf/ops/git.py
            → subprocess("lf-core", "rebase", ...)
                → rust/lf-core/src/git.rs::rebase()
                    → git rebase origin/main
```

## Risks and bottlenecks

- **lf-core must be on PATH.** GitError raised with user-friendly message if missing.
- **Conflict handling.** Rebase aborts on conflict and returns conflict list. Agent-assisted resolution deferred.
- **Only rebase integrated.** Other `lf ops` commands still use Python git directly (pr, land, commit).

## What's not included

- FFI bindings (future optimization)
- `lfd` calling lf-core for git operations (separate phase)
- Agent-assisted conflict resolution
- Integration of other git commands beyond rebase
