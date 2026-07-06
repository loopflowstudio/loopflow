# Chord Review — 2026-07-05

## Summary

Recent work after the architecture baseline is clean and narrow:

- `lf op rebase --plan` became a first-class planning path for branch updates.
- Concerto's local wave launch now treats a live endpoint as the only hard
  one-brain block, reclaims ghost tmux sessions, and uses one
  patience-parameterized endpoint probe path.

The live Asana roadmap was read from the user's shell after this sandbox failed
to decrypt the Asana/Doppler token. Current open work includes journal
engineering, mobile/server/CLI prove-the-language probes, the lf language
grammar/dispatch/channel remainder, Concerto viewer work, wave dynamics, spend
cap, and hosted/rented persistence backends. Several shipped items are still
open in Asana, notably the Wave agent full demo task for PR #796.

## Parallel Work

### File-roadmap leftovers

Remove Concerto's local `wave/<name>/{N-*.md,queue,proposals}` roadmap parsing
surface now that Asana is the roadmap source of truth. Scope should stay in
Swift content parsing/model/tests unless the worker finds a live caller.

**Landed**: Swift no longer has `RoadmapItem`, `RoadmapPriority`,
`roadmapItems`, parser roadmap symbols, or the local priority-renaming helper.
Docs now say roadmap state comes from the live PM surface.

### Dispatch extraction

Move the remaining run placement/worktree dispatch ownership out of
`lfd::executor` toward a top-level dispatch owner, preserving current behavior
and tests. Avoid changing the recent Concerto launch behavior.

**Landed**: `rust/loopflow/src/dispatch/mod.rs` now owns `Placement`,
`create_run_for_placement`, and run worktree creation. Direct callers now import
from `crate::dispatch`.

### Harness drift coverage

Harden the vendor harness conformance surface around recorded traces,
Codex/OpenCode manifest drift, and steer/interrupt coverage. Prefer narrow
tests or trace-recording helpers over a large harness rewrite.

**Landed**: trace manifests are explicit, conformance tests check fixture
coverage and supported wire methods, and Codex driver tests cover start, steer,
interrupt, and no-op interrupt.

## Blocked

Asana write reconciliation remains blocked in this sandbox by local
token/keychain access. SwiftPM tests also remain blocked here by sandbox/network
limitations, though the Rust focused checks pass.
