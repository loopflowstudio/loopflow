# Unified navigation slice review — 2026-09-23

## Disposition

Advances the accepted design; **not yet approved for publication**. The compact
navigator and shared identity join have focused behavioral evidence. Configured
native navigation, focus, draft and scroll retention still lack proof. No PR
publication, landing, Task completion, PM mutation or external-product trial was
performed by this review.

Scope is the first navigation slice from `main-view-task.md`, not the full
LOO-291 directive. Bounded conversations, Task-directive editing, LOO-284 shared
actions/display path, lf-new's nested workspaces, external trials and measured
budgets remain explicitly outstanding. No new kickoff or alternative design is
needed.

## Evidence matrix

Pass describes the stated proof level; model fixtures do not establish native
interaction.

| Claim | Planned behavior | Implemented behavior | Proof | Result |
|---|---|---|---|---|
| Unified inventory | Incomplete autonomous, upcoming and human Tasks share compact Wave/Project groups | Derived planning rows, stable planning IDs, ranked Tasks, no Session prerequisite | WorkspaceNavigationTests.workDoesNotRequireSessions; navigator source | pass, model; configured appearance gap |
| Exact Session association | Typed durable Work edge; never name/cwd/planning-ID guesses | Joins runtime.workId by kind; retains multiple and unmatched Sessions | everyHumanBoundaryRemainsReachable; current Rust/Swift SessionRecord comparison | pass, fixtures |
| Completed and top-level work | Completed-with-Session, repo/Wave/Project and missing planning remain reachable | Completed rows retained when associated; unmatched records remain in Other open Sessions | everyHumanBoundaryRemainsReachable; live read has two unbound Sessions | pass, model; live continuation gap |
| Details and no-Session | Directive, condition/reason, Project definition/KRs, Activity, PR/worktree and explicit absence | Existing inspectors/actions reused; zero only after a readable association | inspectorShowsPlanning, unavailableAssociationIsNotNoSessions; WorkSurfaceView/subjectSessions source | pass, model/view |
| A/D presentation | Toggle list without recreating workspace or launching provider | Same retained workspace and mounted multiplexer; presentation state separate | navigationRetainsWorkspace; SessionsView source | pass, model; native/scroll gap |
| Repository and search retention | Restore selection/expansion/search/splits/focus across navigation | Existing registry and per-repository navigation, expansion preserved during search | navigationRetainsWorkspace; new unavailable/truncated regression | pass, model; native focus gap |
| Unavailable evidence | Failure is never healthy empty; keep useful context | Last-good readings/errors; unknown badge; partial planning warning; selected Work retained | unavailableIsNotEmpty, lastGoodSessionsSurviveRepositorySwitch, new regression | pass, model/view |
| Responsive Session access | Planning cannot gate Session publication/opening | Session read publishes independently; pane selection precedes asynchronous preparation | sessionsArriveBeforePlanning; openSession source | pass, model; measured latency gap |
| Human lifecycle | Exact open/Move here; resolution does not complete Task | Existing shared operations; callback removes resolved Session from reading | Existing SessionsStore focused receipt; resolutionKeepsTask | pass, fixtures; configured resolution gap |
| Native lifetime and input | Same surfaces/processes/drafts/splits; hidden terminals relinquish input | Retained pool, disabled hidden multiplexer, transition-based focus | Native proof and unchanged isolation proof both previously failed at surface creation | gap |
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

First restore Ghostty surface initialization using the isolated reproduction in
`native-surface-diagnostic.md`. Repeating navigation tests against the same nil
surface cannot establish the missing native behavior. The diagnostic narrows the
failure boundary; it neither proves its cause nor waives configured proof.

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
