# Folded return detail and recursive keyboard review

2026-09-27 · LOO-303 · review-slice

The focused repair passes local review. No new product defect was found and no
product code changed in this review. The numeric-target finding in the
[concept review](concept-review.md) is resolved by executed mounted proof, as is
the previously missing recursive keyboard proof. This advances the approved
template model; it does not satisfy the full Task's configured acceptance.

Reviewed the complete active diff from `c832aaede` through `792a3ce40`, with
particular attention to the repair after `587d8fe2f`. Preserved the supplied
compression note through `lf commit` at `b87018173` before authoring this note.
The [design](workspace-ux-on-data-model.md), [interactive feedback](workspace-ux-review-feedback.md)
and Jack's light-only/no-teardown direction govern. The
[implementation receipt](workspace-return-proof.md) retains the original
failures and repairs; the [template review](workspace-template-review.md)
retains the earlier navigation, CLI and resolver evidence.

## Evidence matrix

| Claim | Planned behavior | Implemented behavior | Proof | Result |
| --- | --- | --- | --- | --- |
| Folded return detail | Both returns name the original target through fold/expand/fold, including XOR | Original graph supplies detail; projection supplies layout and endpoints | Fresh mounted production Task view: root `implement`, nested `fix-implement`, both distinct loop labels at each disclosure state | Local pass |
| Recursive keyboard | Tab/Shift-Tab reach nested, empty and XOR disclosures; arrows expand/collapse; Space/Return toggle | One focusable button uses each DisclosureGroup's existing expansion binding | Fresh dispatched AppKit events assert exact focused labels, expansion and visible empty content | Local pass; installed key-loop behavior remains unverified |
| Captured detail | Target name and saved traversal counts retain execution meaning | Captured graph remains the detail source; recursive lookup names targets | Fresh captured-node inspection requires `Iterate returns to implement · taken 1×` for both returns | Local pass; exact attempts remain later work |
| Inspection/input | Inspection invokes no Flow control; hidden terminals reject focus; drafts and surfaces survive | Existing enabled input crosses into AppKit responder eligibility; focus request remains separate | Fresh proof checks unchanged PTY buffers during disclosure, no control requests, same two surfaces, draft and child replies; palette/Monitor/paste/pointer neighbors pass | Local pass with fixture transport and owned cat PTYs |
| Done When 1–2: navigation/links | Exact destinations, retained input, one-window routing, installed cold/warm links | Existing palette, exact reader and generation-fenced router retained | Fresh palette proof; earlier exact CLI and mounted race receipts retain their scope | Local evidence; installed Launch Services gap |
| Done When 3: templates | One resolver, distinct compositions, empty/XOR paths, both returns, Project default on Wave | Shared resolution and template view retained; detail and keyboard repaired | Fresh model/native proofs; prior Rust resolver/DTO and CLI catalog/default receipts | Local pass; configured appearance still owed |
| Done When 4: attempts | Ordered/current Runs at exact position in detail, status and chip; retained children/history | Run still lacks position/attempt projection; status still reads Task time/provider evidence | Source audit of `session.rs`, `TaskFlowView` and captured consumers | Gap: parent contracts and attempt slice |
| Done When 5–6: room/bind/races | Every null-Task orphan tiled, one mount, permanent target confirmation, selected-Run fencing | No Bind command; unmatched-roadmap grouping and hidden checkout host remain | Source audit of Session command/list and workspace consumers | Gap: parent contracts, room/bind and all bind races |
| Done When 7: configured acceptance | Installed candidate, selected Home/identities, providers/drafts, live captures at both widths, Jack's verdict | This review runs local fixture-backed native views only | No installed activation, live provider interaction or walkthrough performed | Gap |
| Done When 8: consolidation/docs | Shared authorities, obsolete consumers removed only after replacement | No new resolver, label store, expansion owner or terminal pool; README matches repair | Reachable-call-site audit; platform/whitespace checks; architecture inventory | Slice ownership pass; final deletion and inherited owner-map gap remain |

## Executed checks

Logs are under `.lf/tmp/workspace-navigation/return-review-*`.
Resource preflight passes with **93.7 GiB free**, this checkout's build outputs
**5.1 GiB / 12 GiB**. The existing `check.py` wrapper supplies fresh LF authority,
clears inherited LF execution identity, limits verification to four workers,
uses nice +10, and kills the process group at 900 seconds. No timeout fired.

```sh
uv run python .lf/tmp/workspace-navigation/check.py \
  swift test --package-path swift --jobs 4 --no-parallel -Xswiftc -gnone \
  --filter 'TaskFlowTests|TaskFlowProofTests|SessionChromeProofTests/paletteRetainsTerminalInput|TaskMonitorTests/mixedMonitorRetainsInput|GhosttyTerminalInputTests/pasteShortcutFollowsFirstResponder|GhosttyTerminalInputTests/pointerFocusFollowsClickedPane'
```

**9 tests in 5 suites pass**, 74.5 seconds including build and 67.3 seconds
executing (`return-review-swift.log`). Swift is Apple **6.2.3**. This includes
the mounted workspace journey, not just model assertions. Return-node activation
uses the mounted accessibility press action; disclosure keys are dispatched
AppKit events. The unhosted fixture explicitly recalculates the key-view loop
and materializes bitmap content for accessibility inspection. That is local
interaction evidence, not installed Launch Services, compositor measurement or
visual acceptance. Ghostty uses its existing timer-renderer fallback.

Swift multiplatform boundaries and `git diff c832aaede --check` pass.
The architecture checker again reports **32/33 SQLite owners**, missing
`wave_chapters`; all seven other inventories pass (`return-review-architecture.log`).
The check remains failed. Its omission is inherited parent-conversion work;
this review does not repair it by relabeling target-model documentation.

No executable, fixture or user-documentation bytes changed after `792a3ce40`.
Inspected the retained successful `return-xcode.log`: app and test-runner
build-for-testing passed in 235.9 seconds. Reuse that compile receipt for these
unchanged bytes; no Xcode or hosted UI run was repeated here. Earlier Rust
resolver/DTO/default and exact-lookup receipts retain only their named scope.
No new Rust behavior result or whole-branch gate is claimed.

## Source and ownership review

`TemplateDiagram` passes the original graph as `detailGraph`, while projected
nodes and remapped `LoopSpan` endpoints still drive `FlowDiagram` layout.
`FlowNodeDetail` resolves targets with the existing recursive `FlowGraph.node`.
Captured rendering omits the extra detail input and uses its own complete graph.
No return key, traversal count, template DTO or captured execution is rewritten.
The original and projected graphs answer different questions; merging them would
lose hidden descriptive facts again. No copied target labels or new YAML reader
are reachable.

The recursive disclosure style consumes `configuration.isExpanded`. Composition
groups retain the repository navigation's revision-keyed set; XOR disclosure
retains local SwiftUI state. Each level uses the same button for pointer and
keyboard input. There is no parallel keyboard expansion store or application
key-loop manager. Empty groups remain reachable even without diagram nodes.

`GhosttyMetalView.updateFocus` now carries the existing enabled input into
`acceptsFirstResponder` and `becomeFirstResponder`. Requested focus remains
separate so a visible unfocused pane can still receive a click. Surface creation,
destruction, pool ownership, native client authority and shell identity are
unchanged. Neighboring pointer, paste, Monitor and palette proofs exercise this
shared boundary. No terminal is released merely because Task details appear.

Followed shared resolution through catalog/fixtures into Task, Wave and palette
views; exact roadmap lookup through RegistryQuery and destination generation;
and Session/Run consumers behind the deferred work. The old `previewGraph`,
`TemplatePathDiagram` and `FlowTemplate.project` have no source callers.
The Session union in `ops/human_session.rs::list`, Work/path DTOs,
`WorkspaceProjection.unmatchedSessions`, reverse lookup and opacity-hidden
checkout host still have live consumers. `SessionCommand` has no Bind and
`session.rs::Run` has no node/iteration/ordered-current-attempt projection.
Those are explicit unfinished parent/replacement dependencies, not new adapters
introduced by this repair. Deleting them now would remove capability.

## Next useful work and limits

The crossing-return and recursive keyboard cases no longer need another
implementation pass. Preserve their focused proofs when integrating the parent.
Next, verify the actual LOO-298 contract checklist: all-kind stable Session/current
Run, typed ancestry/repository, historical lookup, confirmed selected-Run bind,
shared Started, ordered/current attempts, retained invocations and taskless
legality. Table presence alone cannot release room/bind or attempt implementation.
Do not rebuild the parent's storage here.

Installed cold/warm Task links, configured provider/draft retention, exact
attempts including the independent-Run counterexample, all orphan tiles/shared
shells with one mount, bind confirmation/races, live 1440×900 and 1100×800 captures,
Jack's verdict and final deletion/documentation reconciliation remain owed.
Current-Run-only binding stays an explicit inherited assumption.

No publication: applicable configured acceptance and full-design obligations
remain incomplete. No installed app, live Home, provider, Task disposition or
external message was changed. This review selects no Flow navigation edge.
