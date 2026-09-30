# Wave existence and connecting an existing local plan

2026-09-29. Proposal in response to Jack Heart's review question; not yet a fully
confirmed migration contract. [Working design](resolve-tasks-from-linear-and.md),
[command story](idea-to-task-command-story.md), [open choices](questions.md).

Jack initially considered automatically treating company Initiatives as Waves.
Jack later leaned heavily toward the opposite direction: create Waves in Loopflow,
then publish/link them to Linear. The current design follows that steer; automatic
workspace-wide Wave adoption is superseded.

Jack subsequently settled the sharing boundary: open-source local mode shares
Wave goals, memory, Flows and Skills through Git; Linear adds shared planning; shared execution
belongs to a paid layer. The suggested Git-backed shared Task plan is rejected.
Migration details below remain proposals within that accepted boundary.

## Selected mapping and names

Jack selected one Initiative per Wave, including subwaves, with the same path
names on every account: `A`, `A/B`, `A/B/C`. Set the native Initiative parent
relationship as well when supported; otherwise retain the names and Loopflow
hierarchy with flat Initiatives in Linear. Each has its own stable provider UUID;
renaming does not create a new identity. “Parent status” means the parent
relationship, not lifecycle status. Migration details remain proposals.

## Wave definitions originate in the repository

Jack clarified that this is a context-sensitive import into the local store.
Use the owning worktree's current definition files, including dirty additions
and deletions; without that context use the proposed remote-main baseline
(normally last-fetched `origin/main`), excluding dirty or unpublished local main. The [resolver contract](worktree-wave-definitions.md)
preserves independent main/branch views and forbids falling back to main for an
individual Wave deleted on a branch. Importing files is not a provider mutation.

| Responsibility | Owner |
|---|---|
| Which Waves exist, names and hierarchy, goals/memory/Flows/Skills | Repository-authored Wave definitions, shared through Git |
| Private chapter/Task planning without Linear | Local store |
| Shared chapter/Task planning with Linear | Linear, synchronized into the same local store interface |
| Connection and Initiative references | Repository connection and stable per-Wave provider bindings |

A repository selects the Linear workspace. Each Wave/subwave has its own mapped
Initiative. Store workspace/Initiative UUIDs as bindings, preserving Wave identity
through connection and renames. A clone reuses mappings; it does not publish the
same Waves again. Login, reads and pulls must not create provider objects.

Wave creation in a connected repo creates the Initiative as part of that explicit
operation, or links an explicitly selected existing Initiative. Retry reuses
confirmed provider identity. Failure leaves setup visibly incomplete; it does
not claim a shared plan exists or quietly enable private connected planning.
A missing mapping is not permission for a read to create one. Missing/inaccessible
provider records are discrepancies to resolve, not deletion of authored Waves.

Sync reads planning for mapped Waves. Enumerating all Initiatives can help choose
an existing link/import candidate, but cannot register unrelated Waves. Existing
company planning can be adopted explicitly without renaming it, duplicating it
or rotating its Projects automatically. Current-Project ambiguity still needs
resolution for operations requiring that plan.

## Two onboarding paths

**Engineer joining an existing repository:** clone/pull Wave definitions and
provider bindings, authenticate, and sync their shared planning. Unrelated company
Initiatives do not become Waves. A repository adopting preexisting company
Initiatives selects them explicitly and commits the resulting Wave definitions
and bindings. This publishes no teammate execution or private local backlog.

**Local user connecting later:** connect performs an explicit planning migration.
Authentication alone never publishes the user's private planning. Show the chosen
workspace and a concrete preview before any creation or changes in Linear.
Suggested command surface is `lf repo connect linear`; its exact flags and apply
interaction are still proposals, not existing parser capabilities.

For every local Wave the preview offers a concrete disposition:

- Link to an existing Initiative by stable identity. Show the existing provider
  facts; do not silently overwrite them with local titles/objectives.
- Publish as a new Initiative, with its selected Project/Tasks mapped explicitly.
- Leave local planning as inactive history outside the connected current plan.
  Keep source artifacts; do not silently publish private work or run an independent
  local planning mode alongside Linear in the connected repository.

The preview includes Project/Task mappings as well as Wave mappings. Similar names
can suggest candidates but cannot authorize a merge. Show which new provider
objects will be created and which existing ones reused. Completed local execution
history does not automatically become a set of new Linear issues.

Apply only the selected mappings. Retain local identities/history and store the
provider references; mapping planning is not merging execution stores. A retry
reuses already-created provider identities and must not duplicate Initiatives,
Projects or Tasks after a lost response. Read back remote results before reporting
successful connection. A partial migration reports its progress and remaining
work; do not declare a fully synchronized connection or discard the local source.
Use existing operation/migration facilities rather than a general sync queue.

After successful connection, Linear defines current chapter/Task planning for
the repository-defined Waves. New connected Wave creation creates its Initiative;
deleting a local cache entry does not delete it remotely.
A Task execution that no longer matches the resulting plan is invalid under
Jack's accepted rule. Migration does not launch workers, rewrite captured Flows
or implement automatic execution recovery. Ordinary worktree Flows remain usable.

Disconnect is not the inverse of deleting Linear objects. Its exact local-plan
selection remains open and must not be inferred from this connection proposal.

## How this changes Dave's story

Dave can build the invoice export and establish local billing/self-service Waves
before using Linear. A colleague can pull their goals, memory, Flows and Skills but does not
receive Dave's private backlog or progress. When his company adopts Linear,
connection previews those
two Waves: perhaps billing links to an existing Finance Initiative while
self-service creates a new Initiative. Dave reviews the mapping once. His agent
then uses the same Wave and Task commands as before, backed by Linear planning.

An engineer joining afterward pulls those Wave definitions and bindings, then
syncs their existing Linear planning without recreating Initiatives. This is an optional
closing scene to the one-person story, not a second introductory command tour.

## Evidence, consequences and proof

Linear exposes a workspace-level Initiative view; visibility depends on permissions
and visible Initiatives may contain inaccessible Projects.
[Linear Initiatives](https://linear.app/docs/initiatives).
The existing paginated Initiative query can support explicit link/import selection.
It is not the source of automatic Wave registration. Existing Wave configuration
already carries provider bindings; extend that ownership rather than introducing
a separate machine-wide Wave registry.

Prove: a fresh clone reuses mapped Initiatives; an unrelated new remote Initiative
does not create a Wave; explicit connected creation and retries create one
Initiative; connect previews do not write; selected imports preserve existing
provider identities; incomplete publication stays visible; access failures remain
unknown; and neither Git pull nor planning sync publishes private Tasks or acquires
execution authority. No configured provider mutation was performed in this review.

## Subwaves and provider hierarchy — source finding

Jack asked whether Linear has subobjectives and whether an objective could be
named `A/B`. Linear calls the mapped object an Initiative. Its current docs
describe native sub-initiatives on the Enterprise plan, up to five levels, and
permit multiple parents. [Sub-initiatives](https://linear.app/docs/sub-initiatives).

Loopflow's `work/wave/mod.rs::Wave` has one optional `parent_wave_id`; its objective
is not a separate hierarchical record. Current `pm/linear.rs` creates/lists
Initiatives without parent relationships and explicitly excludes sub-initiative
Projects when listing directly owned Projects. Provider hierarchy is therefore
not currently synchronized simply because both systems support nesting.

`LinearClient::create_wave` passes names unchanged, so it does not reject `A/B`.
Server acceptance has not been live-tested. Jack selected those path names as
Loopflow's hierarchy convention plus optional native parent links. Prove create,
lookup, hierarchy and retry with and without the feature. Provider depth limits
must not restrict Loopflow's path hierarchy. Do not hide permission/network
failures as missing capability or claim a failed native parent write succeeded.
Keep direct Project ownership separate from descendant aggregation. Adoption of
existing names that disagree with native parents, or multiple native parents,
remains unresolved; do not silently rename or flatten company planning.

## Alternative: one Initiative for the repository

Jack considered one Initiative for all Loopflow planning, then selected an
Initiative per Wave/subwave after discussing scale. This comparison preserves
the rejected alternative; it is not an open implementation choice.

| Concern | Initiative per Wave | One repository Initiative |
|---|---|---|
| Wave identity/existence | Repository defines the Wave; its Initiative supplies a stable external planning reference. | Repository definitions establish Waves; Linear owns their shared plans. |
| Reading in Linear | Each Wave has its own Initiative overview and grouping of chapter Projects. | One Initiative contains the repository's chapter Projects; each Project needs an explicit Wave reference. |
| Long-lived history | An Initiative naturally groups one Wave's successive chapters. | Wave history needs the Project-to-Wave reference; the Initiative groups the whole repository. |
| Working with an existing company | Explicitly link/import existing Initiatives without duplicating their grouping. | Connect the repository to a selected/new Initiative and explicitly associate its Projects with repo-defined Waves. |
| Wave goals/subwaves | Mapping must reconcile provider objects with authored Wave content and hierarchy. | Goals, memory and subwave structure stay with repository definitions, independently of Linear hierarchy support. |
| Task sync/execution | Synced issue identity and local execution still required. | The same Task sync and local execution mechanisms are required. |

The practical benefit of per-Wave Initiatives is first-class Wave visibility and
durable grouping inside Linear. It does not improve Task execution, synchronize
worker ownership, or eliminate store/runtime routing. Jack selected per-Wave Initiatives while retaining repository-defined Waves;
per-Wave mapping does not require automatic adoption of all company Initiatives.

The unselected repository-level option would require a stable Wave reference
on each chapter Project. No provider objects were created or remapped during
this review. One current Project per Wave and no shared execution remain the
contract under the selected per-Wave Initiative mapping.

## Initiative size limits — verified boundary

Jack clarified that the concern is hundreds of Initiatives/Projects, rather than
field lengths. A further official-doc search on 2026-09-29 found no published
numeric cap on total Initiatives, total Projects, or Projects per Initiative.
This is an unknown capacity guarantee, not evidence of an imminent cap or of
unlimited capacity. [Pricing](https://linear.app/pricing) lists issue/team limits
but no Project/Initiative count quota; Free has 250 issues and paid plans list
unlimited issues.

The documented scaling constraints are API requests and query complexity:
2,500 requests/hour per API-key user, 5,000 per OAuth user/app user, and 10,000
complexity points per query. Some operations have lower endpoint-specific limits.
Linear recommends webhooks, filtered reads and avoiding polling; nested collection
queries multiply complexity. [Rate limits](https://linear.app/developers/rate-limiting).
Lists default to 50 records and expose cursors for further pages.
[Pagination](https://linear.app/developers/pagination).

Design implication: do not choose repository-level Initiatives merely to evade
an unverified count limit. Either mapping still creates one Project per Wave
per chapter: an illustrative 100 Waves over 12 chapters produces 1,200 historical
Projects in total. Jack subsequently clarified that old chapter Projects must be
completed: after rollover that example has 100 current Projects and 1,100
historical Projects. Total accumulated objects should not be presented as the
active working set. One Initiative changes grouping, not that growth. The local-store sync
must paginate completely, refresh changes without one poll per Wave/Project, and
avoid loading all historical issues on every status read. A hypothetical one
request per minute for each of 100 Waves already totals 6,000 requests/hour,
above the documented ordinary OAuth quota before other traffic. No provider
capacity/load test or guarantee of Linear UI performance at this scale exists
in this review; simulated pagination checks cannot supply that evidence.

### Accepted chapter closure

Jack directed completion of old chapter Projects so they stop counting toward
the active plan. Transfer started unfinished Tasks first, settle untouched backlog
under existing cancellation policy, complete the predecessor Project, and retain
history and the existing archival behavior. Completion marks the chapter closed,
not all KRs achieved or unfinished Tasks successful. Current-plan reads select
current Projects; historical reads remain explicit.

At review, `ops/chapter.rs` archived predecessors without setting completed status.
The September 30 implementation now calls `complete_and_archive_project` after
transfer/disposition and the check for newly filed Tasks. The existing retryable
operation owns both effects. Its stateful local proof covers refused completion,
lost completion/archive responses and preservation of Task identity and dated
results. Configured provider completion remains unproven; no live Project was
completed during review or that implementation proof.

Linear distinguishes completion from archival: automatic archiving requires a
closed Project, inactivity and eligible issues. Completion alone therefore does
not establish immediate automatic archival or exemption from any storage quota.
[Archival rules](https://linear.app/docs/delete-archive-issues).

The current adapter's `linear_description` takes the first meaningful paragraph
and truncates to 255 Unicode scalar values before creating an Initiative. This
is confirmed Loopflow behavior, not fresh proof of Linear's server-side limit.
The official Initiative docs reviewed do not specify a numeric title/description
limit or a maximum count of Projects/Initiatives; absence of documentation does
not establish unlimited capacity. Native sub-initiatives are documented as
Enterprise-only with five nesting levels and possible multiple parents.
`A/B` is passed unchanged by the client as a name, but server validation was not
live-tested and punctuation does not establish native parentage.
