# One Wave, one chapter Project

Status: the initial implementation passed its recorded checks. The human has clarified ownership: the objective belongs to the Wave; Tasks, KRs, and metric targets belong to the chapter Project. Reconcile the implementation with this correction before calling the design complete. Live planning changes remain unapplied.

## What to build

Give each Wave exactly one current Project, replaced with fresh content every chapter; automatically move started Tasks forward and abandon unstarted backlog. Use the Wave as the everyday planning and steering surface.

> Projects *are* ephemeral and their tasks and content *do* get restarted every chapter. Each wave gets *exactly 1* project at a time. For most things wave=project and we can ignore the distinction

The human refined the Task reset rule:

> I think open tasks can get auto moved to the next project, but unopened tasks are marked as abandoned/closed

Chapter skills must embed this lifecycle and call a deterministic API; prompts must not implement rotation themselves. The human's subsequent UI requirement supersedes the initial `lf project new-chapter` spelling:

> collapse both the Desktop and the CLI UIs to refer to only waves and let the "projects" be more of an internal detail

Ordinary navigation, commands, and agent steering expose Waves and Tasks. Projects are the internal chapter/provider records; Chapters are history and planning intervals, never another required navigation parent.

Waves persist as mostly human-authored responsibilities, retaining their objective, standards, memory, and conversation. Agents mostly author the current chapter's KRs, metric targets, and Tasks. A Project is the Wave's plan for an interval; it has no independent objective or operator.

The human clarified the ownership boundary:

> I think the objective should be on the wave. tasks, KRs and metric targets on the proejcts.

The Wave UI presents its objective together with the current Project's KRs, metric targets, and Tasks. Internal ownership does not add another navigation tier.

Future direction, explicitly outside this slice:

> the wave<--> project merger clears the way for a wave hierachy. sub waves, split waves, etc. not necessary now, but something ive explroed and abandoned and now think might feel different

Waves can become the hierarchy of durable responsibilities, each retaining one
current chapter Project. Preserve existing Wave ancestry and history during
this consolidation. Do not add hierarchy features now or reintroduce Project
trees as a substitute; subwaves and splitting deserve their own design later.

## Placement

Unresolved: no exact owning Wave or existing implementation Project was named. Do not infer placement from the workspace name or alter the live portfolio during design.

## The demo

Open a Wave in Mac or run `lf status <wave>`: see its enduring objective and the current chapter's KRs, metric targets, and Tasks directly, without selecting a Project. Create a Task using only the Wave; its parent resolves automatically.

Start the next chapter with one Task in progress and one untouched backlog Task. The Wave, chat, and memory remain. The Project has a new identity and newly authored content. The started Task moves automatically, retaining its issue, worktree, PR, and Flow position. The untouched Task is closed as abandoned. Completed Tasks remain with the historical Project. Agents author fresh Tasks for the new plan.

## Desktop and CLI contract

Source map: `scratch/research-wave-ui-collapse-5db824ef.md`. The primary Podium and the earlier repository/Portfolio Wave workspace both remain reachable and must change together.

- **Navigation:** Wave → Task. Remove Project drawers, breadcrumbs, selection, inspector, count, and display-only fader. Do not substitute a Chapter drawer. Wave selection and selected Task identity survive rotation.
- **Wave detail:** the Wave objective, current chapter KRs and metric targets, full Task list (including backlog), metric readings, and conversation. Show one objective, owned by the Wave. The existing Now list is filtered and cannot replace the full Task list. Current empty, completed, loading, stale, and unavailable states remain distinct.
- **Secondary surfaces:** Sessions group by Wave. Task Activity uses stable Wave/Task identity across chapters, without a current-Project filter. Metrics group under the Wave. Current Project chat links resolve to the Wave; historical references open that Wave's chapter history. Actual Run subjects remain intact in diagnostics.
- **Shared reads:** replace current `projects[]` trees in Rust status/roadmap/PM-plan projections and Swift mirrors with one chapter summary and direct Tasks. Preserve unavailable/stranded Task evidence and recovery actions. Cached plan uses the same chapter resolution. Internal external IDs may appear in source details; users never select them. Next-owner output names the actual Wave, Task, User, CI, or external actor; remove Project as an independent current actor.
- **Commands:** keep `lf start`, `lf status`, `lf roadmap`, and `lf chat`. New Tasks use `lf task start --wave <wave> <title>` or `lf pm task create --wave <wave> --title <title>`; existing Task commands resolve by issue. Replace ordinary Project prepare/start/run/status/abandon/promote and `--project`/`--as project:` planning selections with Wave operations or explicit chapter history. Keep low-level historical Work inspection as diagnostics, not a parallel execution workflow. Update skills, help, error recovery, and agent context accordingly.

`lf wave` currently takes a positional listener name. Make it an explicit suite with `lf wave serve <wave>` and the chapter operation below; migrate internal launchers and tests in the same change. Do not add ambiguous positional parsing or retain obsolete management aliases.

## Data and authority

- `Wave`: enduring identity, objective, memory, chat, cadence, placement, and reusable metric instruments.
- `ChapterId`: existing chapter archive identifier; no new Work kind.
- `Project`: existing local and Linear identities plus Wave/chapter membership; KRs, metric targets, and Task plan for that interval. Any plan prose explains how this chapter advances the Wave objective; it must not establish another objective.
- `ChapterMetricTarget`: a reference to a metric in the owning Wave plus the chapter's target. Project content owns this value; the metric instrument does not author or default a chapter target. Closed chapter evidence preserves the target and dated evaluation that applied then.
- `WaveChapter`: one authoritative current binding from Wave to chapter and Project. Persist through the Wave's placed Work authority; readers consume that binding, never guess from names or newest creation time. The repository chapter archive records the accepted plan and application receipts; it does not become a competing runtime pointer.
- `Task`: existing delivery identity and mutable Project parent. Moving chapters preserves identity and execution state; historical receipts retain their original attribution.

Linear remains authoritative for authored Project/Task content. Exactly one binding is current, including when all KRs hold or no Tasks remain. Historical Projects are unlimited; a prepared successor is not yet current. Wave initialization provisions its initial Project as one operation, not a separate user setup chore.

## Deterministic API and CLI

Target commands:

```bash
lf wave new-chapter --wave product --chapter <id> --plan plan.json --dry-run --json
lf wave new-chapter --wave product --chapter <id> --plan plan.json --json
lf status product --chapter <id> --json
```

Wave defaults to bound context. The required chapter ID is the idempotency key with Wave identity: repeating the same call resumes or returns its receipt, never creates another Project. `--plan` supplies fresh KRs, metric targets, and Flow recommendation as chapter content, stored in the internal Project; omit it to create an empty chapter plan, never to copy the predecessor. Neither rotation nor a chapter-plan update changes the Wave objective. Reusing an applied chapter ID with different content reports the difference; explicit chapter-content editing is a separate Wave-scoped operation.

`--dry-run` performs reads only and returns the proposed Project content plus exact Task dispositions and supporting facts. Applying refreshes those facts and records changes since preview; a preview never authorizes abandoning a Task that subsequently started. Neither command launches an LLM, chooses objectives, judges KRs, or starts new Task workers. Existing workers continue.

The structured result contains transition ID, Wave, predecessor/successor chapter IDs, application status, each Task's disposition, evidence timestamps, and pending operations/errors. Exact provider IDs belong in source details. Persist receipts before subsequent side effects; recover ambiguous provider-create outcomes by reconciling the operation's identity before retrying. Return non-success for incomplete application and preserve its resumable receipt. Git publication is reported separately from operational completion.

Rust API boundaries:

- `resolve_current_project(wave: &WaveId) -> Result<Project>`: shared resolution for commands, prompts, and views.
- `preview_new_chapter(request: &NewChapterRequest) -> Result<ChapterPreview>`: read current facts and compute dispositions.
- `new_chapter(request: &NewChapterRequest) -> Result<ChapterTransition>`: the sole rotation writer used by CLI and application callers.
- `chapter_snapshot(wave: &WaveId, chapter: &ChapterId) -> Result<ChapterSnapshot>`: historical content and membership with dated execution evidence for review, even after Tasks move.
- `move_task_to_project(task: &TaskId, project: &ProjectId) -> Result<()>`: reconcile Linear membership and durable parent without restarting the Task.

Fold Project operation into Wave operation; delete the separate management pass. Ordinary Task creation and planning commands require only Wave context. Historical inspection remains explicit.

## Chapter skills own judgment; the API owns rotation

- **`review-chapter`:** read the exact chapter snapshot, including moved Tasks and historical KR wording. Produce one evidence-backed Wave report: user outcomes, KR verdicts, started work that will move, and unopened backlog that will expire. Use the shared disposition classifier; keep its facts separate from recommendations. A moved Task is neither completed nor abandoned, and work shipped after the interval cannot prove an earlier KR. Review never applies rotation.
- **`start-chapter`:** use that report and human direction to author fresh content for each Wave's single Project. Show the deterministic preview alongside the proposed plan, including inherited active work and backlog closures. After the existing final plan acceptance, call `new-chapter` once per Wave and resume failures through the same API. Archive its actual receipts; never reconstruct success from prose or perform a sequence of handwritten PM moves/closures. Create new Tasks through ordinary APIs against the new current Project.
- **Scoped skills:** `wave/start-chapter` and `wave/review-chapter` directly own the single Project's shaping and evidence. Remove the Project chapter delegation tier and its redundant skills; migrate all callers. Repository chapter skills aggregate one result per Wave. Legacy multi-Project history remains reviewable without keeping the old live planning model.

Recommendations may change goals, but never replace the fixed Task carryover rule. Deterministic rotation remains directly callable without running either skill; its receipt provides the boundary evidence, not a fabricated review or human-acceptance claim.

## Reset and recovery

Review the old chapter, curate durable lessons into Wave memory, and author the next plan from current reality. Do not copy its KR checkmarks, metric targets, backlog, or recommended Flow. Moved Tasks keep their selected Flow. The Wave objective and measurement instruments persist. A new chapter can set a different target for the same metric without rewriting the old target, verdict, or underlying measurement history.

Use one pure classifier over structured, refreshed facts:

| Task evidence | Disposition |
| --- | --- |
| Authoritatively completed/canceled/abandoned | Keep historical |
| Unfinished with execution begun, authored work, published PR, or provider in-progress state | Move |
| Unstarted provider state and complete evidence of no execution/work | Abandon as canceled, never successful |
| Missing or contradictory evidence | Report unresolved; never auto-close |

Paused, blocked, and review-waiting work retains its start evidence; process liveness is irrelevant. Preserve provider workflow categories in PM data instead of reducing them to `completed`. Task preparation and `PrStarted` alone are not start evidence: preparation currently emits that event. Authored work means observable commits/dirty changes beyond the prepared baseline, not model judgment about task prose. A cancellation conflicting with a live worker is unresolved until refreshed/reconciled.

Prepare one successor and record Task dispositions. Reconcile starts/completions racing with rotation: a Task that starts before retirement moves; a terminal Task stays historical. Prevent launches of retired backlog through the existing Task claim boundary. Switch the current binding once and reconcile provider moves and local parents resumably. Archive the predecessor after transfers and closures settle; never mark abandoned backlog successful. Partial application remains visible and retries reuse recorded identities.

Moved Tasks continue without interruption, replacement workers, or PR closure. Future boundaries and observations resolve their updated parent; an already-running Run retains launch provenance. The rotation receipt records old/new membership so earlier chapter reports remain true. The new Project accounts for inherited active work without inheriting old KR verdicts.

Git publication still records chapter acceptance and application. Live rotation ahead of archive merge must remain visibly pending publication.

## Current system and deletion path

`work/project.rs`, `planning.rs`, and `pm/mod.rs` model multiple Projects per Wave. `ops/project.rs` prepares and reopens independent Projects; `ops/pm.rs` creates/archives them. `lf/commands/waves.rs` and Swift expose Project collections. Wave and Project operation skills both select Tasks; chapter start has both proposal tiers.

Replace portfolio enumeration with the binding, merge operation skills, flatten CLI/Mac, and update DTO fixtures and doctrine. Existing `pm_task_move` moves Linear membership and refreshes the snapshot; chapter transfer must also update the durable parent without using Task restart. Migrate each existing portfolio into one fresh Project, moving started Tasks and abandoning untouched backlog. Metric contracts currently name Project IDs: enduring instruments remain Wave-scoped; chapter evidence gets current-Project attribution without inherited KR verdicts.

## Constraints and forbidden outcomes

No multiple current Projects, standing Projects, automatic backlog carryover, Task restart on transfer, hidden Project picker, duplicated next-work decisions, separate Project objective, or Wave-owned chapter target. Preserve code and historical receipts. Historical IDs must not reopen expired planning. No live migration until its concrete plan is accepted.

## Internal slices — one PR

This is an indivisible product change; implement in coherent internal slices and ship together:

1. Chapter binding, evidence classifier, deterministic API/CLI, migration, and resumable rotation.
2. Route operations through the binding; rewrite start/review chapter around API snapshots, previews, and receipts; remove duplicate Project skills.
3. Implement the Desktop/CLI contract across both workspaces, Sessions, Activity, references, metrics, command bindings, DTOs, and docs; run the real demo.
4. **This slice:** Reconcile the ownership correction: show the single Wave objective, put metric targets alongside Project KRs and Tasks, and preserve chapter-specific target/evaluation history through rotation. Migrate target authorship, reads, DTOs, and chapter skills together; retain one Wave UI.

No follow-up Tasks proposed.

## Done when

Rotate from a chapter with one metric target to a chapter with a different target for the same instrument. The Wave objective and instrument identity remain unchanged; the new Project owns the new target, and historical reads retain the old target and dated verdict. An omitted target is unset in the new chapter. Task and KR ownership stays on the Project, while CLI and both Desktop workspaces present all of this through the Wave. Do not duplicate a chapter objective or silently reuse a target from the Wave contract.

Prove provisioning and Wave-only Task placement. Run preview and two chapter rotations through the CLI without an LLM. Cover running, paused, review-waiting, completed, canceled, merely prepared, and untouched Tasks; verify continuity, abandonment, attribution, races, and retry after partial writes or lost responses. Repeating a chapter returns the same successor; preview has no writes. Review an old chapter after a Task moves and ships later: membership remains visible without retroactive credit. Exercise the authored start/review skill paths using the shared API; no independent disposition logic survives.

In native Desktop, navigate Wave → Task directly, see objectives/KRs and full backlog on the Wave, and keep the selected Task, Session grouping, Activity history, and PR controls intact across rotation. Cover Podium and the earlier Wave workspace, fresh/cached/unavailable views, and narrow/wide windows. Shared Rust/Swift fixtures prove a singular chapter and flat Tasks; native interactions prove reachability. CLI help, normal text output, agent instructions, and recovery actions require no Project identity. Demonstrate active work continuing while backlog expires.

## Evidence ledger

2026-09-24: inspected docs, chapter/operation skills, models, PM moves/state parsing, Task preparation/restart, CLI and status. Found lossy PM completion state and preparation emitting `PrStarted`. Human accepted one current Project, active-Task carryover/backlog expiration, and deterministic rotation embedded in chapter skills. No live mutation, implementation, or runtime proof performed.

2026-09-24, UI research: implementation paused at the human's request before production edits. Traced Podium, earlier Wave workspace, Sessions, Activity, cached plan, metrics, CLI grammar/bindings, shared DTOs, and tests. Revised the exposed command to `lf wave new-chapter`, made Wave → Task the complete public contract, and retained internal Project provenance. Native rendering and live migration remain untested.

2026-09-24, implementation: authored the chapter binding migration, disposition classifier, resumable rotation, provider fixed-id creation, Wave chapter command and direct Wave Task creation. Ordinary Task updates no longer overwrite a transferred parent; Task event routing reads the current parent. Focused proofs in progress; UI and skills remain to be changed.

2026-09-24, cross-surface implementation: ordinary CLI bindings and both Desktop workspaces now select Wave → Task. Current chapter definition/KRs and full Tasks share one Rust/Swift projection. Task events route directly to the Wave; Project operation/chapter skills and public management commands are removed. Metrics retain Wave identity across chapters. Start/review skills call the deterministic preview, apply, and historical snapshot paths.

2026-09-24, focused evidence: the chapter operation passed local HTTP-provider simulation with fixed-id create recovery, lost move/cancel responses, repeat idempotence, Wave-only Task filing, a start after preview, and no retroactive completion in history. SQLite tests prove transfer preserves claim/Flow/worktree/PR and stale saves cannot restore the parent; retirement fences the first claim. First execution is a durable Task event across Flow resets. Run history lives outside SQLite; removed an invalid assumption about a SQL runs table. A failed migration test exposed a missing Project join; corrected the outbox migration.

2026-09-24, native evidence: 96 focused Swift tests passed, including Task selection during incomplete transfer, Activity scopes, Session/DTO decoding, and chapter history. Eight native fixture captures distinguish populated/loading/unavailable/empty at 900 and 1440 pixels. Inspected selected Wave captures for both Podium and Portfolio; moved Tasks/objectives ahead of metrics. Screenshots are offline fixture evidence; the unbundled development app cannot exercise installed-helper or live PM controls. No live chapter application or publication occurred.

2026-09-24, final review: fixed exact current/historical chapter reference routing
for provider IDs, slugs, and local Work IDs; removed the redundant Task-preparation
Project writer and unused Project-promotion workflow. Corrected remaining agent
instructions and help that taught the deleted management tier. Focused Rust
chapter, wire, binding, Task continuity, and Swift reference checks passed.
Final native captures show objectives and Tasks above metrics in both workspaces.
Review findings and proof limits are recorded in `scratch/review-wave-chapters.md`.

2026-09-24, ownership clarification: the human assigned the objective to the Wave
and Tasks, KRs, and metric targets to its chapter Project, retaining the Wave UI.
Inspection confirms Tasks and KRs are already Project-owned; targets remain in
Wave metric contracts and their revision hash. Revised the full design, current
slice, doctrine, and focused proof requirement. This is a remaining implementation
gap; earlier passing checks do not establish the clarified target ownership.

2026-09-24, slice review: the clarified ownership remains unimplemented; do not
approve the full design yet. Removed an unused independent Project creation
API and restored read-only historical Project Work inspection. Nine chapter
checks, the focused historical CLI check, formatting, and all-target Clippy
passed. Fresh native fixture captures show Wave → Task in both workspaces;
live controls remain unproven. The evidence matrix, source-review limits, and
next ownership slice are in `scratch/review-slice.md`. Recorded the human's
Wave hierarchy direction as future scope while preserving existing ancestry.
