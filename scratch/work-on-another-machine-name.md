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

`7e63c994d0` derives Task/Project save and rotation receipts from the writer's
typed edits, deleting parallel JSON construction and writer-side PM reads.
Ordering/disposition retain distinct receipt owners. Raw-SQL selection conflicted
with the Git journal; fixtures now bind through the writer. Full evidence:
`7e63c994d0:scratch/work-on-another-machine-name.md`.

### Canonical readers and remaining acquisition cut — October 10

`18520565d6`, `96607a9a46` and `eda4a98097` establish canonical Task
creation/status/acquisition and Wave/roadmap/Work-frame summaries. Local IDs,
explicit Linear bindings and separate acquisition validity/age survive; saved
content and local execution no longer depend on fabricated PM views.
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

**Delete — do not maintain:** ordinary `PmSnapshot` results and escaping
Project/Task observations, then the duplicate provider hierarchy and typed journal
vocabularies in the audit. Snapshot confinement is still incomplete; removing the
Project delivery return value does not finish it. Next come bindings, ordinary
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

### Reconciled acquisition boundary — October 10

At `fe5262b093`, the Project delivery return projection is gone; its eligibility
query still joins `pm_projects` for archive and unresolved membership evidence.
Removing that table therefore requires a concrete replacement for those facts,
not merely the accepted revision on Work. The same-revision rejection cases remain
part of delivery acceptance.

At `94cb65b569`, `pm_show`, `load_show_snapshot` and
`read_pm_snapshot` are deleted. Wave status sync refreshes before canonical
reads; OAuth and archived-predecessor fixtures inspect saved Work. At
`1d97aa98b2`, refiling/rotation no longer require local Wave placement.
Public offline fixtures cover both transport configurations and mapping states,
retaining captured Workflow and unknown Process. Sorted planning/checkout locks
and execution admission remain. Neither cut proves installed acceptance.

**Next coherent cut: portfolio validation and snapshot confinement.** October 10
source inspection finds `validate_pm_portfolio` in
`lf/commands/waves.rs`, not `ops/pm.rs`. Its status/roadmap callers reconstruct
`PmSnapshot` solely to feed `pm::PmPortfolioValidator`. That validator checks
repository-scoped duplicate Initiative/Project/issue claims and Team/Project
relations. Canonical Work alone cannot replace contradictory retained claims:
one accepted owner does not represent every observation. Move these concrete
checks into acquisition/retained relation evidence, then remove the ordinary
snapshot read and validator together, without another inventory DTO.
Malformed or unavailable Wave evidence must still leave readable siblings and
local execution visible. Existing `portfolio_ownership_is_scoped_to_repository`
covers duplicate Initiative binding and separate repositories; it is not full
proof of foreign, stale, removed or independently unavailable observations.

The replacement must preserve same-revision archive/unresolved-membership refusal
in `accepted_project_revision`, original acquisition ages and rejected values.
Populated conversion and public status/roadmap tests must demonstrate those
boundaries before persistent observation owners disappear. Bindings, ordinary
follow-ups, cron and the remaining typed journal/provider-hierarchy audit remain
required afterward. No partial publication or readiness.

### Released migration frontier — October 10

Merge `db36f20ba8` incorporates main `21dae527b4`: local planning, optional Task PR,
AgentProcess and process-ID drafts are materialized in `0.13.11.001_release.sql`.
Only this Task's `planning_peers.sql` remains editable. Its `optional_task_pr`
dependency names a retained released marker, not a missing draft to restore.
Populated conversion must start at this actual frontier; pre-merge fixture passes
do not establish that boundary. The focused receipt fixture now passes, preserving
uncertain input and sequence high-water marks, not proving full Work conversion.
Source materialization proves no installation.

### LOO-427 integration coordination — October 10

Jack Heart's October 10 comment `19116a1b-456b-47c4-a2ed-2ee9da13ef62`
resolves the stack hold: prepare LOO-427 on a pinned committed LOO-412 head now;
427 lands only after 412. Integration stays in the 427 checkout, leaving this
checkout's dirty work untouched. The inspected boundary was `94cb65b569`;
427's earlier gate-repaired boundary is `bd295866bb`. Preserve canonical Work,
configured transport and Desktop/delegation behavior. This authorizes no 412
landing and does not waive its full-reduction review boundary.

Preserve Work-keyed Desktop tabs, content-owned Machine locations, nearest-ancestor
delegation for unstarted Tasks, recorded-owner admission before checkout preparation,
started Tasks pinned to their Machine, exact pane targeting and bounded terminal I/O.
Shared first-start admission remains LOO-414. TaskSource carries code, not relocation.
427's `task_checkout_machine.sql` triggers reference the old planning journal;
integration must adapt to the surviving writer, retain null inheritance, uncertain
legacy assignments, checkout_machine_id and repository locators, and migrate from
the actual released frontier. Never edit SQL already released by either delivery.

After the existing writer cut, absorb 427's `ops/task/{explain,planning_explain,
checkout_explain}.rs` duplication: share concrete read-only decisions with execution
where that deletes request/field/effect lists, retaining revalidation and effect-free
`--explain`. No generic command-plan framework; aligned Desktop multiplexer and
surface lifetimes are not deletion targets. No second writer or Flow restart.

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
receipt bytes, identities, attempts and sequence high-water marks. Released
`0.13.11.001_release.sql` remains unchanged; LOO-406 has no draft after `db36f20ba8`.

**The structural acquisition prerequisite is not complete.** The fabricated
`plan_read` Project and Task views are deleted. `PmSnapshot.projects` and ordinary
snapshot consumers still expose `LinearProject`; confining these observations and
removing the second hierarchy remains required before audit sections 1–3 are complete.
Project export reads canonical Project directly. No replaced type alias remains.

The original acquisition tracing is preserved at
`a849341234:scratch/work-on-another-machine-name.md`; the absorbed review is at
`f2ca2063b5:scratch/project-acquisition-dependency.md`. Its missing prose proof is
superseded above, not its private-observation requirement. The public canonical
Project reader cut (`52e7da7f35`) is complete. Remaining snapshot consumers of
`LinearProject` belong to this acquisition prerequisite, before bindings, ordinary
follow-ups, cron and the rest of the audit. No partial publication or readiness.

Compression removes the fabricated Git recovery record/value hierarchy: status and
CLI/Desktop now inspect retained journal mutations directly, with candidate IDs
separate from projection failures and publication receipts. Failed journal reads
use the existing read error and cached WorkReading; successful reads have definite
pending state. No persistent state or import policy changes. The accepted typed
edit/journal rewrite and preceding acquisition/cron/follow-up work remain open.

Conditional retention in the remaining audit requires a concrete fact, owner and
populated conversion proof. Whole-model patch reconciliation is already removed;
remaining provider hierarchy and duplicate edit/journal vocabularies are not.

Project ordering and invalidation review evidence remains at
`7853a22f5c:scratch/work-on-another-machine-name.md`: shared rank wins over edit time;
missing inventory is not deletion. Positive removal and observation age survive.

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
Read coverage: the sole immediate child, Release, has both
its GOAL and complete memory inspected; no child-specific history is moved here.

The typed writer's transport baseline and causal/import proofs remain at
`eda4a98097:scratch/work-on-another-machine-name.md`. The retained contract above
still governs; no execution or delivery-attempt exchange is authorized.

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

Checks: `git diff --check` and `lf context --json` pass for this prose reconciliation (authored budgets); prior build/Clippy/focused-test evidence unchanged, broader verification belongs to gate.

Context: authored memory/scratch fit. The generated Task seed/steers/workspace
exceeds its 16,000-token limit; it is not an authored plan to trim. The complete
launch source was inspected; no limit was raised.

Earlier transport, synthetic demo, recovery and reduction evidence:
`ae96102580e`, `7853a22f5c` and `fc7f5cac81` in this plan and
`scratch/simplification-opportunities.md`. These prove no configured acceptance.
