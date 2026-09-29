# Resolve Tasks across Homes and select the official worker runtime

Status: design draft for LOO-334, 2026-09-29. No implementation or product
approval is implied. Jack Heart's Task and four refinement comments are the
product direction. Source inspection is against `a3820bf7e`.

## Problem

Jack cannot reliably find or continue work because the answering data directory
determines whether Loopflow says a Task exists. LOO-298 has Linear and Git
evidence but the official CLI says it does not exist. An inherited executable
and database wrapper also keeps workers on an older installation.

Jack selected two planning modes per Wave:

- Connected to Linear: Linear owns Task existence, title, status, Project and
  ordering. Git owns branches and commits; GitHub owns PR facts. Local planning
  data is a replaceable observation of those authorities.
- Unconnected: the local store owns Tasks. Linear is neither required nor
  contacted for ordinary operations in that Wave.

Execution has a different lifetime. Flow invocations, Runs, Sessions, claims,
placement, unpushed work and delivery attempts cannot be reconstructed from
Linear. They survive planning changes and remain attached by stable identity.

Infrastructure's chapter KRs require a generic Session to find, create and
advance Tasks without plumbing repairs or manual continuation. This design
addresses the discovery and launch portions, not a claim to complete either KR.

## The demo

In a disposable OS account, create execution history in data directory A and
select an official installation with directory B. From both directories,
`lf task status DEM-334` shows the same Linear Task and Git PR, and identifies
A as the location of its execution history, with a working reach command.
Continue the Task, change the official installation between two Flow boundaries,
and observe the next worker use the new executable while retaining the exact
invocation, review and execution directory. An explicitly pinned Task stays on
its chosen bytes and reports the pin.

Repeat with a Wave that has no Linear connection: create, inspect and run a
local Task without any Linear requests. B identifies the local Task in A when
A is discoverable; it never pretends that a missing local copy establishes
machine-wide absence.

## Approach

### Planning and execution are separate records

Use TaskSpace and TaskOps as the planning and execution concepts named in Jack's
steer, not new services, registries or public CLI namespaces. Reshape the
existing Task model and PM resolver in place. One joined Task view contains:

- Stable planning identity: a tagged Linear workspace/issue UUID or a local
  Task identity scoped to its owning repository and data store. Human-readable
  issue identifiers are lookup aliases, not execution primary keys.
- Planning observation: authority, current facts when known, freshness and an
  explicit result (`present`, confirmed removed, inaccessible/unresolved, or
  unavailable). The last successful observation keeps its own timestamp.
- Git evidence: repository identity, exact branch/ref and PR associations,
  with independent freshness. A PR is optional; a planning-only Task is real.
- Execution observations: zero or more located TaskOps records, each qualified
  by data directory, Home identity, Task ID and inspection result. A local
  execution projection is optional. Two divergent records stay two records.

An empty execution observation is not an idle worker. An unavailable planning
observation is not an absent Task. Never synthesize a worktree, claim, invocation,
Project or successful outcome merely to fill a required DTO field.

Persist execution once in the existing Task/execution tables. Local-mode Task
planning is durable in that store; connected-mode planning uses the existing PM
cache. Replace the unconditional `LinearIssueId` requirement with a tagged
planning reference and migrate all consumers together. Keep existing Task IDs,
Run foreign keys, invocation IDs and historical attribution. LOO-298 owns the
execution schema replacement; do not implement a second invocation/session
schema here. Its integration boundary is specified below.

### Select a Wave's mode explicitly from its connection

Use the existing `pm.linear_initiative` binding in Wave configuration as the
current connection declaration, with the repository's `pm.linear_team` as
provider scope. A valid binding selects Linear mode even when credentials,
network, current chapter cache or Initiative access are unavailable. A malformed
configuration is unresolved configuration, never permission to switch to local
authority. Missing Team configuration reports the incomplete connection.

No binding selects local mode for a known unconnected Wave. Missing Wave
configuration for a record previously observed as connected is insufficient to
disconnect it. Preserve that recorded binding as provenance and report the
configuration discrepancy. Explicit connect/disconnect owns authority changes;
a read, checkout switch or outage cannot do so. Connection must not publish
preexisting local Tasks implicitly. On connection, unmatched local planning
leaves the current connected plan; preserve execution and historical local facts.
Disconnect requires an explicit choice of which observed planning facts become
the local plan, without moving execution or automatically reopening work.

Wave existence itself is not decided here. Jack explicitly left Git versus
Linear versus local storage open. Proposed follow-up decision: connected Wave
existence follows its Initiative; unconnected Wave existence follows the local
store; Git owns authored GOAL/MEMORY content and connection configuration.
That proposal needs Jack's decision and a connection-bootstrap design. LOO-334
can resolve Tasks through existing bindings without rotating chapters or deleting
Waves. Do not infer Wave death from a missing directory or inaccessible Initiative.

### Resolve the issue before requiring local execution

For an issue selector, query Linear directly using the existing
`LinearClient::issue_ownership` operation, refactored so issue existence survives
a missing Project or unresolved Wave mapping. Reuse `pm_resolve_task`'s direct
lookup rather than `task_pm::resolve_task_async`'s cache-first search. An internal
Task selector first resolves its retained planning reference, including locations
outside the selected store, then uses the same authority-specific path.

Return the issue even if its current Project is not registered locally, belongs
to another known Wave, has multiple Initiative associations, or lacks a current
chapter receipt. Resolve the relationship where evidence is singular; otherwise
display the real issue and the unresolved relationship. Only a specific operation
needing that relationship must obtain it. Existence does not depend on launch
eligibility. Count only Initiative bindings belonging to this repository when
mapping Waves; unrelated Initiative membership must not invalidate a read.

Read Git worktrees/refs and GitHub PRs using the existing Git/PR operations.
Associate by exact recorded branch and repository, or the existing PR Task link
resolved to the issue identity. Issue-like branch names are candidates, never
proof of ownership. Preserve ambiguity when multiple PRs or branches match;
merged historical PRs are not automatically active successors. Unpushed branches
remain visible through their owning checkout even when GitHub has no PR.

Refresh the selected directory's connected planning cache after a successful
read; that cache write must not allocate TaskOps. `task status` must stop
completing Tasks as a side effect of observing a merged PR. Completion remains
an explicit delivery/Task operation and, in connected mode, is not effective
planning status until Linear confirms it.

### Find execution without a machine-wide Task registry

Build a transient set of candidate stores from the explicitly selected directory,
existing installation selections and retained switch receipts, and already known
Home routes. Reuse `machine_install::known_installations` and the read-only Work
identity reader. Include the legacy standard store. Canonicalize filesystem
aliases and deduplicate stores by file identity; Home ID alone is insufficient
because branch snapshots preserve the same Home ID.

For source-private directories not represented in installation receipts, include
only the known development-directory layout owned by Loopflow, using the existing
source data-root rules. Do not recursively search arbitrary user directories or
introduce a persistent Task-to-store index. An arbitrary external directory must
be explicitly selected or already reachable by a known route. Report the scope
searched and any unreadable candidates; universal discovery of unknown paths is
not possible without registration or a filesystem crawl.

Read identity/location facts through the minimal read-only schema surface,
without migration, provider login, copied credentials or foreign-store writes.
An unsupported schema yields an uninspected location and reason, not an empty
result. Do not require complete parent objects merely to locate a Task record.
Detailed history is read by a compatible executable in its owning store.

The result prints the exact data directory and Home ID separately. Its structured
reach command uses the official installation entrypoint with explicit `LF_HOME`
and `LF_DB_PATH`; remote destinations use existing `lf ssh` transport. For an
incompatible historical store, return the verified retained executable/store
command only if available and label it as explicit historical runtime selection.
Otherwise report exactly which executable/schema is missing. Never recommend an
unverified pair or silently select a store by modification time.

Read-only discovery can show both A and B. Continuation follows an already bound
invocation or an unambiguous existing execution record. If both contain independent
unfinished execution, show both and accept explicit data-directory selection;
do not merge, take over or launch a duplicate to eliminate ambiguity. An existing
checkout remains usable even if its current planning membership changed.

A local-only Task in another discoverable store is reported as local to that
store, with a reach command and no automatic copy/import. If no accessible store
contains it, say “not found in the inspected local stores,” naming inaccessible
locations, rather than asserting global nonexistence. Explicit `--wave` can
disambiguate a local Task name without probing Linear.

### Make official runtime selection independent of execution placement

Default Task runtime policy is `official`. At each worker boundary, resolve the
current selected CLI artifact through machine installation state, not PATH,
`current_exe`, the prior artifact's selection or inherited `LF_BIN` overrides.
“Official” means the machine's selected installation, not an HTTP lookup for the
newest version and not necessarily the published fallback.

Keep the execution directory and saved invocation attached to the work. A new
official executable does not choose a different Task database. Change installed
startup so the selected official runtime can honor an explicitly addressed
execution directory independently of its default directory. Reuse `LF_HOME` and
`LF_DB_PATH` for this address, with conflict handling and canonicalization shared
by startup and launch. Preserve branch-source isolation: arbitrary source bytes
still cannot migrate or mutate installation-owned data.

The launch boundary captures one resolved artifact/digest for the child and
records it in existing Run evidence. Pinning bytes for the duration of that
process avoids mid-launch substitution; it does not pin later boundaries.
`LF_CONTROL_*` remains same-Run authority and is not a future runtime policy.
Vendor tool wrappers use that Run's executable/store pair. Next-step launch
reloads policy and official selection even when the parent process is old.

Proposed explicit public controls, integrated into existing `task run` options:

```sh
lf task run DEM-334                         # persisted policy, official by default
lf task run DEM-334 --lf-bin /absolute/lf   # explicit pin, future boundaries too
lf task run DEM-334 --official-lf           # clear the pin without replacing Flow
lf task status DEM-334 --json
```

The two flags are mutually exclusive. Save a pin on the existing TaskOps record,
including the canonical artifact path and digest, without replacing the captured
Flow. Status exposes policy, explicit pin, last attempted/used executable, and
currently resolved next executable separately. A changed/missing pinned file
reports the precise failure; never silently follow a new symlink target.
Historical Run executable evidence remains unchanged. Migration defaults to
official and does not reinterpret inherited environment as deliberate consent
to pin. A live worker keeps its authority until settlement.

No-installation source operation remains possible using the invoking source
runtime and private store, visibly reported as source execution. When an official
selection exists, an old environment wrapper cannot select that fallback.

Schema compatibility remains real: a released runtime cannot execute a private
store containing unknown drafts. Status still discovers the Task and names the
location and exact incompatibility. It can provide a verified explicit pin/reach
command; it must not downgrade, merge, discard, or silently migrate private
history. Supported store advancement stays with existing installation/migration
operations. The release-between-steps proof must include a compatible release;
an incompatible release must preserve the pending boundary and report failure.

### Treat planning and execution mismatches as ordinary states

| Observation | Required behavior |
|---|---|
| Linear has Task; no TaskOps here | Render the Task, find Git evidence and known execution locations. Allocate execution only on an explicit run/checkout operation, reusing existing placement where found. |
| Linear canceled/completed Task; TaskOps has unpushed work | Show Linear's exact state plus retained checkout/history. Keep files, refs, review feedback and Runs. Do not infer completion from cancellation or automatically delete/publish/reopen. Read/edit/save remain available; new managed progression must respect current planning intent. |
| Task left the current Wave/Project | Show its new Linear membership and the old execution attachment separately. Refresh planning without rewriting historical Run ancestry. Resolve current operations against current membership instead of refusing because the old parent differs. |
| Explicit provider trash/deletion confirmation | Remove it from the active connected plan; retain TaskOps and last-known planning as history. Do not kill a process from planning evidence alone. |
| Missing issue, missing portfolio membership, permission error or partial GraphQL error | Report unresolved/inaccessible as appropriate, preserving positive history. None proves deletion. |
| Both sides disagree on title/status/ordering | Fresh Linear facts replace the planning cache. Execution facts neither compete with nor overwrite those fields. |
| Linear unreachable | Retain connected mode, show dated cached facts and local execution. Local inspection and file work continue. Provider writes report unavailable and retain their input/attempt evidence; no automatic local-mode success. |
| Local edit awaiting write-back | Treat it as an attempted operation with input and outcome, never current planning truth. Read Linear before retry after an ambiguous response; do not replay stale edits over fresh remote changes. |
| GitHub unavailable but Linear available | Render the Task and local refs, label cached PR facts with their age. Do not collapse the Task read into a PR error. |
| Multiple local execution copies | Show all locations and their own evidence. Continue exact bound work or require an explicit location for an ambiguous write; no timestamp winner. |

The preservation interpretation of “throw away anything that doesn't match”
applies to active connected planning, not destruction of execution or authored
work. This is a conservative design assumption pending Jack's confirmation.
Do not perform destructive migration based on that interpretation.

Keep pending provider changes within existing write-back/operation evidence;
do not invent an offline planning replica or general synchronization queue.
Connected creation commits to Linear before reporting success, retaining existing
idempotent creation markers. Local creation needs a real local planning identity,
not a fake Linear issue UUID. Local run/edit/complete/delete and PR presentation
must all work without provider context or a fabricated Linear chapter.

Keep the one-current-Project invariant in local mode. The existing chapter
operation persists a real local plan and local Project identity without calling
Linear; Task creation uses that plan. Extend the Project planning reference as
well as the Task reference where its current Linear-only type prevents this.
Do not add a Project operator, sibling current Projects or a second chapter
rotation implementation. Initial local plan setup belongs to the existing Wave
setup/chapter path; local-mode tests must exercise it before Task creation rather
than seed a fake provider snapshot. This is required integration work, not a
claim that local lifecycle already works in the current source.

### Preserve admission when startup is slow

The observed 10-second startup failure is supported by source: the caller releases
the worker claim whenever `wait_until_running` times out. A child arriving at
11 seconds can therefore be stale without ever having competed with another child.

Return `starting` after the bounded observation deadline, preserving the exact
claim and launch evidence. Release on a proven failed spawn or proven death under
existing claim/process fencing, not elapsed time. Concurrent retry observes the
same child; a late child cannot regain a claim replaced after proven death.
Keep this repair at Task admission, coordinated with LOO-298's Exec/Run ownership.
Increasing the timeout alone is not the repair.

## De-risking

| Question | Finding | Impact on design |
|---|---|---|
| Can an issue be read without a local PM snapshot? | `pm/linear.rs::issue_ownership` already queries `issue(id: $id)` by identifier/UUID. `ops/pm.rs::pm_resolve_task` already uses it. | Extend this path; remove cache membership as the lookup prerequisite. |
| Is direct lookup enough as written? | `OwnedIssueNode::into_ownership` requires a Project; `resolve_owned_issue` requires exactly one Initiative and a local Wave directory. | Decode issue presence before resolving optional ownership edges. Preserve issue facts on edge failure. |
| Why does status say absent? | `ops/task.rs::task_status` unwraps `get_task_by_issue` before any provider lookup. The official CLI reproduced `Error: no Task exists for "LOO-298"` on 2026-09-29. | Change the status return model and all consumers, not only error wording. |
| Does sync repair missing chapters? | `ops/chapter.rs::current_project` requires a local chapter receipt before using PM data. `ProjectContent` contains KRs/targets/Flow, not an authoritative current-chapter pointer. | Issue reads must not require that receipt. Current-Project creation needs separate resolution; never start a new chapter to fix discovery. |
| Are there existing location sources? | `known_installations` reads selected/retained artifact/store pairs; `WorkCatalog::load_at` reads minimal identities without migration. It currently requires parents during catalog construction. | Reuse location sources and decouple Task identity reads from complete ancestry. No new registry. |
| Why can an older installed worker remain selected? | `resolve_current_home_lf_binary` first asks `selection_for_current_executable`; retained selections are matched by requested store and executable digest. `start_work_session` then captures that binary. | Resolve current machine selection for next-step policy; retain current-process pin only inside its Run. |
| Can the selected binary simply change while carrying the old claim? | Store selection and executable authority are coupled in startup; `task_destination::check_task` compares local/installed Task IDs and rejects disagreement. | Separate runtime selection from execution-store placement before changing workers. Delete the identity-equality refusal after routing uses real planning identity and exact execution ownership. |
| Is local mode already complete? | TaskPlan requires `LinearIssueId`; create/prepare selects PM ownership and a chapter; Task is documented as one Linear Task. | Local mode requires model and operation changes, not a network fallback branch. |
| Can a short wait safely release a launched claim? | `launch_task_process` releases after `wait_until_running` error; its deadline is `CHILD_STARTUP_GRACE = 10s`. | Test an actual delayed child and return starting without withdrawing authority. |
| Can fixtures be isolated with LF_HOME alone? | TESTING.md and machine_install use the OS account's fixed installation root. | Use the disposable-account/container installation harness; no branch executable against installed host data. |
| Has LOO-298 coordination completed? | Corrected request through official `lf --as task:LOO-298 -b : ...` failed before launch: `Task "LOO-298" is not registered`. | Record the blocker; no auth repair, copied store or edits in its branch. Coordinate again before touching shared execution migrations. |

Linear's official documentation confirms identifier lookup, partial GraphQL
success with errors, and default exclusion of archived objects from paginated
results. These support direct lookup and explicit uncertainty; they do not prove
configured credentials or the specific live issue's payload. See
[GraphQL API](https://linear.app/developers/graphql). Bounded reads should retain
rate-limit feedback and use existing webhook/caching paths for repeated lists,
not issue-by-issue polling; see
[rate limiting](https://linear.app/developers/rate-limiting).

## Alternatives considered

| Approach | Tradeoff | Why not |
|---|---|---|
| Merge all Homes into one machine Task registry | Makes identity lookup local but requires conflict resolution, credential merging and execution takeover across divergent live stores. | Recreates a planning authority beside Linear and expands the incident's risk. |
| Keep current tables authoritative and sync every Wave before lookup | Smaller patch; can hide an empty cache when providers are healthy. | Still confuses planning with execution, cannot represent partial ownership, and makes local-only operation or provider failure a mode accident. |
| Resolve planning directly and join optional execution/Git evidence | Changes DTOs and launch/store boundaries; preserves each source's ownership. | Selected. Reuses provider lookup, installation receipts and execution tables while removing hard prerequisites. |

Wild success: Jack stops choosing a data directory merely to discover a Task;
private work remains private, and a newly installed release is used at the next
step without restarting the Flow. Wild failure: an outage looks like deletion,
or a runtime upgrade silently switches databases and forks a Task. The absence
model, execution-location display and release-between-boundaries proof directly
target those failures.

## Key decisions

1. Planning authority is per Wave, not per command, account or network outcome.
2. Execution attaches by stable identity and outlives planning membership.
3. The joined view represents partial knowledge; a read does not allocate or
   complete execution.
4. Discovery is transient over existing locations and explicit addresses.
5. Official executable selection and execution-store selection are independent.
   Explicit pinning is recorded intent, never inferred from an inherited wrapper.
6. Preserve authored work, stopped boundaries and failed attempts. No implicit
   merge of diverged histories or account/usage catalogs.
7. Wave-existence policy remains an explicit product question; no hidden choice
   is smuggled in through the Task resolver.

## Scope

- In scope: Task authority modes; direct issue resolution; optional execution
  joins; Git/PR discovery; location/reach output; connected planning replacement;
  local Task lifecycle; official runtime default and explicit pins; slow startup
  admission; shared DTO/read/action integration and disposable public-CLI proof.
- Out of scope: merging live stores; sharing credentials/routes/usage between
  Homes; remote fleet discovery; provider-account refresh repair; rewriting
  LOO-298's execution model; Wave deletion/existence migration; automatic chapter
  rotation; promotion of this branch; editing or controlling LOO-298's branch.
- The filing failure must become truthful and recoverable: Task reads do not
  depend on a chapter; creation can adopt a uniquely identified existing provider
  Project without inventing a rotation receipt. When several current Projects
  are plausible, expose the candidates and allow an explicit Project selection
  for that operation. Do not select “most recent” or label any In Progress
  Project the current chapter without evidence. LOO-298's repo-wide chapter
  proposal remains its owner's scope.

## Integration and deletion path

| Owner today | Change and consumers |
|---|---|
| `planning.rs`, `work/task/mod.rs`, PM snapshot and Task store writers | Separate authoritative local planning/reference from connected cached facts and retained execution. Forward migration preserves IDs and history. Narrow planning refresh writes cannot overwrite execution. |
| `ops/pm.rs`, `ops/task_pm.rs`, `pm/linear.rs` | One direct planning resolver; remove snapshot-membership prerequisite and all-or-nothing ownership decoding for reads. Keep mutation-specific provider semantics. |
| `ops/task.rs`, `task_execution.rs`, `task_actions.rs` | Join optional planning/execution; local lifecycle; remove status-triggered completion; adapt run/checkout/edit/save/delete/complete to their owning authority. |
| `machine_install.rs`, `store/mod.rs`, `store/branch_data.rs`, `engine/process.rs`, `ops/task_destination.rs`, `ops/run.rs` | Reuse installation selection/location evidence; separate official runtime from explicit execution store; replace branch/installed Task-ID comparison with resolution; retain source isolation. |
| `controller/task`, `durable`, execution store claims | Preserve exact invocation/claim settlement and starting state; agree schema edits with LOO-298. No parallel claim ledger. |
| CLI Task renderer and direct `--as task:` resolution; Wave status/roadmap; Run/Session/usage attribution | Share planning identity and the partial joined result. Historical attribution remains queryable after planning removal. Generic Session lookup must not remain cache-first. |
| Swift Task models, CLI service decoding, Task panes/actions, DTO fixtures | Represent absent execution, planning uncertainty, multiple locations and runtime policy. No empty-string worktrees or implicit idle defaults; one shared wire contract. |
| `docs/lf.md`, planning/Homes architecture docs, TESTING.md | Replace documentation that treats Task as necessarily Linear-backed or a status read as a private-copy truth. Document explicit pins, location reach commands and bounded discovery. |

Before execution migration work, obtain LOO-298's current contract through the
authorized ordinary Work Run. The attempted Run is blocked by Task discovery,
so this draft uses the supplied Wave memory only as prior design evidence.
Preserve its proposed Run/Session/invocation IDs and ownership boundaries; do not
claim its schema is integrated. Rebase through `lf` when that work is available,
then attach the planning reference and runtime policy to the surviving Task
owner. No temporary mirrored execution schema may ship. Planning lookup and
read-result work can proceed locally without editing that branch.

## Done when

Extend `scripts/test_task_installation.py` and the existing real-CLI fixture
support. Use a disposable OS account/container, two data stores with distinct
writes, a Git repository/local bare remote, simulated Linear/GitHub endpoints,
and distinguishable installed binaries. No host installation, credentials or
data directories are mounted. Simulation of providers must be labeled.

Run the public command path, asserting observable JSON/text and retained data:

1. A contains the only TaskOps record; B starts without PM snapshots or a chapter.
   From A and B, status by identifier/UUID finds the same provider facts and real
   branch/PR evidence, with A's exact location. The printed reach command works.
   Repeat from a source-private invocation; discovery does not mutate foreign
   DB/WAL content or create a second TaskOps row.
2. A planning-only issue and an issue with no Project still exist in status.
   A moved issue remains visible. Permission denial, timeout, partial GraphQL
   errors, archive omission and confirmed deletion produce distinct outcomes.
3. Change Linear title/status/order/Project against contradictory cached facts.
   Reads agree with Linear; cancellation/deletion preserves unpushed files,
   commits, active review evidence and failed Run history. Lost-response retries
   preserve pending input and do not duplicate creation or overwrite newer edits.
4. Create/edit/run/complete a local Task with Linear absent. Discover its owning
   store from the second directory; same-name local Tasks do not collapse. A
   connection failure never turns a connected Wave into this local path.
5. Run two worker boundaries. Between them select official artifact R2 instead
   of R1 using the normal disposable installation operation. The second worker's
   recorded executable digest is R2, while invocation ID, cursor, execution
   directory and review feedback continue unchanged. Poison PATH and inherited
   LF_BIN/LF_CONTROL_BIN with R1 so the original defect would fail the proof.
6. Explicitly pin R1, switch official selection, and prove both worker execution
   and status retain that pin. Clear it with `--official-lf`; the next boundary
   uses R2. Missing/modified pin and incompatible store preserve pending work.
7. Hold actual child startup beyond 10 seconds. The caller reports starting;
   the late worker publishes under the same claim. A concurrent retry launches
   no second worker. Separate proven-death/replacement evidence rejects a stale
   late child. A printed argv or version-only helper is insufficient.
8. Two divergent execution copies, one unreadable store, and filesystem aliases
   produce honest locations and uncertainty without migration or timestamp-based
   takeover. Cross-Home and same-Home-ID/different-directory cases stay distinct.
9. Rust/Swift fixture decoding and Task panes/actions agree on planning-only,
   execution-only, unavailable, canceled-with-work and pinned states. Exercise
   generic Session Task selection through the same resolver; CLI status alone
   cannot satisfy the chapter's Session experience.

Primary end-to-end command: `uv run python scripts/test_task_installation.py`
after its extension. Focused proofs belong beside provider resolution, launch
selection and Task startup. Run affected integration suites/DTO checks once at
gate, plus `cargo fmt` and `cargo clippy --all-targets -- -D warnings` for Rust
changes. Add forward-migration preservation tests against populated prior data.
No behavioral checks have run for this design-only kickoff.

## Forbidden outcomes

- Reporting “no Task exists” because this store lacks execution or a PM snapshot.
- Quietly creating an execution row during status to satisfy today's Task DTO.
- Treating canceled, deleted, inaccessible and provider-unavailable as one state.
- Erasing unpushed work, Run/Session history, exact claims or pending reviews when
  replacing connected planning facts.
- Reconstructing ownership from branch naming, choosing a store by timestamp, or
  merging divergent claims because their issue UUIDs match.
- Defaulting to old LF_BIN/PATH bytes, or switching execution databases merely
  because the official executable changed.
- Testing only executable selection while the child fails to consume its claim.
- Using copied provider accounts or a global Task registry to repair discovery.
- Writing a branch binary into the installed host Home, or calling fixture
  provider traffic a live configured proof.
- Shipping a TaskSpace/TaskOps wrapper over two competing planning writers or a
  second execution schema alongside LOO-298.

## Internal slices

1. **Planning lookup and partial Task result.** Refactor direct provider lookup,
   connection-mode resolution and identity joins; update CLI/DTO/Swift readers
   together. Remove status completion and cache-gated existence. This slice
   proves an issue without TaskOps is observable without allocating execution.
2. **Planning writers and local operation.** Migrate tagged identities and local
   planning, preserve execution through mismatches, adapt local lifecycle and
   connected write-back. Integrate with the surviving LOO-298 Task owner before
   changing shared execution schema.
3. **Execution discovery and continuation routing.** Reuse installation receipts,
   minimal reads, explicit directories and existing routes; prove A/B lookup,
   reachable history, unknown schemas and divergent copies.
4. **Worker runtime and admission.** Persist explicit policy, select official
   bytes per boundary while preserving owning data, fix late startup and expose
   policy/provenance through status. Prove real child claim consumption.
5. **End-to-end reconciliation.** Exercise the full matrix, remove obsolete
   paths/documentation and run the affected gate. This is one coherent delivery;
   intermediate slices do not meet LOO-334's Done when.

## This slice

Kickoff only: establish this design and evidence, preserve product questions and
the blocked LOO-298 coordination. The first implementation cut is slice 1:
`lf task status` returns a provider Task with no local TaskOps or chapter and
does not create execution or change disposition. Its focused proof includes a
missing Project and a provider error so the partial-result contract is exercised.

## Slice ledger

- 2026-09-29: clean worktree at kickoff. Read repository guide, Infrastructure
  goal/memory and source paths above. No earlier scratch design existed.
- Official entrypoint `/Users/jack/.local/bin/lf task status LOO-298 --json`
  returned `Error: no Task exists for "LOO-298"`. No branch executable was used.
- Ambient `lf --help` exposed the obsolete run/ops surface. The official absolute
  entrypoint exposed current Task/Session commands. This is direct evidence that
  PATH is not a reliable official-runtime selector in this session.
- Coordination first used an invalid untyped `--as LOO-298`; corrected to
  `--as task:LOO-298`, which failed as unregistered before launch. No owner reply
  was received and no LOO-298 branch was edited.
- Source inspection and official Linear documentation support the de-risking
  findings. No live Linear/GitHub request, store migration, implementation,
  publication, installation promotion or acceptance demonstration was performed.
- Design review caught three false shortcuts and removed them: treating Home ID
  as data-copy identity, assuming Project membership is necessary for existence,
  and changing executable without preserving the claim's execution directory.
  Current-chapter reconstruction and arbitrary-directory discovery remain bounded
  explicitly rather than hidden behind a sync instruction.
- A second review exposed the Linear-only Project reference beneath local Task
  creation. Local-mode scope now explicitly includes a genuine local chapter
  plan through the existing chapter operation; a fake Linear snapshot would not
  satisfy the private-work requirement.
