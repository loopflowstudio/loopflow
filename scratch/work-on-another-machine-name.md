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

The internal model cut removes TaskPlan and ProjectPlan, moves their fields onto
Task/Project, removes Project iteration from the live type (retaining stored values
and events), and moves ProjectKR/MetricTarget to their domain owners with Threshold.
Linear writeback types now name Linear. TaskComment lives under Task; private-to-crate
LinearComment/LinearIssueObservation replace public Issue models. One adapter conversion
owns authorship and both save paths share the typed writer. Receipt bytes, stale-read
handling and direction identity remain unchanged. No schema or transport shape changed.

This is not the complete Next pursuit. Remaining: structured Project persistence,
replacement of ProjectContent/shared PmProject, integration-owned bindings and
observations, Linear configuration/command names, normal
Tasks/comments replacing FollowThrough and its gates/UI, and the repository cron
list/conversion. The PR is not ready to republish for this reduction.

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
- [One-page +/- review](pr-review.html) and its source manifest now describe this
  reduction. The installed CLI has no `lf screenshot` command, so visual capture
  is unavailable. Publication is review-only; do not land.

Checks: `cargo build -p loopflow --bin lf`, fmt, all-target Clippy and four focused comment/DTO tests pass; gate owns full suites/Swift. Foreground fixtures now permit immediate delivery and force an outage before testing conflict adoption.

Context: authored memory/scratch fit. The generated Task launch manifest and retained
steers exceed the 16,000-token goal budget by 2,230 tokens at this check; they are not an authored source to trim.
Do not delete direction to hide the overage.

## Delete — do not maintain

- `ProjectContent` and shared `PmProject`: one Project with structured criteria;
  preserve prose, bindings and historical events. TaskPlan/ProjectPlan, live iteration,
  PmKr/ChapterMetricTarget and public Issue comment/observation models are removed.
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

Sync check: `cargo test -p loopflow --lib edits_imports_and_failed_saves_share_atomic_planning_without_execution` passed (1 test; LF_* cleared, stdin closed); broader verification remains with gate/CI.
