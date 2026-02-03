# 01: lf ops Full Feature Parity

Complete port of Python `lf ops` commands to Rust with full feature coverage.

## Current State

Worktree management is implemented. Other ops commands are minimal stubs.

**Implemented:**
- Worktree commands (`wt create/switch/list/prune/ci`)
- Shell integration (`shell install` for zsh/bash)
- Basic `next` (preserve worktree + create new)
- Worktree primitives in `loopflow-engine`

**Remaining:**
- Stacked branches and wave integration
- PR auto-merge and merge queue support
- Commit message generation via agent
- Lint integration
- Full `next` workflow (PR handling, stack retargeting)

## Commands

### Status by Command

| Command | Python Features | Rust Status |
|---------|-----------------|-------------|
| `rebase` | Fetch, rebase, conflict → agent handoff, force-push | Basic rebase only |
| `push` | Push with --force-with-lease fallback | Basic push only |
| `land` | Rebase, lint, scratch clear, PR refresh, auto-merge, worktree cleanup | Basic squash-merge only |
| `pr` | Auto-commit, rebase if behind, create/update PR, draft→ready, stacked base | Basic gh fill only |
| `sync` | Fetch + reset local main | Working |
| `next` | Preserve worktree, auto-merge PR, stack or fresh start, wave update, terminal open | ✅ Preserve + new worktree |
| `commit` | Stage, lint, agent-generated message, push + draft PR | Requires explicit message |
| `abandon` | Find worktree, check clean, close PR, delete remote, remove worktree | Basic branch delete |
| `wt create` | Schema-based branch names, --stack for stacking | ✅ Schema naming (stacking deferred) |
| `wt switch` | cd to worktree by short name | ✅ Working |
| `wt list` | JSON output with prunable metadata | ✅ Working |
| `wt prune` | Remove worktrees merged into main | ✅ Working |
| `wt ci` | Show CI status, --watch, --logs | ✅ Working |
| `shell install` | Install shell integration for auto-cd | ✅ zsh/bash (fish deferred) |

## Implementation Plan

### Phase 1: Worktrees ✅

Worktree primitives in `loopflow-engine::worktrees`:

```
loopflow_engine::worktrees
├── list()           - parse `git worktree list --porcelain`, enrich with merge status
├── create()         - deterministic naming, consistent layout (../<branch>)
├── preserve_move()  - move worktree to timestamped path
└── remove()         - prune worktree + local branch
```

Shell integration via directive file that wrapper sources after `lf` returns.

### Phase 2: Git Operations

Port core git helpers:

```
loopflow_engine::git
├── rebase_with_abort() - with conflict detection, abort on conflict
├── push_force_with_lease() - with --force-with-lease fallback
├── is_behind_main() - count commits behind origin/main
├── cherry_pick_empty() - check if commits already in main
```

### Phase 3: PR and Commit Operations

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

### Phase 4: Next and Abandon

Full `next` implementation:
1. Auto-commit uncommitted changes
2. Rebase onto main (optional --no-rebase)
3. Check PR state (open, merged, none)
4. Preserve current worktree at timestamped path ✅
5. If PR open: enable auto-merge, create stacked branch from HEAD
6. If merged: create fresh branch from origin/main ✅
7. Update wave worktree/branch mapping
8. Open terminal in new worktree
9. Write shell directive for auto-cd ✅

Full `abandon` implementation:
1. Find worktree by branch name
2. Check for uncommitted changes (error unless --force)
3. Confirm with user (skip with --force)
4. Close PR if exists
5. Delete remote branch
6. Remove worktree + local branch

### Phase 5: Shell Integration ✅

Shell integration commands:
```
lf ops shell install   # Add to .zshrc/.bashrc
```

Write directives to temp file that shell sources after each command.

**Status:** zsh/bash working. Fish deferred.

## Dependencies

### loopflow-engine additions

```rust
// Implemented
pub mod worktrees;     // Worktree operations ✅

// Needed
pub mod messages;      // Commit/PR message generation
pub mod naming;        // Branch naming schemas (partial)

// Extend git module
pub mod git {
    pub fn rebase_with_abort(...) -> RebaseResult;
    pub fn push_force_with_lease(...) -> Result<()>;
    pub fn cherry_pick_empty(...) -> bool;
    pub fn is_branch_merged(...) -> bool;  // ✅
}
```

### External tool requirements

- `gh` CLI for PR operations
- Agent backend (claude/codex) for commit message generation

## Parity Checklist

- [ ] `lf ops rebase` - conflict detection + agent handoff
- [ ] `lf ops push` - force-with-lease fallback
- [ ] `lf ops land` - lint, scratch clear, auto-merge
- [ ] `lf ops pr` - auto-commit, rebase, create/update
- [x] `lf ops sync` - done
- [x] `lf ops next` - worktree preservation (PR/stack handling deferred)
- [ ] `lf ops commit` - agent-generated messages
- [ ] `lf ops abandon` - PR close, worktree cleanup
- [x] `lf ops wt create` - schema-based naming (stacking deferred, path layout needs fix)
- [x] `lf ops wt switch` - auto-cd
- [x] `lf ops wt list` - JSON output
- [x] `lf ops wt prune` - merge detection
- [x] `lf ops wt ci` - CI status
- [x] `lf ops shell install` - shell integration (fish deferred)

## Open Questions

| Question | Options | Decision |
|----------|---------|----------|
| Wave integration | Port wave module, defer, stub | Deferred |
| Shell integration | Fish support? | zsh/bash first, fish later |
| Lint integration | Run lf lint step vs config command | TBD |

## Known Issues

- **Worktree path layout**: Rust uses `../<name>`, Python uses `../repo.<name>`. Need to fix Rust to match Python behavior.
