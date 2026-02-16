# Review: open PR counting correctness + worktree sidebar polish

## What was implemented

- Unified open-PR state checks in lfd HTTP routes so CI hook matching and wave DTO counting use the same `is_open_pr_state` logic.
- Changed open PR semantics to treat unknown PR state (`None`) as **not open**.
- Added `count_unique_open_prs` to dedupe open PR counts by PR number when building `WaveDto.open_pr_count`.
- Added Rust tests covering:
  - unknown PR state handling
  - unique open PR counting with duplicate PR numbers
  - CI target matching rejecting unknown-state PRs
- Updated Concerto sidebar labeling from **"On Disk"** to **"Worktrees"** and adjusted worktree row display-name preference to use `shortName` before raw branch name.

## Key choices

- **Single source of truth for open-state logic:** `hooks.rs` now calls `super::is_open_pr_state(...)` instead of maintaining a duplicate helper.
- **Conservative unknown-state behavior:** PRs without a known state are excluded from open counts and CI target selection to avoid inflated/stale queue signals.
- **Dedupe by PR number:** count logic collapses repeated run snapshots that point at the same live PR number.

## How it fits together

Wave list rendering (`build_wave_dto`) now computes `open_pr_count` from a helper that filters by authoritative open states and deduplicates by PR number, reducing overcount from repeated snapshots. The hooks route reuses the same state predicate, so webhook-derived CI targeting and sidebar/open-count badges now align on what "open PR" means.

## Risks and bottlenecks

- Counting still relies on run snapshots (not live PR sync), so merged/closed state can still drift until broader live-state sync work lands.
- PR entries with no number are still counted as unique if their state is open/draft.
- Full `xcodebuild test -scheme Concerto` run can fail locally in UI-test bootstrap due macOS authentication prompt cancellation; unit/package tests pass.

## What's not included

- No live PR state table/sync migration (foundations phase item).
- No queue lifecycle changes (Draft/Ready/Blocked roles).
- No merge advancement/rebase automation.
- No Combine lifecycle reconciliation changes.
