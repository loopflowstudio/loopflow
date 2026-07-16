# Make ambiguous Task ancestry refusal actionable and fully proved

W2-255. Directive v1:

> When Task PR ancestry is divergent or ambiguous, refuse before mutation and
> name the commits and files on both sides plus a safe recovery command. Complete
> the lifecycle proof matrix around that diagnostic: real task placement ahead of
> origin, stale base, no-remote refusal, squash-merged parent, serial rotation,
> and contaminated range. Done when users can identify exactly which work is
> foreign without opening raw internals, every case has deterministic integration
> coverage, and evidence lists the dogfood PRs rather than a single anecdote.

## Current state

`verify_task_pr_range_with_authority` in `rust/loopflow/src/ops/task.rs` already
handles four cases:

| Case | Condition | Current behavior |
|------|-----------|------------------|
| Parity | `M == B` | Pass — publish |
| Contaminated | `M` ancestor of `B` | Refuse, names foreign commits + files + rebase |
| Stale base | `B` ancestor of `M` | Heal `base_commit → M`, publish |
| **Divergent** | neither ancestor | **Refuse — names nothing** |

The divergent case (line 1259) prints a generic message with a rebase command
but does not name the commits or files on either side. A user who hits it cannot
tell which work is foreign without running raw git commands.

## Changes

### 1. Make the divergent refusal actionable

Replace the generic divergent error with a diagnostic that names commits and
files on **both sides**:

- **Base side** (`M..B`): commits reachable from the recorded base but not from
  the merge-base — work the recorded base carries that the upstream doesn't.
- **Upstream side** (`B..M`): commits reachable from the merge-base but not from
  the recorded base — work the upstream has that the recorded base doesn't.
- Recovery: `git rebase --onto {base_ref} {base} {branch}` (unchanged — replays
  the task's commits onto the current upstream).

### 2. Complete the lifecycle proof matrix

Integration tests in `rust/loopflow/tests/task_pr_range_tests.rs` and unit tests
in `rust/loopflow/src/ops/task.rs` mod tests:

| Case | What it proves | Test type |
|------|---------------|-----------|
| Real task placement ahead of origin | Placement refuses when canonical main is ahead | integration |
| Stale base | Serial PR heals after sibling lands | integration (exists) |
| No-remote refusal | No-remote repo still refuses contaminated range | integration |
| Squash-merged parent | Contaminated base after squash-merge is refused | integration |
| Serial rotation | Rotated continuation PR verifies | integration |
| Contaminated range | Foreign ancestry refused before push | integration (exists) |
| **Divergent ancestry** | Both sides named in refusal | unit + integration |

Dogfood PR references: #877, #882 (contamination root cause), W2-138 PR2 (#977,
the original proof matrix), and the PRs this work ships under.
