# Concept review — Flow occurrences, loops and Task controls

2026-09-25, autonomous pass run by Claude in the control conversation at Jack's
request. It complements the attended review in `concept-review.md` (codex
Session in this checkout; accepted focus-follows-conversation), and does not
repeat it. Source read at HEAD `7123c443e` plus the live cycle-4 implement Run's
uncommitted `engine/flow_graph.rs` / `ops/task_flow.rs` draft, which is still
being edited. No executable change, test run, provider launch or PM write.

## Intent to keep

The accepted design governs (`main-view-task.md`): one Flow diagram drawn from
the pinned definition, occurrence-specific state, both Feature return edges,
queue → land, and Sessions labelled with their exact Flow membership. Jack
deferred the final two-loop visual and asked to keep building meanwhile.
Task, Flow and Session keep separate identities (the attended review confirms this).

## Accepted usage (implementation pending)

> Feature shows two blue loops, both returning to implement. One closes at the
> first loop-decide; the other closes at the loop-decide after demo. Each
> loop's label counts its own returns, so the two don't repeat one shared
> iteration number. A Session's
> chip ("implement · iteration 3") highlights that exact node in the diagram.
> A Task whose worker isn't running shows **Stopped at compress · Resume**;
> there is no Pause button until Loopflow can hold a Flow at a boundary.

## Findings

### 1. One occurrence, three encodings (observed; fix in the cycle-4 slice)

| Encoding | Example | Source |
| --- | --- | --- |
| Graph node key | `4/fix/1` | `flow_graph.rs::node_key` (draft) |
| Boundary key | `4:0/"fix"/1:2` (Debug-quoted path, iteration per level) | `ExecutionCursor::boundary_key` |
| Session membership | `step: "implement", step_index: 1, iteration: 2` | `RunFlowStep::of` → `SessionFlowMembership::Step` |

`step_index` is `cursor.leaf().index`. Inside an XOR it drops the parent path, so
a Session in `4/fix/1` and the root node `1` both report `step_index: 1`.
The Mac therefore cannot join a Session to its diagram node. That join is what
the design's membership chip and graph highlight need, and there's no other
field to join on. `RunFlowStep` already stores `boundary_key`, but the public
membership drops it.

Proposal: one structural node key, computed by the shared cursor/graph code.
Put it in `SessionFlowMembership::Step` as `node` in place of `step_index`, and
keep `step` (label) and `iteration`. Derive the graph key and the boundary key
from the same path function; don't keep a second string format that has to match.
Change the DTO fixtures in Rust and Swift together. Historical manifests without
the key remain `unknown`/unjoinable. Don't parse `step_index` into a guess.

### 2. Feature has two loops sharing one iteration counter — **decided: two loops**

`pursue.yaml`: `decide` and `decide_delivery` both `repeat.from: implement`.
Jack confirmed there are two loops (implement → decide, implement →
decide_delivery). `ExecutionCursor.iteration` is a single per-level counter that
increases on either return. The per-edge `FlowReturn.traversals` records which
edge was taken.

Consequence: labelling both regions "Loop · Iteration N" from `cursor.iteration`
would show the same number twice. Label each loop from its own edge's traversal
count and show the combined pass count only as secondary detail. The draft graph
projection already exposes `returns_to` and per-edge `traversals`, so this needs
no runtime change. (A one-loop framing was briefly recorded here and has been
withdrawn.)

### 3. Pause is a control that can never be used — **decided: not yet required, option (a)**

The design's Task situations include "paused" with Pause/Resume on the real saved
boundary. The draft `task_flow_controls` always emits `Pause` with the fixed
unavailable reason `NO_PAUSE`: `lf task interrupt` restarts the step, and nothing
holds advancement at a boundary. The only "paused" state is
`TaskExecutionState::Idle` with a pinned Flow: the worker is absent and Resume is
legal. The user can't choose to enter that state.

Either (a) keep Pause out of the product until a durable hold-at-boundary request
exists: delete `TaskFlowControlKind::Pause` and present Idle-pinned as
"Stopped at <step> · Resume"; or (b) implement hold-at-next-boundary on the
existing Work request/claim path as its own slice. A permanently disabled button
with an explanation adds a concept without adding a capability, so (a) is the
default unless Jack wants (b) now. The Task 2 prototype's Pause is simulation
only and doesn't establish that (b) is required.

### 4. The Session UI fixture still uses the removed Flow approval model (observed defect)

`swift/LoopflowMac/SessionFixture.swift` gives `.flow` the actions `approve` and
`iterate`, and refuses `session complete` for Flow. It also answers
`session approve|iterate`. Swift `SessionActionKind` is `open | move_here |
complete`, so decoding fails in session-fixtures mode for the flow kind.
`LoopflowUITests/SessionsInteractionTests` expects Complete, and neither
advance nor iterate, for every kind. The flow case therefore can't pass. The
prior receipt records that the hosted UI suite was not run.

The flow fixture also carries Task Work, so after cycle 3 it becomes a Task count,
not a `session-row-ui-flow` leaf. The test's row lookup needs updating too.
Fix: give Flow the same Complete action and handling as Ask, drop the approve and
iterate branches, and reach the Flow case through its Task row. Proof: run that
one hosted UI test.

### 5. Kept deliberately

- `TaskFlowRecord::Finished { flow }` draws no topology, because the definition
  isn't retained after settlement. It's honest; don't fabricate one from the
  current catalog.
- `runtime.started` (sidebar) vs chapter `begun` (carryover) are two facts with
  different correct answers for prepared human Runs. The distinction is already
  documented in `cycle-03-compress.md`.
- `SessionKind` ask/flow/interactive: the Mac branches only on shared actions,
  not kind. Kind remains useful provenance, so there's no merge proposal.

## Unchanged gaps

The attended review's focus-follows-conversation work is still unimplemented.
Also still open: Comments read/thread, New session delayed-prepare race,
configured app/provider demo, both performance measures, external trials and the
authorized Description edit (see cycle reviews). This review supplies evidence
only. It records no navigation decision and doesn't change the live implement
Run's direction; findings 1 and 3 fall inside that Run's slice, and the parent
session should route them there.

## Smallest next actions and proof

1. Cycle 4: replace `step_index` with the shared structural `node` key. Add one
   Rust test with a Session inside an XOR path that joins to its graph node, and
   the matching Swift DTO round-trip.
2. Fix `SessionFixture` Flow actions; run
   `SessionsInteractionTests/testEverySessionKindOpensWithOnlyItsResolutionActions`.
3. Jack decided (2026-09-25): Feature has two loops; Pause is not yet required. Both are recorded in `main-view-task.md`. Cycle 4 should label each loop from its own edge's traversal count and drop `TaskFlowControlKind::Pause`.
