# Share Work across machines — LOO-412

## Accepted scope — Jack Heart, 2026-10-10; reconciled October 11

The current goal includes implementation of both this existing pass and the accepted
[deletion audit](work-model-reduction.md), in that order, then verification and a
fresh +/- PR review. Jack Heart explicitly confirmed the audit is part of the
delivery outcome. The exact WorkEdit shape is delegated to implementation. No
publication of a partial reduction establishes readiness.

Jack Heart requested implementation of the model review and another published
version for review in the ongoing conversation. Continue this Task and PR #1491;
do not land, install, start a later remote-work slice or export real Work records
to the public repository. The [model inventory and review](work-model.md) supplies
source references; this section records the accepted scope and
supersedes its earlier “proposed / not yet selected” labels for the scope below.
The current HTML review is [work-model.html](work-model.html).

### Simplify the domain and its integration boundary

- One `Project` domain record. Fold `ProjectPlan` and `ProjectContent` into its
  direct fields: identity/ownership, name, summary/description, status, optional
  workflow choice, KRs and metric targets. Local readers return that record,
  replacing `PmProject` as the shared domain view. Keep a private `LinearProject`
  response shape only where the adapter needs one. Store structured content once;
  render prompts and Linear Markdown at their boundaries instead of parsing
  internally stored prompt text back into KRs and targets. Preserve authored prose,
  selection, ordering and unrelated provider associations.
- Remove Project `iteration` from the live model: inspection found persistence
  and preservation tests but no production increment. Preserve historical events
  and stored evidence during conversion rather than inventing a new counter.
- Delete `TaskPlan`. Put identifier, title and description directly on `Task`.
  Preserve local optimistic-write revisions, provider IDs and observation ages;
  keep those responsibilities explicit instead of creating another plan wrapper.
- Replace PM terminology with Linear where it names that integration. Wave binding
  becomes Linear Initiative configuration; repository binding becomes Linear Team
  configuration. Domain records remain Work/Project/Task, including in Git mode.
  Remove the redundant provider selector from Linear bindings. Update config,
  commands, docs and consumers coherently; do not blindly rename local data Linear.
- Rename `PmKr` to `ProjectKR`. Remove Chapter terminology from low-level types.
  The target is `MetricTarget`; resolve its current nested threshold name without
  retaining two competing target concepts. An at-least/at-most `Threshold` is an
  implementation choice, not another product entity.
- One domain `TaskComment`. Keep `TaskComments` as the thread/status reading.
  Any `LinearComment` response and `LinearIssueObservation` stay private to the
  adapter. Retain provider comment/author IDs, revisions and incoming-direction
  provenance in integration observations. Ordinary Work readers need no Issue model.

### Replace follow-through with normal work

Remaining concrete work is an ordinary Task; context and links are comments.
Remove `FollowThroughIntent`, `FollowThroughLink`, `FollowThrough` as active domain
models, the special `--finish` / `--none` ritual, dedicated UI state and completion
requirements. No mandatory declaration is needed when nothing remains.
Task creation and comment delivery reuse normal stable identities and receipts.
A saved Task plus a failed linking comment retains the Task and retries only the
comment; uncertainty must not create duplicates. Retain historical follow-through
events and unfinished provider effects through the existing recovery owners.
Keep merge verification and process/checkout preservation separate; comments
neither authorize cleanup nor create a replacement hidden completion gate.

### One cron definition list

Replace Wave-specific declarations and `WaveCronDef` with one list of scheduled
ordinary lf commands. Commands carry `--wave` / `--task` when appropriate; scope
is available for UI filtering through the existing command parser. Use the existing
repository configuration owner for the list, not another registry or per-Wave
schedule store. Remove duplicated Wave/flow/target-kind command interpretation.
Preserve schedule identities, configured timing, installed Machine ownership,
receipts and due accounting through the configuration conversion. A cloned repo
must not silently activate duplicate jobs. Implement and test the conversion in
isolated fixtures; this pursuit authorizes no live schedule activation/change.

### Preservation and proof

One typed transactional writer remains mandatory for local edits and imports.
Only shared Work fields synchronize. Captured Workflow position/history, Sessions,
Processes, checkouts, controls and local delivery attempts remain local. The Task
struct is still not an export document. Provider binding/observation records must
not become a second writable Project/Task owner.

Use the existing single Task migration draft; released SQL is immutable. Prove
conversion from the released frontier with populated Work, prose, KRs, targets,
provider mappings, pending/uncertain writes and historical events preserved.
Exercise ordinary local/Git and Linear reads/edits, offline follow-up creation
plus linking comments, stale provider observations and imported completion without
local execution changes. Add focused isolated cron conversion/filtering proof.
Verify the remote unknown-Task path still acquires pushed code and reuses identity
without touching dirty work. Build the CLI here and test only disposable stores;
gate owns full affected and headless Desktop suites. Regenerate the +/- PR review
against the final diff, publish #1491 and return for Jack's review.

Other speculative rename candidates in the model inventory are not a mandate for
a repository-wide vocabulary sweep. Existing unresolved key pairing and
Git-to-Linear migration choices in `questions.md` remain outside this reduction.

## Retained transport contract — Jack Heart, 2026-10-09

Jack Heart selected one planning transport per repository: Linear, shared Git,
or a personal user-keyed Git ref (the default). Personal configuration supplies
defaults; checked-in repository configuration applies automatically to everyone.
Every process reads and writes its own local planning. Each connected machine
reads/writes Linear independently. A laptop going offline cannot make its worker
depend on a designated publisher. Linear outages leave changes buffered locally,
with no Git fallback. Git still carries code in all modes.

Jack requires one typed transactional planning writer for local and imported edits.
Planning state travels; Workflow positions, Sessions, Processes, checkouts, control
and local delivery attempts do not. Machine assignments and exclusive first-start
scheduling are outside this Task. `--machine` dispatches the selected command.

This supersedes callback routing, a designated Linear publisher, mixed Git/Linear
receipt exchange and manual per-Wave enrollment. No master election, resident,
cross-version compatibility, automatic turn/Flow retry, hidden argument or imported
herdr/cmux implementation. Publication is for Jack's review only; no landing,
installation or real planning export to the public repository.

## Current transport configuration and experience

Published baseline; Linear-binding spellings still await conversion.

```yaml
# .lf/config.yaml, or ~/.lf/config.yaml for a personal default
planning:
  provider: git
  remote: origin
  shared: team
```

Omitting `shared` selects `refs/loopflow/planning/users/<key>`; the first connection
creates the key. Recover the same key on a fresh second Machine with
`lf planning key --recover <uuid>`. Shared refs need no personal key. Repository
`planning` replaces the personal value as a whole. `planning: {provider: linear}`
selects Linear; `pm.linear_team` supplies its Team. An existing Team binding selects
Linear when explicit `planning` is absent. Configuration selects the whole repository
plan and its retained history; ref separation is not access control.

```sh
lf --machine mini task create --title "Fix the parser"
lf --machine mini --task <selector> skill implement
lf planning status --json
```

Cold lookup acquires from the configured transport. Repetition reuses saved Task
and checkout identity. Linear issues select existing provider mappings, preserving
independent legacy IDs. New Linear Task IDs derive from the provider UUID; locally
created Task UUIDs are reused for provider creation. Git preserves authored IDs.

Source code requirements travel separately: branch, pushed commit and selector.
Fetch precedes decisions. Missing/unpushed source work names the branch/commit;
a retained target checkout missing that commit asks for `lf sync` and keeps its
bytes. The origin's Work declaration is not forwarded. An unpublished Linear Task
stays local until provider creation is observed; Git is not a back channel.

Short commands save first, then attempt selected delivery. Linear attempts have a
five-second bound; Git subprocesses retain their own deadlines and effect locks.
Foreground CLI/Desktop/Session lifetimes keep exchange active. Disconnection
retains pending work; reconnect acquires before publishing. No agent turn is replayed.

## Implementation boundary — 2026-10-10

TaskPlan/ProjectPlan and live Project iteration are removed. TaskComment is domain
owned; comment observations are adapter-internal. Public content reads return Work; acquisition, status and admission read narrow
retained evidence.
Project now stores description,
workflow, KRs and metric targets directly. The shared local/import writer and local
readers no longer parse/render stored Markdown; prompt assembly renders on demand.
The one Task draft converts released content before seeding causal history, retaining
saved workflow selection, observation age, mappings and pending delivery. Invalid
criteria roll back the conversion. Review exposed fenced examples being mistaken
for fields. Migration and Linear edits now share boundary-aware field ranges,
preserving nested backtick/tilde examples, unknown paragraphs and target/KR
preambles. The resolved note is retained at
`1b67da0cfe:scratch/project-conversion-review.md`.
Tests cover multiline KR fidelity across restart
and Git import, transactional failure, released content conversion and retained local
Workflow/checkout/effect evidence. These are source proofs, not installed acceptance.

Public ProjectContent is removed; ordinary operations take typed fields and
ProjectStatus belongs to the domain. Private `pm/project_document.rs` owns
Linear/migration parsing through one `ProjectDocument`; field replacement
renders only the selected value. Rotation retains original receipt bytes and
refuses changed requests. Exact input-boundary and fixture corrections:
`1d97aa98b2:scratch/work-on-another-machine-name.md`. These do not finish
snapshot confinement.

Canonical Project readers retain local IDs, optional Linear bindings, sync evidence
and shared rank. Selection, rotation and CLI/Desktop no longer join inventories.
Exact reader-cut evidence: `b4e54f7d44:scratch/work-on-another-machine-name.md`.

`7e63c994d0` in this plan records typed save/rotation receipts, distinct ordering/
disposition owners and the repaired fixture that bypassed the writer.

### Canonical readers and retained provider persistence — October 11

`18520565d6`, `96607a9a46` and `eda4a98097` establish canonical Task
creation/status/acquisition and Wave/roadmap/Work-frame summaries. Local IDs,
explicit Linear bindings and separate acquisition validity/age survive. Saved
content uses Work; acquisition validity and admission still depend on provider evidence.
`planning_task`, `plan_read`, `ResolvedTask`, `ops/task_pm.rs`, `planning_wave`,
`PmTaskSummary` and `task_planning_item` are deleted. CLI creation reloads after
foreground exchange. Cold placement, comments, PR ownership and sweep use saved
identity; provider branch/URL metadata remain explicit.

Project ingestion no longer returns a `LinearProject`; its unused async Store
forwarder and association/validation return plumbing are deleted. Field delivery
checks the fetched non-null revision against the canonical Project binding and
accepted revision, retaining independent archive/membership evidence. A provider
observation with the same revision can still be ineligible: confirmed archival or
unresolved Team membership must refuse delivery without consuming the local save.
Task delivery already uses its accepted-into-Work revision fence. The preservation
fixture now compares against the Task returned by insertion, not its unsaved
constructor revision.

**Delete — do not maintain:** escaping `PmTaskRecord`, the duplicate provider
hierarchy and typed journal vocabularies in the audit. Public snapshot inventory
reads and ordinary Task provider reconstruction are deleted. Next come bindings, ordinary
follow-ups, cron and the remaining audit. No partial publication or readiness.
Earlier reader tracing, fixture corrections and exact proof limits remain at
`dc0d82caa2:scratch/work-on-another-machine-name.md`, the two canonical-reader
headings. Outage/completion fixtures retain captured Workflow; stale and foreign
observations grant no delivery or execution authority.

Rotation and export use canonical Work and typed saves, retaining dates,
prose, bindings, uncertain inputs and later saves. Offline rotation fixtures
cover both transports/mapping states and retry preservation. Exact evidence:
`1d97aa98b2:scratch/work-on-another-machine-name.md`; the provider hierarchy
remains.

### Reconciled acquisition boundary

`fe5262b093` removed Project delivery's provider return; eligibility still needs
`pm_projects` archive and unresolved-membership evidence, including same-revision
rejection. `94cb65b569` deleted `pm_show`, `load_show_snapshot` and `read_pm_snapshot`.
`1d97aa98b2` removed planning-only placement restrictions without changing execution
admission or sorted locks. Proof detail: `f9f9da7d3a` in this plan.

**Ownership reading cut — October 11.** `e0f716aa54` moved status/roadmap
ownership checks into one SQLite read transaction, deleting `PmPortfolioValidator`
and portfolio reconstruction. Contradictory Initiative/Team claims, invalidation,
archival, membership removal and confirmed deletion retain their meaning. Malformed
Wave observations do not hide readable siblings or refresh ages. Scoped roadmap
reads follow the repaired Wave address. Evidence: `f9f9da7d3a` in this plan.

`e3a3479df7` removed the public snapshot reader. `f025e9b7ab` removes
`children.rs` status/admission reconstruction: narrow SQL reads retained membership,
Team, invalidation, removal and age. Unrelated malformed content cannot erase those
facts. Review repaired the canonical Project join: a missing issue cannot hide
independent archival or unresolved membership. Pending membership only affects display.
Populated released-frontier and stale-response tests cover this boundary.
`dfa2cdafba` makes Task acquisition return Work using narrow Linear ownership/age.
`PmTaskObservation`, `pm_task_observation_in` and `read_task_planning_async` are
deleted. Revision-fenced absence, cancellation and foreign/unmapped facts survive;
fixtures replay retained inputs directly. Rollback proof now includes canonical
Project, provider input and Wave acquisition age. The persistent hierarchy, typed
journal, bindings, follow-ups and cron still precede gate and review.
`09709a05cc` removes provider Wave and Project display reconstruction.
`621b4b851b` makes accepted projection, creation attachment and disposition
reconciliation consume typed fields. `f0932031e5` removes receipt-baseline object
reconstruction, preserving independent prose revisions and active-issue deletion
baselines. Work-watch includes the planning cadence.

The next unit is persistent conversion together with its consumers (audit §§1–4):
`linear_issue_evidence`, `project_accepted_planning` and receipt baselines still
extract retained JSON; `put_project`/`put_item` still compare whole observations.
Concrete Linear evidence must retain foreign/unmapped facts, contradictory ownership,
independent membership/removal, partial fields and original ages without another
writable Work hierarchy. Populated released-0.13.11 conversion, delayed updates and
rollback remain unproved. Narrower readers alone do not complete this unit.

Main #1522 entered at `7d9bf0a4d8`, adding Claude recorded-answer Flow settlement.
That merge establishes no fresh planning gate acceptance and grants no replay of
the stopped Flow.

### Released migration frontier — October 10

Merge `db36f20ba8` incorporates main `21dae527b4`: local planning, optional Task PR,
AgentProcess and process-ID drafts are materialized in `0.13.11.001_release.sql`.
Only this Task's `planning_peers.sql` remains editable. Its `optional_task_pr`
dependency names a retained released marker, not a missing draft to restore.
Populated conversion must start at this actual frontier; pre-merge fixture passes
do not establish that boundary. The focused receipt fixture now passes, preserving
uncertain input and sequence high-water marks, not proving full Work conversion.
Source materialization proves no installation.

Main #1523 entered at `c77e72b6b7` after the recorded checks. Its separate
`process_terminal.sql` draft adds local Process terminal placement; it is not a
second LOO-412 draft. AgentProcess liveness and latest-turn completion now own
Session activity. Planning conversion must preserve these local facts; historical
turn outcomes cannot substitute for process liveness. No new product check was
run for this conflict-free merge; final gate includes its affected Work-watch/DTO paths.

### LOO-427 integration coordination — October 10

Jack Heart's October 10 comments `19116a1b-456b-47c4-a2ed-2ee9da13ef62`
and `ca133b19-9455-4460-9da9-865ac0a4d05c` select 427 stacked on committed 412,
landing afterward. 427 reports integration of `fe5262b093` through merge
`74d84dc391` and integration `2f022788df`; its delegation capture/import uses
`planning_write`, removing old SQL capture triggers. It is removing obsolete
`planning_creations` identity fallbacks in `task_follow_through.rs` and its
`children.rs` resolver. This checkout stays independent; no 412 landing authority.
Jack's `d68f7ceb-eda3-44ee-b880-da77b2fd441a` records 427 removing the
`task_remote_tests` cold-target transfer expectation: an existing started Task
cannot acquire another Machine's checkout. That prior pass is not current acceptance.
Pushed-code validation remains at `task_checkout`; 414 owns exclusive first start.

Jack Heart's October 11 comment `ad19b51f-1b33-49df-a34a-912074f66139`
reports parent planning failures in 427's combined gate on `fe5262b093`: immediate
completion/cancellation invalidates pending-receipt assumptions; deletion lacks a
baseline; historical follow-through cannot resolve old filing. 412 owns those
semantics and surviving fixture repairs. Deletion now records an active-issue
baseline; delayed-delivery fixtures create an outage before local save. Canonical
saved-Task lookup reuses Wave locator repair for repository aliases. 427 owns its
separate repository registration fix. Historical follow-through repair remains in
its required lifecycle deletion/conversion, not a revived predecessor.

**Unresolved admission boundary:** Jack Heart's `9462650f-d8b0-4ae4-bd66-61f64e75bd15`
reports default-Git creation enrolling every new Task in `planning_members`, so
427's conservative shared first-start rule refuses ordinary new Tasks without a
recorded owner. Preserve that protection: absent local checkout proves no peer
absence. Reconcile creation/admission with LOO-414 before stack readiness; no
policy relaxation or new authority is approved. The October 11 read of LOO-414's
brief confirms authority identity, repository association, establishment and
unavailable-owner recovery are undecided. A read-only contribution failed because
414 has no checkout; no worker was launched. This blocks stack readiness, not
independent source reduction. 412 still has no landing authority.

Preserve Work-keyed Desktop tabs, content-owned Machine locations, nearest-ancestor
delegation for unstarted Tasks, recorded-owner admission before checkout preparation,
started Tasks pinned to their Machine, exact pane targeting and bounded terminal I/O.
Shared first-start admission remains LOO-414. TaskSource carries code, not relocation.
427 reports its old `task_checkout_machine.sql` capture triggers replaced by
`planning_write` integration. That reported cut must survive composition with the
final journal: retain null inheritance, uncertain legacy assignments,
checkout_machine_id and repository locators at the actual released frontier.
Released SQL remains immutable.

After the existing writer cut, absorb 427's `ops/task/{explain,planning_explain,
checkout_explain}.rs` duplication: share concrete read-only decisions with execution
where that deletes request/field/effect lists, retaining revalidation and effect-free
`--explain`. No generic command-plan framework; aligned Desktop multiplexer and
surface lifetimes are not deletion targets. No second writer or Flow restart.

### Retained proof and current writer cut

Project prose saves/readback preserve field revisions and ages across partial
observations, pending edits and stale/foreign responses. Managed Markdown ranges
preserve fenced examples and unknown prose; migration retains original bytes.
The Task draft retains prose receipts and sequence high-water marks. Exact proof
and contrary evidence: `f025e9b7ab:scratch/work-on-another-machine-name.md`,
“Project prose: composed behavior and remaining ownership cut.” These passes do
not establish adapter confinement or provider-hierarchy conversion.

Root `planning.rs` is deleted. Task creation inputs, Linear binding identities and
stored delivery/status evidence have concrete owners. Team/Initiative edits and
comment authors remain typed through the writer. Git reads the selected native JSON
format; realign removed the unshipped string-format compatibility decoder and its
fixture under Jack Heart's no-old-version constraint. Released SQLite conversion
and causal/losing values remain required. Record creation no longer
reconstructs a generic JSON field map. The target-bearing edit/journal, provider
hierarchy, bindings configuration, follow-ups and cron cuts remain required.

Cron conversion must retain the installed owner independently of file location.
Release's October 5 memory records both jobs installed under Infrastructure,
including the release definition authored in the child; changing the list cannot
reassign those receipts to the child. Its September 28 failure also requires a
public command/Flow regression where an operation failure stays nonzero, not only
parser/filter tests. Conversion must also retain `(canonical repo,wave,flow)` execution-lock identity
and inherited descriptor validation while a release child holds the lock; a new
argv hash or schedule ID must not split exclusion. Original activation, dues and
receipt classifications survive; historical skill receipts never become Flow
settlements. Clone/read activates nothing. This is a source-derived preservation
case, not a reproduced failure (`eda4a98097:scratch/cron-conversion-review.md`).

Writer transport/causal proofs: `eda4a98097` in this plan.

## Remaining proof and review

- Gate owns the full affected suites, release-frontier materialization and headless
  Desktop checks. Existing Linux TLS fixtures own connected public Linear acceptance;
  real two-machine SSH and installation remain unobserved. Source checks use the CLI
  built in this worktree with disposable stores and simulated providers/transport.
- Configuration switches retain prior history and uncertain effects. They do not
  transfer another Machine's delivery attempts. Automatic migration of previously
  Git-imported, unmapped work into Linear remains an open design question; no
  exactly-once or seamless switching acceptance is claimed.
- `pr-review.html` is absent. `work-model.html` and `pr-review-sources.json` retain
  earlier review material, not final acceptance. A fresh one-page +/- HTML review
  follows the entire reduction and affected gate, before review-only republication.

Earlier focused proof: `9dbd6fc2d8`, `dfd3b80611`, `f025e9b7ab`,
`dfa2cdafba`, `09709a05cc` in this plan. Persistence conversion, typed journal,
bindings, follow-ups, cron, gate and review still precede republication.

Checks: `git diff --check` and `lf context` pass October 11; memory/scratch fit both limits. Formatting, Clippy, CLI build and focused baseline/reteam/acquisition/planning passes remain at `3d8b939495` in this plan; unchanged source, no rerun. Full gate remains.

Transport evidence: `0181133849` at this path.
