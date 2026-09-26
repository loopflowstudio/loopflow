# Cycle 4 — review-slice: Task Flow diagram and controls

2026-09-25. Verdict: **the Flow slice moves the full design forward, except for
one unmet claim: the tuple iteration.** Four small presentation fixes went in
here; all are Swift-only. Rust source is byte-identical to the compress receipt,
so the ordered-tuple correction has **not** arrived. The control conversation
owns that change (`jack-iteration-tuple.md`, Status: pending), so the tuple
fields and runtime were not touched here. Comments, the final composition
discussion and configured acceptance are still separate work. This review does
not approve publication.

## Claim matrix

| Claim | Implemented | Proof | Result |
|---|---|---|---|
| Real Feature topology | The shared loader and `FlowGraph` project the captured definition | Real `lf flow list --json` (local debug CLI, catalogue read only) gives 13 nodes (listed below). Design runs once; `decide`→2 and `decide_delivery`→2; demo sits between them; the queue tail (compress, update-wave, gate) leads into the `pr land -c` op. Rust test `feature_draws_both_returns…` pins the same shape | pass |
| Distinct occurrences and independent counts | Structural keys come from the cursor's `node_key`; per-edge `traversals` come from `FlowProgress.repeats` | Rust graph tests (compress receipt, unchanged source); Swift `occurrenceStates` | pass |
| Completion is scoped to one pass | `project_cursor.completed` is limited to the current pass | Rust test; real-Feature capture after a demo return shows only design complete | pass |
| **Ordered iteration tuple** | Still the shared scalar `FlowReturn.iteration`. Both spans read "Loop · Iteration 3" after (1,1) returns | `real-feature-running-after-demo-return.png` shows the rejected shared-scalar display | **gap: tuple owner pending** |
| No substitution for missing history | `Finished` draws no pinned topology; the status names the old Flow as finished and labels the diagram "Preview: X" | Source; fixture snapshot 4 | pass, see limitation below |
| Typeahead preview, cancel and Start | Draft is kept per Task; Start is legal only per Rust controls | Native proof: cancelling or choosing records nothing; Start records exactly `task run W2-156 --flow build` | pass (fixture transport) |
| Stop & restart validates first | `restart_task_async` loads the replacement before the PM refresh, checkpoint and stop | Source; real CLI `task_flow_read` rejects a missing Flow with position and HEAD unchanged (compress receipt) | pass |
| Restart confirmation states the real consequences | **Corrected.** Restart also refreshes from Linear, commits *all* changes, and **starts** the replacement (`launch_task_process`) rather than only pinning it | Source trace through `checkpoint_task_restart` and `launch_task_process` | fixed |
| Pause | Not in the contract or view. An idle worker shows "Stopped · reason" with Resume | Grep; native proof asserts no pause control | pass |
| Legality comes from shared policy | Rust `task_flow_controls` supplies it; human state refuses Resume ("continue it through its Session") | Rust control test; native disabled-Resume assertion | pass |
| Duplicated status text | **Corrected.** Previously "Waiting for you · Waiting for your review at demo" and "Running · Task worker is running …" | Native proof expectations updated | fixed |
| Unknown liveness is not a claim | **Corrected.** An unknown node now reads "current, worker state unknown" instead of "not advancing" | Source | fixed |
| Colors: blue loops/running, yellow pending human, green completed, red blocked | **Corrected legibility.** Translucent fills over the blue loop tint had turned green and yellow grey. Nodes now have an opaque base | Before/after real-Feature captures, visually inspected | fixed |
| Native retention | Surfaces, layout, draft echo and companion reply survive the Flow controls, the rejected restart and a refresh | `TaskFlowProofTests` plus `namedSessionDrillDownRetainsTerminal` (real Ghostty, `/bin/cat`) | pass |
| Parent fixes preserved | Task-keyed New-session feedback, pane focus/rename, structural node key | `newSessionFailureKeepsItsTask` and drill-down test in the final run | pass |
| One authority | No Swift YAML parser or Pause; controls use only `task run/restart/resume` | `rg`; one `flowCatalog` reader keyed per repository | pass |

The 13 real Feature nodes, in order: `kickoff, review-design(H), implement,
compress, review-slice, concept-review, loop-decide#decide, demo(H),
loop-decide#decide_delivery, compress, update-wave, gate, pr land -c`.

## Commands and evidence

- Catalogue: `env -u LF_HOME … target/debug/lf flow list --json`. The binary is
  from 15:36 and postdates the Flow source. The output is saved as
  `cycle-04-review-evidence/loo291-c4r-catalog.json`.
- `swift test --package-path swift -Xswiftc -gnone --jobs 4 --no-parallel --filter
  'TaskFlowTests|TaskFlowProofTests|WorkspaceNavigationTests/newSessionFailureKeepsItsTask|WorkspaceNavigationProofTests/namedSessionDrillDownRetainsTerminal'`
  passed **5 tests in 4 suites**, exit 0 (`loo291-c4r-swift-final.log`). Two
  earlier passing runs are also kept: `swift.log` and `swift2.log`.
- Real-Feature captures: `real-feature-{preview,demo-pass2,running-after-demo-return,blocked-gate}.png`.
  - They come from a temporary test that decoded the real catalogue into
    `FlowDiagram` at 1100 pt. The test was deleted after capture.
  - The execution positions in the captures are synthetic cursors, not
    configured Task state.
  - The captures predate the final `.stopped` rename, which does not change the
    rendering.
- No Rust edits, so fmt and clippy were not rerun. Rust tests were not rerun
  because the source hashes equal the compress receipt. `git diff --check`
  passes on Swift.
- Patch: `cycle-04-review-evidence/correction.patch`. Pre-edit copies are in
  `/tmp/loo291-c4r-before/`.

SHA-256: `flow_graph.rs 4234cff4…c0733`, `task_flow.rs 5beb874f…8660`,
`task.rs 692d2fca…7aa7`, `TaskFlow.swift cba7ee51…021d569`,
`TaskFlowView.swift e0b37a8d…58fdba`, `TaskFlowProofTests.swift 07dc629d…b3e0`,
`PodiumModel.swift 460e2176…87b9`, `task_flow.json f562de82…d4bd31`,
`flow_catalog.json d75defb7…6a`, `target/debug/lf 44fc2039…3c`.

## Current limitations (for the composition discussion, not fixed)

- At 1100 pt, 13 nodes need horizontal scrolling: the tail after the second
  loop-decide is off screen.
- The two return arrowheads land almost on top of each other at implement.
- Completed green is pale but distinct.
- A finished Task draws today's preview under a "finished · Preview: X" status.
  The label is honest, but a same-named preview can still be misread as its
  history.
- An idle-dead worker reads "Stopped · Task worker stopped at X…". One word
  repeats; the reason text is owned by Rust's shared projection.
- After a successful control, `refresh()` refreshes the *current* repository.
  If the user switched repositories, the originating repository waits for the
  next poll. The draft itself settles on the correct owner.
- Nested XOR returns appear in node detail but are not drawn as arrows.
- Hover reveal of Stop & restart is reached only through the name button in
  ViewInspector.
- The fixture Session in the native proof is unmatched to a Task. There is no
  Task-row requirement inferred from that.

## Still required

1. Tuple iteration, owned by the control conversation. It must come with
   focused proof:
   - per-loop labels,
   - a header tuple such as "(2, 1)",
   - Session membership iteration,
   - unchanged exact boundary keys and stale-decision rejection.

   The old scalar tests (`returns.map(\.iteration) == [2, 2]`,
   "Loop · Iteration 3") do not satisfy it.
2. A Session membership chip that highlights its graph node. The `node` field
   exists; the interaction does not.
3. Comments, recent Runs, and a useful provider summary per Session.
4. Real delayed subprocess preparation for New session.
5. Configured demo and measurements, then Jack's two-loop composition
   discussion.

No commit, publication, install, PM or store mutation, live Session action or
Task completion took place.
