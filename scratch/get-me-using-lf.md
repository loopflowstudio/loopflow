# Get me using Loopflow — LOO-462

Implementation authorized · October 10, 2026. Jack Heart requested pursuit after
selecting conversation-first Tasks, repository-name conversation access and
repo/session ownership. Pursue runs through publication for demo review, not landing.
Remaining reversible UI details may be resolved during implementation; live data
cleanup remains separate and requires a reviewed, restorable plan.
Product owns this Task. LOO-427 is integrated and completed; historical stacking
details remain at `0bb13e9261:scratch/get-me-using-lf.md`. Portfolio implementation:
`d43aa248d3`; shared-reader compression: `50e2cf44d1`. Reconciled October 13
after refresh repair `279078f02d`, shared publication `a590ecb7ee` and review-app
receipt `2406680a23`, including main `37ca5fca81` and sidebar
implementation/compression `88d91e53ca`/`0480344945`.
The earlier feedback describing
28-point single-line Task rows is superseded by those commits. Sidebar metadata
is implemented; publication checks and repairs are recorded below. Native review remains.

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

Earlier gate and composition evidence: `0bb13e9261:scratch/get-me-using-lf.md`,
**Done when / unattended gate**. October 11 publication at `73d2861c1e` passed
65 Rust acceptance tests (one Linux-only ignored), three portfolio tests, 193
Desktop tests, eight launcher tests and static checks; those revisions are not a
new gate pass. Display diagnostics were skipped and installed runtime/native
proof remained unresolved. Later bundled-runtime authority supersedes the
installed-only hold, never authorizing landing or live migration.

## Current review order — October 11

Jack Heart prioritized the populated Loopflow repo, Wave and Task views: correct
Wave/Task visibility and minimal buttons/chrome. Get those main views feeling right
before revising Portfolio. The repository/Portfolio view remains in scope and is
wanted, but its present design is not accepted. The isolated empty-clone demo failed
to establish the requested experience; see `work-first-demo.md` for the freeze,
focused repair evidence and remaining native proof.

## Sidebar visibility and cleanup evidence

Jack Heart requested a minimal working set without Session tallies. Shared Started
and unresolved execution now select ordinary Tasks; Session presence alone cannot
admit unstarted or settled Tasks. A stopped failed Flow does not keep a completed
Task current; live/unknown execution, completion refusals and unsettled files do.
Task/Session history, binding and retained panes survive these filters.

Jack authorized exact real-data cleanup after rehearsal in the private copy.
LOO-376 completion and the 126-row legacy repair were applied; restoration receipts
and limits remain in work-first-demo.md. No broader cleanup is authorized.
LOO-276 remains unclassified; merge alone cannot dispose of LOO-443/400/390/382/
380/371. LOO-378 remains explicitly paused; LOO-367/330/389 have unsettled work.
LOO-312 was subsequently recovered without changing its identity. Historical
inventories, transient config failure, native screenshot, exact candidates and
focused checks: `5b62453be7:scratch/get-me-using-lf.md`, **Sidebar cleanup and
recurrence**. These observations are not a current execution inventory.

## Paused and prepared demo — October 13

Jack Heart selected Paused and authorized pausing LOO-389. Independent Task
lf-2086e0a / PR #1544 merged as f2855f7bd46aa93d57b3a2e56fc8ec917f266ea3;
installation/native pause remain outstanding. Preserve identity, history, checkout
and Workflow; resume starts nothing and pause never kills a live process.
LOO-427 is completed after verified merge; no unknown Process was retired.
Earlier CI/queue failures, exact native trust observations and preserved draft
receipts: `185c0b3c2e:scratch/get-me-using-lf.md`, **Paused**.
Older app receipts remain at `c89c789fd1:scratch/get-me-using-lf.md`, this heading.
CMUX title environment scrub (9e79fa4970) awaits deployment; eight checks passed.

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
branch/path clutter. Native Link controls are siblings of the title button; only
the title selects the conversation. Existing workspace and surface owners survive.

Compared the current [Rex OSC 7501 contract](https://www.superlogical.com/rex/docs/build/program-status)
on October 13: the existing reducer matches states, record replacement, clear/exit
lifetimes and source-scoped validation. Sidebar rendering consumes validated shared
Session reports; it does not parse terminal output. Across Sessions, report severity
wins, primary then stable Session ID breaks ties, and the tooltip identifies the
Session. Reports outrank inferred shared Waiting/client activity. Cleared reporters
stay unknown; failed readings stay unavailable. No Claude/Codex rollout is assumed.
Done/Error never supplies Task completion, Paused or Workflow position.

Rust adds the optional Machine label to the existing checkout projection, reading
registered Machines once per planning snapshot and joining the recorded checkout
owner. Changed delegation defaults cannot relabel started work. Swift omits local
names; unknown ownership and unnamed remote Machines stay explicit. No assignment
label, discovery polling, network read or second owner is added. The shared Rust
and Swift fixtures carry the field. Review found saved Session pages retaining
old reports/Waiting; cache restoration now strips both with client liveness.

Focused proof covers report/inference/unknown precedence, independent terminal
record IDs, clear/cache boundaries, stored link destinations outside title actions,
retained layout/Files draft, remote/local/unknown names and recorded ownership after
changed delegation and Machine rename. Browser dispatch and density remain native
demo checks; headless inspection cannot synthesize either.

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

### Delivery and demo

Hook fix `185c0b3c2e` and sidebar/lookup checkpoint `0bb13e9261` are preserved;
their focused checks remain applicable. Main `414d4dd27c` brings the authored
Wave move into `.lf/wave/` and shared Task pause handling; `cf67067b0d` removed
the duplicate pause method introduced by integration. Its five Rust and 31 Swift
focused passes are recorded at `cf67067b0d:scratch/get-me-using-lf.md`.

Affected-suite checks and fixture repairs are complete below; publication is for demo review.
Unlaunched review app:
`/tmp/loo462-oct13-retention-compress/Loopflow Work Demo.app` (`a590ecb7ee`),
with matching validation-only helper and verified signature; receipt in
work-first-demo.md. Main `37ca5fca81`, merged as `c89c789fd1`, changes CI build
scheduling and helper preparation without changing Work or the helper's migration
authority. It supplies no native acceptance or newer app receipt.
Earlier app: `/tmp/loo462-oct13-compress/Loopflow Work Demo.app`,
built from `0480344945` with its matching validation-only bundled lf; signature
verified. Jack Heart subsequently launched it and reported a failed fresh-planning
demo; see work-first-demo.md. Its build-time `launched: false` is historical.
`build.json` beside it records source/signed hashes and revision.
The earlier sidebar app at `/tmp/loo462-oct13-sidebar/Loopflow Work Demo.app`
(`88d91e53ca`) remains preserved.
SwiftPM resource lookup still relies on the retained checkout build directory;
this private review artifact is not portable. Earlier profiling/demo apps remain.
Jack Heart requested
relaunch after UX fixes, preserving both existing apps' live panes and unsent
drafts. A replacement must not destroy that input. Real Loopflow repo/Task review,
six provider/case timing trials, measured-cause optimization and stable-hook proof
remain; Portfolio review follows the main experience. No global install, takeover,
live migration, broader cleanup or landing is authorized. No product decision is
needed for the remaining implementation; visual acceptance stays with demo review.

Earlier sidebar Rust/DTO/cache checks: `69f7005b05:scratch/get-me-using-lf.md`, final check; not a current gate.

### Native failure repair — October 13

Jack Heart observed the 0480344945 app launch, failed fresh planning (`t.paused`),
lost cached rows and “No workflow yet.” The log is empty and input trace absent;
this supersedes the unlaunched-app account above, without supplying diagnostics.
Delete — do not maintain: unconditional replacement/cache writes for partial
planning; stale-null Workflow absence claims; disconnected Task readings left
current. These paths are removed. Last-good navigation, pane/draft identity,
independent Task reads and successful-null Workflow semantics survive. A new
partial reading cannot discard previously available content; already unavailable
Waves do not prevent healthy readings from advancing. The released-schema fixture
reproduces the SQL failure before planning, so the separate partial-frame bug is
not a proven explanation for the native disappearance. The exact native trigger
remains for demo. Current saved selection is LOO-382 (no captured Workflow), while
LOO-462 retains feature/demo; that later inspection is not a screen receipt.

The real store needs a published release containing the currently unreleased
`task_paused` migration, installed through `lf install`; reinstalling 0.13.13
cannot supply it. No live schema change or invented compatibility value. Remaining:
publication, then real-schema review, six provider timing cases, hook trust and
visual acceptance.

Review found and fixed stale Workflow mutation buttons and old tests that expected
Task ancestry to vanish on incomplete planning. No new Workflow/cache authority.
Reconciliation inspected the production reader, shared roadmap publication,
Workflow header and regressions. The repair is implemented; no additional code
mismatch was established. The installation prerequisite remains outside this
publication-only pursuit, and no native acceptance follows from retained fixtures.

Publication review repaired the missing Portfolio architecture owner and integration registration, narrowed toolbar inspection to avoid traversing an unmounted terminal, and updated two hook fixtures to follow the current manifest/environment handoff. The Workflow fixture now supplies valid planning after an action instead of treating the new stale-data disabling as a failure. Production behavior was unchanged by these repairs. The full Rust run reported one leaked child in the passing, unrelated `checkout_restoration_reserves_stashed_sidecar_directories` test; no native acceptance follows from the automated runs.

Check: `uv run python scripts/test.py --reuse-passing --base 37ca5fca81` plus its bounded Rust/Swift repair commands: Python 455 PASS + sandbox 1 PASS, website 75 PASS; full Rust 2590 PASS/2 stale-fixture failures/18 skipped, then affected Flow/steer 32 PASS; full Swift 518 tests/1 stale-fixture issue, then `scripts/test_desktop.sh --jobs 4 --filter DesktopHeadlessTests -Xswiftc -gnone` 12 PASS (toolbar repair 1 PASS before full rerun); architecture, Rust inventory, fmt, Clippy, source CLI build, Swift boundaries and `git diff --check` PASS. One passing Rust sync test leaked a child; hosted matrix remains with delivery/CI, native schema/input/trust/visual proof with demo. Local receipts: `.lf/tmp/gate/run-51987`, `.lf/tmp/gate/loo462-publication-rust`, `.lf/tmp/gate/loo462-publication-repairs`, `/tmp/loo462-publication-desktop-repair.log`.

## Color and activity redesign — October 13 review direction

Jack Heart requested preserving the palette while assigning consistent roles across
Desktop: neutral structure and text, selection independent of status, quiet links,
restrained burgundy emphasis, and evidence-backed blue/amber/red activity. Keep
Task lifecycle and Workflow separate from program activity. Omit unknown activity;
a live client alone cannot claim Working. This supersedes the sidebar's current
“Status unknown”/“Active · inferred” presentation, not its underlying observations.
The semantic-token cutover and rendered redesign remain unimplemented. The review
in `pr-review.html` exposes this mismatch rather than treating the request as done.


### Implementation cut — October 14

Jack Heart requested continuing the redesign after the walkthrough. One coherent
Desktop cut in LOO-462, returning to demo; no landing or installation authority.

- Keep `LoopflowPalette` as the color owner. Add explicit roles for selection
  fill/edge, keyboard focus, link ink/hover, and emphasized versus ordinary actions.
  Reuse cream/charcoal, burgundy/rose, blue, amber, green and failure red. Do not
  invent a new theme or force appearance; both existing appearances need deliberate
  role assignments. Text uses primary/secondary/muted ink, never selection hue.
- Cut over `WorkNavigator`, `TaskRowMetadata`, `WorkBreadcrumbBar`, shared button
  styles, Workflow and Files/control chrome to these roles. Repo identity may retain
  burgundy; Wave/Task titles stay neutral. Links are secondary ink with underline
  on hover/focus. Selection uses tint plus edge; keyboard focus uses a distinct
  outline and survives selection. Ordinary buttons are neutral; emphasized action
  alone uses burgundy. Red remains failures/destructive affordances, not navigation.
- Keep status evidence intact. Presentation omits unknown and client-liveness-only
  labels/dots, without falsely classifying either as idle. Valid working reports
  use blue; supported Waiting/blocked uses amber with wording that preserves their
  distinction; actual errors use red. Successful program reports are not Task
  completion. Read failures remain inspectable without painting every row as failed.
- Delete replaced direct `accentInk` choices and duplicate Desktop status-color
  mappings rather than layer another theme over them. Preserve terminal ANSI colors,
  selected-failed combinations, retained panes/drafts, APIs and all data records.
  Update `VISUAL_DESIGN.md` to separate Desktop semantics from marketing examples.
- Focused checks: palette/row/headless view tests for unknown/active omission,
  reported working/error, shared Waiting, selection plus failure, muted links and
  focus visibility. Build using `scripts/test_desktop.sh` with affected filters;
  contrast checks must resolve actual light/dark colors and composited backgrounds
  (4.5:1 ordinary text, 3:1 meaningful indicators). No empty filter counts as proof.
- Demo: same real Loopflow repository in the matching bundled app, sidebar → Task
  → Files/Workflow. Inspect both appearances, hover/focus/selection and status
  combinations without creating fake live reports or interrupting user clients.
  Refresh `pr-review.html` after implementation so it stops describing old code.
