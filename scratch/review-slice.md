# Unified navigation slice review — 2026-09-23

## Disposition

Advances the accepted design; **not yet approved for publication**. The compact
navigator and shared identity join have focused behavioral evidence. Mounted
native navigation now preserves splits, draft, focus and terminal viewport in
the integration fixture. Configured provider interaction/resolution, navigator
scroll and before/after timing comparisons remain unproven. No PR
publication, landing, Task completion, PM mutation or external-product trial was
performed by this review.

Scope is the first navigation slice from `main-view-task.md`, not the full
LOO-291 directive. Bounded conversations, Task-directive editing, LOO-284 shared
actions/display path, lf-new's nested workspaces, external trials and measured
budgets remain explicitly outstanding. No new kickoff or alternative design is
needed.

## Iteration 2 review — 2026-09-23

Reviewed HEAD `7105903ec` plus the bounded test correction below. Recovered the
complete Task patch with `lf task diff LOO-291 --json`: `truncated: false`,
5,020 lines; `/tmp/loo291-review-iteration2.json` and the extracted `.patch`.
Compared the full change set with the previous reviewed state and read the
intervening delta: one mounted native test and notes, no production changes.
Rechecked the reachable navigator, detail, reading, Session-action and native
ownership paths against the accepted design and its forbidden outcomes.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Native A/D and detail navigation | Preserve surfaces, split, focus, draft and scroll | Same Session and companion PTYs survive toolbar navigation and repository return | `workspaceRetainsNativeSplit`, final focused command below | pass, native integration fixture |
| Retained children remain interactive | Input reaches both original children after navigation | Both cat replies are required in addition to PTY input echo | Strengthened final assertions | pass, local children; provider continuation remains a gap |
| Current planning and Session evidence | Read shared planning and exact Session identity | Live CLI reads expose Product, two Projects, nine incomplete Tasks and two unbound interactive Sessions | `/tmp/loo291-review-iteration2-{roadmap,sessions}.json` | pass, read-only CLI; no live Task-to-Session association in this population |
| Complete configured interaction | Real controls, provider continuation, resolution and pane reconciliation | No new configured interaction result; prior AX probe could not reach workspace controls | Earlier AX receipt below; fixture actions use ViewInspector | gap |
| Navigator scroll and timings | Preserve list position and compare paint/readiness on the same population | Terminal viewport is covered; navigator scroll and timing comparison are not | Source and fixture coverage inspection | gap |
| One shared authority | One planning/Session reader and retained workspace owner | Removed paths remain absent; LOO-284 fields remain unavailable | Negative searches and current Rust/Swift/main/lf-new comparison | pass, source; full contract integration remains Task scope |

The native test originally accepted any occurrence of `companion-alive`. PTY
input echo alone could satisfy that assertion without proving the companion
child responded. Replaced it with the same two-occurrence requirement used for
the Session draft, and waited for both responses within the existing deadline.
This is a stronger behavioral assertion, not a production correction or a new
launch path. Both children and surfaces remain test-owned.

Final focused command:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter WorkspaceNavigationProofTests/workspaceRetainsNativeSplit
```

One test passed, exit 0; `/tmp/loo291-review-mounted-child-proof.log`. No Swift
edits followed. No affected-suite or full gate ran. Existing model/selection,
manual-focus and window-isolation receipts remain evidence at their stated
levels; they were not rerun. `git diff --check` passes.

Live roadmap generation: `2026-09-24T00:31:44.394125Z` (23 September locally).
The current two-Session observation supersedes the earlier three-record count;
it does not establish why the third record disappeared. No live Session was
opened, moved, resolved or otherwise mutated. The prior AX failure is historical
evidence, not a fresh permission diagnosis. Repeating a launch-only capture would
not prove the missing interaction, so this review used the corrected native
fixture as the closest local behavioral path.

Rust/Swift Session and roadmap mirrors retain the same fields. Main and lf-new
SessionRecord files still match the hash below; shared legal actions/display
path are absent. Searches find one Podium caller per inventory read, one root
workspace registry, and none of the removed scope/navigation types. No second
writer, fallback inventory, lifecycle policy or workspace owner was introduced.

Next work is the configured interaction trial below, including navigator scroll
and same-population timing observations. Optional conversations, required lf-new
workspace integration, directive editing, LOO-284 integration, external proving
work and measured budgets remain the full Task target. The new fixture advances
the first slice without changing that target. Publication remains withheld under
review-slice's requirement that all applicable Done When claims hold.

## Earlier review after native restoration — 2026-09-23

Reviewed HEAD `39d4c4653` using the complete Task diff (`truncated: false`,
4,736 lines), the changes since the previous review, and the reachable owners.
Receipt: `/tmp/loo291-review-restored.json`; extracted patch:
`/tmp/loo291-review-restored.patch`. The native changes advance the accepted
design: one retained view owns the focus request, and the CoreVideo adaptation
uses Ghostty's existing renderer. No further bounded source correction was
established in this review.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Real planning paints | Compact Wave/Project grouping with all incomplete Tasks | Product, Desktop and Company Dogfood render nine Tasks from the actual registry | Temporary bundle, live capture; `review-live-planning.png` | pass, real-data rendering only |
| Failed reads stay explicit | Missing evidence cannot look like healthy emptiness | Bare executable shows Unknown, Sessions unavailable and Planning unavailable when its bundled helper is absent | `/tmp/loo291-review-live.png`, inspected image | pass, real failure rendering |
| Native draft/focus retention | Preserve input through hiding and resizing | View-owned request also preserves manually acquired Task-terminal focus | Existing final compression receipt, one real-PTY test | pass, fixture; no new test run |
| Complete navigation interaction | A/D, exact Session access, split/draft/scroll retention and resolution | Source and focused fixtures support it; current external automation did not reach workspace controls | AX/capture attempts below | gap; publication remains unapproved |

Fresh read-only `lf roadmap --json` and `lf session list --json` succeeded.
The roadmap receipt is generated at `2026-09-24T00:13:52.840193Z` (23 September
locally). Sessions now contains **three** records: two active unbound interactive
Sessions and one ready Task FlowStep whose Work is absent from the Product plan.
The earlier two-Session observation below is historical. No Session was opened,
moved, resolved or modified by this review.

Attempted the actual built app before falling back to the existing live capture
mode. `AXIsProcessTrusted()` and `CGPreflightScreenCaptureAccess()` return true
for the probe process. Nevertheless, AX traversal of this review's app returns
application/menu elements even through `AXWindows`, without workspace controls;
setting `AXManualAccessibility` returns `-25205`. External capture of its own
window reports `could not create image from window`. These observations do not
establish the cause or prove that the hosted UI runner has permission. Receipt:
`/tmp/loo291-review-interaction-ax.log`. No blind clicks or global keystrokes were
used to work around the inaccessible controls.

The app's existing `live` capture mode works. The initial bare SwiftPM launch
correctly reports the missing bundled `lf`; it is not a configured-app verdict.
A temporary `/tmp/loo291-review/Loopflow Review.app` then used this checkout's
unchanged SwiftPM executable/resources and the existing development control
configuration pointing to the same installed `lf` used by the live CLI reads.
Its distinct bundle identifier avoids replacing the installed app. With
`LOOPFLOW_UI_TEST_MODE=live`, a snapshot path, a 12-second capture delay, and
`--repo /Users/jack/src/loopflow.main-view-task`, it renders the real scoped plan,
three-Session count, and planning-read timestamp. No fixture records are injected.
The capture delay is not a measured paint time or budget. This proves static
rendering, not interaction, native terminal rendering or external-product use.
All review-owned app processes exited; no user app was stopped.

Rechecked Rust/Swift Session and roadmap fields and current main's Session DTO;
the three Session mirrors retain the hash recorded below. Main and lf-new still
have no shared legal-action/display-path contract to consume. Negative searches
still find one Podium inventory caller per shared read, one root workspace
registry, and none of the removed scope/navigation types. No new persistence,
writer, compatibility path or launch authority was introduced.

No production or test source changed, so no tests were rerun. The final manual
focus correction's receipt is `/tmp/loo291-compress-manual-focus-after.log`;
the previous 25 model tests and independent native window-isolation receipt
remain applicable at their stated proof levels. `git diff --check` passes.

Next work remains the configured interaction trial described below, including
retained split/scroll state and a disposable authorized human boundary. Restore
access to actual workspace controls on that host before attempting it; another
launch screenshot does not close that gap. Full LOO-291 scope is unchanged.

## Evidence matrix

Pass describes the stated proof level; model fixtures do not establish native
interaction.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Unified inventory | Incomplete autonomous, upcoming and human Tasks share compact Wave/Project groups | Derived planning rows, stable planning IDs, ranked Tasks, no Session prerequisite | WorkspaceNavigationTests.workDoesNotRequireSessions; navigator source | pass, model; configured appearance gap |
| Exact Session association | Typed durable Work edge; never name/cwd/planning-ID guesses | Joins runtime.workId by kind; retains multiple and unmatched Sessions | everyHumanBoundaryRemainsReachable; current Rust/Swift SessionRecord comparison | pass, fixtures |
| Completed and top-level work | Completed-with-Session, repo/Wave/Project and missing planning remain reachable | Completed rows retained when associated; unmatched records remain in Other open Sessions | everyHumanBoundaryRemainsReachable; live read has two unbound Sessions | pass, model; live continuation gap |
| Details and no-Session | Directive, condition/reason, Project definition/KRs, Activity, PR/worktree and explicit absence | Existing inspectors/actions reused; zero only after a readable association | inspectorShowsPlanning, unavailableAssociationIsNotNoSessions; WorkSurfaceView/subjectSessions source | pass, model/view |
| A/D presentation | Toggle list without recreating workspace or launching provider | Same retained workspace and mounted multiplexer; presentation state separate | navigationRetainsWorkspace; workspaceRetainsNativeSplit | pass, model/native fixture; configured interaction gap |
| Repository and search retention | Restore selection/expansion/search/splits/focus across navigation | Existing registry and per-repository navigation, expansion preserved during search | navigationRetainsWorkspace; unavailable/truncated regression; mounted native repository return | pass, model/native fixture; configured search and navigator scroll gap |
| Unavailable evidence | Failure is never healthy empty; keep useful context | Last-good readings/errors; unknown badge; partial planning warning; selected Work retained | unavailableIsNotEmpty, lastGoodSessionsSurviveRepositorySwitch, new regression | pass, model/view |
| Responsive Session access | Planning cannot gate Session publication/opening | Session read publishes independently; pane selection precedes asynchronous preparation | sessionsArriveBeforePlanning; openSession source | pass, model; measured latency gap |
| Human lifecycle | Exact open/Move here; resolution does not complete Task | Existing shared operations; callback removes resolved Session from reading | Existing SessionsStore focused receipt; resolutionKeepsTask | pass, fixtures; configured resolution gap |
| Native lifetime and input | Same surfaces/processes/drafts/splits; hidden terminals relinquish input | Retained pool; native view consumes focus requests on attachment/transition; unavailable CoreVideo uses timer rendering | hiddenTerminalPreservesDraft, releaseSurfaceIsWindowLocal and strengthened workspaceRetainsNativeSplit | pass, real PTY fixtures; configured provider interaction gap |
| One authority | Remove root switch/cascade, duplicate Session polling and labels lookup | Podium reads; projection derives; one window registry retains surfaces | Negative searches and complete Swift diff review below | pass, source |
| Source freshness | Distinguish read timestamp from provider sync freshness | Toolbar labels snapshot generation and explains unavailable sync timestamp | SessionsView toolbar, README | pass, honest limitation; full freshness integration remains |
| External trials/budgets | Human-selected workflow, authorized edit, measured long-lived-registry trials | Not implemented/proven in this slice | No workflow selected; no timings collected | gap, remaining Task scope |

## Bounded corrections made

The existing per-repository selection was cleared on return after a successful
roadmap response containing unavailable planning. `setRepoPath` called
`clearSelectionIfOutsideScope`, which treated an unresolvable saved Task as
absent. The new test reproduced `model.selection == nil` after switching away
and back. Switching now restores its existing navigation state without that
redundant clearing. Complete planning refreshes still reconcile removed Work.

The refresh check also accepted truncated planning as complete. It now requires
available, untruncated Projects and no unavailable Project entries before
clearing a missing selection. The same regression covers unavailable and
truncated responses, retaining the selected Task and its unmatched Session.
This changes no lifecycle or ownership boundary.

## Commands and observations

- Read the complete Task patch through `lf task diff LOO-291 --json` (reported
  `truncated: false`), including committed implementation and compression edits.
  Local receipt: `/tmp/main-view-task-review-diff.json`; extracted Swift diff:
  `/tmp/main-view-task-review-code.diff`.
- Live read-only `lf roadmap --json` and `lf session list --json` succeeded.
  Roadmap generated at `2026-09-23T23:42:34.733612Z` contains Product, two
  Projects and nine incomplete Tasks. Sessions contains two unbound records and
  no Work-bound Session. These are configured CLI observations, not proof of a
  live Task-to-Session UI trial. Raw observations remain in
  `/tmp/main-view-task-review-{roadmap,sessions}.json`; no identities or outcomes
  were fabricated to fill the missing population.
- Pre-fix regression: one test failed at the saved-selection assertion.
  `/tmp/main-view-task-review-regression.log`.
- After repository restoration fix:
  `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
  'WorkspaceNavigationTests|PodiumModelTests'` passed 25 tests, exit 0.
  `/tmp/main-view-task-review-proof.log`.
- Final partial-plan correction:
  `swift test --package-path swift -Xswiftc -gnone --jobs 4 --filter
  WorkspaceNavigationTests/unavailablePlanningPreservesRepositorySelection`
  passed one parameterized test with both unavailable/truncated cases, exit 0.
  `/tmp/main-view-task-review-partial-proof.log`.
- Reused the prior 35-test compression receipt for unchanged Session actions;
  did not run the broad gate. Final source includes the subsequent two bounded
  selection corrections with the focused receipts above.
- Re-read native failure receipts: the new draft test has a nil terminal surface;
  unchanged GhosttyTerminalInputTests.releaseSurfaceIsWindowLocal likewise fails
  before behavior assertions. Logs: `/tmp/loo291-native-focus-proof.log` and
  `/tmp/loo291-existing-native-proof.log`. The subsequent diagnostic in
  `native-surface-diagnostic.md` supersedes the hypothesis that this process has
  no rendering environment: it sees a screen and Metal device. Ghostty reports
  `error.OutOfMemory`, also reproduced by a minimal AppKit host without user
  configuration or navigation code. The allocation failure's cause remains
  unknown. Hosted UI test sources were inspected but not executed.

## Negative architectural proof

Searches under the reachable Mac root and tests find no SessionScope,
SessionContext, SessionGroup, SessionRowItem, PodiumConsole, PodiumSurface or
_loadHierarchy. `query.sessions` and `query.roadmap` in the desktop path each
have one owner, PodiumModel. SessionsStore performs existing Session actions
and maintains local prepared/opening/error state; it no longer polls or resolves
labels. The root constructs one window-local SessionsWorkspaceRegistry.
WorkspaceProjection has no persistence/writer/launch operations. No schema,
backend DTO, migration, compatibility adapter, planning write or new Session
legality matrix was added. Existing Task controls remain in inspectors; removed
console fleet switches were not restored.

The SessionRecord files in this checkout, canonical main and lf-new are
byte-identical (SHA-256
`e020dd7922d735ce2406dbbd38485b372ec8340b57891ec6bc7da05fb005485d`).
The Rust mirror still has the same fields, with no projected legal actions or
display path. This slice preserves the existing native action path; consuming
LOO-284 remains a real integration requirement. Sibling checkouts were read
only. The existing repository workspace is retained, not advertised as lf-new's
unimplemented per-worktree layout.

## Next proof, without redesign

Ghostty surface initialization is restored; the previously failing native
draft/focus and window-isolation tests now pass. See `native-surface-diagnostic.md`
for the CoreVideo failure, timer-rendering adaptation, and focus correction.
These focused fixtures do not waive configured proof or establish visual quality.

Use the configured native app on a rendering-capable, permissioned host. In one
repository show autonomous/upcoming/human Tasks; open the exact existing Session;
retain an unfinished draft in a split beside a running shell. Inspect Task and
Project details, toggle A/D, search and collapse groups, switch repositories and
return. Verify the same surface/process identities, draft, layout, focus and
scroll positions. Confirm search input remains in the field during polling and
resize, hidden terminals receive no input, and explicit Continue restores focus.
Complete a disposable authorized Session and prove pane reconciliation without
Task completion. Do not transfer or resolve the human's current Sessions merely
to manufacture evidence. Retain the native regression alongside that configured
trial. Until this succeeds, the skill's publication condition is unmet.

## Second review after native diagnosis — 2026-09-23

Disposition remains **not approved for publication**. Re-read the complete
current Task patch via `lf task diff LOO-291 --json` (`truncated: false`), comparing
its source delta with the already reviewed patch. Receipts:
`/tmp/main-view-task-review-current.json` and
`/tmp/main-view-task-review-delta.diff`. The intervening implement/compress passes
added diagnosis and review notes, without production changes. The claim matrix
above still applies, with this additional corrected selection path:

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Portfolio refresh preserves Work context | Repository discovery cannot declare Work removed during partial planning | Removed discovery's redundant selection reconciliation; complete planning refresh still reconciles | Extended unavailablePlanningPreservesRepositorySelection, both cases; PodiumModelTests | pass, model |

The regression first failed for both unavailable and truncated planning after
`refreshPortfolio(initialRepoPath: nil)` cleared the selected Task. The final fix
removes that one reconciliation call. Explicit selection validation and complete
planning refresh retain their existing behavior. Moving the completeness check
into every selection call was rejected after the existing missing-selection test
caught its changed behavior; that approach is absent from the final source.

Final focused command passed 25 tests, including both partial-planning cases:

```sh
swift test --package-path swift -Xswiftc -gnone --jobs 4 \
  --filter 'WorkspaceNavigationTests|PodiumModelTests'
```

Exit 0; `/tmp/main-view-task-review-portfolio-final.log`. Pre-fix failure:
`/tmp/main-view-task-review-portfolio-before.log`. Intermediate rejected fix:
`/tmp/main-view-task-review-portfolio-after.log`. No production/test source edits
followed the final pass. This is local model/view evidence; no native or external
trial is included in the count.

Repeated negative searches confirm no removed navigation/scope types returned,
one Podium caller for each shared Session/roadmap inventory read, and one root
window registry. No launch path, DTO, schema, migration, Session action policy,
or native ownership changed. The slice advances the accepted design, with the
same native proof boundary and remaining LOO-291 scope. Nothing was published,
landed, or marked complete.
