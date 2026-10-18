# LOO-505 — Tasks with an optional Wave

Status: **Approved for implementation through demo review**, 2026-10-17.
Jack Heart approved the design in the Task conversation (“lg2m”) after selecting
repository defaults, **Task → Wave → repository** workflow precedence, direct
Wave ownership of defaults, and renaming membership `refile` to `move`.
Implementation and the feature workflow's publication for demo review are
approved; landing remains subject to the later review. Baseline: `28dc141671`.
The accepted outcome in [LOO-505](https://linear.app/loopflow/issue/LOO-505) is
singular ownership, optional Wave membership, and usable standalone Tasks.
Decisions are retained in `scratch/direction.md`.

## Outcome and pending demo

Jack wants to capture and work a repository Task before deciding which Wave owns
it. At baseline, `lf task create` without Wave context manufactured an `inbox`
Wave and Project; Task reads depended on that ancestry, and membership changes
refused Tasks with recorded work. The implementation removes those dependencies.

Every Task must retain one identity, checkout, execution owner and optional PR
regardless of how many lists display it. A standalone Task must remain usable
through conversation, checkout, workflow, delivery, recovery and completion.

**Demo:** in a configured disposable repository, run these implemented commands:

```sh
lf task create --no-wave --title 'Fix preview refresh' --json
lf task list --json
lf checkout <task> --json
lf task run <task>
lf task move <task> --wave product
lf task status <task> --json
```

The demo repository has `workflow: code` in `.lf/config.yaml`. Creation returns
a repository identity with `project_id: null` and `wave_id:
null`; repository listing and Desktop show the Task immediately. Open its Task
conversation from the repository, leave an unsent draft, and start its selected
workflow. Move while work is running: the same Task appears under Product,
and its conversation, draft, checkout, branch, workflow position and Process
remain intact. Repository/Available/search appearances select that same Work.
Detach with `lf task move <task> --no-wave`; continue and complete without a
Wave. A second Task stays standalone through completion. The PR variant proves
normal publication/recovery and verified-merge completion rules still apply.

These commands describe the approved interaction. The configured CLI/Desktop demo is
review evidence; unattended tests below prove the mechanics without a display or
real provider account.

## Accepted contract — implemented, acceptance still pending

### Repository identity is required; planning ancestry is optional

Keep the existing `tasks` row and `TaskId`. Add required `repository_id` referring
to the existing Machine-local `repository_plans.id`; make `project_id` nullable.
`Task.wave_id` becomes `Option<WaveId>`, derived through its Project as it already is today.
`store/sqlite/children.rs::TASK_COLUMNS` selects `p.wave_id`; the change is
nullable Project membership, not a new derivation or removal of a stored Task
Wave column. `Task.repository_id` is required in the shared model
and affected DTOs. Paths resolve through repository identity, not Wave names,
GitHub remote names or the current directory of a later caller.

There are two supported shapes:

| Persisted membership | Meaning |
| --- | --- |
| Repository + no Project | Standalone Task; no Wave |
| Repository + one Project | Task belongs to that Project's one Wave |

A Task cannot be waveless while assigned to a Wave-owned Project. This design
also does not introduce Wave-only Tasks: filing to a Wave selects its current
Project, as today. That is the smallest representation satisfying the accepted
outcome; independent Wave-only membership would require a separate product need.

Keep Project→Wave authority and existing Project selection/rotation rules. A
non-null Project must exist and its Wave must resolve to the same repository.
Validate the final relationship in the common planning writer, including import,
sync, rotation and Project moves; enforce the database relationship at writes,
not with consumer fallbacks. Reuse the existing consistency machinery. Refiling
cannot change the Task's repository. Repository relocation changes its address
through the existing repository operation, retaining identity.

`NewTask` receives explicit repository identity and optional Project. Its creation
receipt retains both so a retry cannot change its destination. Creation remains
planning-only and retains the existing admission coordinator. Task assignment
never grants execution ownership. Started Tasks retain their permanent Machine;
unstarted Tasks follow existing explicit assignment/inheritance, with the local
Machine as the existing root default when no ancestor supplies one.

### Creation and discovery

Add mutually exclusive `--wave <wave>` and `--no-wave` to Task creation.
Omitting both retains a **valid explicit/bound Wave** when available. With no
Wave context, create standalone; delete the implicit `inbox` creation path.
`--no-wave` deliberately bypasses ambient Wave/Task ancestry, including creation
from a Wave-bound conversation, but keeps the selected repository. An unreadable,
stale or invalid ambient binding is an error, not proof of no context. Explicit
`--no-wave` does not need an ambient ownership read to choose its destination.
No inferred Wave from an unrelated repository.

Add `lf task list` scoped to the selected repository, with an optional Wave
filter. Reuse the existing Task projection and evidence types. Extend the
planning stream with a repository-scoped Task inventory and explicit membership;
read it from one pinned SQLite snapshot. Share enumeration/projection between
one-shot listing, stream and Wave views. Existing Wave/project detail remains
available; it must not be the only route by which Tasks enter the inventory.
Return unstarted Tasks as well as started ones so capture is discoverable.
Expose required repository identity and optional Project/Wave IDs on the shared
Task summary used by those projections; Rust and Swift mirror nullability with
no DTO defaults. Standalone ordering is scoped to repository + null Project;
Project ordering retains its existing scope. Filtered lists never save rank or
ownership merely by displaying rows.

Desktop uses that inventory through RegistryQuery. Repository entry shows
standalone Tasks alongside its Waves; a Task page has repository context and no
Wave breadcrumb when absent. Needs you, search, palette, history and retained
workspace lookup must include eligible standalone Tasks. Use Task identity for
row/workspace keys, never `wave + task`; refiling changes grouping, not the
selected Work or its panes. Add a deliberate “No Wave” destination to existing
Task creation/refile controls where those controls exist. No sidebar redesign
or independent ownership model: LOO-489 owns its query UI.

### Workflow selection: Task → Wave → repository

Reuse the captured Task Workflow and existing `lf task run <task> <name>`.
An existing captured Workflow survives changes to defaults and membership;
ordinary traversal follows that graph. Preserve the existing explicit Workflow
selection/restart semantics. For a Task without a captured Workflow, selection
is: explicit Task request, then its Wave's default, then the repository
default. An invalid selected name is an error, not permission to try the next
default. With no selection at any level, report selection-needed and list the
available Workflows. Conversation, checkout and direct skills/Flows still work.

The Wave owns its optional selected workflow directly, alongside its existing
Wave-scoped definitions. `lf wave workflow show/set` reads and changes that
selection without resolving a current or historical Project. Project creation,
rotation and history neither copy nor retain a workflow selection. Remove
`Project.workflow`, its storage, planning edits, transport fields and DTO
mirrors; there is no historical Project workflow fallback. Tasks retain their
captured execution graph, which is separate from a default for future Tasks.

Add optional `workflow: <name>` to repository `.lf/config.yaml`, read through
the existing strict `config::load_repo_config`, not personal merged configuration
or `load_config_or_default` (which conceals load errors). The file is the source
of truth; do not copy its selection into a planning table. Omission/null means no
repository default; an empty or malformed name is invalid. This setting changes
future take-up only. No stored Task Workflow is rewritten. A Wave default
survives chapter/Project changes. No implicit built-in default is introduced.

Reuse `workflow.rs::load_workflow` and the existing `.lf/workflows/<name>.yaml`
or `.yml` namespace, where a repository definition overrides a built-in. For an
explicit Task selection with Wave context or a Wave default, use the Wave's
stored definition first, then that repository loader. A repository-selected
default uses the repository loader directly, avoiding an unrelated Wave's
same-named customization. Capture the resolved graph in the existing Task
Workflow owner before running it. Preserve direct named-Flow support without
forcing a Project read; Flow and Workflow namespaces remain distinct.

Desktop shows the effective default and its source before first run, offers the
same explicit selection, and shows “No workflow selected” for known absence.
Read failures retain unavailable/last-good content. CLI explanation and Desktop
must use the same resolver as launch. Observe config and Workflow-file changes
through the existing Work reader's filesystem invalidation path, including a
repository with no Task checkout yet; no second polling loop or stored mirror.

### Move changes planning membership, including active work

Rename the existing `lf task refile` operation to `lf task move` and extend it
with `--no-wave`. Both directions
and moves between Waves are supported within one repository, including Tasks
with Sessions, a checkout, PR, captured Workflow or live Flow. Replace the
“unallocated only” predicate with an atomic membership edit under the existing
planning write/revision rules. A same-destination retry is a no-op; a competing
destination change is a conflict, not an overwritten read. Destination Wave
selection still resolves its current Project. Relocate the existing Workflow
position operation to `lf task workflow move <task> <node>`, retaining its
history, reason and end-completion semantics. Membership moves never alter
Workflow position; Workflow moves never change membership. Delete the old
`refile` command and move the old positional Workflow syntax rather than
keeping ambiguous aliases. Update CLI help, Desktop dispatch, skills, examples,
command-resolution tests and saved-command generation together.

Preserve Task ID, creation/admission authority, Started timestamp, assignment,
checkout, branch/base, PR, primary Session, bindings, Flow/Process rows and
Workflow graph/position. Existing execution continues where it began. No restart,
interrupt, reparented Process, forced completion or revised historical charge.
The membership edit uses the existing planning journal/delivery receipts; no
second refile journal or transfer object.

Keep the existing planning locks for actual source/destination Waves, acquired
in stable ID order. A standalone operation has no Wave lock. Let Task creation's
store wrapper accept optional acquisition, as placement already does, while
retaining blocking-worker ownership of any acquired lock. SQLite's immediate
transaction validates the final membership/revision for every path. Do not
invent a Wave or a second repository-lock subsystem to satisfy that parameter.

Session membership stays Task-based. Each captured input records the Task's
optional Wave for context and prospective attribution. Session displays can
derive the Task home, but captured events and
usage keep their recorded owner, including explicit null. A turn already in
flight remains attributed to its capture. An inherited Wave hint from an older
attachment cannot overwrite the selected Task's current membership; an explicit
contradictory user scope still reports a mismatch. Audit capture, hook and
resume paths together so updating `agent_sessions.wave_id` alone cannot
retroactively move usage or leave future captures under the previous Wave.

## Baseline evidence

The pre-change audit at `28dc141671` found mandatory ancestry in Task storage,
checkout/admission, planning exchange, captures and Wave-shaped Desktop queries.
`Project.workflow` owned selection, while repository definition loading and
Wave-scoped definition storage already existed. The implementation reuses those
owners, repository identity, planning receipts and evidence machinery; it removes
the mandatory joins and recorded-work move refusal in the same cut.

Linear's [issue-create API](https://linear.app/developers/graphql#creating--editing-issues)
accepts team ownership without a Project. Its [Projects guide](https://linear.app/docs/projects#add-issues-to-a-project)
and [Teams guide](https://linear.app/docs/teams#team-structure) describe one Project
and one team per issue. Checked 2026-10-17 without live mutations. Explicit
projectless adoption uses the configured repository team; team membership alone
does not select a repository or authorize bulk import.

No performance target is introduced. The existing Work reader, pinned snapshot,
checkpoint and bounded sync behavior remain the owners. Membership commits
locally before provider I/O; list rendering adds no per-Task provider reads.

## Migration and error contract

The Task-owned draft is `optional_task_ancestry.sql`; the unrelated
`session_transport.sql` and upstream `remove_landing_timeouts.sql` drafts are unchanged. Backfill every existing Task's repository
from its retained Project→Wave→repository mapping, retaining its exact Project.
Never move existing `inbox` Tasks merely because new captures stop creating one.
Preserve all execution, planning journal, export, creation-intent and provider
identities. Existing non-null `TaskProject` journal values keep their encoding;
null adds an actual membership value, not a fallback parser/model.

In the same migration, move each Wave's unambiguous current Project workflow
selection to the Wave, then remove Project workflow storage. Preserve existing
Wave-scoped definitions and captured Task graphs. Never choose the newest
historical Project as a substitute or retain per-Project defaults. With no
current Project, initialize no Wave default. The migration uses the retained `waves.current_project_id` pointer, not a
status/name scan or newest-Project heuristic. Existing consistency machinery
owns that pointer; a Wave without one receives no default. No competing-current
Project election or additional product decision is introduced. Update planning exchange and
Linear content parsing/serialization so historical Project descriptions or old
Project edits cannot restore workflow ownership. Immutable old journal records
remain evidence, not a selectable default or an active Project workflow field.

Use the repository's rebuild/canonicalization conventions to relax required
Project columns, including `task_creation_intents` and any live delivery records
that currently require ancestry. Reinstall foreign keys, indexes and change
triggers; prove released-frontier upgrade with populated linked work. If a
retained Task cannot resolve its existing repository, fail migration with the
record IDs and leave the old transaction intact. Unknown historical ownership
does not authorize making it standalone. Never experiment on installed data.

| Boundary | Known absence | Failed or inconsistent evidence |
| --- | --- | --- |
| Task owner | Optional Project/Wave are null; repository is required | Dangling Project, conflicting repository or unreadable store is an error |
| Status/list/stream | Ordinary standalone row; no empty-string ancestor | Evidence unavailable; retain last-good UI, disable dependent writes |
| First workflow run | Task request → Wave default → repo default; absent throughout needs selection | Invalid selected name/config fails before launch; no substitute graph |
| Sync/import | Observed null clears membership through the common writer | Omitted/unknown/unmapped ancestry does not detach or invent an owner |
| Refile | `--no-wave` clears the one Project field | Competing edit or cross-repository destination leaves membership intact |
| Recovery | Resolve retained Task repository and execution owner | Unavailable execution location stays unavailable; no local takeover |

Task completion, Workflow position and Process liveness stay independent.
Standalone Tasks use the same no-PR completion and verified-merge rules as other
Tasks. Pause, interrupt, abandon/delete, comments, PR linkage, file operations
and repository automation must not require a fabricated ancestor.

## Alternatives and accepted decisions

1. **Accepted: required repository + optional Project, derived Wave.** One
   nullable membership edit is atomic in local and exchanged planning. It
   removes mandatory ancestry without creating two writable owner fields.
2. **Independent nullable Wave and Project fields:** permits Wave-only Tasks,
   but creates additional states and conflict resolution for two coupled edits.
   No accepted need currently justifies that representation.
3. **Synthetic inbox Wave/Project:** already implemented, and directly violates
   the accepted standalone outcome. Delete the implicit path.
4. **Hard-coded standalone `code` Workflow:** silently chooses a review/delivery
   policy for an arbitrary capture. Jack Heart requested a repository default;
   use its authored configuration and existing definition loader instead. A
   separate SQLite repository-Workflow registry would duplicate file ownership
   without improving the requested first-run behavior.
5. **Refuse active refiling or clone into the destination:** preserves the old
   obstacle or loses identity. Planning membership is allowed to change while
   capture and execution receipts remain immutable.

Wild success is an idea captured in a repository, worked immediately, and filed
into a Wave later without reopening a terminal or losing a draft. Wild failure
is a successful create that disappears from a Wave-only reader, or a move that
recharges historical usage and starts a second checkout. The acceptance cases
below target those failures directly.

Jack Heart approved the design on 2026-10-17, including the optional Project
shape, same-repository moves during live execution, and repository defaults.
Workflow precedence and direct Wave ownership supersede the initial Project
selection proposal. Membership uses `task move`; Workflow position uses
`task workflow move`. Wave derivation through Project is preserved existing
behavior. Demo review and landing approval remain separate.

## Implementation and deletion status — 2026-10-17

The coherent cut spans storage, execution, planning transport, CLI and Desktop.
`Task`/`NewTask` require repository identity; optional Project membership derives
Wave. Common writes and SQL triggers enforce repository ancestry and retain
creation/admission receipts. Migration preserves linked work and pending exports.
Repository inventory supplies list, stream and Desktop from the shared reader;
Task identity keys retained workspaces. Task actions resolve their repository
without requiring a Wave breadcrumb.

Workflow selection is shared by explanation and launch. Wave selection and
stored definitions survive Project rotation; strict repository configuration is
the final default. Repository-selected definitions bypass Wave customizations.
Membership writes retain execution and Workflow position. Captures retain their
recorded owner, including null; native turn ownership freezes at dispatch and
later turns use the Task's current membership.

### Delete — do not maintain

- Implicit inbox creation and its explanation, including `default_inbox`
  expectations in `global_commands.rs`. Preserve explicit/bound selection and
  distinguish known standalone creation from unavailable ownership evidence.
- `refile`, `refile_unplaced_task`, and recorded-work refusal, including the
  preview test's refusal expectations. Preserve read-only previews, same-repo
  membership validation, and execution identity regardless of placement.
- `Project.workflow` storage/transport/DTOs and historical selection. Preserve
  Wave definitions/defaults, captured Task graphs and immutable journal evidence.
- Mandatory Task ancestry joins, Wave-only repository discovery, unused
  Task-action Wave parameters and duplicate Task detail workflow resolution.
  Preserve repository identity, nullable membership and shared projections.

These predecessors are removed. Historical Markdown and immutable journal
values remain readable without restoring Project workflow authority.

Review/compression repaired stale Project fixtures, the move command's pending-sync
message, missing repository definitions in the Wave catalog, and projectless
Linear refresh's repository-team validation. The attention filter now retains
both runtime and planning Task references. `operator-review.md` records why
transaction-owned definition validation needs no duplicate command parser.
Follow-up implementation repaired standalone creation preview wording and removed
the remaining inbox/refusal assertions from `global_commands.rs`. Its focused
preview coverage now preserves read-only evidence for standalone creation,
explicit ambient bypass, and attach/detach with retained execution placement.

Compression shares sibling ordering across standalone and Project Tasks, with
provider receipts only for Project ordering, and uses one Workflow source
resolver and name validator. It removes unused Task edit/action Wave arguments
and redundant post-write reads. Membership previews use savepoints so they
compose inside the Work reader's snapshot.

Review found a remaining Wave-only lookup after Session preparation and a root
Task inventory failure that could replace saved planning. Both now use the
repository inventory; headless tests cover standalone opening and retained
rows, selection, panes and drafts. CLI fixtures now choose Project membership
explicitly when exercising Project sync. Adoption tests preserve historical
Project content instead of expecting the deleted Workflow conversion. No
remaining predecessor deletion target was identified in this scope.

Forbidden outcomes: parallel Task stores/types; synthetic planning owners;
independently writable Task Wave and Project; null represented by empty strings;
query results changing membership; loss of a Task on failed sync; retroactive
usage changes; refiling that transfers execution; duplicate creation after an
uncertain provider write; optional fields added while old inner joins still hide
Tasks. No compatibility adapter that preserves the mandatory-ancestor runtime.

Out of scope: LOO-489's sidebar design, LOO-416's retention redesign, cross-repo
Task moves, Wave-only Tasks, bulk live reassignment, arbitrary team-issue import,
new workflow definition stores, execution transfer, and changes to Wave/Project
planning ownership beyond optional Task ancestry and moving workflow selection
from Projects to Waves. Product goal/memory now reflect the accepted Task and workflow ownership;
no chapter KR or unrelated Wave scope changes. This serves the Product goal of one understandable shared Work
contract; it does not by itself prove the chapter's multi-day Desktop adoption
or external-repository progress KRs.

## Done when

1. A no-Wave repository capture creates exactly one Task, zero Waves and zero
   Projects. The same holds with `--no-wave` inside a Wave-bound Session. A normal
   bound capture retains its selected Wave. List, status, stream and Desktop
   agree on identity and known-null ancestry without fallback names.
2. One standalone Task opens its conversation, prepares a checkout, takes up its
   repository default Workflow, runs, recovers and completes. Explicit selection
   beats the Wave default, which beats the repository default. Test custom
   repository definitions, absent/invalid defaults, and default changes before
   and after capture. A captured graph/position never changes on refile or config
   edits. Wave defaults survive chapter rotation, work without a current Project,
   and cannot be restored from historical Project workflow content. A Task in a
   historical Project takes up its Wave's current default, not an old Project
   selection. The no-workflow state is actionable. PR and no-PR paths preserve
   existing delivery rules.
3. A running Task can attach, move between Waves and detach. All Work identities,
   Process liveness, primary conversation/draft, PR and Workflow position remain
   unchanged. Captures before/during the move retain their original attribution;
   the next capture uses the new membership. Repeated query appearances create
   no extra workspace or checkout. Competing moves commit one coherent home.
   `task move` only changes membership; `task workflow move` only changes
   Workflow position, retaining the existing end-completion behavior.
4. Git exchange preserves null membership, comments, tombstones, causal edits
   and admission authority across two disposable Machines. Linear export uses
   one team/no Project, later adds/removes the Project, and retries uncertain
   writes under the original reserved issue ID. Explicit null, omitted fields,
   unmapped Projects and outages have distinct tested outcomes.
5. Released-frontier migration retains every existing owner and recorded work,
   including old inbox Tasks and pending provider writes. Cross-repository and
   mismatched ancestry cannot be persisted through any writer. Repository
   relocation retains standalone membership and checkout recovery.

Gate's primary scenario is the CLI lifecycle above in a fixture repository,
with a finite provider and fake GitHub/Linear boundaries. Feed its produced
Task/status/planning JSON through the Swift RegistryQuery/navigation tests; open
the production leaf views headlessly and prove one retained Task workspace as
membership changes. Extend `local_planning.rs` with
`waveless_task_lifecycle` and `WorkNavigationTests` with
`wavelessTaskKeepsWorkspaceAcrossMove`. The production stream regression is
`WorkCLISubscriptionTests/standaloneMovesKeepWorkspace`; it retains the selected
Task, workspace layout and an unsaved file-editor draft through CLI attach/detach.
It does not mount a provider composer or prove native conversation-draft continuity. Populated
migration, capture/dispatch attribution, two-Machine exchange, null/unknown
Linear and DTO regressions extend the existing suites.

Focused lifecycle command:

```sh
uv run python scripts/materialize_rust_tests.py --prepare-cli -- cargo nextest run -p loopflow -E 'test(local_planning::waveless_task_lifecycle)' --no-tests fail < /dev/null
```

Gate runs `uv run python scripts/test.py --reuse-passing` once for the final
content and command plan. Its affected plan must include full materialized Rust
(Project/admission changes), shared DTOs, Session lifecycle, planning exchange,
Work stream, headless Swift and the Desktop app build. Explicit cross-language
scenario: `scripts/test_desktop.sh --filter WorkNavigationTests -Xswiftc -gnone`
with the fixture CLI pinned by `LOOPFLOW_TEST_LF`. Linux CI owns
`planning_reconnect_tests`, which macOS skips. The disposable installation
harness owns populated released upgrade:
`uv run python scripts/test_task_installation.py --repository-upgrade`; no
installed Machine mutation.
Formatting/static checks: `cargo fmt --check` and
`cargo clippy --all-targets -- -D warnings`. All cargo runs isolate inherited
`LF_*` authority and close stdin through the established wrappers/fixtures.
An unavailable platform check is deferred to its capable CI owner, not reported
as passing. Native demo judges discoverability and conversation continuity;
it is not an unattended gate requirement.

## Remaining work and evidence limits — reconciled 2026-10-17

1. Gate's affected-suite matrix and populated disposable installation upgrade
   remain. Reuse applicable focused results above; broader Rust, headless Swift,
   Desktop build and Linux-only reconnect checks retain their stated owners.
   The earlier passing workflow-document test reported a leaked child; its suite
   cleanup check must settle that evidence before an all-clear claim.
2. Gate retains the live membership regression
   `provider_completion_preserves_a_live_edge_and_the_flow_process_records_its_real_arrival`:
   it detaches/reattaches a held Flow, compares Workflow/checkout/branch, checks
   the child remains alive, then verifies one successful original Flow arrival.
   `waveless_task_lifecycle` separately proves the standalone lifecycle after its
   Flow returns; the store test proves capture ownership and stale-write refusal.
   The full acceptance-3 combination of a between-Wave move, unchanged PR and
   native conversation draft is not established by these component tests; the
   remaining variants belong to Gate's focused coverage and the configured demo.
3. Configured CLI/Desktop demo review remains: repository discovery, conversation
   opening, unsent native input, movement during execution, continued standalone
   use and completion, plus the normal PR/recovery path. Headless workspace/file
   draft retention is useful evidence, not provider-input or visual acceptance.
   A second Task must stay standalone throughout completion as designed.
4. Publication for demo review remains authorized; landing awaits Jack Heart's
   review. No unanswered product decision blocks the accepted implementation.

The design baseline remains `28dc141671`; checkout HEAD, local `main` and
`origin/main` now resolve to `070e5bb6a4` (#1586, inspected without fetching).
That integrated change removes elapsed-time cutoffs from waited landing/repair
and adds `remove_landing_timeouts.sql`. Its Task completion tests share the
updated Task fixture, and its migration remains separate from this Task's draft.
Gate's existing combined migration and Task/delivery coverage includes this
integration; earlier focused passes are not a new combined-suite pass.
Standalone PR/recovery behavior uses that existing waiter, retaining cancellation,
concrete failures and verified-merge completion rules. No additional delivery
mechanism or landing authorization follows. Retained-repository upgrade and
Session reattachment remain the restoration and attachment owners.
LOO-489's query UI and LOO-416's retention scope remain separate. Product has no
immediate child directories/memories in this checkout; its complete goal and
memory were read. Memory's old Project-default and Wave-required guidance is
superseded by Jack Heart's October 17 decisions, not a new design proposal.

Prior implementation/compression evidence records focused Rust lifecycle,
workflow, capture/dispatch, migration, exchange, Linear null/adoption/export and
lost-response recovery, PR completion and DTO passes; focused Swift navigation,
CLI-stream, editor, Work model and leaf-view passes; Desktop build, formatting
and Clippy passes. These are retained results, not a fresh full-matrix claim.
No production code changed during reconciliation.

Retained focused results: compression's Rust batches passed 18/18 across ordering,
membership, Workflow defaults, CLI sync/selectors and adoption after fixture
repairs, including `legacy_project_adoption_preserves_plans_across_lost_responses`.
Swift destination/incomplete-planning filters passed 4 tests/12 cases;
`task_planning_explain` passed 6/6, with `global_commands`, formatting and Clippy
also recorded passing. Reconciliation changes prose only and does not rerun them.

Check: 2026-10-17 — `git diff --check` passed; `lf context` fits memory/scratch token and byte limits; combined suites remain with Gate and configured native proof with demo review.
