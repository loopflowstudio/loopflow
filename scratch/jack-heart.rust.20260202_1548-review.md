# Review: Rust lf ops Worktree Parity

## What was implemented

Added Rust implementations of `lf ops wt` worktree commands and shell integration to match Python functionality. Key additions:

**New modules in loopflow-engine:**
- `naming.rs` - Branch name generation with configurable schemas (`{user}.{name}.{ts}.{words}`)
- `worktrees.rs` - Worktree operations: create, list, preserve, path resolution

**New CLI commands in Rust lf:**
- `lf ops wt create <name>` - Create worktree with schema-based branch naming
- `lf ops wt switch <name>` - Switch to existing worktree
- `lf ops wt list [--format json]` - List worktrees with merge status
- `lf ops wt prune [--force]` - Remove merged worktrees
- `lf ops wt ci [--watch] [--logs]` - Check PR CI status
- `lf ops shell init [zsh|bash]` - Output shell integration script
- `lf ops shell install` - Install shell integration to rc file

**Python changes:**
- Simplified `lf ops next` to use wave metadata as source of truth
- Fixed worktree path layout to use `repo.name` format (matching Python)

## Key choices

1. **Worktree path format**: Changed from `repo-name` to `repo.name` to match Python implementation. Paths are siblings of main repo: `../loopflow.rust.feature-name`.

2. **Shell integration via directive file**: Uses `LOOPFLOW_DIRECTIVE_FILE` environment variable to communicate `cd` commands to wrapper function. Same pattern as Python.

3. **No wave integration yet**: Rust `lf ops next` doesn't interact with wave metadata. Python version does. Deferred per questions.md.

4. **Branch collision detection**: `create_with_schema` checks both worktree branches and orphan branches to prevent conflicts.

## How it fits together

```
User runs: lf ops wt create feature-x

1. find_repo_root() → current working dir's git root
2. main_repo_root() → find main repo (not worktree)
3. load_config() → get branch_names.schema
4. format_branch_name() → "jack-heart.feature-x.20260202_1548"
5. worktree_add() → git worktree add
6. write_shell_directive() → "cd /path/to/worktree"
7. Shell wrapper sources directive file
```

## Risks and bottlenecks

1. **Rust/Python parity drift**: Two implementations of naming and worktree logic. Word lists are synced, schemas match, but subtle differences possible.

2. **No tests for shell integration**: Shell wrapper functions aren't tested. Behavior verified manually.

3. **`lf ops next` incomplete in Rust**: Uses timestamp-based branch names (`next-{ts}`) instead of wave-based names. Python version is the production path.

## What's not included

1. **Wave integration in Rust** - `lf ops next` doesn't update wave metadata
2. **`--sync` and `--full` flags for `wt list`** - Flags accepted but not implemented
3. **Fish shell support** - Only zsh and bash
4. **Auto-merge integration** - Python has it, Rust doesn't
5. **Confirmation prompts for prune** - Uses `--force` flag instead

## Files changed

| Area | Files |
|------|-------|
| Rust engine | `naming.rs`, `worktrees.rs`, `lib.rs`, `git.rs` |
| Rust CLI | `commands/ops/mod.rs`, `main.rs` |
| Python | `naming.py`, `ops/next.py`, `ops/land.py` |
| Tests | `config_tests.rs`, `context_tests.rs`, `git_tests.rs`, `test_naming.py`, `test_next.py` |
| Docs | `01-lf-ops-parity.md`, `02b-summarize.md`, `docs/lfops.md` |

## Test results

- Rust: 53 tests passing (`cargo test`)
- Python: 678 tests passing (`uv run pytest tests/`)
- Linting: `cargo fmt` and `cargo clippy -- -D warnings` clean
