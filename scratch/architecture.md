# Current cut: make Steer the only authored direction

[`docs/architecture.md`](../docs/architecture.md) remains the canonical target.
This file names the current executable cut for `lf code`.

## Why this cut is next

Migration `0.11.031_durable_input_spine` already removes
`child_directives`, but production Project/Task control and runners still read
and write it. The branch compiles and then fails behavioral tests with `no such
table: child_directives`. Adding compatibility storage would preserve the
architecture being removed.

The existing `agent_turns` table is the sole Turn and usage authority. Extend
it with fixed Basis and use its id for Send. Do not add another `turns` table.
The existing `agent_launches` lineage is the Launch authority to extend or
replace in place; do not create a shadow Launch ledger.

## Change

Make the durable input spine authoritative end to end:

1. Route initial Project and Task direction and every authored follow-up,
   steer, replacement message, and resume message through one Steer append.
2. Render the next boundary from `boundary_seed(work)` in revision order.
   Runners must not query a current directive or incorporation version.
3. Store the boundary's immutable Basis on `agent_turns`. A successful later
   Turn derives which Steers applied; transport acceptance never does.
4. Fence terminal completion with current and successfully applied Basis.
5. Keep interrupt, stop, abandon, CI wake, and process recovery as typed
   lifecycle/evidence operations. They may not carry authored prose. Preserve
   main's current actionable-CI behavior: an incident arriving beside active
   execution preempts a parked Turn once and settles before background flow;
   it does not become Steer or launch a second writer.
6. Rename the narrow machine response to ToolResponse throughout. It persists
   before optional provider notification. Do not retain generic Decision or
   Approval domain APIs.
7. Delete `ChildDirective`, its ids/kinds/events/store APIs/SQL/tests and every
   directive version/incorporation field that no longer carries distinct
   truth. Delete authored-prose variants from `ChildCommand`; do not emulate
   them with compatibility methods.
8. Remove unused target types or wire them into this cut. A warning-producing
   public type/table with no behavior is not progress.

This cut may leave typed lifecycle commands temporarily while the shared Run
controller replaces them, but no command may represent direction.

Do not synthesize a `ChildDirective`-shaped compatibility view from Steers.
Command linkage, source enums, directive kind, version, and incorporation are
the old model, not missing Steer fields. Change consumers to `BoundarySeed`,
ordered `Steer`, and Basis, or delete the behavior and its legacy assertion.
No store method may return `ChildDirective` after this cut.

## Done when

- `rg 'ChildDirective|DirectiveKind|current_directive_version|incorporated_directive_version' rust/loopflow/src rust/loopflow/tests` has zero production references;
- `rg 'FollowUp|replacement:|Resume \{[^}]*message|ChildCommandKind::Steer' rust/loopflow/src` has zero authored-direction paths;
- User, Wave parent, and Project parent direction call the same Steer function;
- crash after a confirmed live Send still leaves the Steer in a later seed;
- a live Steer makes completion from the older Turn Basis stale;
- ordered outstanding Steers seed exactly once as one projection;
- current actionable CI still preempts a parked Review once, while stale and
  land-time-only failures do not interrupt it;
- `agent_turns` remains the only Turn table and additive usage store;
- no separate `turns` or shadow Launch table survives;
- migration from a copied pre-cut database succeeds without `child_directives`;
- focused durable/controller/migration tests, full Rust tests, fmt, and clippy pass with no warnings;
- Rust LOC decreases from the current checkpoint rather than growing.

After this cut, the next pass collapses InteractionReview/Handoff into flow +
Launch + attention and wires that attention into the active parent agent loop.
