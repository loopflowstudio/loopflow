# Repository connection, synced planning and Task validity

Interactive review for LOO-334, 2026-09-29.
[Working design and evidence](resolve-tasks-from-linear-and.md).
[Remaining choices](questions.md).
[Requested sync-store research](planning-store-sync-research.md).

## Jack Heart's feedback

- “I think the connection should live at the repo level”.
- “if we are using Linear as a source of truth it should be the source of truth
  for what planning there is”.
- Refuse explicit Tasks that do not exist in the applicable store. Always use a
  local store, synchronized to Linear when available, to simplify implementation.
  Jack compared this to mobile GraphQL stores such as Apollo.
- Treat execution that disagrees with the plan as invalid for now; workers cannot
  sensibly continue a missing Task. Preserve ordinary Flow execution in a worktree
  regardless of Task status, separately from managed Task Flow execution.

## Agreed design changes

Repository owns connection; Wave Initiative IDs only map provider relationships.
One local planning interface serves connected and unconnected repositories. Linear
sync supplies authoritative planning in connected repositories. Task lookup syncs
before concluding absence in a fresh store; it does not bypass the local model.

Explicit absent Tasks cannot launch. Execution incompatible with current planning
is invalid and cannot progress its managed Flow. Remove automatic reparenting,
mismatch continuation and reconciliation scope. Ordinary worktree Flows remain
available without gaining authority over that invalid invocation. No deletion of
files/history was requested, and retaining bytes does not require recovery UI.

The design's architecture, integration/deletion path, first slice, acceptance
matrix and scope now follow these decisions. Official runtime selection and pins,
cross-Home location and slow-start repair retain their original acceptance work.

## Evidence and remaining review

Source already stores repository provider/Team fields in `engine/config.rs` and
reads them through `ops/pm.rs::read_repository_team`. Task resolution currently
rejects missing cached Wave membership before refresh. Status currently requires
an execution Task and can complete it after a PR observation. These findings
motivate shared synced planning and separating planning from execution allocation.

Still decide whether cached Tasks can execute while Linear is unavailable, and
exact connection transition behavior. Wave existence remains undecided. LOO-298
coordination remains blocked as recorded in the design. No implementation or
behavioral tests were performed. The remaining policies do not prevent the first
planning-store slice. No Flow navigation is selected.

## Research follow-through

Jack requested study of Apollo and similar stores, including Realm. The linked
research compares Apollo iOS, Relay, historical Atlas Device Sync and PowerSync
using primary documentation, and examines the existing snapshot/refresh code.
The research recommends entity normalization in existing SQLite, shared
acquisition policy and explicit freshness. Jack approved proceeding (“lets do it”)
and requested that the design reflect the research. The working design now adopts
these contracts and maps each to integration, deletion and acceptance work.
No library dependency was selected, and offline managed execution remains open.

## Implementation handoff

Build the normalized planning lookup first, through the shared local reader,
without allocating execution. The full design retains local lifecycle, invalid
Task refusal, ordinary worktree Flow independence, cross-Home location, runtime
selection/pins and delayed-start proofs. LOO-298 coordination precedes shared
execution migrations. Session completion returns this feedback to the saved Flow;
this review does not choose Advance or Iterate.

## Command story refinement

Jack requested that command selection tell a story, beginning with worktree
creation or `lf skill design` creating a worktree automatically, then
`lf skill launch-plan`, Tasks, and only later accounts and monitoring. Jack
selected one story for a composite of the described technical founder/CTO personas.
[Dave's invoice-export script](idea-to-task-command-story.md) is now the primary
command walkthrough in the design. Auto-placement is target behavior, not verified
current behavior; existing command spellings are used for accounts and observation.
This refinement adds placement/handoff proof to the full acceptance sequence.

Jack then suggested ending with Wave commands that set up future work around two
aspects of the initial feature. The script now ends with the shipped export
leading to billing accuracy and customer self-service responsibilities, queued
Tasks and Wave status. Cancellation moved to a companion acceptance case.
`wave create --objective` is explicitly proposed, absent from current source;
Wave bootstrap/existence policy is not silently resolved by the story.

## Wave existence and local-to-Linear transition

Jack proposed that engineers joining a company inherit its Linear Initiatives as
Waves regardless of local setup, and raised explicit migration for local users
connecting later. The [proposal](wave-existence-and-linear-migration.md) separates
workspace-owned existence from repository execution placement and previews
link/create/inactive-history mappings before provider writes. The main design
links and summarizes it as a review proposal; no migration was run or approved.

## Sharing boundary — accepted

Jack explicitly selected: shared execution belongs to the paid layer, shared
planning comes from enabling Linear, and open-source Loopflow without Linear
shares goals, memory, Flows and Skills. The design now makes local plans private, rejects the
Git-backed shared-plan alternative, and limits Git import to authored goals, memory, Flows and Skills.
Cross-Home discovery retains its original single-operator continuity scope; it
does not introduce team execution coordination. The story's company epilogue
now introduces Linear as the step from shared context to shared planning.

Jack clarified that the repository-shared layer also includes Flows and Skills.
All design/story references now include those definitions; captured invocations
and progress remain execution state and are not shared through Git.

## Initiative mapping remains open

Jack raised one Initiative for the repository/Loopflow instead of one per Wave
and requested the concrete benefits of each mapping. The comparison records
per-Wave native provider identity/overview/history versus repo-defined Waves with
Project-to-Wave references. Neither changes Task sync or adds shared execution.
The main design now marks the earlier Initiative-is-Wave proposal as conditional.
Size research found the local adapter's 255-character summary truncation, but no
verified remote title/body/count cap. Native sub-initiatives are Enterprise-only,
five levels, and can have multiple parents. No external mutation was performed.
# Chapter closure and active scale

Jack clarified that old chapter Projects should be completed so they stop counting
toward current planning. The [design](resolve-tasks-from-linear-and.md) now requires
completion after Task transfer/disposition, preserving history and existing
archival behavior. The [scale notes](wave-existence-and-linear-migration.md#accepted-chapter-closure)
distinguish 100 current plus 1,100 historical Projects from 1,200 active Projects.
The existing chapter operation archives predecessors but does not explicitly
mark their provider status completed at that boundary. Next implementation work
must integrate closure there and prove retries preserve transferred Tasks and
historical KR results. This does not choose an Initiative mapping or claim a
provider storage-quota exemption. No provider mutation or Flow navigation occurred.

## Selected Wave mapping and portable hierarchy

Jack selected one Initiative per Wave, including subwaves, then chose `A/B`
naming on all accounts. Also set Linear's native parent relationship when the
account supports it; absence of that feature must not block subwaves. This
supersedes the repository-wide Initiative alternative and the earlier undecided
naming choice. Parent relationship does not mean lifecycle status. Keep provider
UUIDs stable through renames and each chapter Project directly owned by its Wave.

The [design](resolve-tasks-from-linear-and.md) and
[mapping notes](wave-existence-and-linear-migration.md#selected-mapping-and-names)
now reflect this. Next proof must cover slash names, hierarchy and retry on both
account capabilities without duplicate Initiatives or descendant Project joins.
Existing conflicting names/multiple parents and connection migration remain open.
No provider mutation, implementation or Flow navigation occurred in this review.

## Repository-created Waves, published to Linear

Jack leaned heavily toward creating Waves in Loopflow and pushing/linking them to
Linear, rather than reading all Initiatives to define available Waves. The current
[design](resolve-tasks-from-linear-and.md) follows that direction. Repository
Wave definitions and bindings establish the Wave set in both modes; Linear owns
connected chapter/Task planning. One Initiative per Wave/subwave and `A/B` names
remain selected. Existing company Initiatives require explicit link/import;
login, sync and pull do not adopt all Initiatives or publish private planning.

The [onboarding notes](wave-existence-and-linear-migration.md) and
[command story](idea-to-task-command-story.md) now have colleagues clone definitions
and reuse bindings. Next proof must distinguish explicit publication from reads,
handle retries without duplicates, and keep unrelated Initiatives out of the Wave
set. Import/migration controls, conflict handling and disconnect remain open. No
provider objects were changed and no Flow navigation was selected.
# Worktree-aware Wave definitions

Jack clarified that the local store imports the repository's Wave definitions
with context: use the owning worktree, including dirty adds/deletes, or main when
no clear worktree owns the request. The [resolver notes](worktree-wave-definitions.md)
and [design](resolve-tasks-from-linear-and.md) now require independent main/branch
answers and immediate local API visibility. A missing Wave on a branch does not
fall back to main, delete provider planning or replace another checkout's view.
Next useful proof is main plus two worktrees through one store, including edits,
reverts and read-order independence. Exact main-source freshness and outward sync
of edited mapped definitions remain open. No implementation or provider mutation
occurred; no review navigation is selected.

## Remote main as the shared baseline

Jack specified that main should stay clean and dirty main edits are normally
discarded, then proposed using remote main throughout instead of local main.
The [resolver design](worktree-wave-definitions.md#remote-main-baseline-and-clean-main)
now proposes the configured remote's last-fetched main ref as the default,
excluding dirty main and unpublished local commits. Explicit feature worktrees
retain their current files. Reads observe definitions and do not clean Git state.
No-remote repositories, explicit main-checkout context and refresh cadence remain
policy details. No cleanup, fetch, reset or provider mutation occurred here.

## Review approved — 2026-09-29

Jack Heart requested “mark review approved and complete / proceed with the flow”.
The current design review is approved. Complete the exact supplied review Session;
the following saved loop-decide step owns navigation. Do not start another Task
Flow or mark LOO-334 complete merely because the review ends.

Approved design: [resolve-tasks-from-linear-and.md](resolve-tasks-from-linear-and.md).
Research: [planning-store-sync-research.md](planning-store-sync-research.md).
Command story: [idea-to-task-command-story.md](idea-to-task-command-story.md).
Wave mapping: [wave-existence-and-linear-migration.md](wave-existence-and-linear-migration.md).
Contextual definitions: [worktree-wave-definitions.md](worktree-wave-definitions.md).
Remaining policy choices: [questions.md](questions.md).

Takeaway: one local planning interface; repository-level Linear connection;
repository-owned Wave definitions imported from the owning worktree, otherwise
remote main as a shared baseline; one Initiative per Wave/subwave with `A/B` names
and optional native parent links; old chapter Projects close after transfer;
invalid managed Tasks refuse progression while ordinary Flows remain usable;
official runtime per worker boundary with explicit pins. Remote main is not a
blanket replacement for local main in all Git operations.

Next useful action: proceed through the saved Flow to the shared planning-lookup
slice and its public-CLI proof. Resolve recorded policy choices before dependent
behavior and coordinate with LOO-298 before shared execution migrations. Preserve
the full end-to-end acceptance matrix. This review ran documentation consistency
checks only, with no implementation, behavioral test or provider mutation.
