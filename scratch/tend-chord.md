# Chord — 2026-03-18

## Context

Both waves are blocked and diverging. The root cause is agent-embedding building lfd infrastructure in chord-model's area — a predictable collision given that Concerto features need daemon support. The chord needs to untangle the dependency, not reorganize what was working.

## Mutations

### 1. Expand chord-model's area to explicitly own lfd store/types/triggers

**Wave**: chord-model
**Lever**: area
**Before**:
```yaml
area:
  - rust/loopflow/src/lfd/
  - rust/loopflow/src/lfd/http/
  - rust/loopflow/src/engine/
  - python/loopflow/
  - rust/loopflow/src/engine/builtins/steps/
  - rust/loopflow/src/engine/builtins/flows/
```
**After**:
```yaml
area:
  - rust/loopflow/src/lfd/
  - rust/loopflow/src/lfd/http/
  - rust/loopflow/src/engine/
  - python/loopflow/
  - rust/loopflow/src/engine/builtins/steps/
  - rust/loopflow/src/engine/builtins/flows/
  - rust/loopflow/src/lfd/store/
  - rust/loopflow/src/lfd/types/
  - rust/loopflow/src/lfd/triggers/
  - rust/loopflow/src/lfd/executor/
```
**Rationale**: The area already includes `rust/loopflow/src/lfd/` which covers these paths. But making the subpaths explicit signals to both waves (and their agents) that lfd internals are chord-model territory — not something agent-embedding should build ad hoc. The explicit listing also makes `lf tend` scans more precise.
**Risk**: Low. These are already under `rust/loopflow/src/lfd/`. This is clarification, not expansion.

### 2. Add terminal session infrastructure as a chord-model item

**Wave**: chord-model
**Lever**: items
**Before**: No item for terminal session store/API/types.
**After**: New item `02-terminal-session-infra.md`:
> lfd terminal session infrastructure — store tables, types, HTTP routes, and executor hooks for daemon-tracked terminal sessions. Extracted from agent-embedding PR #567. Ships as a standalone PR so Concerto can consume the API.
**Rationale**: The terminal session work that agent-embedding built in lfd genuinely needs to exist — it just needs to land through the right wave. Making it a chord-model item gives it a clean path: resolve merge conflicts, extract the lfd changes from #567's branch, ship them, then agent-embedding rebases the Swift layer on top.
**Risk**: Adds an item to chord-model's queue when it already has items in flight. But the work is already written — this is extraction and landing, not greenfield development.

### 3. Narrow agent-embedding's PR #567 to Swift-only changes

**Wave**: agent-embedding
**Lever**: items
**Before**: Item 02 ("Terminal Embedding" / "Daemon-Owned PTY Transport") covers both lfd infrastructure and Concerto UI in a single PR.
**After**: Item 02 becomes strictly the Concerto/Swift layer — workspace views, wave detail redesign, terminal session UI, attention store updates. The item description adds: "Depends on chord-model shipping terminal session infra first. Rebase onto main after that PR lands."
**Rationale**: This is the split that unblocks both waves. chord-model ships the lfd layer, agent-embedding ships the Swift layer. Each PR is reviewable in isolation. The 4358-line PR becomes two PRs of ~2000 lines each — still large but within range.
**Risk**: The split requires someone to manually extract the Rust changes from agent-embedding's branch and move them to chord-model's worktree. There may be tight coupling between the Rust and Swift changes that makes a clean split difficult.

### 4. Prune stale chord-model worktrees

**Wave**: chord-model
**Lever**: lifecycle
**Before**: Three worktrees — `20260316_1856` (stale), `20260318_0010`, `20260318_0020` (active with merge conflicts).
**After**: Prune `20260316_1856` and `20260318_0010`. Single active worktree `20260318_0020` with merge conflicts resolved.
**Rationale**: Three worktrees for one stream of work creates confusion. The stale ones serve no purpose and risk accidental work in the wrong directory.
**Risk**: Minimal. The stale worktree branches are behind main. Verify no uncommitted work before pruning.

### 5. Silence agent-embedding until chord-model ships terminal session infra

**Wave**: agent-embedding
**Lever**: silence
**Before**: Active with a failing 80-file PR and 20 uncommitted files.
**After**: Silent. No active items until chord-model lands terminal session infrastructure. The uncommitted work is stashed or committed to the branch for later rebase.
**Rationale**: agent-embedding can't land anything until the lfd layer ships through chord-model. Keeping it active means it continues accumulating drift. Silencing it focuses attention on chord-model — the bottleneck — and prevents the branch from diverging further. When chord-model ships the infra, agent-embedding wakes with a clean rebase target.
**Risk**: The uncommitted 20 files in agent-embedding's worktree need to be preserved before silencing. If they're lost, work is lost. Commit them to the branch first.

## Coherence

Mutations 1–3 form a dependency chain: clarify chord-model's ownership (1), create the extraction target (2), narrow agent-embedding's scope to match (3). They can be approved independently but achieve the most when all three land.

Mutation 4 (prune) is independent — it unblocks chord-model's worktree regardless of the other mutations.

Mutation 5 (silence) depends on 2 and 3 — silencing agent-embedding only makes sense if the lfd work has a path through chord-model. If mutation 2 is rejected (e.g., the user prefers to land the lfd work through agent-embedding despite the area violation), then mutation 5 should also be rejected.

Landing order: 4 → 1 → 2 → 3 → 5. Prune first to get a clean worktree. Then establish ownership, create the item, narrow the PR, silence the dependent wave.

## Deferred

**wave/lfd/ bootstrapping.** The scan noted an uncommitted `wave/lfd/` directory in agent-embedding's worktree. This could become a third wave for lfd-specific infrastructure. Deferring because: the immediate problem is landing existing work, not reorganizing wave structure. If terminal session infra is the only lfd work agent-embedding needs, a dedicated lfd wave is premature. Revisit after the current deadlock is resolved.

**agent-embedding run worktrees.** Two run worktrees (`run-82e6075a`, `run-e96079a0`) exist. Not pruning because they may be from active or recent lfd runs. Check their state before cleaning up.

**Item 02 renumbering.** agent-embedding renamed item 02 from "Terminal Embedding" to "Daemon-Owned PTY Transport" on the branch. The original name should be restored on main since the scope pivot happened inside a branch that hasn't landed. But this is cosmetic — defer until the PR split is done.
