# One SQLite owner per product object

LOO-298 · Infrastructure · 2026-09-26

Status: reviewed specification with partial storage implementation. Resource
pressure cleared on 2026-09-26; all owed slice proofs have executed. The
latest [slice review](data-model-slice-review.md) owns actual results.
The current participant (name unresolved) approved proceeding with taskless
Invocations, Session owning Runs plus a current Run, and Flow / Invocation
naming. This supersedes the earlier Session-as-Run-child and Task-required
Invocation model. The implemented cut removes four derived Task SQL columns;
the full owner cutover and real-Home migration remain outstanding. See
[review feedback](data-model-review-feedback.md) for approval scope and assumptions.

## Problem

Jack needs a Session to retain its identity and Task when a PR lands, and every
reader to agree about that Task. Four storage formats assembled into one Session
list make rename, bind, completion and recovery separate mechanisms. Run ancestry
encoded in selector strings makes each reader reconstruct relationships that
should be foreign keys. The desktop pays for repeated scans and regrouping.

The Infrastructure objective is dependable execution with one owner per fact.
This work supports the chapter's generic Session → Task → progress KR and its
no-chasing-Sessions KR by removing attribution and recovery plumbing. It does
not establish either KR merely by changing schema or passing fixture tests.
The supplied chapter has no metric targets; the latency goal below is a proposed
acceptance measure, not an invented chapter commitment.

Accepted decisions live in [the handoff](from-loo291/demo-native-workspace.md),
especially “Main objects are main tables,” “Green light,” and the final nullable
parent correction. [The synthesis](from-loo291/data-model.md) summarizes them.
Earlier manifest-only and Session-owned ancestry proposals, sidecar binding,
Wave-default Flow, per-Wave Chapter clocks and six-pane limits are superseded.
The last Task steer explicitly commissions docs in present tense before code.

## The demo

Open a taskless interactive Session, rename it, bind it to a Task whose PR has
landed, and read the same Run ID and Task ID from `session list --task`,
`runs --task`, usage and the desktop. The same terminal retains its draft;
an independent conversation stays independent of that Task's invocation.

```sh
lf --interactive : "Review the parser"
lf session rename <session-id> "Parser review"
lf session bind <session-id> --task INF-123 --json
lf session list --task INF-123 --json
lf runs --task INF-123 --json
```

Also resume a Task review after its template files are unavailable: the same
Session opens, Complete saves feedback once, and the captured next step receives
it. Inspect an earlier Chapter while the transferred Task keeps progressing;
its old KR evidence remains unchanged.

These are end-state demos. The documentation and preparatory storage reduction
do not establish that these commands work in this checkout.

## Approach

The canonical field and invariant contract is
[Architecture Reference](../docs/architecture-reference.md#core-models-and-apis).
The short model is Repository → Chapter, Chapter × Wave → Project → Task →
Flow invocation → Run for Task-owned execution; Session separately owns Runs
and selects its current Run. Flow invocations may
also have no Task; their Runs still carry exact invocation membership. A present
Task implies Wave, but invocation does not imply Task. Session never has
independent ancestry. Authored Flow templates,
Wave files and large immutable evidence remain files. Mutable product facts
have SQLite owners.

### Documentation ownership

| File | Owns |
| --- | --- |
| `docs/architecture.md` | Entry example, model overview and links to detailed contracts |
| `docs/architecture-reference.md` | Fields, validators, durable owners, launch/settlement, Session and invocation contracts, historical vocabulary |
| `docs/waves.md` | Planning experience, repository-wide rotation and per-Wave chapter history |
| `docs/lf.md` | CLI examples, binding, Session lifecycle, Task Flow selection and recovery |
| `STYLE.md` | Concise contributor rules; `CLAUDE.md` and `AGENTS.md` are symlinks here |
| This design | Implementation choices, current evidence, migration protocol, proof and remaining review decisions |

The docs use the target's present tense as Jack requested. They are not release
notes. Source-linked specialist pages under `docs/architecture/`, authoring/config
docs, builtin skills and Swift documentation still describe parts of the current
implementation; reconcile those with the implementation before shipping. Do not
mask this boundary by teaching both storage models as supported modes.

### Replace owners, then switch all callers together

Introduce typed `Chapter`, `FlowInvocation`, `Run` and `Session` records through
the existing Store/SQLite boundary. Keep execution traversal in the engine;
move persistence from its two adapters into invocation transactions. Replace
the old tables/files and their callers in one coherent implementation change.
Internal commits are sequencing, not independently shipped dual-write phases.

`runs` is the record, not a manifest index that can be rebuilt and discarded.
Indexes begin with `(task_id, created_at, id)`, `(wave_id, created_at, id)` and
`(invocation_id, created_at, id)`. Runs also index `(session_id, created_at, id)`.
Sessions have a stable Session ID, a `current_run_id` FK and an open-state index.
`Session.runs` queries Run's nullable `session_id`, not a duplicate stored list.
Query filters apply before pagination. A joined Session
read obtains current Run ancestry/provider/cwd without scanning all Run directories or
calling launch preparation. Detailed transcript/usage reads can open artifacts
for the already selected Runs.

The existing Work selector resolver remains a CLI input facility. It resolves
IDs once without requiring a live PR or producing launch side effects on a
read. Task launch preparation and delivery authority stay separate operations.
Project-attributed planning Runs retain their Wave; Project is not a fourth
Run parent. Project-level historic launch context remains in immutable evidence.

### Writers and transaction boundaries

| Entry | Required write |
| --- | --- |
| Direct Skill, inline prompt, interactive CLI | Resolve explicit selectors, else registered checkout Task; reserve one Run and optional Session |
| Wave/Project operation | Independent Run with Wave attribution; no invocation authority |
| Flow driver (Task-owned or taskless) | Claim invocation; create its exact Run with node and launch-time iteration tuple; copy the invocation's nullable Task |
| Human Flow step | Reserve Run + `flow_review` Session and bind the pending boundary atomically |
| Ask | Child Run with causal caller ID, inherited Task/Wave, independent membership; `ask` Session stores request, retry key and result |
| Replay | New Run with inherited ancestry; causality is not authority to join the previous invocation |
| Bind | Confirm the exact target once; fill current Run missing parents/provenance transactionally. Never change or clear its Task or existing Wave; historical Run scope remains below |
| Rename | Update Session title/provenance with human-over-generated ordering |
| Replace conversational Run | Append a Run under the same Session; compare the expected current Run, then update current Run and any pending review attempt atomically; preserve earlier Runs and Session attributes |
| Ready/Complete | Persist Session feedback; Complete closes once and settles the exact waiting boundary before teardown |
| Provider start/attach/stop | Update Run-owned native identity and exact process receipts; reconcile liveness against the OS |

Choose the store and artifact root from one resolved Home. Existing
`LF_CONTROL_HOME`/`LF_HOME` divergence must not split a Run row from its evidence.
Carry the selected store identity into child processes. An unbound launch needs
no planning provider, Wave or daemon, but does need its local writable store.
The recorder gets already validated typed input; the engine does not load Work.

SQLite and artifact publication are not one transaction. Reserve a prepared row,
publish final manifest/context, mark launchable, then spawn. A failed preparation
is visible and retryable under the same ID. Publish terminal evidence before
settling the row so interruption between those writes is recoverable. No second
provider launch is authorized merely because a spawn receipt is missing. Keep
the existing exact process-ownership mechanism, moved to Run-owned operational
storage, and preserve uncertainty. Telemetry remains best effort.

“No sidecars” removes mutable name, resolution, Ask, provider-session, client,
client-stop and prepared/launching-marker owners. It does not remove immutable
manifest/context/terminal evidence, append-only streams, provider-native history,
or unrelated Git/process exclusion locks. Multiple exact client receipts may be
normalized as Run children; they are operational records, not another Session
store. Never infer signal permission from `sessions.state = active`.

### Validation and denormalization audit

The three parent checks are `Run.invocation.task == Run.task` when invocation
is present, `Run.task.project.wave == Run.wave` when Task is present, and
`child_invocation.task == parent.task` when a runtime parent is present.
Invocation Task is nullable; the equality checks include null. Taskless Flow
Runs retain their node and tuple and may have a Wave or no Wave. The one-current-
root-per-Task constraint applies only to Task-owned invocations.
Constructors fill omitted ancestors; explicit mismatches fail atomically.
The reference groups these with planning and structural constraints. Import,
bind and parent-changing writes use the same checks as launch.

| Pair or duplicate | Treatment |
| --- | --- |
| Task Wave vs Project Wave | Remove stored Task Wave; derive through Project |
| Project Wave/Chapter repository | Validate same repository; unique pair; one current Chapter per repository |
| Run Task/Wave and invocation/Task | Retain nullable indexed parents; validate in the write transaction |
| Invocation parent/Task, current root | Same nullable Task, acyclic parent chain, one current root per Task when Task-owned; restart closes the old tree |
| Session Task/Wave/provider/cwd | Read through current Run; cross-Run bind scope remains an explicit review assumption |
| Session ID/Run ID | Distinct identities: Session owns Runs through `runs.session_id` |
| Session current Run/Run Session | Validate `session.current_run.session_id == session.id`; reserve both in one transaction; a late replacement cannot overwrite a newer current pointer |
| Node/cursor/return counts | One invocation cursor; node must belong to captured graph; no parallel flat cursor projections |
| Run tuple vs mutable invocation counts | Launch-time snapshot; validate while holding the chain, then immutable, never compare it to later counters |
| Session readiness in Flow/Ask and Session | Session owns feedback; invocation stores only pending Run reference and settlement identity |
| Run outcome vs terminal artifact | Row lifecycle is query authority; immutable terminal receipt is settlement evidence; validate receipt identity during recovery |
| Native provider ID vs attachment/process evidence | Different facts; each has one Run-owned record, exact process receipts still fence stop/move |
| Generated and human names | One conversational title; conditional update prevents generated overwrite |
| `started_at` vs Run existence | Shared Run writer sets the timestamp once at first assignment (launch or bind); validate non-null iff any Run names the Task. Readers use the column; retire event writer only after conversion |
| Swift Work/path vs Run parents | Typed IDs in DTOs; labels computed from cached readings, no selector decoding or cwd-based regrouping |

Jack's 2026-09-26 correction selects one Started fact: any Run with `task = X`.
His final timestamp correction stores `tasks.started_at` once at first assignment
by launch or bind, using that operation's time. It is not `MIN(created_at)`;
later assignment of older or newer Runs leaves it byte-identical. The single
Run writer sets it atomically and validates non-null iff a Task has Runs.
Offline import sets it for Tasks with Runs and leaves all others null. Sidebar
and roadmap read the column through one reader. His write-once bind decision
makes this monotonic. Bind fills a null Task
and its Wave, or a Wave on a parentless Run; a Wave-only Run can take a Task only
in that same Wave. No rebind, unbind or clearing form exists. The operation and
every UI state the exact target and confirm once before writing. Usage and
history never move between Tasks. Invocation nullable Task equality also still
applies: binding a taskless invocation's Run independently to a Task rejects.
Authorized cross-Wave Task moves are separate and retain their existing rule.
Chapter retirement still checks authored work, PRs and claims; missing Runs alone
never prove untouched backlog. The two intervening alternative Started steers
are superseded, not additional requirements.

Cross-Run attribution is an implementation assumption, not a newly confirmed
product decision: preserve the existing Run-owned ancestry model, project the
Session through its current Run, and let replacement inherit that attribution.
Bind updates the current Run; it does not bulk-rewrite earlier Runs. Each Run's
usage follows its own attribution. If Session-wide historical binding is needed,
resolve that ownership explicitly before extending the operation; do not silently
add a second Task field or rewrite invocation membership. The participant's
approval covered cross-Run identity, not this unasked consequence.

New membership is known by construction. Old missing capture is not proof of
independence. Proposed stored `membership_known` distinguishes that missingness
without a file fallback: false implies no claimed invocation/node/tuple; a new
independent Run is true with null location. This is a historical evidence field,
not a second attribution mechanism. Binding never changes it.

### Flow graph and runtime nesting

Fully unrolled means expand template composition, including all Xor alternatives
and their captured Skill content. Keep loops finite as backward edges; do not
expand hypothetical future passes. Use typed local node IDs, not a string path
through templates or cursor children. Selected Xor paths traverse those captured
nodes and do not manufacture invocation parents merely for composition.

Each entered nested loop body has a child invocation for that pass, with its
own graph/cursor/counts and a parent entry node. Create the child and parent wait
link together. Retry uses that child, and a subsequent pass creates a fresh one.
Child completion and parent resumption commit together. The child graph comes
from the parent's capture, never a fresh catalog load. Overlapping return edges
in one body remain local counters, not false parent relationships.

Preserve the exact decision fence: invocation identity, version, worker
generation and original successful Run. A saved candidate from a failed Run
cannot be consumed after reclaim replaces its binding. Restart does not reuse
execution identity. Retain finished invocations; completion clears the current
root without selecting a successor or completing Task Work.

### Sessions and desktop

One list query returns all three kinds. Complete and Ready retain their current
meaning; provider exit leaves resumable history rather than resolving a review.
A published Run that cannot be recovered stays in its Session's history.
Its replacement is a new Run belonging to that same Session, linked as a retry.
Update `current_run_id` and the exact pending boundary atomically, rejecting a
stale expected-current-Run value. Keep Session identity, name and feedback;
delete `carry_session_name` rather than moving the copy into SQL. An unpublished
preparation retry retains its reserved Run ID. Closed Sessions retain their last
current Run. Migrate pane IDs once to stable Session IDs using the import mapping;
Run replacement never changes the pane's Session key. This preserves UI identity,
not a promise that a dead provider process or its native state can be recovered.

Move Rust, Swift and JSON fixtures together. Session DTOs expose distinct Session
and current Run IDs, access to Run history, and typed current Run
parents and membership once, plus Session attributes/actions; remove `work`,
derived `work_path`, duplicate ancestor fields and Project variants. Required
fields have no DTO defaults. Historical unknown membership stays explicit.
Cache grouping and reverse lookup at reading changes; bound Tasks missing from
the current roadmap stay reachable and are never labeled orphan. Orphan means
null Task, including Wave-only conversations.

This checkout predates the handoff's workspace projection. Reconcile the
available Product branch at implementation time through the normal integration
path; if it has not landed, apply these contracts to existing `PodiumModel` and
`SessionsView` rather than creating a competing workspace. S6 control-room
presentation, Session streaming, palette and S9 folded-template UI are later UX
slices. LOO-298 supplies their model, bind API, correct grouping and cache.

### Repository Chapter rotation

Reuse classification, provider reconciliation and frozen evidence from
`ops/chapter.rs`; change the operation's scope and persistence owner. Prepare all
Wave successor plans, including explicit empty plans, before local activation.
Snapshot stable membership; if Wave membership changes before activation, refresh
the preview. One transaction selects the new Chapter and local Project bindings.
External Issue moves and predecessor archival can remain pending and retry from
the fixed boundary. Never claim a distributed transaction with Linear.

Moving unfinished Tasks inside their Wave retains invocation, claim, Run, Session,
worktree and PR. A cross-Wave reassignment is separate from rotation: reconcile
existing mutation authority first, then update dependent current Run attribution
atomically if the move is authorized. It cannot confer delivery authority through
the new FKs or rewrite the launch artifacts. Frozen plan membership and targets
keep history investigable even after a Task moves.

### One-time Home migration

Schema migration and filesystem import are distinct work. A new ordinal-free
draft creates the owners and constraints; released migrations are never edited.
The install/promotion path must not activate table-only readers over an unimported
Home. Use the existing promotion boundary and explicit offline import tooling;
do not teach ordinary reads to create rows lazily or read old subjects forever.

1. Inventory actual selected stores and artifact roots by Home, binary frontier
   and checksum. Do not assume `$HOME`, `LF_HOME` or the handoff's counts describe
   the installed selection. Account for pinned old writers and UI/client owners.
2. Rehearse on consistent store/artifact copies. Retain SQLite backup plus exact
   input bytes and an import report outside runtime read paths. Record original
   row/Run IDs, file hashes and old Session → Run ID mappings.
3. Import planning identities first. Equal chapter labels across Waves do not
   prove synchronized history. Preserve dated per-Wave predecessor plans; assign
   the initial common current Chapter only from a reviewed mapping. Do not mint
   a fictional historical all-Wave rotation. Ambiguities block activation until
   resolved in the report.
4. Import captured invocations, cursors, pending reviews, claims and failures.
   Do not recompile templates. Map saved indices/paths to captured local nodes.
   Retain original identity relationships and recovery facts. Import taskless
   invocations with null Task, preserving their captures, progress and reviews;
   no Task assignment or forced terminal disposition is required.
5. Import Runs from manifests and exact receipts. Resolve subjects using stable
   IDs and unambiguous historical aliases, with ancestry used to disambiguate.
   Never use the launch resolver, active PR, cwd spelling or a current display
   label as historical authority. Preserve Declared/Inherited as recorded; do
   not retroactively relabel old inferred launches Checkout without evidence.
6. Import Sessions from interactive evidence, pending/completed Asks and human
   boundaries, preserving conversational identity separately from Run identity.
   Map recorded replacements into that Session's Run history and select its
   recorded current Run. Never merge conversations by matching name or cwd.
   Carry names/provenance, readiness,
   completion, native identity and exact attachments. Old boundary records with
   no published Run reserve a prepared identity, not a fabricated successful Run.
   Missing execution capture remains Unknown. Ambiguous attribution is repaired
   explicitly before cutover; it is not silently nulled as a successful import.
7. Verify constraints, counts by kind, identity mapping, terminal outcomes,
   completed Ask answers and captured cursor bytes/meaning. Re-run the importer:
   identical inputs must be a no-op, conflicting inputs a named error. Test
   interruption before and after each durable stage.
8. At real cutover, quiesce exact old writers using existing control authority,
   take final backups, import the final delta and activate the matching binaries
   and database together. Do not stop this Task's own worker mid-migration;
   hand that maintenance boundary to an external invocation. Unknown writer
   liveness blocks destructive cleanup, not read-only rehearsal.
9. Read back through the configured CLI and app. Only then delete retired active
   sidecar/cursor files. Keep archival evidence and rollback bytes. Rollback is a
   matched old binary/store/artifact restore before new writes; after new writes,
   preserve those writes and forward-repair rather than blindly restore a backup.

Jack authorized one-time repair on his machine. That authorization does not make
guessed identities true or authorize unrelated provider/Task mutations. The
import tool may live under `scripts/` with preservation tests; runtime code has
no historical fallback. Normal schema migrations remain supported for published
databases. The final deletion pass removes conversion-only runtime helpers.

## De-risking

Read-only findings below are from this checkout after documentation checkpoint
`ef817d4f9`; line citations in LOO-291 are not assumed current.

| Question | Finding | Impact on design |
| --- | --- | --- |
| Are the handoff's files already here? | `WorkspaceProjection.swift` is absent; `RunManifest` has subjects but no captured membership; `human_session.rs` has no rename writer. `PodiumModel.swift:50` owns readings; `SessionsView.swift:193` reconciles them. | Coordinate source integration later; do not promise deletion of code absent from this branch. |
| Is there an old `runs` table to repurpose? | `0.12.15.001_release.sql:332` drops it and earlier execution tables; the schema checker discovers 31 live tables without `runs`. | Create the new record in a forward draft; preserve older-frontier history during migration, not by editing the earlier table creation. |
| Are there still four Session sources? | `ops/human_session.rs:269` concatenates Task, Ask, standalone Flow and interactive lists. `:911` parses manifest subject IDs; it does not call the launch resolver in this base. | Four-owner defect is confirmed. The handoff's post-merge resolver bug is evidence from another tree, not a reproduced local symptom. This base also loses identifier-form ancestry. |
| Can ordinary Flow invocations lack a Task? | `ops/flow_run.rs:34` stores optional Task/Wave selectors and `create` at `:149` persists them without Task ownership. | Interactive review confirmed that Flows must remain taskless-capable. Invocation Task is nullable; use the same SQLite owner and execution machinery for both paths. |
| Does the engine already have runtime loop children? | `engine/execution.rs:22` has a cursor child for Xor; `NestedCursor` has only Xor. Loop progress lives on a cursor. | A table rename is insufficient. Preserve routing and introduce runtime loop parentage deliberately; do not label Xor nesting as loop nesting. |
| Is repository-wide rotation already atomic? | `store/sqlite/chapters.rs:33` clears current only for one Wave; `work/chapter.rs:51` gives Chapter one Wave. | Reuse provider recovery but change the aggregate and transaction boundary. |
| Can Started writes disappear safely? | `chapters.rs:78` writes an event and `:97` combines it with position generation. Retirement rechecks those facts in its transaction. | Replace live started queries; retain historical start evidence and authored/PR/claim checks for retirement. |
| Are ordinal races a reason for sidecars? | `store/MIGRATIONS.md` specifies ordinal-free drafts, dependency ordering, backups and typed JSON validation. | No. Use the migration system and populated preservation proofs. |
| Can ordinary rows and artifacts select different Homes? | `human_session.rs:290` resolves observability Home; `ops/flow_run.rs:64` resolves current Home. Wave memory records this divergence. | Resolve once and test with conflicting environment selections. |
| Can a documentation-only spec pass source/schema parity now? | `check_architecture.py` reports missing current `task_flow_positions` and `wave_chapters` from the target owner map. The other seven inventories pass. | Record the exact gap; do not modify code or dilute the checker before review. |

No new live Home inventory or latency measurement was taken. The handoff's
~340 manifests and 0.8–3.5 s reads are dated motivation, not this branch's baseline.

## Alternatives considered

| Approach | Tradeoff | Why not |
| --- | --- | --- |
| Mutable manifest with typed parents and inline name | Smallest change; still scans files, Ask/Flow recovery remains elsewhere | SQLite ownership was selected; the interactive review further makes Session a stable owner of Runs. |
| Session table plus manifest-backed Runs | Makes rename easy; current attribution still crosses two owners and headless Runs need another mechanism | Preserves the defect and makes bind/usage disagree unless another adapter is added. |
| One SQLite owner for every mutable product record | Larger coherent migration; simpler readers and common validation | Chosen. Complexity belongs in a bounded conversion and exact settlement, not perpetual reader dispatch. |

Wild success is mundane: an idle desktop does little work, every view names the
same Task, and resuming a review does not involve finding which file owns it.
Wild failure is a new SQL projection beside the old files: each “temporary”
fallback hides a lost import and old writers keep resurrecting stale truth.
The cutover and deletion conditions rule out that outcome.

## Key decisions

- Interactive review, 2026-09-26: the current participant (name unresolved)
  requires taskless Flows. Invocation Task is nullable. Preserve taskless
  execution and recovery through the same SQLite invocation owner and driver;
  no synthetic planning records or separate legacy adapter.
- Naming confirmed in interactive review: Flow is the template; Invocation is
  an execution (Flow invocation in full). Project's Flow remains a default.
- Session has its own ID, owns its Runs, and selects a current Run. Replacement
  keeps the same Session, title and feedback; previous Runs remain history.
  Validate the current Run belongs to the Session and fence replacement writes.
- The documentation review changed no code, tests, fixtures or migrations.
  The subsequent storage cut is recorded below; it does not complete the design.
- Use table-only current readers after one import. Keep raw evidence and the
  import report, not a parallel legacy runtime.
- Prefer truthful unknown history over fabricated independent membership.
- Run records require SQLite before spawn. This is an intentional change from
  file-only launch resilience and must be called out in review.

## Scope

- In scope: listed documentation; complete model/validator and migration design;
  after review, Chapter/Project ownership, invocation/Run/Session writes and
  reads, bind/rename, DTOs, Swift identity/grouping cache, real-Home conversion
  and the requested deletion research followed by deletion.
- Out of scope: visual redesign, dark mode, first-run onboarding, control-room
  composition, palette, Session event streaming, provider authentication
  redesign, new orchestration platform and unrelated Wave recovery work.
- Preserve: source-independent continuation, exact process authority, saved
  feedback, terminal Work semantics, PR chains, original provider counters,
  Work placement and existing promotion history.

## Done when

For the full implementation, collect these proofs on the final integrated bytes:

1. **Ancestry matrix:** none, Wave-only, Task-only input, invocation-only input
   fill correctly; wrong Wave/Task, wrong parent, cycle, invalid node/tuple,
   duplicate Session ID, foreign current Run, duplicate current root and duplicate Project pair reject
   without partial writes. Race bind/launch against an ancestry-changing write.
   Include taskless invocations with and without Wave attribution, nested
   taskless children, and rejection of mismatched nullable Tasks. Launch a
   taskless Flow through the CLI and resume its review after template removal.
   Replace a Session's Run twice; retain its ID/name/feedback and both previous
   Runs. Reject a late replacement or completion from the superseded Run.
2. **One CLI reader:** isolated actual CLI launch, rename, confirmed write-once bind,
   landed/done Task and explicit-selector-vs-checkout cases; Session and Run
   outputs agree. Initial attribution fills without changing counters or artifacts; Task attribution
   never moves. Reject rebind, clear and Wave conflicts; confirm the exact target
   once in CLI and UI, including a competing bind. Launch and first bind set `started_at`; later
   launches and binds of older/newer Runs preserve its exact bytes. Compare
   timestamp presence against Run existence, including after offline import.
   Reject a conflicting bind on an invocation Run atomically.
3. **Execution preservation:** multiple provider turns, nested loop returns,
   Xor selection, review completion and keyed Ask retry; remove template sources
   before recovery. Test interruption, restart with reused numeric generations,
   late results and failed candidate Runs. Helpers cannot settle a cursor.
4. **Chapter operation:** two Waves including an empty plan, concurrent start
   versus retirement, loss of a provider response, missing Task/Project reads,
   external reassignment and retry after activation. One current repo Chapter;
   preserved active identity and frozen predecessor evidence.
5. **Populated import:** old SQL frontier plus real-shaped Run/Ask/Flow files,
   human name, completed Session, unknown membership, unresolved aliases and
   controller-only evidence. No silent discard; rerun idempotence and fault
   injection at publication/cutover boundaries. Canonical materialization passes.
6. **DTO/desktop:** shared fixtures in every consumer; bound Session absent from
   roadmap stays bound; Wave-only is orphan; cached projection changes on data
   changes only. Mounted PTY proof retains the same surface/draft across bind
   and rename; Run replacement retains the Session pane key and its Run history.
   Real installed proof is separate from fixture transport.
7. **Configured acceptance:** backed-up real Homes converted and read back via
   matching installed CLI/app; exercise the demo and preserve Run IDs, exact
   binary/store identities and failure evidence. No substituted development
   Home and no installed-success claim from a simulated provider.
8. **Deletion and consistency:** no runtime subjects/sidecar reader or writer,
   four-way Session union, file Flow adapter or duplicate Started writer.
   `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, affected
   Rust/Swift suites, migration checks and architecture checker pass. Update
   specialist docs and generated HTML to the final implementation.

Use TESTING.md's isolated environment and executable rules. Focused behavioral
proof belongs to each implementation cut; affected suites run once at gate.
Do not claim a full CI, deployed recovery or UI acceptance result from this spec.

## Forbidden outcomes

- Creating tables while `session list` still reconstructs product records from
  four stores, or fallback reads that silently repair missing imports.
- Changing or clearing a Run Task, moving its existing Wave through bind, or
  rebinding execution membership to satisfy a requested ancestry change.
- Replacing a Run by replacing its Session, copying its title, dropping earlier
  Runs, or letting a late old Run change the current pointer or settle a review.
- Inferring membership, Task or process authority from cwd text, provider identity,
  taskless missing capture, listener absence or a stored Active flag.
- Destructively migrating while old writers can still publish sidecars.
- Discarding a captured cursor, completed Ask result, human name, historical Run
  or provider receipt merely because a new model cannot represent it yet.
- Counting a prepared Run as provider success, a cursor as exactly-once external
  effects, a fixture as live proof, or renamed docs as implemented behavior.
- Requiring a Task to execute or recover a Flow, inventing a synthetic Task,
  or retaining a separate file-backed path for taskless invocations.

## Internal slices

1. **Docs and review:** this design plus the five requested doc owners. Jack
   reviews the contracts and conflicts before implementation starts.
2. **Domain and storage:** forward drafts, typed constructors, indexes, three
   parent validators plus structural checks; populated conversion rehearsal.
3. **Execution and planning owners:** invocation driver, runtime nesting,
   Session/Ask transactions, Chapter aggregation and publication recovery.
4. **All consumers:** CLI launch/inference/bind/rename and Run queries, DTOs,
   Swift grouping/cache; remove alternate writers/readers in the same cutover.
5. **Real-Home conversion:** inventory and backup, exact-writer quiescence,
   verified import, binary activation and configured proof.
6. **Deletion research, then deletion:** inspect every obsolete resolver,
   sidecar function, type, field, cache, test and doc paragraph; record path and
   dependency evidence, then delete proven dead code and review the result.

Slices 2–4 form one architectural change; do not publish an intermediate reader
that depends on keeping the old store alive. Migration rehearsal precedes real
data mutation even though the real-Home pass follows implementation.

## This slice

Move Run ancestry construction into one transactional writer, used immediately
by Task review reservation and replacement. The writer accepts nullable
Invocation, Task and Wave, fills omitted ancestors, and rejects explicit
mismatches, including a Task supplied for a taskless Invocation. It requires a
SQLite transaction; the existing callers hold immediate write transactions.
Database constraints continue to protect direct imports and parent-changing
writes. Task-owned and taskless execution drivers are not unified by this cut.

Add `Run.work_source` with the four specified values. New review attempts record
Inherited. A forward draft leaves existing rows null: their provenance remains
unknown until the offline importer establishes it from retained evidence. The
draft changes no old capture, title, feedback, ancestry or publication value.
Current Session and history queries now use one Run decoder, replacing the two
copies that parsed ancestry independently.

Replacement inherits the current Run's ancestry and records a fresh creation
time. Its launch cwd comes from the Task's current worktree; previous attempts
remain unchanged. Session title, feedback and the current-pointer/version fence
stay in their existing transaction. The Run constructor is used code, not a
second writer, and does not introduce another launch path or public DTO.

Focused proof covers independent, Wave-only, Task-only and Invocation-only
inputs; taskless Invocations with and without Wave; mismatch/missing-parent
rollback including a reserved Session; and a competing Project Wave update.
The existing repeated-replacement proof now checks ancestry/provenance and
unchanged prior attempts. The populated migration fixture also checks that
adding provenance retains unknown values and exact capture/feedback bytes.
These fixtures now pass, including the populated migration after canonical
materialization. The [current review](data-model-slice-review.md) records actual
results and the corrected corrupt-neighbor fixture. They do not establish the
complete target contract or the newly selected bind/Started semantics.

The next cut must use this constructor for general launches while completing
the common invocation owner/driver, captured runtime nesting and all Session
kinds. Node/tuple validation, remaining Run lifecycle/process facts, Ask and
interactive conversion, common readers/bind, DTO/Swift identity/caches,
repository Chapter operation, populated offline import, real-Home maintenance,
configured acceptance, comparable measurements and deletion research remain.
Current-Run-only bind remains an assumption. All eight Done When obligations
and the supplied publication/stacking reports still govern; no intermediate
publication is selected.

## Slice ledger

- 2026-09-26 executed review: **58 distinct library tests pass**, including
  every previously owed filter, the 29-test Task-controller suite and 16-test
  durable-store suite (overlap counted once). The corrupt-neighbor fixture
  initially failed on omitted required Task planning/writeback fields; the
  repaired fixture and its full suite pass. Three populated preservation
  tests plus the schema proof pass again after canonicalization in a disposable
  source copy. Two real CLI status/roadmap integration tests pass against
  isolated registered-Task/current and prior-release data. Fresh-Home runs,
  usage and doctor all exit 0; doctor warns that the disposable binary has no
  known revision. Full details, initial failures, commands and evidence limits
  are in [the current review](data-model-slice-review.md). No installed proof.
- Latest decisions carried into the spec: confirmed write-once bind and
  set-once `tasks.started_at` at first assignment time, not MIN(Run.created_at).
  These still need the common Run writer and all-reader conversion. No new
  production owner cut occurred during proof-debt repayment. Jack's new PM
  cancellation/refused-start cleanup report joins the publication and stacking
  scope; no issue was mutated here.

- 2026-09-26 Run-construction static verification: `cargo fmt --all --check`,
  isolated `cargo clippy --all-targets -- -D warnings` (**18.14 s**) and
  working-diff whitespace pass. Migration validation passes with four ordered
  drafts and 52 unchanged shipped files. Architecture still reports **32/33**
  SQLite owners, missing `wave_chapters`; its seven other inventories pass.
  Clippy compiled the new matrix, concurrent-writer and preservation fixtures;
  no behavioral result or full-design acceptance is claimed. Historical branch
  whitespace and all earlier execution obligations remain.

- 2026-09-26 Run-construction cut: checkpointed the supplied concept review
  through `lf commit` before edits. Replaced the Task-specific Run INSERT with
  one ancestor-resolving writer requiring a transaction; both initial review
  reservation and replacement use it. Joined Session and history reads share
  the Run decoder. Added one forward provenance draft after
  `own_sessions_and_runs`; old provenance remains unknown.
- 2026-09-26 source review: kept nullable Invocation/Task equality, existing
  parent-update triggers, exact review/current-Run fences and immutable history.
  Replacement inherits ancestry but reads the current Task worktree for its
  launch cwd; copying old cwd would disagree with the actual launcher after
  relocation. Required a transaction in the constructor and its invoking store
  functions so ancestor lookup cannot become an unfenced read followed by insert.
- 2026-09-26 preflight and safe recovery fail at active `main-view-task`
  **15.3 GiB / 12 GiB**, **97.2 GiB** free. Recovery preserved the foreign
  active build. No behavioral test, materialized rehearsal, Home migration or
  configured provider/app acceptance ran. No PR or Task disposition changed.
- Focused proof owed for this cut, retaining every earlier command:

  ```sh
  cargo test -p loopflow --lib run_constructor_infers_ancestors_and_rejects_conflicts_atomically
  cargo test -p loopflow --lib run_reservation_serializes_with_project_ancestry_changes
  cargo test -p loopflow --lib review_session_retains_feedback_and_history_across_replacement_and_corrupt_neighbors
  cargo test -p loopflow --lib session_ownership_import_preserves_nested_reviews_and_nullable_parent_constraints
  ```

  Repeat the populated migration proof after canonical materialization in a
  disposable exact source copy. The constructor matrix exercises real SQLite
  transactions; the competing-writer proof uses two database connections and
  a barrier, with no provider. Neither establishes taskless CLI or native recovery.


- 2026-09-26 final selected-attempt static verification: `cargo fmt --all --check`,
  isolated `cargo clippy --all-targets -- -D warnings` (**1m 43s**), and
  working-diff whitespace pass. Clippy compiles the new action matrix and native
  handoff fixture; neither executed. Initial compile failures from the private
  Open split (unused arguments and an old test caller) were corrected before
  this final pass. No migration, DTO or canonical documentation bytes changed;
  previous receipts retain only their recorded scope. The `wave_chapters`
  inventory gap, historical branch whitespace and all eight Done When
  obligations remain unresolved.

- 2026-09-26 selected-attempt implementation: preserved both supplied review
  notes with `lf commit` before editing. Completion uses the caller's expected
  position; native resume/stop share replacement exclusion; exact/prefix rename
  compares the selected Run in its write transaction. Existing invocation,
  worker, publication and exact process fences remain.
- 2026-09-26 source review found two secondary identity hazards and repaired
  them within this cut: Open must return the reserved Run after child exit, not
  reread the current pointer; native startup must release its launch lock after
  publishing the client receipt, not retain it for the interactive lifetime.
  Kept continuation-before-teardown ordering. Ask/standalone callers use the
  shared lock handoff without converting their storage or claiming their
  selected-attempt contract is finished.
- 2026-09-26 preflight and safe recovery both fail: active `main-view-task`
  **15.3 GiB / 12 GiB**, **97.2 GiB** free. Recovery preserved the foreign active
  build. TESTING.md blocks behavioral execution and materialized migration
  rehearsal. No provider, installed Home, PR or Task disposition changed.
- Focused commands owed for this cut, in addition to every prior unexecuted
  command and populated materialization proof:

  ```sh
  cargo test -p loopflow --lib review_actions_preserve_selected_attempt_across_replacement
  cargo test -p loopflow --lib intentional_session_move_exits_cleanly
  cargo test -p loopflow --lib reopening_ask_cannot_overwrite_a_concurrent_completion
  cargo test -p loopflow --lib ops::flow_run::tests
  ```

  Retain the preceding replacement, historical-selector, Ready/stale-provider,
  publication, schema, Task controller and durable-store proofs. The new action
  matrix calls public operations with simulated native effects; it is not actual
  CLI dispatch, configured provider recovery, a mounted pane or installed proof.

- 2026-09-26 final static proof for the recovery/lookup cut: `cargo fmt --all
  --check`, isolated `cargo clippy --all-targets -- -D warnings` (25.34 seconds),
  and working-diff whitespace pass. Clippy compiled the authored fixtures; no
  behavioral pass is claimed. No migration/DTO bytes changed, so earlier migration
  receipts retain their scope; Chapter architecture coverage and historical
  branch-range whitespace remain unresolved.
- 2026-09-26 simulated code review: fixed prefix-selector ownership bypass and
  removed redundant final SQL lookup; moved native-history lookup before client
  transfer so a non-resumable attempt is not stopped merely while probing Open.
  Preserved SQL compare-and-write authority, independent historical Run identity,
  immutable artifacts and identified corrupt-capture bytes. Remaining model
  owners are still live dependencies and were not deleted speculatively.

- 2026-09-26 recovery and lookup implementation: checkpointed the supplied concept
  review with `lf commit` before editing. Added indexed Run → Session lookup and
  removed open-review enumeration from exact lookup. Historical exact and prefix
  references report retained ownership before any file-backed interactive action.
- 2026-09-26 publication review: moved SQL publication ahead of capture/recorder
  construction. Source review found that an earlier rejected publication would
  drop its CaptureHandle and write a failed terminal receipt; that alternate
  settlement is removed. Staged and published artifacts reconcile exact inputs;
  conflicts and terminal evidence remain unchanged. Unknown post-publication
  execution now stays unresolved, instead of being replaced from missing history.
- 2026-09-26 availability implementation: public review rendering exposes an
  invalid capture on its own Session and keeps neighboring conversations visible.
  Exact action lookup continues to fail with the identified capture error.
- 2026-09-26 resource receipt: preflight and safe recovery both fail at active
  `main-view-task` **15.3 GiB / 12 GiB**, **97.3 GiB** free. Recovery preserved
  the foreign active build. No behavioral or materialized migration test ran.
  No installed Home, provider, PR, Task disposition or Flow navigation changed.
- 2026-09-26 focused proofs authored for this cut (in addition to every earlier
  owed command):

  ```sh
  cargo test -p loopflow --lib reserved_publication_
  cargo test -p loopflow --lib review_publication_retry_preserves_identity_and_claims_sql_once
  cargo test -p loopflow --lib flow_session_name_and_membership_survive_sql_run_replacement
  cargo test -p loopflow --lib session_list_and_open_preserve_valid_reviews_beside_unreadable_captures
  ```

  The SQL-backed publication fixture uses the production capture entry and real
  store claim, with interruption injected at the publication callback. Its Open
  path proves retention of an uncertain attempt, not successful provider recovery.
  The corrupt-neighbor fixture runs public list and Open preparation (`resume=false`),
  not a native provider. Preserve those limits when the tests eventually execute.


- 2026-09-26 Task review draft static checks: `cargo fmt --all --check`,
  `cargo clippy --all-targets -- -D warnings`, migration validation and current
  working-diff whitespace pass. Three drafts are dependency-ordered; all 52
  shipped migration files remain unchanged. Architecture inventories pass except
  the existing `wave_chapters` gap (**32/33** SQLite owners). The older whole-
  branch whitespace findings remain unchanged. No behavioral pass is claimed.
  In addition to the earlier owed commands, run these under TESTING.md isolation
  after resource preflight permits execution:

  ```sh
  cargo test -p loopflow --lib review_session_retains_feedback_and_history_across_replacement_and_corrupt_neighbors
  cargo test -p loopflow --lib session_ownership_import_preserves_nested_reviews_and_nullable_parent_constraints
  cargo test -p loopflow --lib controller::task::planning_tests
  ```

  Repeat the populated migration proof after draft materialization in a
  disposable source copy. Before a full gate, complete the preparation recovery
  boundary and the rest of the owner/consumer cutover; these tests cannot stand
  in for those changes or the configured CLI/app acceptance.

- 2026-09-26 Task review ownership draft: added `sessions` and `runs` with a
  deferred current-Run/member constraint, indexed history and nullable ancestry
  checks. Task boundary writes reserve conversations, Session feedback feeds
  execution reads, and completion shares Task settlement's transaction. Launch
  reserves/publishes one Run identity; replacement retains the same Session and
  feedback. `lf session rename` now updates SQL-backed Task reviews. Other kinds
  are not converted and cannot use that operation yet.
- 2026-09-26 source review: removed the now-unused Task-position inventory;
  direct Session discovery does not decode unrelated captures. Exact execution
  errors name the Task and preserve the unreadable bytes. Corrected discovery
  of superseded/completed Runs so they cannot reappear as independent interactive
  Sessions. Closed and replaced conversations retain queryable store history;
  opening that history through the final CLI/desktop surface remains unfinished.
  Ready now also rejects claimed boundaries. No process authority comes from the
  new Session row or the Run's publication flag.
- 2026-09-26 proof authored: repeated Run replacement with retained human title,
  feedback and three history rows; stale Run publication/Ready/completion;
  unrelated corrupt invocation; exactly one completion event. Migration proof
  preserves nested captured review bytes and rejects a foreign current Run and
  taskless Invocation Run's null-to-Task binding. Controller fixtures now publish
  reserved review identities before Ready. These are storage/simulated-provider
  proofs, not configured acceptance, and have not executed.
- 2026-09-26 latest resource preflight: active `main-view-task` remains at
  **15.3 GiB / 12 GiB**, with **98.7 GiB** free. Earlier safe recovery preserved
  the active foreign build. No product test or materialized migration rehearsal
  ran. Clippy compiled all targets successfully; compilation does not validate
  runtime SQL or establish the new behavior. No installed Home, provider,
  publication or Task disposition was changed.
- The complete eight-part Done When remains owed. In addition to the unconverted
  owners and consumers, the manifest-to-SQL preparation recovery boundary above
  requires repair and fault-injection proof before this owner cutover can ship.
  Current-Run-only bind remains an assumption, with no new approval inferred.


- 2026-09-26 invocation-retention checks: final `cargo fmt --all --check`,
  `cargo clippy --all-targets -- -D warnings`, migration validation (two ordered
  drafts, 52 shipped files unchanged), and working-diff whitespace pass. Clippy
  compiles test targets but does not execute them. Architecture coverage is now
  30/31 SQLite owners, with only `wave_chapters` missing; its other seven
  inventories pass. Whole-branch whitespace still reports the older draft's
  blank dependency header and copied patch context, unchanged in this cut.
  The full-design Done When remains unsatisfied; there is no behavioral gate.

- 2026-09-26 invocation retention: replaced Task's singleton position table with
  `flow_invocations`, keyed by captured invocation identity. A partial unique
  index selects one current Task invocation; completion, restart and authorized
  reopen retain the previous row with its ending state/time. All Task writers
  and readers now use that table; no compatibility view or duplicate write was
  introduced. Nullable Task is supported by the schema, but ordinary/taskless
  Flow execution still uses its existing file path and has not been converted.
- 2026-09-26 preservation review: retained capture, cursor, feedback, pending Run,
  claim and failure bytes on close. Current claim reads exclude closed rows;
  chapter retirement still sees their historical execution. Added invocation-ID
  comparison to unclaimed Session writes so version reuse cannot let an older
  review overwrite a replacement. The old test that changed an invocation ID
  in place now uses explicit restart and a fresh version, preserving its stale
  completion counterexample. No current SQL writer addresses the retired table.
- 2026-09-26 new focused proofs authored: populated forward migration compares
  all retained bytes, exercises rollback for duplicate invocation IDs and missing
  Task parents, and retains the first-start trigger. Store proofs retain human
  feedback and final worker claims, reject duplicate completion/root identity and
  stale writes after restart, and keep started Tasks out of backlog retirement.
  The fixture applies its draft dependency explicitly before canonicalization;
  it resolves both drafts by marker after materialization.
- 2026-09-26 resource observation: preflight and safe recovery both fail at
  `main-view-task` 15.3 GiB / 12 GiB, with 98.9 GiB free. No foreign build was
  removed. Product tests, including the earlier two owed commands and the new
  preservation proofs, remain unexecuted. No Home inventory/import, installed
  promotion, provider launch, desktop proof or publication was attempted.
- This is an internal invocation-lifetime cut, not completion of design slices
  2–4. Session/Run tables and transactions, taskless persistence, shared driver,
  bind/rename, DTO/Swift consumers, Chapter conversion, offline Home import and
  deletion research remain. In particular, current Session discovery still
  decodes unrelated current Task invocations, and replacing a conversational Run
  still needs the stable Session transaction; retaining completed invocation
  feedback does not solve either counterexample.


- 2026-09-26 implementation: checkpointed reviewed documentation with `lf commit`.
  Selected removal of the four derived Task step columns as the first storage
  cut. Kept old root cursor inputs because the existing decoder still needs
  them for historical review records. This does not complete the owner cutover.
- 2026-09-26 implementation evidence: removed `flow`, `step`, `node_id` and
  `human` from current Task SQL writers/readers via a forward draft. Exact reads
  and review discovery share the captured-state decoder. Added populated
  preservation and nested-review discovery regressions; updated the fixture to
  use `open_ephemeral`, so its draft frontier does not depend on ambient authority.
- 2026-09-26 verification: `cargo fmt --all --check`,
  `cargo clippy --all-targets -- -D warnings`, migration checks and whitespace
  checks pass. Clippy compiled the new tests but did not execute them. Resource
  preflight and safe recovery both fail on another active checkout's 14.5 GiB
  build (12 GiB limit); TESTING.md therefore blocks behavioral execution. The
  architecture inventory still has exactly the two documented owner gaps.
  This is a compiled first draft, not a passing behavioral or implementation gate.
- 2026-09-26 code review: retained root cursor inputs after finding their
  historical consumer; retained every mutation's version/claim predicate and
  the chapter start trigger. Review discovery now also surfaces malformed
  autonomous captures; recorded that changed failure surface in questions.
  No public API, DTO, source-independent capture or process authority changed.

Focused commands still owed once resource pressure clears (isolated environment
per TESTING.md; materialization only in a disposable source copy):

```sh
cargo test -p loopflow --lib dropping_task_step_projection_preserves_execution_and_review_evidence
cargo test -p loopflow --lib store::sqlite::durable::durable_store_tests
```

The latter covers review discovery, historical progress, nested cursor recovery,
claim races, exact worker settlement and completion. Gate still owns affected
suites. Next implementation must continue the invocation/Run/Session ownership
cutover; this preparatory reduction is not an independently publishable slice.

- 2026-09-26 interactive review: the current participant (name unresolved)
  confirmed Session owns Runs plus a current Run, accepted Flow / Invocation
  naming, and approved proceeding with those changes. Updated identity,
  replacement, validators, migration, CLI examples and desktop proof. Cross-Run
  bind scope remains an explicitly labeled implementation assumption.
- 2026-09-26 interactive review: removed the proposed Task requirement for
  Flows following explicit feedback from the current participant (name
  unresolved). Updated model docs, contributor guidance, migration and proof
  requirements. Other review items remain open; no overall approval recorded.
- 2026-09-26: supplied worktree contained five documentation edits and the
  committed LOO-291 handoff. Preserved those edits at `ef817d4f9` through
  `lf commit`; no source changes were present.
- 2026-09-26: read current source and migration contracts. Corrected the premise
  that an unused `runs` table still exists; identified absent Product-side code,
  taskless Flow conflict, missing membership evidence and old-writer cutover.
- 2026-09-26: review of the inherited spec found direct bound Flow language
  contradicting the Task-only model, an `owner.json` assertion conflicting with
  receipt relocation, and “bound Run” incorrectly implying invocation membership.
  Fixed these in the docs; retained substantive tradeoffs for Jack's review.
- 2026-09-26: final documentation validation: `git diff --check` passed;
  a read-only local-link probe checked 112 paths/anchors with no failures;
  `render_architecture_html.py --check` passed after regeneration; the prescribed
  website command selected and passed both portable-architecture and README/index
  tests (79 deselected). No Rust/Swift behavioral suite ran for this docs slice.
  Architecture inventory still fails only the two recorded SQLite owner gaps;
  this is not a green implementation gate.

### Documentation review diff summary

- Architecture entry and reference replace file-backed Run/Session unions with
  table ownership, typed ancestry, explicit lifecycle and transaction boundaries.
- Waves and CLI docs use the repository Chapter and per-Wave Project model,
  Project Flow defaults, Task invocations, and Session bind/rename examples.
- The contributor guide points to those owners and validators; both guide
  symlinks still resolve to `STYLE.md`. Portable HTML is regenerated from source.
- This design retains the complete implementation, preservation and deletion
  path. [Review agenda](questions.md) records resolved decisions and remaining
  assumptions. That review changed only documentation; the subsequent storage
  cut is described in the slice ledger.

### Projection-cut compression review (2026-09-26)

No further executable reduction selected. The implemented model fits in one
ownership map:

| Fact | Current owner and consumers | Disposition |
| --- | --- | --- |
| Flow name, selected step, node and human policy | `QueuedInvocation` plus `ExecutionCursor`; `FlowPosition.current_checked` derives `StepRef` | The forward draft removes all four SQL copies. `StepRef` remains a derived execution value, not another store. |
| Captured cursor and recovery feedback | `task_flow_positions.review_json`; exact reads and review discovery share `decode_flow_position` | Retain the shared decoder. `step_index` and `iteration` still reconstruct flat historical records, as exercised by `legacy_flow_decisions_preserve_pinned_progress`. |
| Pending review Run and readiness | Task position, consumed by `ops/human_session.rs` and the exact settlement transactions in `store/sqlite/children.rs` | Retain until Session owns these facts and the importer preserves existing boundaries. |
| Execution authority | Position version, invocation identity, generation and exact worker claim | Retain; these fence different races and are not display duplicates. |
| Session inventory and wire shape | Four current sources feed Rust `SessionRecord`, mirrored by Swift `SessionRecord` and DTO fixtures | Still live. Removing the union or wire ancestry before table-backed replacement would remove capability. |

Followed the changed SQL through `store/durable.rs`, Task settlement, Session
discovery, `ops/task_execution.rs`, the Swift Session model and fixture consumers.
No current SQL caller still names the four removed columns. Historical migrations
and their populated fixtures intentionally retain the old shape. The migration's
capture/claim/readiness preservation proof and nested review discovery proof
remain necessary; neither merely tests the deleted representation.

The important review finding remains the larger read failure surface: review
discovery now validates autonomous positions too, so an invalid autonomous
capture can fail Session inventory. The full Session owner cutover must address
inventory directly; another stored human flag or fallback reader would restore
the duplication this cut removes.

Resource preflight and safe recovery both still fail on the active
`main-view-task` checkout (14.5 GiB / 12 GiB; 99.7 GiB free). No product test ran
and no other checkout's active build was removed. Existing formatting, Clippy
and migration-check evidence applies to unchanged executable bytes; this pass
changes only these working notes. The two focused commands above remain owed.

### Invocation-retention compression review (2026-09-26)

Reviewed `60639b3e6` from a clean working tree against the full amended design.
No further executable reduction selected. The earlier compression table records
the projection cut before invocation retention; `task_flow_positions` is now
retired from current SQL, not an additional live owner.

| Fact | Owner and consumer | Retained because |
| --- | --- | --- |
| Captured Task execution and lifetime | `flow_invocations`, selected by its partial current-Task index | Completion/restart preserve history; current reads exclude closed rows. No separate current-pointer table or compatibility view was added. |
| Execution identity | Row `id`, checked against captured JSON `id` | SQL indexes identity; the capture carries it through engine and worker claims. The database equality constraint guards this duplication. |
| Cursor and historical root inputs | `review_json`, `step_index`, `iteration` through `decode_flow_position` | Flat historical progress still consumes the root columns. Removing them requires conversion, not merely dropping current writes. |
| Final claim, pending Run and feedback | Retained invocation row | Closed claims are evidence, not active authority. Chapter reads distinguish historical start evidence from current claims. Session ownership has not yet replaced these fields. |
| Task execution API | `FlowPosition`, Store methods and Task controller | These still carry claims, review settlement and recovery. Renaming the wrapper alone would not unify ordinary Flow persistence. |
| Conversation and client identity | `human_session` dispatch, Rust/Swift `SessionRecord`, `SessionsView` pane keys and DTO fixtures | The four sources and existing wire fields remain live. Their deletion depends on stable Session/Run owners and migration of all consumers. |

Followed completion, restart, reopen, unclaimed review writes and claimed worker
settlement through the immediate transactions in `store/sqlite/{durable,children}.rs`.
Human completion compares the entire expected position; unclaimed updates match
invocation ID; worker claims include that ID. Version reuse therefore still has
an identity fence. Keep the distinct transaction predicates rather than hiding
them behind a generic terminal-update helper. Historical migrations and their
populated fixtures retain the old table intentionally.

The two material product gaps persist: Session lookup decodes unrelated current
Task invocations, and `open_boundary` clears feedback before replacement.
Invocation retention fixes neither. No new fallback, stored human flag or
parallel Session projection is selected. Taskless file persistence, Run/Session
owners, Chapter scope, import, desktop integration and the final deletion
research remain required by the full design.

Preflight and safe recovery both fail at `main-view-task` **15.1 GiB / 12 GiB**,
with **98.8 GiB** free. Recovery preserved the active foreign build. TESTING.md
therefore still blocks product tests; no behavioral or materialized proof ran.
Executable bytes are unchanged in this compression pass, so earlier static
receipts retain their stated scope. Only this review note changed; its whitespace
check does not clear the recorded branch-range whitespace failures.

### Task review ownership compression (2026-09-26)

Reviewed `136e37862` from a clean working tree through the new Session/Run types,
SQLite draft and transactions, Task controller, launch publication, CLI Session
dispatch, Rust/Swift DTOs, pane reconciliation and preservation fixtures. The
amended approval remains authoritative; the older Task title is superseded.

Removed the duplicate update path in `store/sqlite/sessions.rs::save_review_in`.
Previously an execution checkpoint could insert a replacement Run, switch the
current pointer, mark a Run published and overwrite saved feedback from copied
`FlowPosition` fields. Existing Sessions now retain those facts during cursor
checkpoints. Run reservation, publication and Ready are their mutation owners.
The only direct caller found depending on the removed update path was the
controller's restart fixture; it now uses the existing Session reservation,
publication and Ready operations. No schema or public wire shape changed.

| Fact | Owner / remaining representation | Compression decision |
| --- | --- | --- |
| Task execution, capture, cursor and claim | `flow_invocations`; `FlowPosition` carries the execution snapshot | Keep version and invocation fences. Cursor writes no longer update an existing conversation. |
| Title, feedback, completion and current Run | `sessions`; joined feedback and published Run ID still appear in `FlowPosition` | Keep the read snapshot for exact completion comparisons and feedback delivery; delete its authority to overwrite an existing Session. |
| Attempt identity, publication and history | `runs`; Session history queries `session_id` | Keep reservation/publication separate from native identity and process receipts. A row's publication flag grants no process authority. |
| Initial human boundary | Session + Run creation inside the invocation transaction | Retain initial creation, including its supplied seed fields. Removing all seed inputs requires migrating the remaining construction fixtures; this pass removes existing-Session updates only. |
| Historical inputs | `historical_session_run_id`, `historical_ready_summary`, root cursor columns and shipped migrations | Keep until populated offline conversion proves preservation. No current writer uses the historical Session columns. |
| Public Session and desktop identity | Rust/Swift `SessionRecord`, composite review ID, pane key | Keep until every conversation kind and DTO consumer moves together. The final stable Session ID and Run-history wire contract are still unfinished. |

Extended the existing repeated-replacement proof: checkpoints carrying an old
Run and either replacement or absent feedback leave the Session and all three
history rows unchanged, then the saved answer completes once beside an unrelated
malformed invocation. This is authored proof, not an executed result. The
controller restart proof retains its capability assertion with the production
Session operations supplying its state.

Source review confirmed that existing-Session current-Run, publication and
feedback UPDATEs now each have one writer in the Session transaction module.
Initial creation and offline import remain distinct creation paths. The
four-source public inventory, taskless file driver, manifest-based Run readers,
Ask feedback reset and provider sidecars remain live; deleting them before the
all-caller cutover would remove capability. The recorded manifest-publication
interruption gap also remains. This reduction does not solve that recovery
boundary, installed conversion, Chapter ownership or the full-design acceptance.

Preflight and safe recovery both fail at active `main-view-task` **15.3 GiB /
12 GiB**, with **98.7 GiB** free. Recovery preserved the active foreign build.
TESTING.md therefore prevents product test execution, including materialized
migration rehearsal. Focused proof owed after preflight permits:

```sh
cargo test -p loopflow --lib review_session_retains_feedback_and_history_across_replacement_and_corrupt_neighbors
cargo test -p loopflow --lib restarting_a_human_node_reuses_the_same_task_position
```

Use TESTING.md isolation and retain every earlier unexecuted proof. No Home,
provider, PR, Task disposition or Flow navigation mutation is part of this pass.
All eight Done When obligations still govern publication.

Static checks pass: `cargo fmt --all --check`, isolated
`cargo clippy --all-targets -- -D warnings`, and working-diff whitespace.
Clippy compiled the changed fixtures but did not execute them. Migration files
and the public DTO/documentation spec are unchanged; earlier receipts retain
only their recorded scope, including the Chapter inventory and branch-range
whitespace gaps.

### Recovery and lookup compression review (2026-09-26)

Reviewed `d57ab33d7` from a clean working tree against `4cd64be3d`, following
the amended Session ownership and taskless Invocation approval. No further
executable reduction selected. The model before and after this pass is unchanged:

| Fact | Current owner | Why the remaining representation stays |
| --- | --- | --- |
| Task execution capture, cursor and claim | `flow_invocations` | `FlowPosition` is still the controller's checked snapshot; its joined Session feedback supports exact settlement, without regaining feedback-write authority. |
| Conversation title, feedback, completion and current attempt | `sessions` | `runs.session_id` supplies retained membership. Exact and prefix lookup must consult it before manifest-based interactive dispatch. |
| Review attempt identity and publication | `runs` | Publication is a one-time SQL claim, distinct from provider start, terminal outcome and exact client ownership. None can be derived from the published flag. |
| Immutable launch inputs | Manifest and context artifacts | Reserved publication reconciles exact bytes before the SQL claim; ordinary new-ID publication requires exclusive creation. Combining them into an idempotent launcher would weaken the ordinary launch boundary. |
| Ask and taskless review preparation | Existing Ask/Flow files and prepared Run path | These remain live callers of `start_prepared` and `carry_session_name`. Their removal requires conversion to the common owner, not deletion of their recovery capability. |
| Desktop identity and grouping | Existing Session DTO, pane keys and `WorkspaceProjection` | Rust/Swift still expose the intermediate projection. Stable history DTOs and all-kind ownership must move together before removing grouping/lookup consumers. |

Traced the SQLite Session/Run API through reservation, publication, Ready and
Task completion; the launch callback through recorder construction; historical
lookup through rename/Open/Complete; and public inventory through corrupt-capture
presentation. Also inspected Taskless Flow persistence, Ask replacement, Rust and
Swift Session fields, desktop reverse lookup, Chapter activation, the populated
migration and the current architecture contract. No retired SQL owner was
restored and no compatibility path was added.

The apparent publication duplication is intentional at this boundary:
`prepare_manifest` already owns common construction. Artifact reconciliation
cannot subsume the SQL claim, and the claim must precede `CaptureHandle`
construction because Drop settles a Run. Likewise, historical membership lookup
and current-actor checks answer different questions. Preserve both. The duplicate
composite review-ID formatting and optional lookup wrapper are local cleanup
candidates; changing those alone would not remove the storage split or a product
concept, so no cosmetic API churn was selected.

Fresh resource preflight and safe recovery both fail: active `main-view-task`
is **15.3 GiB / 12 GiB**, with **97.3 GiB** free. Recovery preserved the active
foreign build. Under TESTING.md no product test or materialized migration
rehearsal ran. The latest implementation's formatting and all-target Clippy
receipts retain their scope on unchanged executable bytes; no new behavioral
pass is claimed. All focused commands in the slice ledger remain owed.

This pass changes only this review note. No interfaces, persisted fields,
migration bytes or user-visible behavior changed. The complete owner/caller
cutover, repository Chapter operation, offline import, configured acceptance,
measurements and final deletion research remain required before publication;
the architecture and historical branch-whitespace gaps remain unresolved.

### Selected-attempt compression review (2026-09-26)

Reviewed `830e9af7b` from a clean working tree, including the branch model against
`4cd64be3d`. No further executable reduction selected. The amended approval
still governs: taskless Invocations and Session owning Runs plus a current Run.
The model before and after this pass is unchanged:

| Fact | Owner / representation | Retention decision |
| --- | --- | --- |
| Conversation title, feedback, completion, current Run | `sessions`; member history through `runs.session_id` | Keep one conversation writer per operation; cursor checkpoints cannot overwrite these facts. |
| Selected attempt | `SessionTarget::Flow.run_id` and expected `FlowPosition` | Not interchangeable: `FLOW_POSITION_SELECT` omits unpublished Run IDs. Rename needs the selected reserved identity too. Neither value is another persistent owner. |
| Boundary identity across processes | Serialized `FlowSessionToken` | Still consumed by launch, Ready and captured Skill preparation. It cannot replace the full settlement snapshot; deleting it is not enabled by adding that snapshot. |
| Exact settlement | Original position/version/feedback compared in the SQLite transaction | Keep the comparison through completion; a fresh token check cannot replace the caller's expectation. |
| Native resume and stop exclusion | Session launch lock, handed to client publication | Keep through native effects, release before waiting for provider exit. It protects effects outside SQLite; current-Run validation alone cannot do that. |
| Rename scope | Optional expected Run in the existing write transaction | `None` means a conversation rename; `Some` preserves an explicit attempt selector. One optional expectation expresses the distinction without another command or owner. |

Followed lookup, Complete, Open, rename, controller settlement, reservation,
publication and native client handoff through their direct consumers and the
deterministic action fixture. Also checked Ask/taskless callers, migration parent
constraints, Run readers, Rust/Swift Session fields, desktop reverse lookup,
Chapter activation and canonical docs. The native-launcher fixture observes the
publication lock separately from the simulated client matrix; both proofs remain
necessary. The token and snapshot checks were retained rather than collapsed
into a new generic action abstraction.

The four-source list, Ask/name-copy path, taskless file driver, manifest Run
queries, Started writes and existing desktop projection remain live dependencies.
Their deletion still requires the complete owner/caller conversion and populated
import. No compatibility wrapper, schema change or public interface was added.
The prepared-Run paragraph in `docs/lf.md` still needs reconciliation with the
final all-kind publication contract; this review does not broaden Task-only
behavior into a documented all-kind claim.

Fresh preflight and safe recovery both fail: active `main-view-task` **15.3 GiB /
12 GiB**, **97.2 GiB** free; this checkout **332.9 MiB**. Recovery preserved the
active foreign build. TESTING.md therefore blocks behavioral execution and
materialized rehearsal. No tests ran. The implementation's formatting and
all-target Clippy receipts retain their recorded scope on unchanged executable
bytes; the selected-attempt matrix and every earlier owed proof remain unexecuted.

Only this note changed; its working-diff whitespace check passes. No Home,
provider, PR, Task disposition or Flow navigation changed. All eight Done When
obligations, the Chapter inventory gap, historical whitespace findings and the
unreproduced publication/stacking reports remain. No intermediate publication is
selected.

### Run-construction compression review (2026-09-26)

Reviewed `90cc69572` from a clean tree, tracing the latest constructor change
and the branch's owner model against `4cd64be3d`. No further executable reduction
selected. The amended approval governs: taskless Invocations and stable Session
identity with member Runs and a current Run. Model before and after this pass:

| Fact | Owner and consumers | Retention decision |
| --- | --- | --- |
| Run ancestry and provenance | `runs`; transactional `insert_run_in`, shared `read_run` | Initial review and replacement now use one constructor; Session joins and history use one decoder. Keep nullable provenance for imported history. |
| Ancestor consistency | Constructor plus SQL constraints and parent-update triggers | Constructor fills omitted ancestors; constraints also protect direct imports and later parent writes. These enforce different entry points, not competing owners. |
| Conversation and history | `sessions`; `runs.session_id` and current pointer | Keep title/feedback on Session. Replacement inherits Run ancestry, refreshes launch cwd from Task and retains previous attempts. |
| Execution and selected attempt | `flow_invocations`, checked `FlowPosition`, expected Run | Keep identity/version/claim comparisons and launch exclusion. An ancestry-valid Run does not grant settlement or process authority. |
| Publication | Reserved Run row plus immutable artifacts | Keep reconciliation before SQL publication and recorder construction; published is not proof of provider start. |

Inspected `session.rs`, both Store layers, the Run constructor/decoder, Session
reservation and replacement, Task settlement, publication dispatch, ancestry
triggers and populated migration fixtures. Followed public Session discovery,
Run filtering, the Rust/Swift Session DTO and `WorkspaceProjection`, Chapter
activation, and the canonical model contract. The repeated-replacement test
retains old attribution; the constructor matrix and competing-writer fixture
exercise distinct failure boundaries. None is a disposable representation test.

The Task-specific initial-review helper still supplies real launch inputs; merely
inlining it or centralizing enum string matches would not simplify ownership.
The remaining file-backed Ask/taskless paths, name copying, manifest Run readers
and desktop projection still have live callers. Their deletion requires the
approved common-owner conversion and populated import. No fallback, wrapper,
schema change or public interface was added or removed in this pass.

Fresh preflight and safe recovery both fail at active `main-view-task`
**15.2 GiB / 12 GiB**, with **94.4 GiB** free; this checkout is **332.9 MiB**.
Recovery preserved the active foreign build. TESTING.md therefore blocks product
tests and materialized rehearsal. No behavioral proof ran. Executable bytes are
unchanged, so the latest formatting/Clippy/migration receipts retain only their
recorded scope; all four Run-construction commands and earlier proofs remain owed.

Only this note changed; working-diff whitespace passes. All eight Done When
obligations remain, including all-kind ownership, runtime nesting, Chapter scope,
DTO/desktop conversion, offline import, configured acceptance, measurements and
deletion research. Existing architecture/branch-whitespace gaps and supplied
publication/stacking reports remain unresolved. No intermediate publication.

## Measure

The [2026-09-26 slice review](data-model-slice-review.md) records the current
evidence matrix and publication gaps. Behavioral proof remains blocked by the
resource envelope. It traces malformed autonomous captures through Session
listing and exact review lookup, and records the branch-range whitespace
failure separately from earlier working-tree checks.

Before implementation, capture 20 comparable `session list --json` and
`runs --task <id> --json` samples on a copied representative Home, recording
binary, row counts, environment and p50/p95. Repeat on the same data after import.
Use query plans and filesystem instrumentation to show filtering uses indexes
and Session inventory opens no manifest per row. Proposed target: local p95
below 300 ms at the handoff's ~340-Run population, including CLI overhead.

Desktop projection measurements use the existing harness if integrated from
LOO-291. Do not import its old cold-start/idle figures as new measurements or
add the separate streaming project to make this Task's numbers pass.
