# Navigation repairs and template slice review

2026-09-27 · LOO-303 · review-slice

Reviewed the complete active diff from `c832aaede` through `3f84174d0`, the
supplied compression checkpointed as `da64c5ee3`, and the review changes below.
The [approved design](workspace-ux-on-data-model.md), Jack's evening scope
correction and [interactive feedback](workspace-ux-review-feedback.md) govern.
The [implementation receipt](workspace-template-proof.md) and
[earlier navigation review](workspace-navigation-review.md) retain their stated
proof limits. This review advances neither the Flow nor Task disposition.

The independent navigation and template slices advance the approved model.
The two concept-review defects are repaired and exercised locally. One status
overclaim found in this review is also repaired. Publication still requires the
outstanding applicable acceptance; the parent contract checklist still gates
attempts and room/bind. No installed acceptance or parent completion is inferred.

## Finding and repair

**Started did not prove independent Run membership.** The new status sentence
`Flow not started · independent Runs exist` used only `task.runtime.started`
and the absence of a recorded Flow. The current shared roadmap still combines
the Store's Started evidence with PR publication/merge evidence
(`rust/loopflow/src/lf/commands/waves.rs::snapshot_task_detail`). Historical
missing capture likewise does not prove independence. Even the final any-Run
Started contract would not supply individual membership by itself.

Extended the mounted Task Flow proof to visit the fixture's started Task with
no Flow record. It failed on the status sentence before repair. The view now
says **No Flow recorded**, keeps the folded template, and shows no iteration.
The same proof passes after the one-line correction, with no control invoked
by inspection and the retained terminals still checked. No new membership
heuristic, historical rewrite or early parent conversion was added.

**Failed-inventory activation now has an executed proof.** The implementation
receipt established last-good row retention, but its test then replaced the
inventory successfully before pressing Return. Extended the existing dispatched
AppKit proof to press Return while the inventory remains failed: the retained
Session opens with its exact shell surface and keyboard focus. Reopening the
palette and then removing all matches still makes Return a no-op; Escape
restores the prior terminal. This required no production change.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| 1: navigation | Exact Wave/Task/Session activation, recents, stale rows, retained input and two windows | One effective selection; bounded per-repository descriptors; exact historical readback; existing Session dispatch | 16-test Swift filter, followed by expanded failed-inventory Return proof; owned cat PTYs and mounted production views | Local pass; configured provider acceptance remains separate |
| 2: Task links | Exact historical/completed/unstarted lookup without starting work; one window, late-result fencing; installed cold/warm delivery | Shared roadmap filter, selected-page evidence, generation fence and one-window router | Fresh mounted two-link/click proof; unchanged prior actual CLI no-start and collision receipts | Local pass; installed Launch Services `open` remains a gap |
| 3: templates | Distinct repeated/empty composition uses, XOR, both returns, revision reset, Task/Project default presentation | One Rust resolution tree and execution flattening; graph-referencing template DTO; shared Swift disclosure | Fresh actual CLI catalog on repository and disposable definitions; Swift fixture/native tests; unchanged Rust resolver/DTO/default receipts | Local pass; recursive keyboard interaction and live appearance at both widths remain unproven |
| 4: invocation/attempt | Exact current Run, ordered attempts, historical/child invocation selection, source-independent capture | Captured graph remains separate; attempt/current-Run projection is still absent | Source audit of `session.rs`, `TaskFlowSnapshot`, running line and Session chip | Gap: parent contracts and attempt slice |
| 5: room/bind | Every null-Task orphan tiled, one native mount, universal exact-target confirmation, draft retained | Existing grouping/terminal hosts remain; no Bind command exists | Session command and current consumers inspected | Gap: parent contracts and room/bind slice |
| 6: races | Replacement/bind/poll/repo-switch/Undo preserve exact identity | Navigation races are fenced; bind lifecycle is not implemented | Mounted navigation race passes; source shows no bind writer or picker | Gap for all bind races |
| 7: configured acceptance | Installed candidate, selected Home/IDs, providers/drafts, 1440×900 and 1100×800 captures, Jack's verdict | This review uses source CLI in isolated authority and fixture native views | No installed activation, provider interaction, capture or walkthrough performed | Gap |
| 8: consolidation | Shared readers/actions; delete replaced presentation without a competing authority | Old flat-only template preview and duplicate template diagram path removed; execution diagram retained | Complete diff and reachable-call-site audit below; architecture checker | Local ownership pass; final deletion and inherited `wave_chapters` map gap remain |

## Configured source path and local checks

Logs and the disposable CLI driver are under `.lf/tmp/workspace-navigation/`,
prefixed `template-review-`. Every product command uses the existing isolation
wrapper: fresh LF authority, no inherited LF execution identity, nice +10, at
most four build workers and a 900-second process-group limit. No timeout fired.
Resource preflight passed with 94.4 GiB free before execution.

- Built the current `lf` in 30.0 seconds. Binary SHA-256:
  `b62355336768f248641fc0b99296b51da7b256198f6ee1252e3ad53276b743d2`.
  Actual `lf flow list --json` from this repository reports 13 `feature` steps,
  distinct `task-design`, `pursue` and `queue` groups, and two returns.
- The same CLI in a disposable definition directory proves separate repeated
  uses, an empty group, both XOR alternatives, stable unchanged revision,
  changed revision after source edits, and named unavailable results with null
  graph/template on cycles and missing sources. All checks pass in 3.0 seconds.
  No Run manifest is created. This is real CLI/resolver evidence with isolated
  definitions, not a configured provider run or a desktop transport proof.
- Initial Swift filter `WorkspaceDestinationTests|SessionChromeProofTests/paletteRetainsTerminalInput|TaskFlowTests|TaskFlowProofTests`:
  **16 tests pass**, 17.6 seconds including build, 16.1 executing.
- New Started/missing-Flow assertion: fails before repair
  (`template-review-membership-before.log`, 12.4 seconds). After repair the
  five-test Task Flow filter passes in 13.3 seconds, 7.5 executing
  (`template-review-membership-fixed.log`). The first failure remains evidence.
- Extended failed-inventory Return proof: **1 test passes**, 14.2 seconds
  including build, 9.8 executing (`template-review-stale-activation.log`).
  Search/Return/Escape are dispatched events; provider input uses owned cat
  PTYs. These are not live provider or compositor measurements.
- Swift platform boundaries and whole active-PR whitespace pass. The architecture
  checker reproduces **32/33 SQLite owners**, missing `wave_chapters`; the other
  seven inventories pass. This is a failed architecture gate.
- Rust resolver, fixtures and shared default bytes are unchanged from
  `3f84174d0`. Reuse the implementation receipt's 49 resolver/graph, 10 DTO and
  one CLI default-projection passes, formatting and Clippy only within their
  recorded scope. No Rust behavioral suite was rerun in this review. Earlier
  exact-lookup and scope-consumer receipts likewise retain their original limits.

- XcodeGen and signed ad-hoc Xcode `build-for-testing` pass in the documented
  fallback configuration, 31.7 seconds for the build
  (`template-review-xcodegen.log`, `template-review-xcode.log`). This compiles
  the app, bundled CLI and test runners; hosted UI tests did not run. Final
  resource preflight passes at 93.5 GiB free.

## Negative architectural proof

Traced `expand_flow` and catalog loading into `resolve_flow` and
`flatten_resolved`. Composition survives the same resolver that applies Skill
lookup, policy, cycle and return validation; no second YAML loader or grouping
by equal parent names exists. Each composition use receives its own local ID.
Template items reference the graph's node keys rather than storing another copy
of node policy. Captured execution still consumes `ConcreteStep`; no template
revision or disclosure ID becomes invocation identity or runtime parentage.

Followed Task preview, current Project Flow on the Wave page and catalog
inspection through `FlowTemplateView` and the shared `TemplateDiagram`. A pinned
invocation still draws its captured graph first. The old `previewGraph`,
`TemplatePathDiagram` and `FlowTemplate.project` have no current callers.
Project default projection uses the existing Task default resolver; there is
no new Wave Flow owner or Wave execution control. User documentation describes
these routes and remains accurate after the status wording correction.

Followed recents through `remember`, `paletteRows`, `openPaletteTask`, exact
readback and destination-generation invalidation. Descriptors hold display and
lookup data, not full historical snapshots. `selectedTaskEvidence` remains the
selected-detail owner. Inspection adds no Task/Run write, preparation call,
Session store, provider launcher or terminal pool. Source uses the same
effective selection for visible highlighting and Return.

The four-source Session inventory, Work/path DTOs, unmatched-roadmap grouping,
reverse Session lookup, hidden mounted checkout host and Task-based running
provider/time remain reachable. They are explicitly deferred dependencies, not
replacement adapters introduced by this slice. `SessionCommand` still lacks
Bind; `Run` still lacks node/iteration/attempt ordinal and current-attempt
projection. Deleting their existing consumers now would remove capability.
Schema presence has not released the parent's contract checklist.

## Remaining proof and next useful work

Keep the complete eight-part design acceptance in force. Before table-dependent
work, integrate and verify the actual LOO-298 all-kind Session/current Run,
typed ancestry, exact lookup, confirmed selected-Run bind, Started, ordered/current
attempt, retained invocation and taskless legality contracts. This review does
not implement or infer them from another checkout's state.

Independent acceptance still needs installed cold/warm Task links and recursive
template keyboard interaction, followed by the configured captures and Jack's
walkthrough. The local template proof establishes topology and bound disclosure
controls, not a visual verdict. After parent integration, prove exact attempts,
one-mount room/shared shells, all bind races and configured draft/provider
retention; then delete the superseded consumers and reconcile final docs.

No PR publication, landing, Task completion, live Home conversion, external
message or Flow navigation was performed. The following deciding step owns
navigation; this note is evidence and recommended work only.
