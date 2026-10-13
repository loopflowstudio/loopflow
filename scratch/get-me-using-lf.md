# Get me using Loopflow — LOO-462

Implementation authorized · October 10, 2026. Jack Heart requested pursuit after
selecting conversation-first Tasks, repository-name conversation access and
repo/session ownership. Pursue runs through publication for demo review, not landing.
Remaining reversible UI details may be resolved during implementation; live data
cleanup remains separate and requires a reviewed, restorable plan.
Product owns this Task. Latest LOO-427 base `7bcff15648` is integrated at
`7eedf5b060`; parent landing remains a delivery dependency. Portfolio implementation:
`d43aa248d3`; shared-reader compression: `50e2cf44d1`. Reconciled October 11
against those changes, parent integration and the failed configured demo.

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

Earlier discovery observed 2,419 historical Sessions, 138 interactive, 2,281
headless; the unlimited default interactive inventory had 106 (11 active,
18 interrupted, 77 unknown), 61 without Task association. These are prior
observations, not a fresh inventory or a deletion candidate set. Read-only cmux
inspection showed Loopflow, Etude, Fantasia and Kata windows with task workspaces.
The original uncommitted discovery was preserved at
`/tmp/loo462-discovery-ddc009d/` before consolidation; its quotes and open decisions
are retained here and in questions.md. The attempted lf checkpoint returned an
empty error; no successful checkpoint is inferred.

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

No deletion targets remain in this navigation cut. The launcher now reuses
`cmd_install` for build/install in both launch modes, leaving their presentation
separate. Task entry uses the pane's existing `sessionId` projection to detect a
retained conversation instead of repeating its content switch.

## Implementation and remaining work

October 11 continuation after parent integration `c040d9fa03` preserves the
completed Work-first cut. The configured demo failed with an unrecognized
`repo identity` command despite successful bundled help. The launcher repair
removes its misleading runtime claim: `run`/`run-debug` now check
the actual `repo identity --json` operation through the bundled helper before
opening the app. This is ordinary repository registration, not an effect-free
capability query. October 12 bundled selection supersedes installed forwarding;
migration authority and data selection stay unchanged. No identity fallback,
global installed-CLI replacement or private-Machine substitution is added.
October 12: Jack Heart authorized bundled-runtime work for real-data demo;
see work-first-demo.md **Bundled runtime**. The installed-release-only hold is
superseded; schema and process-control boundaries remain.
[The failed demo](work-first-demo.md) records the observed runtime mismatch; no
new configured demo has succeeded. [Portfolio discovery](portfolio-discovery.md) records the prior-art investigation.
Jack Heart added portfolio management to LOO-462 on October 11. The shared
CLI/Desktop implementation is present; affected gate coverage and native review
remain. It belongs to this Task and the existing Portfolio surface.

The consumer cut removes the generic composer, skill picker/storage/launch DTO,
Debug Session inventory, orphan projection and Session-title search, including
palette Session destinations. Scoped pane-owned conversations and full underlying
inventory remain. WorkPalette was an additional predecessor not named initially.
Repository-name opening uses the existing primary owner; first-use acquisition is
one-shot per retained repository entry. Current empty planning is only a candidate:
a full historical Wave list must also be empty. A late history check cannot start
entry after navigation; a late ensure populates only its originating retained Work.

Task rows and palette entry reuse primary preparation and existing linked opening
with no Changes companion. Bare CLI links retain passive inspection. Returning to
a retained conversation skips preparation. Flow-log inspection remains passive.
The existing Workflow header adds a collapsed Overview description. Repo/session
invites ideas without setup when no Work exists.

The shared `tests/fixtures/dto/work_first_entry.json` now comes from the real Rust
CLI against an isolated Home with a pinned stand-in provider. It retains empty
planning, repeated primary admission, mixed Session history and populated Work.
The lifecycle test checks configured provider/model, exact primary reuse, no Flow
creation and the assembled repo/session invitation. Swift consumes that capture
through the actual repository-entry callback, outline and retained Work owners:
loading/error entry, hide-all, reopen, collapsed Overview and unsaved Files state.
No mounted provider or native usability is inferred.

Remaining acceptance work:
- Earlier gate suites passed on `8efc1234a1`; parent conflict checks and the
  launcher/Task-entry compression checks below cover subsequent changes. These
  are distinct revisions, not a claim that a full gate ran at `df40c477ef`.
- Native usability/demo review remains separate from the headless build.
- Cleanup remains a proposal only. No exact iteration-owned candidates are known
  or approved. Remaining investigation needs a private candidate/artifact inventory,
  consistent backup and isolated restore with fake providers before exact deletions
  can be reviewed.
  Unknown/interrupted records remain untouched. No live cleanup occurred.

Review found and fixed two concrete hazards: current-only Wave readings could
misclassify historical repositories as empty; making Task opening asynchronous
could redirect the Flow-log companion into the previously selected workspace.
Earlier focused attempts failed on deleted navigation expectations, a wrong Layout
member and an accessibility-prefix collision. The composed fixture initially used
a private Rust DTO import and an invalid provider model; Swift first ran before
the capture existed. Those fixture failures are repaired, not native evidence.
Review kept the unrelated Session in a separate checkout so Rust's real Task
association cannot silently turn that case into a Task member.


## This slice — one repository portfolio owned by lf

Jack Heart requested default discovery of main repositories under `~/src`, a
changeable discovery folder, or manual selection across folders/subfolders. Jack
explicitly included the existing Portfolio UI: build on or replace it in this
same cut. Jack selected the Portfolio view as the primary configuration surface:
choose a discovery folder or individual repositories there without requiring CLI
setup. The view calls the lf API for reads and mutations through RegistryQuery;
it must not scan folders or write configuration itself. lf owns persistence and
discovery, not a competing Swift list. Jack considered multiple discovery roots
and said they are probably unnecessary; defer them in this slice.
This extends LOO-462's Work-first experience through publication/demo, not landing.

**Implemented contract (reversible implementation choices):** one
Machine-local portfolio selection, with folder and manual modes. Folder mode
uses the existing `repo_root` (default `~/src`) and discovers immediate main Git
checkouts, excluding linked worktrees. Manual mode uses explicit main-repository
paths anywhere, including nested directories; it never silently unions folder
scan results or the last opened repo. Preserve the inactive mode's settings when
switching modes. Recursive folder scanning and per-repo folder exclusions are
not required by the current request. No GitHub remote, Wave or Task is required.

Use `lf portfolio` as the owner: list/read JSON, select folder/manual
mode, set the root, and add/remove explicit repositories. Final subcommand syntax
comes from the existing CLI conventions, not a parallel Desktop API. Changing
the folder reuses `repo_root`, never a second root preference. Portfolio membership
and the open-window set are separate: opening a repo does not enroll it; removing
one affects navigation only, not files, Work, live panes, Sessions or execution.
Use existing repository identity and Git main-checkout normalization. Paths remain
Machine-local locators, not portable identity or execution assignment.

Rust owns selection persistence, normalization, enumeration and mutations. Source
inspection found no general Machine-local configuration writer; `config.rs` now
provides the atomic writer, with portfolio mutations holding the config lock.
No planning or execution table is needed. RegistryQuery exposes one typed reading
with selected mode/root, repository entries and unavailable-path/read errors.
Inspection creates no Work or conversations. Missing manual entries remain removable/unavailable, not silently
deleted. A failed folder read is not an empty portfolio. Surface the same errors
in CLI and Desktop; unknown state retains existing panes.

**Existing UI, not a new window:** `LoopflowApp`'s `Window("Portfolio")` currently
mounts `Views/WavesView.swift`. Adapt that view's repository rail and management
controls, preserving its useful Wave/status contents. Provide folder/manual mode
selection, a folder chooser, and add/remove individual repository controls here.
Native pickers select paths only; shared lf operations validate and save them.
Show pending/error state and render the authoritative result without requiring a
terminal or silently assuming a write succeeded. `RepoView`, Open Repo/Move
to Repo menus and repository windows consume the same shared membership. Keep
window-local selection/last-opened presentation separate from membership. Respect
configured data locations and normal installed CLI dispatch.

**Delete in the cut:** Swift `PortfolioService` UserDefaults membership writer,
`PortfolioDiscovery.repos`' hard-coded scan/persisted/launch union, and the
portfolio-only `RepoScanner` duplication after surviving callers use the shared
reader. Retain authored-Wave discovery until its own replacement exists. Reuse or
reshape `PortfolioRepo` as the shared DTO rather than keeping two representations.
The old `portfolioRepos` preference needs a one-time explicit import into shared
manual membership; never discard it, auto-publish it or re-import on every refresh.
Offer import only when shared selection is absent; CLI-authored selection wins.
Remove obsolete exclusive tests; migrate behavior tests to the shared owner.

**Implementation, October 11:** `portfolio.rs` owns the selection and reading;
`lf portfolio [folder [PATH]|manual|add PATH…|remove PATH|import PATH…] --json`
exposes it outside repositories. The Machine's `config.yaml` stores selection beside
`repo_root`, preserving other keys with a locked atomic write. No existing general
Machine-config writer was present; the atomic writer now lives in `config.rs`.
No database migration or repository registration is needed.

PortfolioService now retains shared readings and errors; it writes no preferences.
WavesView configures both modes with native path pickers and calls RegistryQuery.
Repo windows and menus read the same owner, refreshing on app activation. Import
reads old `portfolioRepos` without changing it; Rust refuses import after any shared
selection exists. The service serializes mutations after pending reads.
`PortfolioDiscovery.repos`, its scan/launch union, Swift membership persistence,
old PortfolioRepo and scan-only tests are removed. `RepositoryPath` retains only
explicit-open validation from RepoScanner; authored-Wave discovery remains.
Compression also removes `WorkModel.refreshPortfolio`'s duplicate CLI reader and
error state, `WavesView.repos`' copied membership, and RepositoryPath's unused
FileManager field. Repository windows apply PortfolioService's shared reading;
the Portfolio view derives membership directly. Failed reads retain membership,
activation still refreshes authored Waves, and canceled/superseded discovery cannot
replace newer data. The delayed-read test now covers serialized service mutation.
No deletion targets remain in the portfolio cut.

Review fixed a global Clap `--json` identity collision and a rewritten-path mismatch
between shared membership and authored Waves. Failed folder reads retain the last
usable membership and expose the authoritative mode/root plus an error. Opening
and removing membership do not touch retained Work. No acceptance is inferred from
native path-picker code; both modes still require demo review.

**Evidence and remaining acceptance:** Rust fixtures cover isolated roots with two
repos, a linked worktree and a nested repo; folder changes, manual membership
across directories, alias normalization, unavailable entries, explicit import and
read-without-store-creation. Swift fixtures cover the shared DTO, error retention,
serialized mutations and CLI readback through an isolated runtime. The retained-Work
case removes nonempty membership without replacing panes or the unsaved document.
These are automated owner/retention checks, not native path-picker acceptance.

Publication checks now cover the affected Rust portfolio/CLI, lifecycle/DTO and
Desktop suites at integrated revision `73d2861c1e` (results below).
`PortfolioServiceTests.desktopWritesReadBackThroughCLI`
requires `LOOPFLOW_PORTFOLIO_TEST_BIN` pointing to the built branch CLI; otherwise
it is skipped. Native review must demonstrate both modes entirely from Portfolio,
CLI readback, menu refresh and retained conversation/draft. The installed release
must support both repository identity and portfolio commands; the launcher probes
identity only. Multiple discovery roots remain deferred; cleanup is unauthorized.

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

Publication check commands (inherited LF_* removed, stdin closed; lifecycle uses
`TMPDIR=/private/tmp uv run python scripts/test_network.py`):
- `cargo test -p loopflow --test session_lifecycle_tests` — repo-primary reuse and
  mixed membership pass with isolated, pinned fake providers.
- `cargo test -p loopflow --test dto_fixtures` — shared capture decodes in agreement.
- `scripts/test_desktop.sh --filter 'WorkNavigationTests|WorkNavigationProofTests|WorktreeWorkspaceTests|DTOFixtureTests' -Xswiftc -gnone`
  — app builds and nonempty headless tests pass, including new entry cases.
- `cargo test -p loopflow --test documented_commands` — changed examples resolve.

Gate owns the above suites once. Prompt fixtures inspect assembled input and
assert no mandatory setup or operating pass before the no-work invitation, without requiring a real model.
Rendered judgment is demo/review, not a headless gate. This Task supplies no native
provider acceptance, installation, or three-day adoption credit. Chapter KRs remain
historically readable; October 7 retired numeric/soak acceptance, not retention or
usefulness. No external-product progress credit follows from Loopflow UI work alone.

Earlier navigation, launcher, parent-integration and gate evidence is preserved in
`20298cddcb:scratch/get-me-using-lf.md` (the portfolio-scope checkpoint before this cut).
The failed configured demo remains in `work-first-demo.md`. Earlier isolated
provider and headless passes do not establish native usability or installed
runtime support.

The portfolio pass also regenerated `work_first_entry.json` through its isolated
Rust lifecycle producer: the parent had added Task acquisition metadata absent
from the older capture. Swift round-trip assertions compare shared readings;
Foundation's `/var` spelling differs from Rust's canonical `/private/var` path.
The retained-Work test removes a nonempty membership while preserving the exact
conversation, pane layout and unsaved document.
WorkModel fences asynchronous authored-Wave discovery; PortfolioService serializes
shared reads and mutations so a delayed read cannot overwrite a later removal.

Earlier Rust portfolio, formatting/lint and fixture-generation checks remain at
`d43aa248d3:scratch/get-me-using-lf.md`; compression evidence is retained at
`50e2cf44d1:scratch/get-me-using-lf.md`. Parent integration at `7eedf5b060` also
removes mandatory follow-through APIs and Task-header delivery controls; the
conversation-first opening and portfolio owners remain. No gate on this combined
revision is inferred from the earlier passes. Publication/demo remains the limit
of LOO-462 authorization; the parent’s changed completion contract adds no landing
authority here.
Review preserved successful-read gating at startup and authored-Wave refresh on
activation while removing duplicate membership reads.

Check (October 11 realign): `git diff --check` and `lf context` — pass, both context budgets fit; prose-only reconciliation reuses prior focused evidence, integrated suites belong to gate and native review awaits a compatible installed release.

Check (October 11 publication, `73d2861c1e`): network-isolated `cargo test -p loopflow --test session_lifecycle_tests --test dto_fixtures --test documented_commands --test portfolio_cli_tests` — 65 passed, one Linux-only ignored; `cargo test -p loopflow --lib portfolio::tests` — 3 passed; `LOOPFLOW_PORTFOLIO_TEST_BIN="$PWD/target/debug/lf" scripts/test_desktop.sh --filter 'WorkNavigationTests|WorkNavigationProofTests|WorktreeWorkspaceTests|DTOFixtureTests|PortfolioServiceTests|PortfolioRepoStateTests|WorkDestinationTests|WorkCacheTests|WorkModelTests|SessionControlsTests|SessionChromeProofTests' -Xswiftc -gnone` — successful 193-test run including CLI readback, display diagnostics skipped; `uv run pytest python/tests/test_loopflow_dev.py -q` — 8 passed; `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, Ruff and diff whitespace — pass; native review still requires compatible installed identity/portfolio commands.

## Current review order — October 11

Jack Heart prioritized the populated Loopflow repo, Wave and Task views: correct
Wave/Task visibility and minimal buttons/chrome. Get those main views feeling right
before revising Portfolio. The repository/Portfolio view remains in scope and is
wanted, but its present design is not accepted. The isolated empty-clone demo failed
to establish the requested experience; see `work-first-demo.md` for the freeze,
focused repair evidence and remaining native proof.

## Sidebar cleanup and recurrence — October 11

Jack Heart rejected numeric conversation controls and requested a minimal visible
working set. Remove sidebar Session counts/dropdowns; retain exact conversations
in their Work and contextual opening. Jack authorized real-database cleanup after
figuring out and rehearsing the specific changes in the private copy. This supersedes
the blanket live-cleanup hold, not the requirement to preserve active work and review
uncertain cases. The exact rehearsed LOO-376 and 126-row repairs are recorded
in `work-first-demo.md`; no broader cleanup is implied.

Keep two tracks separate: iteration debris from retired implementations/experiments
can receive a one-time restorable repair; states reachable through ordinary current
use need a lifecycle/visibility correction and regression proof. Unknown provenance
is neither a deletion candidate nor evidence of recurrence.

Read-only audit of the copied Loopflow data found 358 Tasks, 32 with unfinished
interactive conversations: 22 completed, two canceled, seven unstarted in planning,
and one with unknown planning state. Every one has recorded Started evidence.
This is a snapshot, not current process liveness. Exact inventory is retained at
`/private/tmp/loo462-native.FgtDmE/sidebar-data-audit.json`.

Confirmed recurring bug: Task completion is independent of Session completion,
but `inWorkingSet` returned true for any associated visible conversation, ahead of
Task state. Settled completed/canceled/duplicate Tasks now stay out of ordinary
navigation unless shared execution evidence remains unresolved. October 12 removes
the same shortcut for unstarted Tasks: Session presence cannot establish Started. Conversations,
Wave plan/history, retained panes and binding are not changed. Tests cover all three
terminal planning states with/without unresolved execution while retaining Sessions.

Iteration candidates remain unclassified. LOO-276 under abandoned Wave `list`,
with unknown planning state and retained conversation, warrants provenance inspection;
neither its age nor its parent name authorizes deletion. The shared Work reader
currently reports an unrelated parse error in main's `.lf/config.yaml`; audit must
not interpret that unavailable planning as an empty inventory or repair another
checkout's active contribution.

Check: `scripts/test_desktop.sh --filter WorkNavigationTests -Xswiftc -gnone` — 36 passed after count removal and terminal-visibility regression; native populated re-review remains.

### Remaining classification after rendered sidebar review

The config parse error was transient and is now gone. Native capture
`/tmp/loopflow-20261011-213231.png` shows the current smaller sidebar without
conversation tallies; all four main Waves fit in the viewport. The app still lands
on “Choose Work”; repository conversation entry and Task workspace review remain.

Second reproducible current-state bug: a completed Task retains a failed Flow,
and the condition projection treats that stopped failure as unresolved execution
forever. LOO-440 has a recorded merge and later completion at 1791767222, clean
observed checkout, and an earlier blocked landing attempt. Exclude stopped failed
Flow state from the terminal Task's unresolved-execution predicate, while retaining
Starting/Running/Stalled/Unknown evidence, completion refusals and observed unsettled
files/commits. Do not rewrite the execution reading or delete the failure history.

Data-repair candidates, not dispositions: LOO-443/400/390/382/380/376/371 have
recorded merges but no completion events in the copied history. This can also arise
through ordinary unfinished delivery, so merge alone does not classify them as
iteration debris. Inspect accepted remaining scope before completion. LOO-378 is
explicitly paused with unpublished work in Product memory; preserve it. LOO-312 has
a missing checkout and remains unfinished; missing files do not authorize closure.
LOO-367/330/389 retain dirty files or unsettled commits; preserve them pending review.
No real-data or copied-history deletion/retirement has been performed.

Jack Heart distinguishes iteration debris from reproducible bugs. Repair current-use
bugs in code; rehearse exact data repairs with restoration before cleanup. Unknown
or interrupted Sessions never authorize closure.

Check: `cargo test -p loopflow --lib a_finished_task_stays_current_only_while_execution_is_unresolved`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt` — pass; native refresh of the stopped-failure correction remains.

Earlier LOO-376 completion and 126-row legacy reconciliation, including exact
selection, rehearsal and rollback limits: `d65dff0cbb:scratch/get-me-using-lf.md`,
**Applied legacy reconciliation** and preceding paragraphs. Applied changes remain;
this grants no broader cleanup authority. Current receipts: work-first-demo.md.

## Paused — accepted October 12

Jack Heart selected Paused and authorized pausing LOO-389. October 13: ship the
independent lf-2086e0a / PR #1544 first. Repair head 9c6f263ebc is armed; CI pending.
Preserve Task identity, history, checkout and Workflow. Default active navigation
omits paused Tasks; the Wave plan keeps them reachable. Resume clears the hold,
never launches execution. Pause does not kill an already-running process.

The common writer owns portable Paused, independent of Linear disposition.
CLI/Desktop share it; pause blocks future Flow/automatic repair launches.
Verify native pause before applying LOO-389.
October 13 native recheck: CI opens its correct Codex conversation and input prompt;
LOO-380 retains Claude and its draft. LOO-312/367 initially showed hook review;
Jack trusted LOO-312 on October 13; native input/tool execution now appears.
LOO-367 still shows its own trust prompt; no choice submitted there.

Paused is not deployed: main's schema needs supported installation; preserve
active clients. LOO-427 completed through `lf task complete` after verifying merge
`9211b47c47bf986348af00b5a546c7cce2048c8c`; native sidebar removed it.
Checkout/unknown Process `f1482ca7-35a6-4bea-89f8-1f2d5c3de5b2` remain.
Earlier failures/repairs:
`81ae973322:scratch/get-me-using-lf.md`, **Paused**.
Local checks/limits: `9e79fa4970:scratch/get-me-using-lf.md`, **Paused**.

Title leak: GUI now scrubs inherited CMUX keys to prevent host renaming;
eight launcher tests pass (9e79fa4970). Deployment pending.
October 13: candidate build/signature check PASS at
/tmp/loo462-next-demo/Loopflow Work Demo.app (50d6451b4, bundled lf).
Not launched; both live apps/drafts preserved. Jack queued PR #1544 October 13;
position 1, AWAITING_CHECKS, run 38452715134. Earlier auto-merge did not enqueue.
