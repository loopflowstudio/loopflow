# Get me using Loopflow — LOO-462

Implementation authorized · October 10, 2026. Jack Heart requested pursuit after
selecting conversation-first Tasks, repository-name conversation access and
repo/session ownership. Pursue runs through publication for demo review, not landing.
Remaining reversible UI details may be resolved during implementation; live data
cleanup remains separate and requires a reviewed, restorable plan.
Product owns this Task. LOO-427 is integrated and completed; historical stacking
details remain at `0bb13e9261:scratch/get-me-using-lf.md`. Portfolio implementation:
`d43aa248d3`; shared-reader compression: `50e2cf44d1`. Reconciled October 14
against HEAD `87f275c282` and the working rendering diff over `9f19e3c5d1`.
Implementation and compression are complete; affected-suite gate, publication and
native acceptance remain. The reconciliation changes documentation only. Product
has no immediate child Wave memories in this checkout; parent objective and memory were read in full.

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

The cut deletes Debug Session/Orphan navigation, the generic New Session composer,
SessionSkillPicker/SessionSkillLaunch and selected-skill storage, DiscoveryEntry,
Session-title search and palette Session destinations, and the headless sidebar
toggle with its exclusive state/tests. CLI launching, explicit Session links,
Run inspection, pane-owned new conversations, membership and missing-Work evidence
remain. There is no replacement Session index or second primary/layout owner.

October 13 removed the SessionsContentView Work-tab strip,
Task sibling Menu and ordinary Task Flow-membership chip. Their exclusive chrome
expectations are removed; full Session membership, explicit links, retained panes,
rename/binding and non-Task chrome survive. Duplicate opening labels collapse into
one message. No new Session inventory or readiness store replaces them.

The Session inventory controls and top strip are deleted; contextual exact-Session
links and full membership still serve retained Work. Do not delete those owners.
The 28-point single-line Task row and Flow-only taskTone dot are removed; the
surviving metadata reads shared Session evidence without changing Task state.
The summary severity order now lives once in `ProgramStatusState.summaryOrder`,
shared by terminal and sidebar reports. Wave headings skip Task-status aggregation;
outline Task data is immutable. Local Machine labels return nil rather than an
empty-string sentinel. No deletion targets remain in this metadata slice.
Task action feedback now shares one helper across run/move/pause/complete, retaining
the originating navigation and Task. Profiling has one endpoint decision that returns
status and elapsed time together; failure cannot leave a usable comparison number.
An empty CPU reading removes an older SVG instead of leaving misleading evidence.
Review also repaired the headless stream fixture's obsolete assumption that a
data-directory frame confirms Machine identity; it now confirms the actual ID.

Earlier compression checks and launcher/entry reductions remain at
`83fa49adf4:scratch/get-me-using-lf.md`, **Delete — do not maintain**. No remaining
obsolete owner was found in the refresh repair. Stream and one-shot roadmap reads
now share `publishRoadmap`: acceptance, failure retention, saved-state reset and
cache replacement stay together. Absent wire text stays absent instead of becoming
an empty cache entry.

October 14 rendering cut removes `WorkModel.linkedSession`, the view's
`onChange` mailbox and `openLinkedSession`, unprepared-Task container swapping,
and unused directory threading through pane views. `WorkModel.placeSession`
now installs both resolved Task openings and exact contextual links. It preserves
Machine/location checks, request generations and separate readiness observation.
Opening uses one per-workspace request token instead of a resettable Boolean;
an old request cannot clear a newer request's feedback.

Audit-backed removals: Task details' ordinary Session inventory and row helpers;
`ReferenceTextView.swift` and its exclusive Mac/absent-UIKit tests;
`PlatformHelpers.swift`; uncalled launcher interrupt/PR-review wrappers;
`WorkActivityView.turnIntentError`; constant-nil Roadmap selection/detail branches;
and `WorkPresentation` with its obsolete picker-only test/benchmark paths.
Waiting retains exact contextual links; history, raw diagnostics, public
`TaskSessionGroups` and `ChatReference`, platform callbacks, surface pools and
drafts survive. Portfolio rows now call its existing repository/Task opener.
SessionControls and retained native WorkNavigation proof keep takeover/input
coverage; the deleted UI inventory journey supplied no current navigation proof.

Compression completes the Portfolio cut through `RoadmapWaveCard`, `RoadmapTaskRow`,
`NowSectionView` and `NowRowView`: remove always-nil selection and unused callbacks;
rows open their repository Task or Wave directly. Exact Session opening first reuses
retained placement before resolving membership. `LinkedSession` no longer needs the
mailbox's equality conformance. No remaining deletion targets in this rendering cut;
the unresolved exported-API inventory remains retained.

## Implementation and remaining work

The Work-first entry/navigation cut, isolated persisted fixture, configured-runtime
failure and repaired launcher/Task entry are preserved at
`0bb13e9261:scratch/get-me-using-lf.md`, this heading. Current owners remain:
Rust primary admission, WorkProjection navigation, WorkModel opening and the
retained SessionsWorkspaceRegistry. No native usability follows from fixtures.
Current pursuit and remaining native proof are below; failed real-repository
attempts and exact restoration boundaries stay in work-first-demo.md.

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

Publication uses `uv run python scripts/test.py --reuse-passing --base 37ca5fca81`
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

## Paused and prepared demo — October 13

Jack Heart selected Paused and authorized pausing LOO-389; native pausing remains
unverified. Released v0.13.14 supplied its migration. Preserve identity, history,
checkout and Workflow; resume starts nothing and pause kills no process.
Earlier Task/release/hook receipts: `9f19e3c5d1:scratch/get-me-using-lf.md`, this heading.
No additional installation, migration or cleanup is authorized.

## Current pursuit — UX and end-to-end profiling, October 13

Jack Heart requested this design update and a new pursue through publication for
review, not landing. This section supersedes earlier visible Work-tab and Session
picker proposals. Main repo/Task experience first; Portfolio revision stays second.

### UX implemented; native review remains

`0bb13e9261` removed the visible Work-tab strip while retaining the workspace
registry. `5b62453be7` removed ordinary Task sibling menus and Flow-membership
chrome, unified opening feedback, and added truthful Workflow refresh errors.
The existing Rust primary owner still prepares first opening; a deliberately
selected historical conversation remains selected on return. Explicit links,
all Session membership, cached surfaces, layouts, focus and drafts survive.
There is no new Session inventory, Workflow launch or client takeover.

TaskWorkflowHeader distinguishes an unread Workflow, an unavailable reading, a
successful null Workflow and a last-good value with refresh failure. It does not
backfill Workflow records. LOO-462's recorded feature/demo position remains the
real-repository review case. Conversation-first layout, compact breadcrumb and
expandable Overview exist; sizing and native usability remain unaccepted.

Stable-hook declarations are implemented in `185c0b3c2e`. Native approval-once
proof across distinct Tasks, resume and compaction remains for each provider.
Workspace/account trust is separate from hook review; no fabricated approval or
approval of unrelated hooks is part of this work.

### Sidebar metadata — implemented, native review remains

Jack Heart's October 13 steers selected compact title/metadata rows. Task rows now
use 44-point minimum height, with program status, stored Task/PR links and recorded
remote Machine labels. Native density remains unaccepted. No Session counts or
branch/path clutter. The full row background selects its conversation; independent
Task/PR links do not. Existing workspace and surface owners survive.

Shared reports outrank inferred Waiting; severity, primary then Session ID order
them. Unknown/client-only activity stays hidden, failures inspectable. Done/Error
never changes Task or Workflow state. Recorded checkout ownership supplies remote
Machine labels; changed delegation defaults cannot relabel started work. Cached
pages strip old reports, Waiting and client liveness. Detailed contract comparison,
implementation and focused proof: `9f19e3c5d1:scratch/get-me-using-lf.md`, this section.
Native link dispatch, density and retained draft review remain outstanding.

### Profiling and optimization

Implemented in `5b62453be7`, compressed in `ae691073c9`: one opt-in recorder joins
Desktop click/key/draw events, lf preparation/lock/provider-launch milestones,
process CPU deltas and exported Time Profiler stacks. Exact-Session ordered native
captures are required for an observed endpoint; failed/incomplete trials have no
elapsed comparison value. Running CPU samples alone enter the flamegraph; missing
samples remove an older SVG. The real headless export validates the parser only.
Commands and observer limits live in `scripts/benchmarks/desktop-performance/agent-input.md`.

Remaining acceptance:

Measure Codex and Claude separately for cached workspace switching, native resume,
and fresh conversation launch. Start at the actual Task click and end at visible
coding-agent input responding to an unsent draft keystroke. Process spawn, connect
receipt, terminal creation, first draw and focus are intermediate events only.
Use owned disposable conversations for input probes; never type into Jack Heart's
existing drafts. Record build/helper revision, provider version, Machine/data
selection, exact identities and cold/warm conditions; preserve failed attempts.

Produce correlated wall-clock timelines across Desktop, lf preparation/connect,
PTY/relay and provider startup, plus CPU flamegraphs for observable local processes
across the full interval. Account explicitly for subprocess, lock, I/O and network
waits; CPU samples cannot explain elapsed waiting alone. Closed provider internals
and unavailable stack symbols stay explicit, not synthetic spans. Report hook
approval separately alongside total elapsed time, never silently subtract it.

The instruments reuse existing owners, not a second readiness lifecycle. Native
trials still need raw artifacts, readable flamegraphs and phase breakdowns with
observer overhead/limitations. Compare before/after on
comparable cases and optimize measured bottlenecks. No arbitrary numeric target
or retired soak requirement is restored. Unavailable native facilities do not block
headless implementation; they leave named demo proof outstanding, never a claimed
end-to-end pass. Current lookup repair and 63 passing tests are in
scratch/task-opening-latency.md; historical read durations are not endpoint timings.

### Delivery and remaining demo

October 13 publication checks, fixture repairs and earlier app receipts remain at
`945686275c:scratch/get-me-using-lf.md`, **Delivery and demo** through **Native
failure repair**. The full-suite run plus focused repairs is not a clean rerun.
v0.13.14 installation and release-main integration resolved the schema prerequisite;
current real-repo navigation evidence is in work-first-demo.md. None supplies
responsive input, six provider timing cases, hook approval-once, Portfolio or
complete visual acceptance. No new installation, live migration, client takeover,
broader cleanup or landing is authorized by this pursuit.

The October 13 app used pre-semantic Swift `a590ecb7ee`. October 14 semantic,
row-hit-target and navigation-polish relaunches supersede that app receipt
(work-first-demo.md); they establish populated navigation and the observed contrast
repair, not complete visual acceptance. A bundle containing the current rendering
ownership cut and its native review remain outstanding.

Main `e3c3a038aa` adds `lf clone` (merged here by `0fe7f5da8a`): isolated
navigation and fresh stand-in conversation fixtures can use the shared clone
facility instead of inventing another snapshot runner. Its documented boundary
refuses copied Session continuation and external provider traffic; it cannot prove
real Codex/Claude input readiness or replace the accepted populated-repository
demo. Native clone acceptance belongs to lf-4e4cc84. No clone trial was performed
in this reconciliation; [data-clones.md](../docs/data-clones.md) owns its limits.

## Color and activity redesign — October 13 review direction

Jack Heart requested preserving the palette while assigning consistent roles across
Desktop: neutral structure and text, selection independent of status, quiet links,
restrained burgundy emphasis, and evidence-backed blue/amber/red activity. Keep
Task lifecycle and Workflow separate from program activity. Omit unknown activity;
a live client alone cannot claim Working. This supersedes the sidebar's former
“Status unknown”/“Active · inferred” presentation, not its underlying observations.
The semantic-token cutover is implemented below; rendered judgment remains with
demo. `scratch/pr-review.html` describes the implementation; it is a review artifact,
not native validation evidence.


### Semantic cut — implemented October 14

Accepted role assignments, contrast checks and native acceptance remain at
`9f19e3c5d1:scratch/get-me-using-lf.md`, **Implementation cut — October 14**, and
VISUAL_DESIGN.md. Preserve palette/ANSI, neutral selection, distinct keyboard focus,
quiet links, restrained burgundy actions and source-backed activity. Unknown and
client-only activity stay hidden; read errors remain inspectable. Done/Error never
sets Task completion or Workflow state. Native light/dark, focus/hover/selection,
link dispatch and provider input remain unverified; no synthetic live reports.

### Current pursuit — rendering ownership, October 14

Jack Heart approved implementing the previous rendering plan combined with the
cmux research, then explicitly requested an updated design and background pursue.
Return through publication for demo review; no landing, installation, live database
cleanup, native takeover, or restart of the running demo is authorized by this cut.
Prior semantic-color implementation and passing checks are preserved at
`5ade59f428:scratch/get-me-using-lf.md`, **Implementation state — October 14**.

The rendering cut is implemented in the working diff:

- `WorkModel.installConversation` installs the resolved record and pane before
  preparation ends; `placeSession` also serves exact contextual links. The existing
  workspace carries a request token, not the proposed new workspace-opening API
  or a view-observed mailbox. Generation and Machine/location checks survive.
  Superseded preparation results are discarded; late readiness cannot redirect
  navigation. Cached switches reuse their selected conversation without preparation.
- The selected Work renders immediately through the retained pane host. Initial
  opening/recovery feedback lives in its terminal area. Last-good Work/Workflow,
  panes, native surfaces, focus targets and drafts retain their existing owners.
  Prepared permits mounting; it does not prove responsive coding-agent input.
- Current Workflow position/actions stay compact; Overview holds graph/details.
  This is a reversible implementation choice for Jack Heart's suggested expandable
  header, pending native geometry and interaction review.
- Audit-backed deletions remove the obsolete inventory, handoff, presentation
  branches and exclusive tests described above. Portfolio Task/Wave rows call
  the existing destination opener. Unresolved exported APIs remain retained.

The historical proposal in `rendering-state-review.html` is pinned to
`5ade59f428`; its green APIs are not the final API. `pr-review.html` describes the
working implementation. The cmux study is pinned to
`321d7d4458abb4005584f8459cf98ade967b38ad`: retained terminal ownership, mounted
visibility, focus and diagnostic readiness are separate. Its source research
establishes no cmux benchmark or Loopflow native frame sequence.
The Swift audit/inventory preserves 183 unresolved entries, not deletion authority.
Reconciliation reviewed its metadata and eight cuts, not all 14,483 inventory entries.

Jack Heart accepted broad update-boundary tests, preferring useful coverage to
brittle exact SwiftUI body counts or private scheduling assertions. Existing
headless opening/retention tests cover this rendering cut. They do not prove that
unrelated database writes or transcript output avoid every expensive query.
`scratch/datastore-reactivity-study.html` records separate, initial source research:
current subscriptions are coarse, mutations still request broad refreshes, and
revision tags alone do not establish a coherent SQLite snapshot. Query-dependent
invalidation, second-process write coverage, read-your-write acknowledgment and
external liveness/file adapters remain separate substantial work. No datastore
framework rewrite or new Swift SQLite reader is part of this cut; Rust/lf remains
the sole internal-schema reader and legal-action owner.

Upstream `f9a16d6f84` adds provider continuation with `lf resume -a`; that does not
supply native approval-once or input-readiness evidence for either provider here.
The existing clone facility can isolate stand-in checks but still cannot substitute
for real provider input or the accepted populated-repository demo.

### Rendering implementation and remaining proof

Implemented against `9f19e3c5d1`: immediate Work selection, model-owned synchronous
Session/pane installation, terminal-local feedback and stable pane host; compact
Workflow position/actions with graph in Overview. Last-good readings and surface
ownership are unchanged. Headless tests observe ensure → installed/opening pane →
prepared, same-pane/draft return, delayed request cancellation, companion visibility
and readiness failures. Prepared still means mountable, never responsive agent input.

Review findings fixed: contextual links and Task entry had duplicate placement;
both now call `placeSession`. Per-workspace request identity prevents older cleanup
from clearing a newer opening. Portfolio Task clicks previously wrote a constant-nil
binding; they now route to the existing opener. The missing-Machine path must fail
before connection instead of bypassing the unavailable workspace.

All 183 unresolved audit entries remain intentionally retained: exported library
APIs and their tests lack downstream-use evidence. Authored-Wave discovery/write
ownership and SwiftPM/XcodeGen resource parity remain separate follow-up candidates,
not deletion targets in this rendering cut. The pinned audit/research artifacts
remain historical evidence; `pr-review.html` now describes actual APIs.

Remaining: affected-suite gate and publication, then native geometry,
focus, exact links and Portfolio review. Six Codex/Claude input trials, paired
wall/CPU evidence and hook approval-once proof remain open. No install, app restart,
takeover, live cleanup or datastore framework rewrite occurred.

Publication review repaired TaskFlowProofTests' stale expectation that the Workflow
graph remains visible in the collapsed header. It now checks compact position/actions
and the graph under Overview. The initial Desktop run failed only that assertion;
the repaired full run passes. The required consolidated gate did not start product
suites: resource recovery exceeded its 60-second wrapper deadline while build pruning
was still running. That invocation's orphaned pruning child was stopped. A subsequent
read-only inventory also failed its five-second uv-cache measurement deadline;
64.4 GiB remained free. Publication is blocked on a successful resource preflight and
affected-suite gate. No PR update, app restart or installation occurred; the separate
datastore study remains outside this checkpoint.

Check: `scripts/test_desktop.sh --jobs 4 -Xswiftc -gnone` PASS (524 tests, 61 suites; optional native diagnostics skipped); architecture, Swift boundary and diff checks PASS; `uv run python scripts/test.py --reuse-passing --base f9a16d6f84 < /dev/null` BLOCKED (resource preflight, no product suites); logs `/tmp/loo462-publish-{desktop-final,gate}.log`, inventory `/tmp/loo462-publish-resources.json`. Native proof remains with demo; affected suites with gate.

### Next pursuit — database-driven UI

Jack Heart authorized starting this after the rendering pass finishes. That pass
finished at `e480b4bc7c`; publication remains blocked as above. Implement
`scratch/datastore-reactivity-study.html`, including **Accepted next pursuit**.
This supersedes the earlier research-only boundary, not the no-second-schema-reader
rule. Preserve rendering and native-proof limits. Return to demo; no landing.
