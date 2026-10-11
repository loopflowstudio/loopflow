# Get me using Loopflow — LOO-462

Implementation authorized · October 10, 2026. Jack Heart requested pursuit after
selecting conversation-first Tasks, repository-name conversation access and
repo/session ownership. Pursue runs through publication for demo review, not landing.
Remaining reversible UI details may be resolved during implementation; live data
cleanup remains separate and requires a reviewed, restorable plan.
Product owns this Task. It stacks on LOO-427; parent integration remains a delivery dependency.
Reconciled October 11 against `8f95d9e4e6`; no implementation changes in this pass.

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

## Findings that change the plan

Historical discovery at `ddc009d780` (LOO-427 foundation), retained to explain
the implemented cut. Present-tense descriptions below refer to that baseline:

1. **The sidebar rows are already Work-only.** `WorkProjection.outline` emits
   Waves/Tasks, not Session rows. Remaining inventory exposure is concrete:
   `WorkNavigator.repositoryActions` → Debug → Sessions / Orphan Sessions;
   `orphanSessions` also includes legitimate Wave/repo conversations because it
   means no Task association. Remove that menu, not the underlying Session records.
   `unmatchedSessions` also contains missing-planning Work, so it is not evidence
   that a conversation is unscoped. Retain membership and missing-Work evidence.
2. **A second launch route creates clutter.** `WorkNavigator.sessionComposer`
   offers New Session and a skill picker. `SessionSkillLaunch` launches in the repo
   even with a Task selected; `ConversationLaunch` starts a fresh prompt, rather
   than reusing a primary. Empty-repo entry must not use either as an ensure path.
3. **The primary owner exists.** `ops/human_session/primary.rs::ensure` locks the
   repo/Wave scope, uses `ensure_primary_session`, prepares the persistent scope
   checkout, and starts/reuses the conversation. `ensure_task` separately chooses
   a Task conversation. `RegistryQuery` wraps the Task path but has no repo ensure
   wrapper here. Add the repo call to that existing API, not a Swift primary ID.
   Ensure has effects: deliberate repository-name opening or first-use entry may
   acquire; ordinary reads must not.
4. **Configuration is not the skill picker.** `repository_session` chooses
   `repo-session`; `conversation` resolves provider/model through
   `resolve_task_agent` and ordinary config. An existing conversation retains its
   provider history. The picker remembers a skill for fresh launches, not the
   repo primary's default. Proposed interpretation: preserve those existing
   primary/config semantics; no new default-session preference in this Task.
5. **The prompt contradicts onboarding.**
   `rust/loopflow/src/builtins/ops/skill/repo_session.md` requires an operating pass
   immediately. Amend that skill's no-work entry to invite ideas first, retaining
   ordinary operation for existing work. A separate repo/onboard skill would
   duplicate scope/continuity without a distinct setup mechanism.
6. **Retention already has an owner.** `SessionsWorkspaceRegistry` keys by Work;
   tabs hide, native surfaces and drafts remain. `WorkModel.isSessionVisible`
   filters closed/headless Sessions, not age. Changing it globally could strand
   retained panes. Sidebar suppression must not change the complete inventory,
   membership, native focus/attachment, or Session completion.
7. **Cleanup is not a migration.** Historical
   `0.11.036_delete_sessions.sql` concerns retired schema, not disposable current
   conversations. `scripts/desktop_performance.py::_snapshot` uses SQLite online
   backup from a read-only source and restricted output permissions; it is a
   useful backup primitive, not a full restoration recipe for provider history,
   artifacts and active clients. No cleanup policy or reviewed candidate IDs exist.

No external API or library change is proposed; local source supplies the relevant
mechanisms. No live provider, database cleanup or native rendering was exercised.

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

Compression shares Task selection/presentation before preparation. Two cache
fixtures still held the retired `roadmap` verb; they now hold exact `wave show`
or `session list` prefixes. No deletion targets remain in this navigation cut.

## Implementation and remaining work

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
- Publication checks below pass on `8efc1234a1`; the composed capture is not
  a mounted provider or native usability result.
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

Check: `git diff --check` and `lf context` pass; documentation-only reconciliation reuses prior focused checks; affected suites remain with gate and native usability with demo/review.

Check (October 11, `8efc1234a1`): `cargo test -p loopflow --test documented_commands --test dto_fixtures` passed 3 + 43; isolated `session_lifecycle_tests` passed 21, one Linux-only ignored; planned Desktop filter passed 92; `git diff --check` passed. Initial lifecycle run with ambient TMPDIR failed readonly SQLite; documented canonical TMPDIR repaired the fixture context. Native usability remains with demo/review.
