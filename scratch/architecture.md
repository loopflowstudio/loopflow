# Current cut: delete Review/Handoff and Session control

[`docs/architecture.md`](../docs/architecture.md) is the canonical target.
This file names the next executable cut for `lf code`.

## Starting truth

The durable input spine is now authoritative:

- authored direction is only `Steer`;
- every Steer allocates one Epoch revision;
- the next boundary renders ordered outstanding Steers from `BoundarySeed`;
- `agent_turns` stores immutable starting Basis and remains the sole Turn and
  usage store;
- `Send` records live or seeded delivery without acknowledging application;
- successful later boundary Basis derives application;
- `ChildDirective`, directive versions, replacement/follow-up/resume prose,
  and writable Ack are gone;
- interrupt carries no direction;
- current actionable CI still preempts a parked boundary once, while stale and
  land-time-only failures do not.

Do not recreate any of those removed shapes. The remaining `ChildCommand`
variants are lifecycle bridge debt, not a place to put direction.

## Why this cut is next

`InteractionReview`, `InteractiveHandoff`, Project/Task Session leases, and the
two child runners encode the same reality several times. A Task waiting for its
parent gets a Review row, reviewer generation, disposition, observation,
command, and Project-owned scheduling dependency. An opaque TUI gets a Handoff
row beside the process that already is its Launch. Parent agents service those
facts only after background pursuit, which is the dogfood starvation bug.

The target has less state:

```text
interactive flow position + live Launch + attention route = Review
Steer + Turn                                           = conversation
Run + Launch containment                              = execution authority
```

Delete the aggregates instead of projecting them over Steer or copying their
ids onto the new spine.

## Change

1. Replace stored Review with a projection over the current interactive flow
   step, active Launch, and `attention: User | Parent(WorkRef)`. At most one is
   current for a Work. It has no id, disposition, reviewer generation, prompt,
   evidence copy, or terminal row.
2. Route every Review message through ordinary `steer(work, ...)`. A parent
   message requires its active Run lease; an external client is `User`.
   Transport delivery never clears attention.
3. Add `close_review(work, if_basis)`. It advances the current interactive flow
   step only when the route, flow position, and Basis are current. It records no
   approval/changes-requested decision. Extra findings are a Steer.
4. Represent an attached tmux/TUI as an opaque Launch. Keep only generic attach
   route, containment, and explicit `succeeded | failed | interrupted | unknown`
   handback evidence. Delete Handoff identity, parent variants, outcomes, and
   parallel liveness state.
5. Make Wave and Project drain one derived control lane before background
   pursuit: direct User input, oldest child attention, then other unblocking
   child evidence. The already-running flow agent handles the child; do not
   create a reviewer agent, Review inbox, or secondary Launch.
6. Live-deliver child attention to the exact active parent Turn when accepted.
   Otherwise interrupt that background boundary and seed the already-durable
   Steer next. Preserve the playhead so completed background steps do not replay.
7. Replace Project/Task body reservation, activation, revocation, reaping, and
   settlement with shared `Run reserve | advance | stop`. Bind operations to
   the exact Run id/lease, never a current Session lookup or generation guess.
   Keep Task/Project closure and flow selection as typed domain functions.
8. Replace lifecycle `ChildCommand` rows with direct controls and typed input:
   bare interrupt/abandon/resume become Run/Work operations; CI remains typed
   evidence and trigger state. Delete command claim/delivery/effect/source
   machinery once no caller needs it.
9. Extend the existing `agent_launches` lineage in place for provider/TUI route,
   containment, resume token, Home, and Run. Do not add a shadow Launch table.
10. Keep stable Work/Epoch across nonterminal execution. A terminal restart
    opens a new Epoch. Historical recovery may read the predecessor Epoch but
    may not pretend it is the current open Epoch.
11. Move operational write fences onto Run authority. Global-promotion
    preflight and `lf pm reteam` currently inspect Project/Task Session leases;
    after the cut they must query live Runs and containment without learning a
    Session id or body generation.

## Delete

Delete these concepts and their production references, not just their top-level
files:

```text
interaction_review.rs
interactive_handoff.rs
task/interactive_rendezvous.rs
store/interaction_reviews.rs
store/sqlite/interaction_reviews.rs
store/interactive_handoffs.rs
store/sqlite/interactive_handoffs.rs
lf/commands/handoff.rs
lf/commands/reviews.rs
InteractionReview*
InteractiveHandoff*
InteractionReviewer
InteractionReviewDisposition
reviewer_generation
phase_epoch as a review fence
ChildCommandSource
ChildCommandState
ChildCommandEffect
authored or lifecycle ChildCommand variants
```

Remove the matching Rust/Swift DTOs, fixtures, CLI commands, snapshots, and
tests. Preserve terminal-opening mechanics only behind a smaller Launch surface.
Replace succession/review tests with Work/Epoch/Run/attention behavior tests.

## Done when

- no Review or Handoff table, id, disposition, reviewer generation, or DTO
  remains;
- `rg 'InteractionReview|InteractiveHandoff|AwaitingHuman|Author::Human'` has
  zero production references;
- User and active parent Run call the same Steer mutation;
- a stale parent Run cannot steer or close a child Review;
- Project and Wave never start background work while child attention is queued;
- one parent provider identity conducts background flow and child conversation;
- a seed-only parent interrupts and handles child attention without replaying
  completed playhead steps;
- dirty canonical main cannot block Review Steer or close;
- closing Swift leaves the opaque Launch live, reopening attaches to it, and
  killing tmux removes attention and produces recovery evidence;
- one Run transition suite covers Wave, Project, and Task reservation, stop,
  absence fencing, and recovery;
- `Unprovable` containment never releases the active Run slot;
- no Session/body generation is required to locate current Work or authorize a
  mutation;
- `ChildCommand` has zero production references after direct control cutover;
- `agent_launches` and `agent_turns` remain the only Launch/Turn authorities;
- global-promotion preflight and `lf pm reteam` protect active writers through
  Run authority, not Session status or body leases;
- current actionable CI preemption still passes without a command-ledger wake;
- migration, full Rust tests, fmt, and clippy pass together;
- Rust code falls by at least another 8,922 lines, reaching at most 120,220
  physical code lines on current main (119,127 after normalizing out main's
  1,093-line orthogonal addition).
