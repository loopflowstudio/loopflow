# Cycle 4 — compress: Task Flow projection and controls

2026-09-25. Two reductions: one removes a second copy of topology, the other a
control Jack decided against. The parent's counter/node-identity and
pane-focus corrections are preserved. No commit, publication or live action.

## Model before → after

Traced: shared loader/catalogue (`flow_catalog`) → pinned `FlowPosition`
(`QueuedInvocation` + `ExecutionCursor`) → `FlowGraph` / `project_cursor` →
`TaskFlowSnapshot` + Rust control legality → JSON fixtures → Swift mirrors →
Podium catalogue reading and `flowDrafts` → `TaskFlowView`.

1. **Return edges had two representations.** Before: the definition carried
   `FlowNode.returns_to`, and the pinned record repeated the edge as
   `FlowReturn { decider, target, traversals, iteration }`. Rust also resolved
   the target twice, in `nodes` and `collect_returns`. Swift built fake
   `FlowReturn(traversals: 0)` values for previews, using a public initializer
   that existed only for that purpose.
   After: the edge exists once, as the deciding node's `returns_to`.
   `FlowReturn { decider, traversals, iteration }` carries only the execution
   evidence, keyed by its occurrence. A single `return_target` helper resolves
   targets for both the graph and the evidence. Swift draws spans from the
   definition and attaches evidence when a Flow is pinned. Previews have no
   counts and read "Loop".
2. **Pause was a permanently disabled control.** Before:
   `TaskFlowControlKind::Pause` always carried `NO_PAUSE`, and Swift rendered
   a disabled Pause button while the Flow was running.
   After: the kind, the constant, the Swift case and the button are deleted,
   following Jack's decision (option (a) in the concept review and the Flow
   section of `main-view-task.md`). An idle pinned Flow's status reads
   **Stopped · <reason>** and Resume uses Rust's legality. Nothing implies a
   pause exists.

Deleted: `TaskFlowControlKind::{Pause,pause}`, `NO_PAUSE`, `FlowReturn.target`
(Rust and Swift), Swift `FlowReturn.init`, the Pause button, the duplicated
Rust target lookup, and the tuple type `(edge: FlowReturn, from, to)`. The
tuple is replaced by a private `LoopSpan` with optional evidence.

Wire change: `target` is removed from each `returns` entry, and the `pause`
control is removed from `controls`. Rust, Swift and all four affected fixtures
(`task_flow`, `task_condition_states`, `wave_detail`, `roadmap_snapshot`)
moved together. The fields are required and have no defaults.

## Retained deliberately

- **`FlowCatalogEntry { graph?, unavailable? }`.** These are mutually
  exclusive, but they are explicit and read clearly in both languages. Turning
  them into an enum would churn the wire format for little benefit.
- **`FlowReturn.iteration` is separate from `traversals`.** This is the
  parent's correction: iteration is the saved cursor iteration at the edge's
  nesting level, and traversals counts returns on that edge. Labels stay
  one-based ("Loop · Iteration 3"). The two arrows show different counts under
  a shared iteration.
- **Structural node keys and `RunFlowStep.node: Option`.** Old manifests keep
  their unavailable node; nothing is inferred from a leaf index.
- **Nested XOR returns** are counted and listed in node detail, but not drawn
  as arrows, as before.
- **`Finished { flow }`** draws no topology, and there is no Swift YAML parser
  or catalogue re-expansion of a pinned Flow.
- **Separate lifetimes.** The draft lifetimes (`flowDrafts`: preview choice,
  picker, pending restart, error, acting) are kept separate from the pinned
  record and from the last-good catalogue reading.
- **Restart validates first.** `task restart --flow` still validates the
  selection before PM refresh, checkpoint, push or stop, and the confirmation
  text still names the checkpoint/push consequence.
- **`flow_tests.rs:506`** already asserts `occurrence == "current"`, so no fix
  was needed. The parent's focus test, which waits for the actual refusal, is
  untouched.

## Proof

Ambient `LF_*` Home/Run/Task/Flow/Session variables were cleared for all Rust
commands.

- `cargo test -p loopflow --lib -- engine::flow_graph ops::task_flow`:
  **5 passed** (`/tmp/loo291-c4c-lib.log`).
- `cargo test -p loopflow --test flow_tests task_flow_read`: **1 passed**
  (`/tmp/loo291-c4c-flow_tests.log`). This drives the real CLI in an isolated
  Home. It checks the returns array without `target`, and checks that the
  source edit leaves the pinned graph intact and the rejected restart leaves
  the position intact.
- `cargo test -p loopflow --test dto_fixtures`: **8 passed**.
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --all --check`
  pass. `git diff --check` passes.
- `swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel
  --filter 'TaskFlowTests|TaskFlowProofTests|DTOFixtureTests|WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal'`:
  **18 tests in 4 suites pass** (`/tmp/loo291-c4c-swift.log`). Coverage:
  - Controls are now exactly `[start, resume, restart]`.
  - Deciders and node `returnsTo` agree.
  - There is no Pause button.
  - Native PTY retention of the draft and companion holds through Flow
    controls.
  - The parent's pointer-focus/rename proof passes.
- Native capture: `/tmp/loo291-c4c-captures/task-flow-running.png`, visually
  inspected. It shows both arrows returning to implement, "Loop · Iteration 3"
  on both spans, and no Pause button.

A first attempt at the CLI/DTO commands did not run: a zsh variable holding
`env -u …` was not word-split. The commands were rerun inline.

SHA-256:

```text
engine/flow_graph.rs         4234cff44814aefd2ed74563523e6b1fc8ddf7b022034461cab1a4911e8c0733
ops/task_flow.rs             5beb874f5780aed9ee0b70fbbbf48a4f47d2463e9c03033bf037e43a76df8660
tests/flow_tests.rs          6bfece2ef933e57ae2a949f3362389223b5306bf22cbc0779ef23b155c0593d3
TaskFlow.swift               cba7ee51b432d7d8846af476375106d4df25ef261c6cf6888d49fb9f7021d569
TaskFlowView.swift           168342b1960841ce673156a2881c2a46abb353a614047db96c1212c13a44ba39
TaskFlowProofTests.swift     4be44c83f254d146bbe1c9fc60ee687adbc9572f5445497bbd289795b8004e03
swift/README.md              feab1a7b8d26f659d0cd15552875f026cc5db47d9bcf174d9aeeb8ab68d01178
task_flow.json               f562de8274924b22c34ebfc437b7664b54d6fe0c9cd8482e9f80cec43dd4bd31
task_condition_states.json   b365e62900cf22922be35d9b258ad1540a7571744da2ffd4377391a034932030
wave_detail.json             1b38204ea6e7e01a4f7d233b599104527e1706aabbc65f32da91b31004cac0be
roadmap_snapshot.json        b1b8bce92cc124e4e8febb1d432831985ce4c173aa8f33a025d90ae2162bc3cf
```

## Outstanding full-design gaps (unchanged)

- Comments: the real count and thread through the planning boundary.
- The final two-loop composition discussion with Jack.
- Success paths for real `task run`/`restart` against configured PM/provider.
- The Xcode fallback compile.
- The hosted Session UI test for the parent's `SessionFixture` Complete
  correction.
- The configured demo and both performance measures.
- External trials and the authorized Description edit.
- Hover reveal of Stop & restart is still exercised only through the name
  button.
