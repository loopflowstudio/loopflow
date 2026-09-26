# Shared Session contract review — 2026-09-24

The slice advances the accepted design: Rust now supplies Session actions and
Work display paths, and Swift consumes them without a second legality matrix.
This review reproduced and fixed one presentation defect: rejected Approve or
Iterate replaced the live pane with an opening failure, then ordinary polling
erased the rejection. Both decisions now retain the terminal and display their
error through polling and retry.

## Scope and evidence

Read the Task directive, handoff, current design and its later amendments,
product memory, iteration 15 implementation and prior review. The current slice
owns shared action descriptors, stable Work paths, Ready enforcement and retained
opening. The full target still includes unified A/D navigation, current planning,
nested checkout workspaces, directive editing, configured retention, ten trials
on human-selected external work and twenty trials against published budgets.
Earlier one-conversation/cardinality and Task-first creation proposals do not
supersede the later scoped fresh-conversation amendment.

Recovered the unrestricted tracked base-to-worktree patch at
`/tmp/loo291-review15-tracked.patch`: 209 sections, 188 unchanged from the prior
review snapshot, 21 changed, none removed. Reviewed the changed production/test
sections and new action fixture/helper; reused the prior review of unchanged
sections. This review's subsequent two Swift edits, README correction and receipts
are additional to that snapshot. Existing dirty and untracked work is preserved.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Shared Session actions | Rust supplies labels, help and unavailable reasons; Mac does not infer legality | One Rust policy feeds projection and operation checks; Swift dispatches descriptors | Source trace, twelve-case fixture, unchanged producer hashes and prior Rust/Swift receipts | pass at contract/local scope |
| Ready decisions | Premature Approve/Iterate cannot stop a client or advance the playhead | Operations check before stopping; settlement rechecks its loaded position; Iterate checks its predecessor | Prior premature/Ready/predecessor behavioral receipts, unchanged Rust | pass locally; configured FlowStep still open |
| Stable readable ancestry | Exact Work identity plus readable path without a labels-only roadmap read | Stored typed ancestry supplies the path; missing Work is explicit | Fresh four-record CLI/production Swift comparison; exact LOO-291 path agrees | pass for observed interactive population |
| Rejected decisions | A rejected action remains visible beside a usable terminal | Shared local resolution error survives refresh and retry; opening state is unchanged | New mounted native regression, both Approve and Iterate; same cat PTY responds | pass after correction; injected transport rejection |
| Existing Session behavior | Complete, explicit Move here, prepared launches and opening failures retain their meanings | Existing operations and surface ownership remain; Complete shares resolution error presentation | Fresh 13-test focused pass, including shell Complete success/rejection | pass locally |
| Configured new controls | Actual configured app renders the shared contract and supports human boundaries | CLI returns the new fields; configured app controls were not observed | One owned launch, AX trust true, one reported AX window, no count control within 12 seconds | gap; cause unestablished |
| Full navigation/retention | A/D, exact Session selection, nested panes, drafts and scroll survive | Prior configured navigation/picker/viewport evidence and native retention remain bounded evidence | Iterations 7–14; no new configured keyboard interaction in this review | gap for remaining configured nested input |
| External edit and trials | Human-selected external workflow, one authorized edit, ten configured trials | Editor and shared PM path exist; selected external workflow/edit are still absent | Prior editor tests/Cancel/injected rejection receipts only | gap |
| Performance | Published scoped paint/interaction budgets, twenty long-lived-registry trials | Prior fixed-population count measurements are narrower evidence | Iteration 7 six-run comparison; no new budget series | gap |

## Fixed finding

`SessionsStore.decideFlow` assigned `.failed` on rejection, the same enum case
used for failed opening. A retained surface then existed while `SessionItem.surface`
returned nil; the next inventory refresh restored `.live` and discarded the error.
The new test failed both decision cases before the fix, including its visible-error
assertion after polling. This is a reproduced defect, not a hypothetical risk.

Generalized the existing `completionError` to `resolutionError` for Complete,
Approve and Iterate. The pane derives its associated Sessions once, displays
resolution errors independently of opening state, and retains Complete controls
where applicable. No new store, wire field, lifecycle or process owner was added.
Opening errors remain separate. Retry clears the preceding resolution error;
success removes the Session through the existing reconciliation path.

Focused command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter 'WorkspaceNavigationProofTests/(rejectedFlowDecisionRetainsTerminal|shellSessionCompletion)|SessionsStoreTests'
```

**13 tests pass**, including two Flow decision cases and two shell completion
cases. The native fixture mounts the real workspace and Ghostty PTY, injects only
the CLI rejection, verifies visible error retention and enabled retry controls,
and reads the same child's response afterward. It calls the existing store
operation directly; it does not prove configured dialog submission, provider
Ready/settlement, or caller release. No executable edits followed the pass.
[Before](configured-ui-evidence/iteration15-review/decision-before.log) ·
[after](configured-ui-evidence/iteration15-review/decision-after.log).

## Configured boundary

The existing signed `/tmp/loo291-iteration15/Loopflow Shared Sessions.app` and
matching local CLI were used with the explicitly aligned development Home from
the implementation receipt. No fixture mode or installed-app replacement.
The probe requested activation, enumerated AX elements by equality rather than
hash alone, and saved its observed tree. PID 85856 reported AX trust true and one
AX window; it remained inactive. No `podium-sessions` value was observed within
12 seconds. The saved 203-node traversal consists of application/menu elements;
it does not establish a rendered workspace or a permission failure. The process
accepted termination and was absent afterward. No AX action, keyboard input,
provider launch, Session open, transfer or resolution occurred.
[Probe/log/tree](configured-ui-evidence/iteration15-review/).

This bundle predates the review's resolution-error correction. Its failed
observation is not relabeled as validation of the corrected build. The fresh
read-only CLI query returned four real interactive Sessions; production Swift
DTO decoding/round-trip and CLI labels/reasons agree, including exact Task Work
identity. This supplies four comparisons, not the required twenty or Ask/Flow
interaction. [Decode receipt](configured-ui-evidence/iteration15-review/decode.log)
and [source/binary receipt](configured-ui-evidence/iteration15-review/receipt.json).

## Ownership and disposition

Negative searches retain one Podium caller per roadmap, Session and process
inventory and one production root workspace registry. Removed Session scope,
context/group/row wrappers, PodiumConsole, hierarchy loader, FlowResolutionAction
and completion-help policy remain absent. No new table, writer, fallback reader
or dual write exists. Work attribution, checkout location and actual attachment
remain independent. Prepared commands, last-good reads, local opening and
resolution errors retain different lifetimes. The two Rust decision checks
protect different moments; neither is a competing action authority.

The design forbids implicit launches on selection, hidden unmatched Sessions,
healthy emptiness after failed reads, terminal resets during inspection and a
second planning/Session/workspace authority. This correction preserves those
boundaries and supplies a coherent next slice. No broader redesign is required.
`git diff --check` passes. No broad gate or Rust rerun was warranted by this
Swift-only correction; producer hashes match the prior passing receipts.

Publication remains pending under [review-slice](/Users/jack/.agents/skills/review-slice/SKILL.md):
“When all applicable `Done when` claims hold and the slice is coherent, publish or
refresh the Task PR with `lf pr publish`.” Configured contract controls and the
remaining configured retention claims do not yet hold. Next work should establish
the owned window's actual workspace/controls before creating another trial
Session, then exercise fresh owned Ask/Flow controls and nested input. Do not
repeat a blind count timeout or replay retired provider probes. The human-selected
external workflow and exact edit remain required inputs for the external trials;
local Loopflow receipts cannot substitute for them. Publish scoped budgets before
scoring the twenty-trial series. No commit, publication, landing or Task completion
occurred in this review.
