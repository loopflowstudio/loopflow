# Share Work across machines — LOO-412

## Accepted scope — Jack Heart, 2026-10-10; reconciled October 11

The current goal includes implementation of both this existing pass and the accepted
[deletion audit](work-model-reduction.md), in that order, then verification and a
fresh +/- PR review. Implementation owns WorkEdit; both passes precede review.

Jack Heart requested implementation of the model review and another published
version for review in the ongoing conversation. Continue this Task and PR #1491;
do not land, install, start a later remote-work slice or export real Work records
to the public repository. The [model inventory and review](work-model.md) supplies
source references; this section records the accepted scope and
supersedes its earlier “proposed / not yet selected” labels for the scope below.
The accepted model review is [work-sync-redesign.html](work-sync-redesign.html);
`work-model.html` is the earlier inventory, not the final +/- review.

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

Implemented October 11: `Config.crons` is one list of named ordinary lf argument
lists. `WaveCronDef` and GOAL declarations are deleted. The CLI parser supplies
Wave/Task filtering; the runner executes saved argv without kind-based dispatch.
Released job metadata decodes into that representation. Existing label/lock bytes,
Machine, activation, dues and receipt classifications survive. Release remains
under Infrastructure; historical Skill receipts remain Skill receipts. Scope-filtered
sync preserves other Tasks, Waves and repositories. Missing configuration refuses
pruning; explicit `crons: []` permits it. Reading installs nothing. No live schedule
was changed. Configured product `wave/operate` remains an explicit Flow declaration;
this conversion does not reinterpret it as a skill or claim that missing Flow works.

### Preservation and proof

One typed transactional writer remains mandatory for local edits and imports.
Only shared Work fields synchronize. Captured Workflow position/history, Sessions,
Processes, checkouts, controls and local delivery attempts remain local. Provider binding/observation records must
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

Current source configuration:

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
selects Linear; `linear.team` supplies its Team. An existing Team binding selects
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

## Implementation boundary — October 11

Canonical Work owns direct Project/Task content, status, criteria and comments.
ProjectPlan, TaskPlan, ProjectContent, live iteration, ordinary PM inventories and
dual-ID readers are removed. Linear Markdown stays at acquisition/delivery and
released conversion boundaries, preserving fences, unknown prose and partial fields.
Prose retains independent revision/age. Exact completed cuts and contrary evidence:
`1930805f40:scratch/work-on-another-machine-name.md`, Implementation boundary.

The Task draft replaces whole provider JSON bodies with concrete observations.
Private Linear acceptance persists observations and Work in one transaction; projection
reads retained IDs. Contradictory ownership, foreign/unmapped facts, independent
membership/removal, archive/invalidation and original ages remain. Rejection dispute
markers outlive rollback; blocking writers retain Wave locks after cancellation.
Delivery requires accepted Work revision plus archive/membership eligibility.
Historical receipts retain bytes; new disposition conflicts keep state evidence only.

Concrete `linear.team` and `linear.initiative` replace duplicate provider bindings.
The generic `pm` facade and nested Wave context are now removed. Linear operations
use `linear/` and `ops/linear.rs`; TaskUpdate and TaskComments belong to the domain.
Task comments and saved lookup belong to `ops/task.rs`; refiling returns canonical
Task. PlanningSync, store access and Wave locks belong to `ops/planning_sync.rs`.
The duplicate global issue resolver is removed. Git lookup misses cannot select
Linear, and Linear refresh skips unmapped local Projects without changing them.
Existing credential fencing, receipt recovery, lock names and refresh ages remain.

`dac4cb1ba6` completed the public acquisition API cut; `06e95d98fe` removed the
PM facade. `5a8ae4cdb6` and `c7bb8a58e0` then completed the typed journal and
concrete observation/projection owners; feedback based on `6b7015d056` predates
those cuts. Remaining:
Cron and ordinary Task/comment conversion are implemented (§13–14 of the audit);
materialized/headless affected gate and fresh +/- HTML review precede review-only
republication. Populated Session/attachment preservation is covered below. Concrete adapter naming alone does not
finish the remaining writer/delivery hierarchy. No installation was performed.

Main `7d9bf0a4d8` supplies Claude recorded-answer settlement, not planning gate
acceptance or stopped-Flow replay authority. Private CLI fixtures build through Cargo
artifact output, never guessed stale paths; earlier fixture and acceptance proofs are
recorded at `1930805f40` in this plan.

### Released migration frontier — October 10

Merge `db36f20ba8` incorporates main `21dae527b4`: local planning, optional Task PR,
AgentProcess and process-ID drafts are materialized in `0.13.11.001_release.sql`.
Only this Task's `planning_peers.sql` remains editable. Its `optional_task_pr`
dependency names a retained released marker, not a missing draft to restore.
The populated `provider_columns_preserve_released_observations_and_local_execution`
fixture starts here: foreign/partial observations, contradictory membership,
independent removal/order/age, Task identity/path, captured Workflow, Process and
uncertain state/comment receipts survive. Invalid scalar conversion rolls back
schema and original rows. The fixture now retains completed and unresolved Session
turns, unpublished input, native identity, selected capture/Session, AgentProcess
ancestry, attachment token/endpoint and pending-input/open-tool activity. The same
fixture on a Git recipient proves repeated completion/content import preserves
those rows and an uncertain local cancellation attempt. This is populated storage
proof, not a configured provider-control or installation trial. Earlier receipt
proof retains uncertain inputs and sequence high-water marks; broader observations
and materialized/headless acceptance remain gate obligations.

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

Root `planning.rs`, PM facade, duplicate object/edit vocabularies and whole-model
receipt patching are removed. Local writes and Git retain target-bearing `WorkEdit`;
Linear owns observation/projection. Typed creation/field baselines retain attempted
inputs and sequence boundaries independently of current Work. Partial omissions,
explicit-null membership, canonical Project IDs, older acknowledgments and uncertain
effects retain their meaning. Exact cuts and proof: `52f7f3f99f` at this path.
Released SQL stays immutable; unshipped Git encodings need no compatibility decoder.

`planning_members`, `planning_active`, `use_peer_planning` and enrollment are
removed. Current ownership selects new exports; retained mutation routes preserve
old destinations, rejected imports and missing Work rows. Passive status takes its
selection from configuration; Desktop rereads it without a database write. Populated
routing tests cover switches, ownership moves, missing rows and rejected orphan
imports; existing import/execution preservation fixtures remain required.
This changes no execution admission: LOO-427 must adapt its stacked membership
consumer without interpreting a route or absent placement as first-start authority.
LOO-414's unresolved authority still blocks stack readiness. Affected gate and fresh +/- review remain.

Cron checks: 51 focused Rust tests pass; two conversion/filtering proofs and the
public direct/Flow nonzero regression pass, plus Clippy and generated CLI reference.
Logs: `/tmp/loo412-cron-{tests,conversion-tests,filter-tests,entry-tests,clippy}.log`.
The first filtering run exposed an ambiguous fixture command, fixed and rechecked.
Conversion preserves released metadata and excludes a second execution while the
original child holds its inherited lock. These are isolated source proofs, not
installed schedule acceptance. Original counterexample requirements remain at
`07a3586ac3:scratch/work-on-another-machine-name.md`.

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
`dfa2cdafba`, `09709a05cc` in this plan. Gate and review still precede republication.

Earlier focused checks and seven intermittent nextest leak observations remain at
`97c482815c:scratch/work-on-another-machine-name.md`. The full October 11 Rust
run reported no leaks; that observation does not diagnose the earlier handles.

Gate repair (October 11): `.lf/tmp/gate/run-66673` failed. Rust materialization
ran 2,416 tests: 2,386 passed, 30 failed, 18 skipped. Repairs move sync store
initialization into its existing runtime thread, use the canonical store selector
without requiring an existing checkout, exclude null placements from pruning,
and retain revision triggers for mutable route rows. Fixture repairs remove old
follow-up calls, share capture/process ledgers, isolate native Claude inspection,
update cron argv and prompt goldens, and correct required Swift sync fields.
The remote fixture now saves its directly seeded Task through the ordinary writer
before dispatch; it does not substitute an import for Git exchange.

Checks: 19 isolated checkout tests, architecture/release-config regressions,
the self-sandboxed Desktop observation check, 19 repaired headless Swift tests and
Swift boundaries pass. Materialized recheck: 150/153 pass, including migrations,
public SSH dispatch, Session recovery and live exchange; no leaks. Three failures
exposed ordinary-folder resolution and cron classification; both repairs pass
all three focused regressions. Final fmt/Clippy and three source-built remote CLI
checks pass at `bb93899d1f`. Logs: `/tmp/loo412-gate-*`.
Website reports horizontal overflow at 1440px (1475px content width); homepage,
CSS and its test are unchanged against base. This is retained counterevidence,
not a passing gate. Fresh review remains.

Transport evidence: `0181133849` at this path; earlier simulated adoption: `5960415b3:scratch/remote-task-demo.md` (not real SSH or integrated acceptance).

Follow-up cut sanity: `cargo check --all-targets`, fmt/Clippy, 4 recovery/export tests, 3 CLI/Flow tests, 2 merge/process preservation tests, 29 wire/relation/history/catalog tests, headless Desktop build/31 Swift tests and 4 skill-alignment tests pass; full affected/materialized gate remains.
