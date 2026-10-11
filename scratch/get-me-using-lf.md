# Get me using Loopflow — LOO-462

Implementation authorized · October 10, 2026. Jack Heart requested pursuit after
selecting conversation-first Tasks, repository-name conversation access and
repo/session ownership. Pursue runs through publication for demo review, not landing.
Remaining reversible UI details may be resolved during implementation; live data
cleanup remains separate and requires a reviewed, restorable plan.
Product owns this Task. LOO-427 is integrated at `c040d9fa03`; its landing remains
a delivery dependency. Launcher repair: `1787d94bc9`; compression: `df40c477ef`.
Reconciled October 11 against that implementation and the failed configured demo.

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

## Removed predecessors

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
capability query. Normal installed dispatch and migration authority remain intact;
no identity fallback, installed-CLI replacement or private-Machine substitution
is added. The existing development app build/install remains.
Real-data native acceptance still requires a compatible installed release.
[The failed demo](work-first-demo.md) records the observed runtime mismatch; no
new configured demo has succeeded. [Portfolio discovery](portfolio-discovery.md) records the prior-art investigation.
Jack Heart added this outcome to LOO-462 on October 11; it is now the next
implementation slice, not a separate Task or a second Portfolio surface.

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
setup. The view must CALL the lf API for reads and mutations through RegistryQuery;
it must not scan folders or write configuration itself. lf owns persistence and
discovery, not a competing Swift list. Jack considered multiple discovery roots
and said they are probably unnecessary; defer them in this slice.
This extends LOO-462's Work-first experience through publication/demo, not landing.

**Proposed concrete contract (reversible implementation choices):** one
Machine-local portfolio selection, with folder and manual modes. Folder mode
uses the existing `repo_root` (default `~/src`) and discovers immediate main Git
checkouts, excluding linked worktrees. Manual mode uses explicit main-repository
paths anywhere, including nested directories; it never silently unions folder
scan results or the last opened repo. Preserve the inactive mode's settings when
switching modes. Recursive folder scanning and per-repo folder exclusions are
not required by the current request. No GitHub remote, Wave or Task is required.

Use `lf portfolio` as the proposed owner: list/read JSON, select folder/manual
mode, set the root, and add/remove explicit repositories. Final subcommand syntax
comes from the existing CLI conventions, not a parallel Desktop API. Changing
the folder reuses `repo_root`, never a second root preference. Portfolio membership
and the open-window set are separate: opening a repo does not enroll it; removing
one affects navigation only, not files, Work, live panes, Sessions or execution.
Use existing repository identity and Git main-checkout normalization. Paths remain
Machine-local locators, not portable identity or execution assignment.

Rust owns selection persistence, normalization, enumeration and mutations. Use the
existing Machine-local configuration writer for this preference; no new planning
or execution table unless source investigation demonstrates a real need. Expose
one typed reading through RegistryQuery, with selected mode/root, repository
entries and truthful unavailable-path/read errors. Inspection creates no Work or
conversations. Missing manual entries remain removable/unavailable, not silently
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

**Order:** shared Rust selection/CLI and fixtures; cut all Desktop membership
consumers and existing Portfolio controls over together; remove superseded owners;
prove cross-surface round trips. Preserve the existing conversation-first work.

**Proof:** isolated roots with two repos, a linked worktree and a nested repo;
default/root-change selection excludes the worktree, manual mode selects across
roots/subfolders, aliases deduplicate, errors retain entries, and reads launch
nothing. Rust/Swift DTO fixtures agree. CLI edits update the existing Portfolio
and repo menus; Desktop edits read back identically from CLI. Switch modes and
remove membership while retaining an open conversation/draft. Run focused Rust
portfolio tests and existing PortfolioServiceTests/RepoScannerTests replacements,
WorkNavigationTests and DTO fixtures; gate owns wider suites. Native review shows
configuration of both modes entirely from Portfolio, CLI readback of the same
selection, and retained Work. Multiple discovery roots are deferred. No live
migration or cleanup is authorized.

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

Supplied compression evidence for `8f95d9e4e6`: passing Swift build and 101 navigation,
destination, controls and cache tests under denied network/WindowServer access;
earlier Rust fixture/fmt/clippy results remain applicable. Initial failures from
retired fixture commands and helper framework lookup were repaired. These are
focused checks, not gate completion or native acceptance. Product has no immediate
child memory directories in this checkout; the complete selected memory was read.
Uncommitted pre-reconciliation notes are preserved at
`/tmp/loo462-realign-8f95d9e4e6/`.

Prior gate evidence (`8efc1234a1`): documented commands 3, DTOs 43, isolated
Session lifecycle 21 (one Linux-only ignored), Desktop 92 passed. The first
lifecycle attempt failed with ambient TMPDIR/readonly SQLite; canonical TMPDIR
repaired the fixture context. Parent sync `c040d9fa03` passed both exact
WorkCache Session-read conflict tests. These results remain applicable; native
usability remains with demo/review. Pre-continuation notes are preserved at
`/tmp/loo462-implement-c040d9fa03/`.

Latest focused evidence (October 11 compression, `df40c477ef`): `uv run pytest python/tests/test_loopflow_dev.py python/tests/test_ghostty_artifact.py -q` passed 9; Ruff and `git diff --check` pass; `scripts/test_desktop.sh --filter 'WorkDestinationTests/taskEntryOpensConversationThenRetainsWorkspace|WorkDestinationTests/delayedPrimarySessionPreparationDoesNotOpenChangesInAnotherTask' -Xswiftc -gnone` built and passed 2 tests (3 cases) headlessly; `lf context` is within both budgets; configured native demo remains release-dependent.

Check (October 11 realign): `git diff --check` and `lf context` pass; prose-only changes reuse focused evidence above; configured native acceptance remains with demo/review after compatible release.
