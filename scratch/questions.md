# LOO-334 decisions and open choices

Interactive review, 2026-09-29.
[Current design](resolve-tasks-from-linear-and.md).
[Review feedback](repository-planning-connection-review.md).

Jack Heart approved this design review on 2026-09-29 and requested completion
of the review Session so the saved Flow can continue. The questions below remain
explicit implementation choices, not an unapproved review or permission to invent
answers. They do not block the first planning-lookup slice. The following
loop-decide owns navigation; this review supplies approval and feedback only.

## Accepted direction

- Jack requires context-sensitive Wave imports: the owning worktree's files
  define its available Waves immediately, including uncommitted additions and
  deletions. Without a clear owning worktree, use the main baseline. Jack subsequently
  proposed remote main instead of local main and excluded dirty main definitions. The local store retains
  independent contextual answers; do not union a branch with main or let one
  worktree replace another's view. [Contract](worktree-wave-definitions.md).

- Jack steered Wave existence toward repository-created definitions published or
  linked to Linear. Follow that direction; do not automatically adopt all workspace
  Initiatives. Git shares the Wave set and bindings; Linear owns connected planning.
  Explicit import/link can adopt existing company Initiatives. Clones reuse UUIDs.

- Jack selected one Linear Initiative per Wave, including subwaves. Name them
  `A`, `A/B`, `A/B/C` consistently on every account. Also set native parent
  relationships when supported; missing capability must not block subwaves.
  Provider UUIDs retain identity through renames. “Parent status” is interpreted
  as the parent relationship, not a lifecycle-status change.

- Jack requires outgoing chapter Projects to be completed after unfinished work
  transfers, preserving historical results and existing archival behavior.
  Ordinary planning reads cover current Projects; historical Projects do not
  accumulate in the active plan. Completion is not proof that every KR passed.

- Jack selected the sharing boundary: local/open-source Loopflow shares Wave
  goals, memory, Flows and Skills via Git; Linear adds shared plans; shared execution is a paid
  layer. Local plans remain private. Do not implement Git-backed shared Task
  plans or cross-user execution coordination in this Task.

- Jack Heart placed the connection at repository level. All Waves share it.
- Normalize planning entities in existing SQLite, replacing competing Wave
  snapshot readers/writers. Share identity and updates across detail, lists,
  mutations and webhooks. Keep acquisition/freshness policy behind one reader.
  No new library or sync service is selected.
- When Linear is selected, it defines what planning exists. Task operations
  always use a local store, synchronized to Linear when available. Without
  Linear, that same local interface owns planning.
- Refuse work on an explicit Task absent from the applicable planning store.
  Sync must resolve a fresh connected store before absence can be concluded.
- Execution that does not match the plan is invalid. Do not reconcile it or keep
  progressing its Task Flow in this change.
- Ordinary Flows must remain runnable in a worktree regardless of Task status.
- Official installed lf is the requested default at each worker boundary;
  deliberate visible pinning remains available.

## Still open

- **Project relationship repair authority:** public SDK schema inspection at
  `b37823be308a42f837277671f3ded66d33d92e6c` found Initiative join revisions, but no
  established deleted-join recovery contract or Project–Team relationship revision.
  Timestamped Project history has untyped entries with no documented complete
  replay contract. Webhook documentation does not close these gaps. The
  [working design](resolve-tasks-from-linear-and.md#relationship-acquisition-contract-gap--2026-09-29)
  records exact evidence and limits. Request a bound decision: retain strict
  revision/removal proof and unresolved ownership pending provider guarantees,
  or explicitly permit a newly acquired complete relationship set to repair it,
  accepting the documented consistency limits. Request time, Project `updatedAt`,
  list omission and replay remain insufficient under the currently accepted rule.
  Dependent implementation is stopped; this is not permission to relax admission.
  External `archivedAt` acquisition is separately feasible but not implemented;
  existing chapter archive acknowledgements remain intact.
  The official `lf ask` opened bound Session
  `ask_0e28c0a520064375960a1806c5007e3f` for this exact choice. Its waiting command
  has not returned a decision. Session readiness is not completion; no repair
  policy may be inferred while it remains pending.

- **Remote-main details and outward sync:** Jack accepted remote main as the shared definition baseline,
  then clarified that this is not a blanket substitution for local main everywhere. The working assumption is last-fetched
  configured remote main, excluding dirty files/unpublished local main commits.
  Specify no-remote repositories, explicitly supplied main checkouts and refresh
  cadence. Main should stay clean and dirty changes are normally discarded through
  Loopflow operations; reading definitions must not itself reset files. When edits
  to mapped definitions should write Linear remains open.

- **Unavailable Linear:** the design retains dated cached planning for inspection
  and refuses uncached/unresolved selectors. Decide whether a cached Task can
  start/resume managed execution while Linear is unreachable, or must wait for
  fresh validation. Neither policy should manufacture deletion or local authority.
- **Connection transition:** repository-originated Waves supersede automatic
  Initiative discovery. Exact preview/apply/import controls, existing company
  Project selection, disconnect and Wave deletion remain unresolved. Connecting
  must explicitly map private plans; login/pull/read never publishes them.

- **Existing provider hierarchy:** `A/B` naming and optional native parent links
  are selected. Adoption of existing names that disagree with native hierarchy,
  or multiple parents, remains unresolved. Do not silently rename company planning
  or choose one parent. Slash-name server acceptance and capability detection
  require implementation proof.

## Command narrative refinement

Jack requested one story ordered as worktree/design → launch-plan → Task →
accounts and monitoring when needed. [The script](idea-to-task-command-story.md)
uses a fictional technical founder/CTO and one invoice-export feature. Automatic
worktree creation by design is the requested target, not verified existing
behavior. The exact context handoff and Task-checkout transition must be resolved
in implementation; current launch-plan does not adopt arbitrary unbound code.
Account/monitoring concepts do not by themselves authorize command renames.
Jack suggested ending with two Waves for future work arising from the feature.
The story proposes `lf wave create <name> --objective "..."`; current source has
no such command. Settle that creation/bootstrap surface separately from merely
showing existing Wave status. Repository connection remains shared. The original
Task stays historical; new Tasks are queued without execution.

## Implementation constraints

Use existing planning/sync/storage; the Apollo analogy selects a simpler local
interface, not a library or service. Keep IDs and existing execution bytes without
building recovery for invalid records. No files/history deletion was requested.
Retain bounded cross-store location and official-runtime proof from the Task.
Do not consolidate provider accounts, routes or quota readings. No branch binary
may touch the installed host Home.

## Coordination blocker

The authorized ordinary request through official
`lf --as task:LOO-298 -b : ...` failed with `Task "LOO-298" is not registered`.
No Run launched or owner reply established the latest execution schema. Retry
coordination before shared migrations; do not edit that branch, copy stores,
repair auth or treat prior Wave memory as current agreement.

## Implementation evidence and limits — 2026-09-29

At `01f7814ef`, issue revisions, UUID-scoped webhook invalidation/removal receipts,
Project fact revisions and retained planning inspection are implemented. Missing
or empty webhook revisions invalidate through the existing event path; ordered
steering waits for complete evidence. The grouped OAuth capture failure is
resolved by an isolated process/subscriber. The
[working design](resolve-tasks-from-linear-and.md#implementation-checkpoint--2026-09-29)
owns proof receipts and the full remaining acceptance matrix.

The last-fetched `origin/main` remains `a3820bf7e`; no new execution contract was
available there. This reconciliation made no coordination retry, fetch, provider
mutation or installation promotion. The recorded LOO-298 unregistered response
remains a blocker only for dependent execution migrations. Reuse local planning
proofs without claiming live provider or full command-story acceptance. Resolve
outage and transition policy before dependent work; the saved Flow owns navigation.

The current implementation now retains invalid/removed facts publicly, returns
unavailable status without execution, and preserves last-good facts/age after
hard-stale or forced inspection failure. `planning_state` and Project `revision`
are mirrored in Rust/Swift fixtures. All new status fixtures have null execution;
execution/action parity, local lifecycle, contextual Wave definitions and the
full story remain open. Keep fixture shape, public command behavior and
managed-admission policy separate.

Realign reproduced and repaired a null lookup response hiding a removal receipt
that arrived during acquisition. Confirmed removal remains explicit in inspection;
the dated facts remain visible and managed reads still refuse. This follows the
accepted evidence contract without selecting new product policy.

Safe implementation choice: a later list cannot establish Project removal, and
Project `updatedAt` alone cannot order separate Initiative/Team relationships.
Known Project facts use provider revisions; contradictory relationship sets stay
explicitly unresolved and block managed readers. Ordered association acquisition
and repair are still needed. Replaying older/same facts cannot clear the recorded
uncertainty. No outage-admission or connection-transition policy is inferred.

Confirmed chapter archive acknowledgements supply positive retirement evidence,
so rollover does not get stuck on its own predecessor's list omission. The forward
migration preserves completed chapter archive receipts. This is local planning
evidence, not an execution-schema change or the still-required provider Project
completion behavior. External archive acquisition/restoration remain unimplemented.

The working design owns the focused proof receipts and remaining acceptance work.
No live-provider, full command-story, coordination reply, publication, promotion or
Flow-navigation claim follows from these local changes.

Compression retained these boundaries. A reproduced local path-alias failure is
repaired through the existing repository canonicalization operation: Waves and
normalized facts move atomically to the same canonical scope. This is not a
connection transition or cross-repository planning migration. Conflicting stored
observations remain a transaction failure; no merge policy was selected.
Populated planning during explicit cross-repository Wave relocation remains
unproven. The full acceptance matrix and LOO-298 execution-contract blocker remain.
