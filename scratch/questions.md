# Open questions

- Wave integration: do we stub with local metadata only, or defer until wave module is ported?
- Optional `wt` CLI dependency: should we emit events if `wt` is installed, or ignore it entirely for Phase 1?

# Bugs to fix

- **Worktree path layout**: Rust creates worktrees at `../<name>` but Python uses `../repo.<name>`. Fix Rust to match Python.
