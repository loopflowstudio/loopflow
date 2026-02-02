# lf ops: Full Feature Parity

Complete port of Python `lf ops` commands to Rust with 100% feature coverage.

## Current State

The Rust `lf ops` commands are minimal stubs. They perform basic git operations but lack:

- Worktree management (create, switch, prune, list, CI status)
- Stacked branches and wave integration
- PR auto-merge and merge queue support
- Commit message generation via agent
- Lint integration
- Shell directives for auto-cd
- Worktree preservation on `next`

## Commands

### Fully Implemented (Python → Rust gap)

| Command | Python Features | Rust Status |
|---------|-----------------|-------------|
| `rebase` | Fetch, rebase, conflict → agent handoff, force-push | Basic rebase only |
| `push` | Push with --force-with-lease fallback | Basic push only |
| `land` | Rebase, lint, scratch clear, PR refresh, auto-merge, worktree cleanup | Basic squash-merge only |
| `pr` | Auto-commit, rebase if behind, create/update PR, draft→ready, stacked base | Basic gh fill only |
| `sync` | Fetch + reset local main | Working |
| `next` | Preserve worktree, auto-merge PR, stack or fresh start, wave update, terminal open | Basic checkout + new branch |
| `commit` | Stage, lint, agent-generated message, push + draft PR | Requires explicit message |
| `abandon` | Find worktree, check clean, close PR, delete remote, remove worktree | Basic branch delete |

### New Commands Needed

| Command | Python Behavior |
|---------|-----------------|
| `wt create` | Schema-based branch names, --stack for stacking |
| `wt switch` | cd to worktree by short name |
| `wt list` | JSON output with prunable metadata |
| `wt prune` | Remove worktrees merged into main |
| `wt ci` | Show CI status, --watch, --logs |
| `shell install` | Install shell integration for auto-cd |

## Implementation Plan

### Phase 1: Git Operations

Port core git helpers to `loopflow-engine`:

```
loopflow_engine::git
├── rebase() - with conflict detection, abort on conflict
├── push() - with --force-with-lease fallback
├── sync_main() - fetch + reset local main
├── land() - local merge, PR merge, squash loop-main
├── is_behind_main() - count commits behind origin/main
├── find_worktree_by_branch() - locate worktree path
├── worktree_move() - preserve worktree at new path
├── worktree_create() - create with branch from ref
├── worktree_remove() - remove + delete local branch
└── cherry_pick_empty() - check if commits already in main
```

### Phase 2: PR and Commit Operations

Add PR message generation and commit helpers:

```
loopflow_engine::messages
├── generate_commit_message() - LLM-generated from diff
├── generate_pr_message() - title + body from commits
└── generate_pr_message_from_diff() - for PR updates
```

Update commands:
- `commit`: Stage all, lint, generate message via agent, push + draft PR
- `pr`: Auto-commit, rebase if behind, create/update, draft→ready
- `land`: Lint, clear scratch/, refresh PR, enable auto-merge

### Phase 3: Worktree Commands

Add `wt` subcommand group:

```
lf ops wt create <name> [--base <branch>] [--stack]
lf ops wt switch <name>
lf ops wt list [--format json] [--full] [--sync]
lf ops wt prune [--dry-run] [--force] [--debug]
lf ops wt ci [--watch] [--logs]
```

Dependencies:
- Branch naming schema from config
- Wave integration for tracking worktree↔wave
- Shell integration for auto-cd directives

### Phase 4: Next and Abandon

Full `next` implementation:
1. Auto-commit uncommitted changes
2. Rebase onto main (optional --no-rebase)
3. Check PR state (open, merged, none)
4. Preserve current worktree at timestamped path
5. If PR open: enable auto-merge, create stacked branch from HEAD
6. If merged: create fresh branch from origin/main
7. Update wave worktree/branch mapping
8. Open terminal in new worktree
9. Write shell directive for auto-cd

Full `abandon` implementation:
1. Find worktree by branch name
2. Check for uncommitted changes (error unless --force)
3. Confirm with user (skip with --force)
4. Close PR if exists
5. Delete remote branch
6. Remove worktree + local branch

### Phase 5: Shell Integration

Add shell integration commands:
```
lf ops shell install   # Add to .zshrc/.bashrc
lf ops shell directive # Write cd directive for current shell
```

Write directives to temp file that shell sources after each command.

## Dependencies

### loopflow-engine additions

```rust
// New modules
pub mod messages;      // Commit/PR message generation
pub mod worktrees;     // Worktree operations
pub mod naming;        // Branch naming schemas

// Extend git module
pub mod git {
    pub fn rebase_with_abort(...) -> RebaseResult;
    pub fn push_force_with_lease(...) -> Result<()>;
    pub fn find_worktree_by_branch(...) -> Option<PathBuf>;
    pub fn worktree_move(...) -> Result<PathBuf>;
    pub fn worktree_create(...) -> Result<PathBuf>;
    pub fn worktree_remove(...) -> Result<()>;
    pub fn cherry_pick_empty(...) -> bool;
    pub fn is_branch_merged(...) -> bool;
}
```

### External tool requirements

- `gh` CLI for PR operations
- `wt` CLI for worktree events (optional, falls back to git)
- Agent backend (claude/codex) for commit message generation

## Testing

Unit tests:
- Branch naming schema generation
- Merge detection logic
- Worktree path resolution

Integration tests:
- Full next workflow in temp repo
- Prune with various merge states
- Stacked branch creation

## Migration

No breaking changes. Rust `lf ops` commands gain features incrementally. Python `lf ops` remains authoritative until Rust reaches parity.

Parity checklist:
- [ ] `lf ops rebase` - conflict detection + agent handoff
- [ ] `lf ops push` - force-with-lease fallback
- [ ] `lf ops land` - lint, scratch clear, auto-merge
- [ ] `lf ops pr` - auto-commit, rebase, create/update
- [ ] `lf ops sync` - done
- [ ] `lf ops next` - full worktree preservation workflow
- [ ] `lf ops commit` - agent-generated messages
- [ ] `lf ops abandon` - PR close, worktree cleanup
- [ ] `lf ops wt create` - schema-based naming, stacking
- [ ] `lf ops wt switch` - auto-cd
- [ ] `lf ops wt list` - JSON output
- [ ] `lf ops wt prune` - merge detection
- [ ] `lf ops wt ci` - CI status + logs
- [ ] `lf ops shell install` - shell integration

## Open Questions

| Question | Options | Decision |
|----------|---------|----------|
| Wave integration | Port wave module, defer, stub | TBD |
| wt CLI dependency | Required, optional, inline | Optional with fallback |
| Shell integration | Fish support? | zsh/bash first |
| Lint integration | Run lf lint step vs config command | lf lint step |
