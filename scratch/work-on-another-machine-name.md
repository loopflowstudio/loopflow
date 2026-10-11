# Share Work across machines — LOO-412

## Next pursuit — Jack Heart, 2026-10-10

The current goal includes implementation of both this existing pass and the accepted
[deletion audit](work-model-reduction.md), in that order, then verification and a
fresh +/- PR review. Jack Heart explicitly confirmed the audit is part of the
delivery outcome. The exact WorkEdit shape is delegated to implementation. No
publication of a partial reduction establishes readiness.

Jack Heart requested implementation of the model review and another published
version for review in the ongoing conversation. Continue this Task and PR #1491;
do not land, install, start a later remote-work slice or export real Work records
to the public repository. The [model inventory and review](work-model.md) supplies
source references; this section is the current implementation instruction and
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
owned; comment observations are adapter-internal, but Project/Task observations
still escape through ordinary readers. Project now stores description,
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

Public ProjectContent is removed: ordinary operations take direct typed fields,
Chapter input is flat, and ProjectStatus belongs to the domain. Linear/migration
Markdown parsing stays private in `pm/project_document.rs`. One `ProjectDocument`
now supplies parsed fields and source ranges to both Linear and migration;
`LinearProjectFields`, its parser/validation forwarding and test-only serialization
are deleted. Field replacement renders only the selected typed value rather than
mutating a second content model. The composed prose path is described below.
Rotation compares
retained JSON values without rewriting original receipt bytes; changed requests
still refuse. The public edit fixture exposed a raw-SQL selection inconsistent
with its journal; fixture setup now uses ordinary binding, not a production bypass.
This is only the Project input boundary, not the full reader cut.

October 10 canonical-reader cut: selection, ensure/bind, Project edit/workflow,
rotation results and Wave/roadmap summaries now use the canonical Project reader.
`ProjectPlanning` and the public `planning_project(s)` reads are deleted. The
CLI/Desktop summary no longer joins two Project inventories or exposes `work_id`;
`id` is local, `linear_id` is the optional binding, and sync is required evidence.
Readiness also returns local identity. The surviving reader orders by shared rank,
creation time and ID rather than edit time. Provider adapters compare explicit
bindings. Rotation retains original receipt bytes and local execution is unchanged.

`7e63c994d0` derives Task/Project save and rotation receipts from the same typed
edit slice as the writer, through `PlanningChanges::save`. Task saves no longer
read `plan_read::task_in`; the parallel JSON receipt construction, Project
`write_edits`/`write_content` and rotation's PM reconstruction are deleted.
Ordering and disposition keep distinct receipt owners. This removes writer-side
reader dependencies, not the Task/status/snapshot stack or string-field journal.
Offline fixtures bind through the writer: raw SQL selection contradicted the Git
journal and foreground acquisition removed that unsaved selection. Earlier detail:
`7e63c994d0:scratch/work-on-another-machine-name.md`.

### Canonical Task acquisition — October 10

Task now reads its existing persisted rank, assignee, due date, state, completion
and completion time directly. Creation saves those fields through the typed writer
once, instead of capturing an incomplete initial Task and then correcting it.
Acquisition returns the saved Task; `ResolvedTask` and `ops/task_pm.rs` are removed.
Cold placement, comments, PR ownership and sweep consume that identity; sweep
compares canonical Project IDs. Concrete Linear ownership validation remains at
acquisition. No migration or execution write is added.

### Remaining reader cut — reconciled October 10 at `7e63c994d0`

The preceding feedback's `ResolvedTask` finding is superseded by `8c2b64029f`;
that type and `ops/task_pm.rs` are absent. Canonical fields and the acquisition
return are implemented, but ordinary read composition remains provider-shaped:

- `plan_read::task_in` selects saved due dates, then overwrites them from the
  Linear observation (including `None` when its record is absent). `project_in`
  fabricates `LinearProject` with `description: None` and dual-purpose IDs.
- `ops/task.rs::task_planning_item` still feeds create, placement and snapshot
  callers. `TaskStatus.planning` exposes `PmTaskRecord`; known Tasks use the local
  reconstruction while cold status uses `inspect_task_planning`. Canonical
  acquisition alone therefore does not make these paths agree.
- `lf/commands/waves.rs::wave_tasks` reads Tasks and `planning_wave` separately;
  `snapshot_tasks` joins them by either local or Linear ID. Its retained-Task path
  reconstructs another item through `task_planning_item`. CLI/Work-frame/Swift
  consumers must change with this join, not after the model is declared complete.

The coherent cut replaces these reads with canonical Work and explicit binding,
availability and acquisition evidence. Pending local membership must remain valid
beside an older provider Project; invalidation, positive removal and foreign Team
facts remain visible. A missing provider record must not erase a saved field;
a local update timestamp must not invent provider observation age. Local/Git/Linear
status and snapshots need matching IDs/content and unchanged populated execution
proof. These are remaining implementation/tests, not results of this reconciliation.
Bindings, ordinary follow-ups, cron and the full deletion audit still precede review.

### Project prose: composed behavior and remaining ownership cut

October 10 implementation adds ordinary `lf project edit --description` saves,
selected Linear delivery and acquired receipt settlement through the typed writer.
Boundary whitespace is trimmed; prose segments separated by managed Markdown are
joined with one blank line. Migration still preserves original prose bytes.
Managed Workflow/KR/target spans and fenced examples survive outbound replacement.
Full content and explicit null/empty values are observed; omitted full-response
content fails decoding without settling a receipt or replacing canonical Work.

The common save, baseline, delivery and readback path now has focused composed
proof: unchanged baselines preserve saves, an older acknowledgment rebases but
never clears its successor, and conflicting Linear values retire delivery while
retaining both values. Stale and foreign observations cannot write prose or settle
receipts. The fixture also retains a populated Workflow, unknown Process and Task
placement. Public offline CLI proof covers both transport configurations and mapping
states. These source fixtures establish no installation or real two-machine proof.

Review exposed a separate retained-evidence gap: full observation → newer partial
observation → local save lost the prose baseline. `pm_projects.prose_json` now owns
its value, original provider revision and acquisition age. Partial Project facts
advance independently without refreshing that evidence. A later full observation
at the partial revision can complete the field knowledge. Receipt baselines use the
field's revision; the attempt fence still uses the current entity revision. The
single `planning_peers` draft adds that evidence and prose receipts, preserving
receipt bytes, identities, attempts and sequence high-water marks. Released SQL and
LOO-406's draft remain unchanged.

**The structural acquisition prerequisite is not complete.** `PmProject` moved to
concrete `pm::linear::LinearProject`, but ordinary `plan_read::project_in`,
`PmSnapshot.projects` still use that representation.
The local projection still substitutes provider/local IDs and cannot represent
saved prose. Moving its name did not delete this second domain view. Complete
canonical Project consumers and confine the observation before calling audit
sections 1–3 complete; do not defer these dependencies to a post-review follow-up.
Project export now reads canonical Project directly. No replaced type alias remains.

The original acquisition tracing is preserved at
`a849341234:scratch/work-on-another-machine-name.md`; the absorbed review is at
`f2ca2063b5:scratch/project-acquisition-dependency.md`. Its missing prose proof is
superseded above, not its private-observation requirement. The public canonical
Project reader cut (`52e7da7f35`) is complete. Remaining Task/snapshot consumers of
`LinearProject` belong to this acquisition prerequisite, before bindings, ordinary
follow-ups, cron and the rest of the audit. No partial publication or readiness.

Acquisition now selects new and existing Tasks in one query and applies one typed
edit slice through the common writer. Existing identities, local order,
pending disposition and cross-Wave membership fences remain distinct from first
admission. The duplicate import branch and `item_edits` forwarding function are
deleted. `PmResolvedTask`, both `pm_resolve_task` wrappers and now `ResolvedTask` are deleted.
Acquisition returns canonical Task; ordinary snapshot/status projections still need
replacement. These reductions do not complete canonical acquisition or the audit.
Focused checks exposed fixtures predating foreground exchange: offline completion
now simulates an outage; repeat refiling preserves Task/receipts rather than freezing
sync status. Mappings enter through the writer, not SQL inconsistent with its journal;
changed provider content carries a revision.

Compression removes the fabricated Git recovery record/value hierarchy: status and
CLI/Desktop now inspect retained journal mutations directly, with candidate IDs
separate from projection failures and publication receipts. Failed journal reads
use the existing read error and cached WorkReading; successful reads have definite
pending state. No persistent state or import policy changes. The accepted typed
edit/journal rewrite and preceding acquisition/cron/follow-up work remain open.

The earlier pass still includes acquisition, concrete Linear bindings, ordinary
Task/comment follow-ups and cron unification. The full deletion audit then removes
remaining provider hierarchy, duplicate edit/journal vocabularies and Task
projections. Whole-model patch reconciliation is already removed. Conditional retention requires a concrete fact,
owner and populated conversion proof. No partial publication or readiness follows.

Review found edit-time ordering would reshuffle the replacement Project list;
the canonical reader now owns shared order and the passing public Wave/roadmap
regression covers two Projects whose shared rank disagrees with update time,
then edits one without reordering either. It also checks local IDs, explicit
Linear bindings and required sync evidence. The independent static finding is
resolved here; its original text is retained at `/tmp/loo412-project-reader-review.md`. The old invalidation fixture also treated missing inventory
membership as deletion, contrary to the surviving reader and omission contract.
Its coverage now distinguishes missing inventory (retain saved Work) from positive
archive, invalidation, contradictory relation and observation-age evidence.

Cron conversion must retain the installed owner independently of file location.
Release's October 5 memory records both jobs installed under Infrastructure,
including the release definition authored in the child; changing the list cannot
reassign those receipts to the child. Its September 28 failure also requires a
public command/Flow regression where an operation failure stays nonzero, not only
parser/filter tests. Read coverage: the sole immediate child, Release, has both
its GOAL and complete memory inspected; no child-specific history is moved here.

The published transport baseline remains one typed transactional writer for local
creation/edit, comments, disposition, selection/order and imports. Git capture shares
that transaction; imports emit no echo. Immutable causal changes retain losers;
accepted heads exclude rejected projections. Savepoints isolate projection failures.
Only the configured destination exchanges Work, never execution or delivery attempts.
Earlier baseline detail: `c8fe4628a:scratch/work-on-another-machine-name.md`.

## Remaining proof and review

- Gate owns the full affected suites, release-frontier materialization and headless
  Desktop checks. Existing Linux TLS fixtures own connected public Linear acceptance;
  real two-machine SSH and installation remain unobserved. Source checks use the CLI
  built in this worktree with disposable stores and simulated providers/transport.
- Configuration switches retain prior history and uncertain effects. They do not
  transfer another Machine's delivery attempts. Automatic migration of previously
  Git-imported, unmapped work into Linear remains an open design question; no
  exactly-once or seamless switching acceptance is claimed.
- The [one-page +/- review](pr-review.html) and source manifest retain the prior
  cut. Regenerate them after the entire accepted reduction, before review-only
  republication. No readiness or landing follows from this persistence checkpoint.

Prior implementation checks: `cargo build -p loopflow --bin lf`, fmt, `cargo clippy --all-targets -- -D warnings`, diff check and five focused offline Task/Project edit, refile, older-acknowledgment and assignee-clear tests pass with LF authority cleared; gate owns affected/materialized/Desktop verification.

Realign check: `git diff --check` passes; source tracing confirms the remaining reader cut; no product code changed or suites rerun (gate owns broader verification).

Context: authored memory/scratch fit. `lf context --skill realign` reports the
generated Task seed/steers/workspace at 19,434/16,000 tokens (3,434 over).
Its complete launch source was inspected; retained history/generated metadata
cannot be reduced by curating authored plans. No history or limit changed.

## Delete — do not maintain

- Ordinary `LinearProject` (formerly `PmProject`)/`PmItem` reads: use canonical Work;
  preserve prose, bindings and historical events. Public ProjectContent, plan
  wrappers, live iteration and public Issue comment/observation models are removed.
- FollowThrough models, completion gates and dedicated UI: ordinary Tasks/comments;
  preserve unfinished effects and history with their existing recovery owners.
- WaveCronDef and duplicate cron command vocabulary: repository command list;
  preserve due identities, timing and installed ownership without activating jobs.

Earlier transport evidence: `ae96102580e:scratch/work-on-another-machine-name.md`.
The mixed model is superseded; its synthetic proofs establish no installed acceptance.

The complete mixed design and prior acceptance are retained at
`ae96102580e442fe582dd17ce5e6fef9b8cee69d:scratch/work-on-another-machine-name.md`;
that model is superseded, not proof of this cut. Original adoption demo: `0cd8e7f14`.
Mixed receipt recovery: `e37099e50`/`231e75898`. Native Codex exchange used synthetic
Responses and retained a live Session plus a subsequent turn; its Workflow was
seeded, not a running edge. It proves neither configured accounts nor the new route.
Release's operation-entry lesson still applies: a helper-only pass is not dispatch.
