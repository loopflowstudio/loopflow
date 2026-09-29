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

The shared normalized lookup now has focused proofs for cache miss, missing
Project, unknown selector and provider failure. Next, finish provider ordering,
webhook convergence and public freshness/error representation. Resolve the outage
policy before dependent managed-admission work or any offline execution claim.
This review records no Advance/Iterate verdict; the saved Flow owns navigation.

## Implementation observations — 2026-09-29

The official coordination retry for LOO-298 again returned
`Task "LOO-298" is not registered`; no Run or owner reply exists. Keep shared
execution migrations blocked. Planning-only storage work proceeds independently.

Reversible storage choices: normalize provider facts by canonical repository,
provider and UUID; keep Wave Project membership separately; never erase Task
facts on list omission. An empty exact lookup invalidates cached admission but
is not a provider deletion receipt. Full detail repairs that invalidation.
Complete-detail responses require nullable relationship fields to be present;
malformed/partial payloads cannot clear known facts. Detail does not observe rank,
so retain existing relative order when the Project is unchanged.

These choices do not settle offline managed admission, Wave migration controls,
or provider revision ordering. The complete design remains unfinished; the working
design contains the remaining implementation list and proof limits.

Realign source inspection at `dfd7563d4` found two consumer limits: automatic
refresh failure returns retained planning with its original timestamp but no
status `planning_error`; the new status envelope has no Swift/DTO fixture parity.
Invalidated records are retained but hidden by both planning readers. Keep these
gaps separate from managed execution policy. No new behavioral checks or
coordination request ran during reconciliation; the prior grouped OAuth tracing
failure and LOO-298 coordination blocker remain unresolved.
