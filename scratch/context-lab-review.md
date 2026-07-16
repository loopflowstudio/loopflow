# Context Lab review

## Current product shape

Context Lab is one Wave-scoped research surface with three linked views:

```text
Initial prompts     Agent sessions     Sources by impressions
       \__________________|__________________/
                          ↓
         exact revision evidence and trace addresses
                          ↓
        new Task → running task-worker → source diff
```

It is progressive disclosure from the selected Wave header. The default Wave
surface remains purpose, Projects, and Chat; Context Lab opens separately at
research width with repo and Wave fixed for the life of the window.

The header no longer frames incomplete cost capture as a total. It separates
initial prompt load, lifetime provider input, and peak request/window pressure,
with a visible measured-session denominator for each.

Sources answers a different question from the token views: which current
instructions agents see most often. One impression is one distinct agent
session whose captured initial prompt includes the source. Current repo-native
sources stay visible at zero; provider-native sources without source-level
capture stay unavailable rather than becoming zero.

Selecting a source makes main's current file the primary pane. Historical
revision evidence remains in the right rail. A raw-file receipt prevents a
historical or changed revision from silently seeding work.

## Handoff review

The previous sheet and existing-idle-Task requirement are gone. The handoff now
refreshes the selected Wave plan and Context Lab snapshot, validates the source
path and raw hash, then checks main's source again immediately before it runs:

```text
lf task start "Refine <source> <hash>" \
  --project <refinement-project> \
  --directive <trace-linked seed>
```

and opens the created Task workspace on its Agent section. The directive starts
with `Refine text for X`, names the repo-relative source, requires the `refine`
skill, pins the starting SHA-256, and carries the exact Wave query,
measurements, and representative trace addresses.

Wave scope and Task ownership are deliberately separate. Project and Task no
longer appear as population filters. A Wave with one Project routes there; a
Wave with several Projects asks for one Refinement Project and remembers it for
later handoffs. A synced plan that no longer contains that Project blocks before
Task creation.

## Live design review

The first Wave empty state exposed an intrinsic-height `HSplitView` regression:
all panes were pinned to the bottom of a mostly blank window. Each pane now owns
the full available height, and the center is a fixed toolbar plus a filling
content region. The Context Lab scene also has a real minimum content size.

A clean window-only capture over the July 15 `0.11.009` production snapshot
showed 15 product runs and 64 agent sessions. Sources ranked:

- `headless surface`: 50 impressions, 100% observed reach;
- `wave_pursue`: 18 impressions, 36%;
- `wave_clarify`: 15 impressions, 30%;
- `wave_mutate`: 15 impressions, 30%.

Selecting `headless surface` opened main's
`rust/loopflow/src/engine/builtins/surfaces/headless.md`, its current raw hash,
50-session evidence, four distinct representative sessions, and the product
Wave's three real Project choices. A second review pass removed the redundant
Project/Task analysis filters and pinned short source files to the top of their
document pane.

After rebasing onto current main, Context Lab uses main's converged migration
chain: context pressure and input normalization are `0.11.009` and `0.11.010`,
followed by profiles and provider account lifecycle at `0.11.011` and
`0.11.012`. The feature adds no competing migration tail.

## Mitchell-style code review

- The public concepts map to real things: Wave population, agent session,
  source, revision, impression, provider input, peak request, Project-owned
  Task. The old “assembled turn,” cost-total, and existing Task-selection
  accidents are absent from the UI.
- Rust owns all reconciliation and canonicalization. Swift does not recreate
  trace joins, count impressions, or infer source precedence.
- Provider cache accounting is normalized once per provider adapter; cached
  input remains included without double-counting provider totals.
- Source observations deduplicate by canonical path and content hash. Current
  source inventory and historical revisions meet at that identity rather than
  through labels.
- External mutation has one guarded path. Source drift or Project drift stops
  before `lf task start`; the source is rechecked after Wave-plan sync and
  immediately before Task creation. No terminal injection, alternate editor,
  hidden Task, or compatibility shim remains.
- The Task title includes the source hash prefix, making repeated interventions
  distinguishable while keeping the human-readable source first.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo clippy -- -D warnings`: passed.
- `cargo test`: 1,409 tests passed and two tests remained skipped.
- `swift test --package-path swift`: 133 tests across 23 suites passed.
- `xcodebuild build-for-testing`: passed for the app and UI-test targets.
- `uv run pytest python/tests/`: 59 passed.
- Website tests: 59 passed, 3 skipped.
- `tests/e2e/test_smoke.sh`: passed.
- Migration order and checksums passed through `0.11.012`.
- Swift multiplatform boundaries and `git diff --check`: passed.

## Remaining human proof

The branch deliberately has not created a real Task as test debris. Human
review begins at the only external-write boundary: choose the product Wave's
intended Refinement Project and click **Refine in task-worker**. The reviewer
should confirm the new Task opens on its running Agent view, inspect the seeded
context, make or decline the source edit, review the real Task diff, and follow
the Context Lab backlink.

Until that click, the source edit → natural future session → new revision cohort
is intentionally unclaimed. All read paths, guards, command construction,
workspace routing, and backlink serialization are covered locally.
