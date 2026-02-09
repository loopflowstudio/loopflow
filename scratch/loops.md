# Loops: Run History, Collapse, and Absorb

## Scope

Waves now support stacked iteration history and PR stack management in Concerto + lfd.

This doc captures the **current behavior**, backend/UI integration points, and remaining follow-ups.

## Current behavior

### Run history in Concerto

- Wave detail panel includes a segmented control: `Current` and `Runs (N)`.
- Runs are fetched on-demand when the Runs tab is selected.
- Runs tab shows:
  - run iteration, flow, status, and relative timing
  - PR metadata (number/state/link)
  - expandable row details
- Detail header includes an iteration timeline strip (colored dots by run/PR state).

### Collapse stacked PRs

- Endpoint: `POST /v0/waves/:wave_id/collapse`
- Ops entrypoint: `loopflow::ops::collapse::collapse_prs(...)`
- Behavior:
  1. Find open PRs for the wave.
  2. Gather commits from stacked PR branches.
  3. Create a fresh collapsed branch from the default base branch.
  4. Cherry-pick commits in order.
  5. Push and open a new PR.
  6. Close superseded PRs.
- Returns:
  - `new_pr_url`
  - `closed_prs`

### Absorb unpublished work into an existing PR

- Endpoint: `POST /v0/waves/:wave_id/absorb`
- Ops entrypoint: `loopflow::ops::collapse::absorb_into_pr(...)`
- Behavior:
  1. Resolve target PR branch.
  2. Compute commits on current branch not in target.
  3. Cherry-pick onto target branch.
  4. Push target branch.
- Returns:
  - `target_branch`
  - `commits_absorbed`

### UI actions

- Collapse action appears in Runs tab when there are 2+ open/draft PRs.
- Absorb action appears when current wave has unpublished commits and no PR, but prior open/draft PRs exist.
- Waiting state card no longer owns collapse behavior.

## Key files

### Rust

- `rust/loopflow/src/ops/collapse.rs`
- `rust/loopflow/src/ops/mod.rs`
- `rust/loopflow/src/lfd/http/mod.rs`
- `rust/loopflow/src/lfd/http/routes/waves.rs`
- `rust/loopflow/src/lfd/http/dto.rs`
- `rust/loopflow/tests/collapse_tests.rs`

### Swift

- `swift/Concerto/Views/WaveDetailPanel.swift`
- `swift/Concerto/Views/WaveRunsTab.swift`
- `swift/Concerto/Views/IterationTimeline.swift`
- `swift/Concerto/Views/WaitingStateCard.swift`
- `swift/LoopflowCore/Services/LocalWaveService.swift`
- `swift/LoopflowCore/Models/WaveRun.swift`

## Known constraints

- Collapse/absorb require `gh` CLI access and auth in repo context.
- Cherry-pick conflicts fail fast and require manual resolution.
- Collapse candidate selection currently depends on branch naming matching the wave token.

## Follow-ups

1. Add a stronger wave↔PR linkage than branch-name filtering.
2. Decide whether absorb should allow selecting any open PR instead of auto-targeting the most recent one.
3. Deferred: stimulus editing redesign.
