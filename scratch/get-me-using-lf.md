# Get me using Loopflow — LOO-462

Implementation authorized · October 10, 2026. Jack Heart requested pursuit after
selecting conversation-first Tasks, repository-name conversation access and
repo/session ownership. Pursue runs through publication for demo review, not landing.
Remaining reversible UI details may be resolved during implementation; live data
cleanup remains separate and requires a reviewed, restorable plan.
Product owns this Task. LOO-427 is integrated and completed; historical stacking
details remain at `0bb13e9261:scratch/get-me-using-lf.md`. Portfolio implementation:
`d43aa248d3`; shared-reader compression: `50e2cf44d1`. Rendering, database
subscriptions and selected-terminal retention are implemented through `27613222eb`.
Affected-suite gate, publication and native acceptance remain. Product has no
immediate child Wave memories in this checkout.

Reconciled October 14 against the current source and the preceding 86-test pass.
No further implementation mismatch was found in the subscription/terminal-selection
cut. The accepted scope is unchanged; native header geometry remains a proposal
awaiting review. The latest integrated main is `92b88800ec` (v0.13.15); its shared
Session preparation is adopted below, not a second Desktop launch path.

## Problem and accepted direction

Jack Heart wants Loopflow to replace his cmux arrangement: repository windows,
retained task workspaces, and conversations beside supporting material. Session
clutter makes current work hard to find. Jack requested a top-down design anchored
in `af48fd81db:website/home.html` and `website/content.yaml`, not production.
The homepage groups Tasks under Waves and shows Workflow participation points.
Its CLI examples are not an implementation specification.

Jack's October 10 decisions:
- Daily navigation represents Repo/Wave/Task work. Delete non-work-scoped Session
  navigation, including unassigned groups; shell-launched unscoped Sessions remain
  possible without sidebar entries. Power-user Session search is deferred.
- A repository with no Waves or Tasks opens directly into its configured repo
  conversation. Begin with what Jack wants to work on, not organizational setup.
  Define and track ideas as they develop; do not manufacture Work on opening.
- Jack Heart confirmed that clicking the repository name always opens its retained
  repo conversation, whether or not Waves or Tasks exist or are open. Reuse the
  repository Work tab and conversation; do not create a new Session on every click.
- Preserve useful history and active work. Investigate iteration debris with a
  restorable backup and reviewed candidates, not age/status-based deletion.
- Reuse LOO-427's repository window, Work identity, multiplexer and control owners.
  Relaunch persistence belongs to LOO-416; existing native proof gaps to LOO-391.

Historical Session inventory, cmux inspection and original discovery preservation
receipts remain at `83fa49adf4:scratch/get-me-using-lf.md`, **Problem and accepted
direction**. They are neither a current inventory nor a deletion candidate set.

## The demo

Open an owned empty fixture repository. Its existing repo conversation appears,
or exactly one is created through the existing primary owner; the opening asks
what to work on. Reopen without duplicating the conversation. In a populated
fixture, move from a Wave's objective/Workflow/Task list into a Task workspace,
then back: conversation, Files draft, shell and layout are retained. An unrelated
shell-launched Session never becomes a sidebar item. A failed planning read shows
unavailability rather than launching onboarding. Hide the last tab: nothing starts.

Jack Heart selected conversation-first Task opening in the ongoing LOO-462
conversation on October 10, suggesting “maybe a small expandable overview header.”
The conversation is the main surface, not an overview that requires another click.
Proposed header: collapsed title and Workflow position; expand for description and
supporting Task details. Header contents and sizing remain a proposal, not a settled
visual design. Returning restores retained panes, drafts and layout; expanding the
header must not replace or restart the conversation.

First opening now uses the existing Task primary preparation and linked-opening
owner. This supersedes the passive-first proposal without running a Workflow or
authorizing takeover. Preparation refreshes the exact Task after ensure; lookup,
placement and unavailable-owner failures retain the existing failed-opening/retry
surface. No local fallback is added. Native usability of these states remains
for demo review; no separate preparation mechanism is planned.

## Surviving owners

`WorkProjection.outline` owns Wave/Task rows; `WorkModel` retains full Session
membership and missing-Work evidence. `SessionsWorkspaceRegistry` retains Work
layouts, native surfaces and drafts. Navigation suppression changes none of those
lifetimes or Session completion.

Rust's `ops/human_session/primary.rs` owns repo/Wave ensure and Task primary
selection. RegistryQuery calls that owner; configuration supplies provider/model
and existing conversations retain native history. Only deliberate opening and
one-shot first-use entry acquire conversations. Repo/session invites ideas before
an operating pass when no Work exists.

Historical discovery, rejected predecessors and earlier failed checks are preserved
at `1787d94bc9:scratch/get-me-using-lf.md`; pre-compression uncommitted notes
remain at `/tmp/loo462-compress-1787d94/`. The SQLite backup primitive in
`scripts/desktop_performance.py::_snapshot` is not a restore recipe for provider
history, artifacts or active clients; cleanup remains separate.

## Chosen approach and alternatives

Reshape the existing Work navigation and opening path. Do not add another home
screen, Session index, primary store or layout persistence layer.

- Reject filtering old/interrupted/unknown Sessions as the solution: it confuses
  history and liveness with navigation and cannot establish disposability.
- Reject a new onboarding conversation type: the primary repo conversation already
  has scope, history, provider selection and an isolated workspace.
- Jack Heart accepted keeping repo/session: begin with “What do you want to work
  on?” and organize ideas as they emerge. No separate onboarding mode or setup
  checklist. Beginning without Work is ordinary repository conversation.
- Jack Heart selected conversation-first Task opening, superseding the passive-first
  draft. Reuse existing primary preparation; opening never starts a Workflow or
  implicitly takes over another client.

Success means Jack follows Work and returns to exactly the context he left,
without maintaining a parallel set of Session windows. Failure would be a cleaner
sidebar that quietly starts clients, loses history, or hides missing-Work recovery.
The acceptance cases below target those failure modes, not just row counts.

## Authority, surfaces, and error states

Planning and Rust Session association remain authoritative; WorkModel projects
those records. Workflow position, Session Waiting and Process liveness remain
separate. Show the recorded node/edge and shared legal actions; never derive
approval or completion from a displayed 'waiting' label.

`SessionsView`, WorkNavigator, WorkSurfaceView, WorkProjection and their tests
consume the same Work stream and retained registry. CLI Session list/connect and
shell launch remain compatible. No schema or Session DTO change is expected;
if a necessary freshness/completeness fact is absent, extend the existing shared
reading and Rust/Swift fixtures rather than guessing in the view.

Empty requires fresh successful current planning plus the full repository Wave
history, establishing no Waves or Tasks, not zero filtered rows, zero started Tasks, cached wire text, search
results, or a missing Project. A repository with only completed history is not
proven empty. Failed/truncated planning retains existing content and an error;
it neither launches onboarding nor evicts panes.

An entry request captures repository/window/Work identity before awaiting ensure.
Concurrent requests use the primary owner and one existing opening operation;
late replies belong to the originating Work and do not steal focus after navigation.
A failed launch retains an actionable error and retry for the same scope. Missing
provider configuration uses existing errors; no login, install, provider switch or
new picker is silently performed. Do not auto-retry per stream update. Closing the
last tab must not trigger entry again. Existing remote ownership and takeover rules
remain unchanged; no unavailable owner becomes a local fallback.

## Delete — do not maintain

Completed cuts; exact rationale and earlier checks remain at
`240ad4821f:scratch/get-me-using-lf.md`, this heading:

- Debug/Orphan navigation, generic New Session composer, SessionSkillPicker/
  SessionSkillLaunch and selected-skill storage, DiscoveryEntry, Session-title
  search/palette destinations, headless sidebar toggle and their exclusive tests.
- SessionsContentView's Work-tab strip, Task sibling Menu, ordinary Flow-membership
  chip, 28-point Task row and Flow-only taskTone dot. Session membership, explicit
  links, history, rename/binding and retained panes survive.
- `WorkModel.linkedSession`, its view mailbox/`openLinkedSession`, unprepared-Task
  container swapping and unused pane directory threading. `placeSession` serves
  Task entry and exact links; per-workspace request tokens fence late feedback.
- Task details' Session inventory/helpers; `ReferenceTextView.swift` and exclusive
  tests; `PlatformHelpers.swift`; unused launcher interrupt/PR-review wrappers;
  `WorkActivityView.turnIntentError`; `WorkPresentation`; constant-nil Roadmap
  selection/detail branches and unused Portfolio row callbacks.

Surviving owners: Rust primary admission and shared status; WorkProjection rows;
WorkModel opening and `publishRoadmap` acceptance/cache retention; retained
SessionsWorkspaceRegistry layouts, surfaces and drafts. `ProgramStatusState.summaryOrder`
owns summary severity. CLI launch, pane-owned new conversations, missing-Work
recovery, Waiting links, raw diagnostics, TaskSessionGroups and ChatReference remain.
No obsolete owner remains in these cuts. The 183 unresolved exported-API audit
entries remain retained; absence of local callers is not deletion authority.
The subscription cut's completed deletion boundary is recorded below.

## Portfolio — implemented, second review priority

Jack Heart selected lf-owned folder/manual portfolio configuration through the
existing Portfolio view. Default immediate-child main repositories under ~/src;
change one root or choose individual repositories across folders. Exclude linked
worktrees, preserve inactive selections and missing-path errors. Multiple roots,
recursive discovery and exclusions remain deferred. Membership changes never
remove Work, panes or history. RegistryQuery calls the single Rust writer/reader;
no Swift scanner or configuration writer. Legacy membership import stays explicit.
Implemented scope, failed attempts, focused/CLI readback tests and native-proof
limits: `185c0b3c2e:scratch/get-me-using-lf.md`, **This slice — one repository
portfolio owned by lf**. Native folder/manual picker, menu refresh and draft
retention review remains after the main repo/Task experience feels right.

## Done when / unattended gate

The implemented fixture crosses Rust's persisted repo primary and mixed
Session/Work inventory, its JSON projection, and Swift's repository
entry/outline/retained tabs. Acceptance remains:
empty-repo entry reuses one exact primary with configured provider/model; refresh
and reopening do not duplicate; unrelated Sessions remain in history without UI
inventory; Wave → Task navigation preserves exact pane/document IDs and unsaved
state; partial/error readings do not launch or drop anything; hiding all tabs
starts nothing. Existing lifecycle/DTO and navigation suites own this proof;
a filter selecting zero tests is not proof.

Publication uses `uv run python scripts/test.py --reuse-passing --base 92b88800ec`
with stdin closed. Its Rust runner materializes migrations only in a disposable
checkout, clears inherited LF_* and isolates network side effects. The consolidated
integration target includes lifecycle, DTO, Portfolio and documented-command proof;
Desktop runs headless through its configured source CLI.

Gate owns those suites once. Prompt fixtures inspect assembled input and
assert no mandatory setup or operating pass before the no-work invitation, without requiring a real model.
Rendered judgment is demo/review, not a headless gate. This Task supplies no native
provider acceptance, installation, or three-day adoption credit. Chapter KRs remain
historically readable; October 7 retired numeric/soak acceptance, not retention or
usefulness. No external-product progress credit follows from Loopflow UI work alone.

Earlier gate/publication and review-order evidence remains at
`9f19e3c5d1:scratch/get-me-using-lf.md`, **Done when** and **Current review order**.
Jack Heart prioritizes populated repo/Wave/Task usability before Portfolio.
Fixture checks and publication never grant installation, takeover or adoption credit.

## Sidebar visibility and cleanup evidence

Started and unresolved execution select ordinary Tasks; Session presence alone
cannot admit unstarted or settled Tasks. Live/unknown execution, completion refusals
and unsettled files remain evidence. History, binding and panes survive filtering.
Exact cleanup and unresolved Task dispositions remain in work-first-demo.md and
`9f19e3c5d1:scratch/get-me-using-lf.md`, this heading. The bounded LOO-376 and
126-row repairs grant no broader cleanup authority; LOO-312 was recovered intact.

## Earlier accepted cuts and native proof

The October 13 UX, sidebar metadata, profiling and October 14 semantic-color
cuts are implemented. Full accepted constraints and historical checks remain at
`240ad4821f:scratch/get-me-using-lf.md`, **Paused and prepared demo** through
**Semantic cut**. Preserve sidebar-only retained navigation, primary Task entry,
truthful Workflow readings, recorded remote Machine ownership, neutral selection,
distinct focus, quiet links and source-backed activity. Unknown/client-only
activity remains hidden. Palette/ANSI and native surface/draft ownership survive.

Remaining demo proof: native light/dark geometry, focus/hover, exact Task/PR links,
Portfolio folder/manual changes and retained drafts. Measure Codex and Claude
separately for cached switch, resume and fresh launch, from click to responsive
unsent input, with paired wall/CPU evidence. Approval-once across Tasks/resume/
compaction remains unverified for both providers. Instruments and limits live in
`scripts/benchmarks/desktop-performance/agent-input.md`; historical failed native
attempts remain in work-first-demo.md. Prepared/first draw is not responsive input.
No install, migration, app restart, takeover, broader cleanup or landing authority.

### Rendering ownership — implemented

Jack Heart approved the rendering plan plus cmux research, then authorized the
sequential database pursuit after rendering finished at `e480b4bc7c`. Full
proposal, repairs, failed publication and check receipts are preserved at
`27613222eb:scratch/get-me-using-lf.md`, **Current pursuit — rendering ownership**
through **Next pursuit — database-driven UI**.

`WorkModel.installConversation` places the resolved Session/pane synchronously;
`placeSession` also handles exact contextual links. Per-workspace request tokens
fence late feedback. Cached switching skips preparation; missing Machine fails
before connecting. Initial feedback stays in the terminal area, while the selected
Work, last-good readings, panes, native surfaces, focus targets and drafts retain
their owners. Prepared means mountable, never responsive agent input. Compact
Workflow position/actions and Overview disclosure remain pending native judgment.

The rendering-state review is historical at `5ade59f428`; its proposed green APIs
are not final. The cmux study pins `321d7d4458abb4005584f8459cf98ade967b38ad`:
retention, mounted visibility, focus and readiness are separate; it provides no
Loopflow frame-sequence or benchmark evidence. The Swift audit's 183 unresolved
exported APIs stay retained; only metadata/eight cuts, not all 14,483 entries,
were reconciled. Authored-Wave file ownership and SwiftPM/XcodeGen resource parity
remain follow-up candidates. No new deletion authority follows from these studies.

### Gate and publication blocker

The repaired rendering run passed 524 Desktop tests. Consolidated gate never
started product suites: resource recovery exceeded the 60-second wrapper deadline
during pruning, and its orphaned child was stopped. Subsequent read-only inventory
failed its five-second uv-cache deadline with 64.4 GiB free. Gate must resolve the
resource preflight and run affected suites before publication; do not bypass it.
Evidence: `/tmp/loo462-publish-{desktop-final,gate}.log` and
`/tmp/loo462-publish-resources.json`. No PR update or native acceptance followed.

### Database update implementation — October 14

Jack Heart accepted subscription-owned reloads and coherent read-your-write
confirmation. The existing Rust stream remains the owner. A checkpoint request
samples a read transaction after command completion, projects only invalidated
subscriptions, then acknowledges delivery even when content is unchanged. Failed
readings cannot confirm a save. External process/file facts remain observations,
not transactional claims. Reconnect starts from current state; retained content
and request/draft identities survive.

Removed: ordinary post-write `WorkModel.refresh` calls in
Task actions, Workflow selection/save and directive save; Session write-triggered
global Refresh. Explicit Retry/wake recovery, non-stream transport reads,
and file-catalog invalidation remain. Projection batches now share one
read snapshot; nested joined reads use savepoints. No migration, second
change bus, Swift schema reader or generic query framework.

Implemented: snapshot-backed checkpoint delivery/replay, subscription-owned
Task/Workflow updates, unchanged-scope query reuse and Flow-independent Work
history. Wave history uses the same reader connection; joined reads use savepoints.
Failures retain last-good UI, newer requests/drafts win, and checkpoints follow
replacement frames even for throttled dependencies. Replay does not query.

Stored `wave_workflows` definitions move the planning revision; open catalogues
invalidate even on unchanged-body heartbeats. File activation/customization remains
separate, and the catalogue is not part of the Work frame's SQLite snapshot.
Retained manual reads: Retry/wake/non-stream recovery, Flow/file catalogues,
Portfolio config/authored Waves, Task files, telemetry and OS/configuration status.
No second schema reader, new change bus, migration or generic query framework.

Review repaired the context-hook fixture's committed conflict markers and the
peer-planning test's incorrect binding/export atomicity assumption. Tests cover
another process's writes, coherent joins, unchanged replay, failed confirmation,
reconnect, external Git/process changes and newer drafts. Earlier focused checks
passed (24 stream, 7 revision, 30 Desktop tests and one overlapping catalogue case;
fmt/Clippy/diff passed). Nextest reported two leaky exits, including a pure usage
predicate; gate owns diagnosis. Exact commands and logs:
`27613222eb:scratch/get-me-using-lf.md`, **Database update implementation**;
pre-compression evidence `/tmp/loo462-compress-before.patch` remains preserved.

Sync `b10cc7ce10` merged main `92b88800ec`. Adopt returned Task preparation without
preparing twice; ended terminals allow explicit preparation. The 65-test Desktop
opening/store pass and one Rust primary-identity test passed; commands and logs
remain at the same archive's **Sync review**. Native gaps are unchanged.

### Selected terminal Tasks and CLI regression — October 14

Jack Heart requested real CLI creation/start into an already open subscription and
production sidebar projection, exactly once under the correct Wave; unstarted
backlog stays hidden. Jack selected completed/canceled Tasks remaining in sidebar
and main pane while selected, with green Completed or red Canceled header text.
Navigating elsewhere removes only this selection exception; retained Sessions,
surfaces, files, drafts and history survive. Jack Heart selected navigation Back
to restore the same workspace and temporary sidebar row with its lifecycle header,
without reopening execution. Test completion/cancellation → away → Back.
Late replies cannot steal the new selection. Headless model/projection proof is
accepted here but is not rendered native proof.

Implemented: the outline admits the selected terminal Task; navigation away drops
only that exception. The header uses shared planning lifecycle text and success/
failure colors. Repository-local Back (⌘[) selects existing Work without preparation;
Machine changes clear its history. CLI create/run/complete/abandon and subsequent
edits flow through the real subscription into the production outline. The retained
Session/native view is synthetic and unmounted; no provider or display was used.

Deleted the unconditional working-set-only filter for the selected terminal Task
and unlabeled terminal header. Ordinary active/history filters and workspace owners
survive. Review repaired a nil-selection/nil-runtime comparison that could admit
unselected terminal Tasks; a regression covers that boundary. Back does not record
its own return or leak visits across repositories. No cleanup or execution restart.
Remaining: normal gate/resource preflight, publication, and native proof above.

Earlier implementation checks (123 Desktop tests plus the final CLI/Back boundary
filter and CLI build) remain at `27613222eb:scratch/get-me-using-lf.md`,
**Selected terminal Tasks and CLI regression**.

Review: the outline now resolves selected Task identity through the existing
breadcrumb resolver, removing its second planning/runtime-ID matcher. Machine
changes clear selection, Back history and retained selection evidence through one
navigation operation. Back's history-control overload is private. No further
obsolete owner was found; retained workspace/history and unknown audit APIs stay.

Check: `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone --filter 'WorkCLISubscriptionTests|WorkNavigationTests|WorkModelTests|WorkCacheTests'` PASS (86 tests, 5 suites, preceding unchanged-code result reused from `/tmp/loo462-compress-selection.log`); `git diff --check` PASS. Broader suites/resource preflight: gate; native acceptance: demo.

Gate recovery: the 60s outer resource timeout was shorter than its 120s cleanup
child. Recovery now has a bounded 300s allowance for cleanup plus inventories;
read-only measurement remains 60s. Timeout kills the owned process group rather
than leaving cleanup children. Check: focused resource budget/child-exit tests PASS
2/2 (initial fixture interception repaired); original full gate still needs replay.

## Shipping approval and release — October 14

Jack Heart accepted this diff as shippable after the configured demo and requested
a minor release after it lands. This supersedes the no-landing boundary above.
Finish gate and Task-completing shipping; then run `lf release run minor` only
after verified merge. Native proof gaps remain honest, not retroactive acceptance.
Final demo corrections: full-width green/red lifecycle header; completed/canceled
Tasks appear only while selected, even with unresolved historical execution.
Slow initial Workflow reading remains a reported usability issue; retain a concrete
follow-up disposition rather than claim it solved. No live history deletion.

### Shipping gate — October 14

Reviewed the complete change against `92b88800ec`, including the retained worker
edits. Two broader-suite failures were stale assertions: ordinary Task creation
prints its resolvable identifier, and Codex hook declarations preserve existing
trust without supplying new approvals. Repaired those tests to prove the intended
behavior; no production change was needed. Updated TESTING.md's inventory deadline
from five to 15 seconds to match the retained resource recovery change.

- `uv run python scripts/test.py --reuse-passing --base 92b88800ec < /dev/null` — architecture, Python (473 + one separately sandboxed test), website (75), Desktop app/build/boundaries (534 tests), fmt and Clippy PASS; Rust 2,619 PASS, two stale assertions FAIL; clone phase not run after that failure. Resource preflight/postflight PASS at 51.8/51.7 GiB free; busy uv cache and timed-out build cleanup remain explicit warnings. Log: `/tmp/loo462-gate-final.log`; artifacts: `.lf/tmp/gate/run-55782/`.
- `cargo fmt --all -- --check` and `cargo clippy --all-targets --jobs 4 -- -D warnings` — PASS after the test repairs; `/tmp/loo462-gate-repairs.log`.
- `uv run python scripts/materialize_rust_tests.py --prepare-cli -- cargo --config 'target."cfg(all())".runner = ["uv", "run", "--no-sync", "python", "/Users/jack/src/loopflow.make-loopflow-a-work-first/scripts/test_network.py"]' nextest run -p loopflow --lib --test integration --build-jobs 4 --test-threads 4 -E 'test(ordinary_task_creation_returns_resolvable_identifier_without_a_public_retry_token) | test(codex_terminal_delivers_large_input_and_approvals_then_reconnects_without_replay)'` — PASS 2/2; `/tmp/loo462-gate-repairs.log`.
- `uv run python scripts/materialize_rust_tests.py --prepare-cli -- cargo test -p loopflow --test integration --jobs 4 data_clones:: -- --test-threads 4` — PASS 4/4; `/tmp/loo462-gate-repairs.log`.

The original aggregate failure receipt remains intact; focused repair evidence
does not claim an identical-tree aggregate pass eligible for reuse. No previously
passing suite was replayed. The earlier leaky Rust exits did not recur.

Jack Heart's shipping acceptance stands. Before Task completion, the shipping
owner must disposition the slow initial Workflow read and remaining native proof
through supported follow-up operations. The production performance signal remains
Task click to rendered response to unsent input, separated by Codex/Claude and
cached/resume/fresh cases, with paired wall/CPU evidence. No paired measurements,
native trust acceptance, or speedup are claimed by this headless gate. Optional
display diagnostics remain with demo/review; no app restart, release, or landing
was performed in this gate step.
