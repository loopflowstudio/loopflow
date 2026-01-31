# Stage 4 Review: lf ops Rust Bridge

Rust git operations for `lf ops` with Python fallbacks. Removes `lfops` binary.

## What was implemented

**Rust `git.rs` expansion:**
- `get_default_branch`, `is_clean`, `stage_all`, `commit`
- `push_with_upstream`, `delete_remote_branch`, `delete_local_branch`
- `pr_exists`, `pr_create_draft`, `pr_merge_squash_auto` (shell to `gh`)
- `sync_main`, `worktree_remove`, `worktree_move`, `worktree_add`
- `rev_parse`, `is_ancestor`, `checkout`, `checkout_new_branch`, `current_branch`, `fetch`

**Rust `lf-engine` CLI:**
- All git commands exposed as JSON-over-subprocess interface
- `Version` command for `lf --version` integration

**Python `lf.ops.git` module:**
- Dual-implementation routing: Rust via `lf-engine` or Python fallback
- Config flag `internal.use_rust` controls routing
- Backend indicator printed once per session (`[git: lf-engine (rust)]`)

**`lfops` removal:**
- Console script removed from `pyproject.toml`
- Docs updated to reference `lf ops` instead

**Swift/Concerto updates:**
- `WaveService.createWave()` for HTTP wave creation
- `AreaTypeahead` component for path autocompletion
- `WaveRow` shows cron schedule and PR limit text
- `LoggingService` for structured debug output

## Key choices

| Decision | Rationale |
|----------|-----------|
| Shell to `lf-engine` vs PyO3 | JSON subprocess is simpler for ops, no wheel rebuilds |
| Shell to `gh` for GitHub ops | `gh` handles auth, caching, rate limiting well |
| Python fallback always available | Graceful degradation if Rust not installed |
| `internal.use_rust` flag | Opt-in during transition, default to Python |

## How it fits together

```
lf ops <command>
    │
    ├── git.py: _use_rust() check
    │   ├── True → shell to lf-engine <command> → JSON → result
    │   └── False → Python subprocess to git/gh
    │
    └── Returns typed result (RebaseResult, BranchInfo, etc.)
```

The Rust engine is stateless. Wave state (base_commit for squash-aware rebase) comes from the daemon or CLI args.

## Risks and bottlenecks

| Risk | Mitigation |
|------|------------|
| `lf-engine` not in PATH | Falls back to Python; prints `[git: python]` |
| JSON serialization changes | Tests cover parsing; Rust types derive `Serialize` |
| External `lfops` references | Docs updated; Concerto uses `lf ops` now |
| Config parsing failure blocks `lf --version` | Falls back to Python `loopflow.__version__` |

## What's not included

- Rust `lfd` daemon (Stage 3+)
- Summary loading in prompt assembly (TODO in `prompt.rs`)
- `lf ops wt` commands migrated to Rust (still Python)
- Agent-assisted conflict resolution (future enhancement)

## Test coverage

| Suite | Tests | Status |
|-------|-------|--------|
| Rust (`cargo test`) | 37 | Pass |
| Python (`pytest tests/`) | 673 | Pass |
| Swift (`swift test`) | 70 | Pass |

All tests pass. `cargo fmt` and `cargo clippy -- -D warnings` clean.

## Polish applied

- Fixed duplicate import in `_helpers.py` (lines 11, 21 and 16, 22 were duplicated)
