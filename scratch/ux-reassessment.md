# LOO-303 UX reassessment

Research only, 2026-09-30. Jack Heart requested it after LOO-298 settled its model
and LOO-353 was accepted. No code changed.

## Summary

Most of LOO-303 was specified on 2026-09-26 for a world that has since moved.
Three things changed underneath it:

1. **LOO-298 removed Runs.** Exec, AgentSession and FlowSession replace them.
   Loop passes are lenses over one FlowSession, not child invocations. Bind is
   write-once null → Task on the AgentSession and ships as `lf session bind` with
   a Desktop "Bind to Task… → Bind permanently" beside the Session name.
2. **#1360 removed resident Waves, lfd and Wave chat.** Nothing in LOO-303 depended
   on them, but LOO-353's primary Wave Sessions now fill that space.
3. **LOO-353 (accepted 2026-09-30) replaced the orphan story.** Jack: the primary
   Sessions "basically replace the orphan sessions window, which becomes more of
   like a debug screen", and "sessions are grouped into the task based on
   worktree, even if not part of a flowsession". Its branch memory records the
   control room as superseded.

What survives cleanly is what this branch already built: ⌘K, `loopflow://task`
links, folded Flow templates and hidden-terminal input isolation. Those are
navigation primitives LOO-353 needs and doesn't plan. Recommendation: land them
as LOO-303's single PR once LOO-298 lands, and give everything else away. The
control room and room-centric bind are cut. The invocation view and running line
move into LOO-353's participatory Flow strip. Dark mode goes to its own later
Task.

Jack's direction today favors this: autonomous landing, one Home, one client,
toss history, and "don't become a slave to your agents". A room built to triage
orphan terminals is agent-shepherding by design.

## Keep / change / cut

| Experience | Verdict | User-facing reason | Evidence |
| --- | --- | --- | --- |
| ⌘K palette (Waves, Tasks, Sessions, Flows, actions; recents first) | **Keep; ship in LOO-303** | Jack reaches any work without clicking the tree. It works with LOO-353's workspace and needs no primary Sessions. | Built on this branch (`abfa364b0`, `5def26ba0`, `f20f06234`, `3f84174d0`); survived the LOO-298 merge `01dda6a26`. LOO-353 plans no keyboard route. |
| `loopflow://task/ISSUE` links | **Keep; ship in LOO-303** | One link from Linear or a PR opens the Task without starting work. | `lf roadmap --task ISSUE [--all] --json` reader in `waves.rs`; cold/warm delivery in `WorkspaceDestination.swift`. PR #1354 (LOO-321) reworks Task lookup through shared SQLite records, so re-point the reader there if it lands first. |
| Folded Flow template on Wave and unstarted Task pages | **Keep, but reframe as LOO-353's edge-inspection layer** | Jack sees the plan before it runs. In LOO-353 the primary view is interactive stages with edges as Flows; folded composition is how an edge expands. | Built in `engine/flow_graph.rs` plus `TaskFlowView.swift`; uses the captured numeric node IDs after the merge. LOO-353 plans `project_interactions(graph)` over the same `FlowGraph`. Keep both in one Rust traversal. |
| Hidden retained terminals refuse first responder | **Keep** | Keystrokes never land in a terminal Jack can't see. LOO-353's collapse and focus need the same guarantee. | `GhosttyTerminalView.swift` change; product memory: "only the first responder may consume terminal Command-V". |
| Unrolled FlowSession invocation view (cursor, per-edge return counts, loop passes, retained history) | **Change → merge into LOO-353** | The Task's Flow strip is where Jack sees progress. Two Flow views on one Task page would compete. | LOO-298 made passes node/iteration positions of one FlowSession ("more of a lens than an operational entity"), so the old "child invocations for entered loop bodies" spec is gone. LOO-353's inspector must "remain truthful about branches, loops, and the current pass". Same data, one surface. |
| Exact running line and Session chip | **Change → merge into LOO-353** | The row tells the truth about which conversation is at the current step. | Current code picks the "first active Task Run"; Runs no longer exist. LOO-353 separates *available*, *preparing* and *Ready* from boundary evidence. Build the chip once, from the FlowSession cursor's selected AgentSession. |
| Orphan sessions section + control room (tile every orphan, bind at rest) | **Cut** | With worktree grouping and primary repo/Wave Sessions, almost nothing is an orphan. A room to triage stray terminals is agent-monitoring, the opposite of Jack's stated differentiator. | Jack, 2026-09-30: primary Sessions "replace the orphan sessions window, which becomes more of like a debug screen". LOO-353 memory: "The earlier orphan control-room direction is superseded." Worktree grouping lives in Rust (`ops/human_session/workspace.rs` on LOO-353's branch), so the old "never match by checkout path in Swift" constraint is met in Rust. |
| Room-empties-then-jump, >6-orphan tiling, shared-shell "Show here" | **Cut** | These only exist to serve the room. | Follows from the room cut. The `.controlRoom` content mode and conditional host mounting are unbuilt. |
| Universal bind from room, ⌘K and Task-page rows | **Change: keep the existing single bind; add a ⌘K action only** | Bind is now a rare permanent correction. Worktree grouping handles the common case without writing attribution. | `lf session bind` and the breadcrumb "Bind to Task…" already ship with LOO-298. LOO-353: location grouping "needs no bind to present a Session under its checkout's Task". Bind is prospective only; LOO-347 re-evaluates usage attribution. |
| Pre-model orphans in a Task worktree with a preselected bind | **Cut** | Worktree grouping already shows them under the Task. | Jack: "we toss history"; "one machine". LOO-298 discards historical attribution with no importer. |
| Global Session browser | **Change → LOO-353 diagnostics** | Debug only. | LOO-353: "Move the orphan/global Session browser to Diagnostics." |
| Dark mode | **Cut from LOO-303; refile when light is accepted** | Jack said "when we're at a happy place". LOO-353 is still reshaping the light workspace. | Product memory (2026-09-26); installed 0.12.22 light build "is working well". Designing dark against a layout that is about to change is wasted work. |
| Configured demo + 1440×900/1100×800 captures accepted by Jack | **Change: run it on LOO-353's workspace, not a separate LOO-303 demo** | Jack judges one workspace, once. | Jack waived review for LOO-298's landing ("no need to review with me"). The KRs are about Desktop handling real sessions, which LOO-353's demo exercises. |
| Wave-page New session, launch-point inference | **Cut from LOO-303** | Primary Wave and repo Sessions answer "where do I start a conversation". | LOO-353 Unit 2: Desktop discovery starts one Session per repo and per Wave. |
| Waveless first run (deferred here) | **Cut from LOO-303 → LOO-353's repo Session** | The repo Session is the onboarding path. | LOO-353: repo Session "must be useful before any Wave or Task exists"; its memory records that it supersedes the deferral. |
| Perf targets (≤100 ms warm nav, ≤50 ms palette filter) | **Keep as LOO-300's scenarios** | Palette and link latency are measurable now. | LOO-300 owns the harness. Keep `hierarchy_interaction_ms` / `task_workspace_ready_ms`. |

## Conflicts to watch

- **LOO-353 is written against Runs.** Its branch sits on main before LOO-298:
  `SessionRecord.runId`, `PrimarySession { run_id, replacement_run_id }`, "Run
  manifests/provider history own conversations", `task_flow_positions`-era Flow
  capture. After LOO-298 lands, these become AgentSession IDs and FlowSession
  state. The Unit 2 pointer record and the switch-now/finish transition both need
  a restatement on the three-owner model before they're built.
- **File overlap.** Both branches edit `SessionsView.swift`,
  `WorkspaceProjection.swift`, `RegistryQuery.swift`, `lf/mod.rs`, `bin/lf.rs`,
  `dto_fixtures.rs` and `swift/README.md`. Landing LOO-303's small PR first keeps
  LOO-353's Unit 1 merge cheap.
- **Two graph projections.** Folded templates and `project_interactions` must
  share one Rust traversal and one fixture, or the Wave page and the Task strip
  will draw different graphs of the same Flow.
- **#1296 is not landed.** As of 2026-09-30 19:51Z it is open and conflicting,
  and checkpoint publication is refused because the Task records a pre-merge base.
  LOO-303's stack parent is #1296, so nothing here ships until that clears.

## Open questions for Jack

1. **Ship LOO-303 as just palette + links + folded templates + input isolation,
   and close it?** Everything else would move to LOO-353 or be cut, as above.
2. **Does bind keep a Desktop entry point at all?** Worktree grouping covers the
   common case. The breadcrumb control already exists. The question is whether to
   add ⌘K "Bind to Task…" or leave it CLI-plus-breadcrumb.
3. **One Flow view or two?** LOO-353's interactive-stage strip versus LOO-303's
   folded template versus unrolled invocation. Proposal: the strip is primary,
   and an edge expands into the folded template before start and into the
   unrolled FlowSession after.
4. **Dark mode:** refile now as its own Task, or leave it unfiled until light is
   accepted?

## Evidence

- LOO-298 plan and decisions: `loopflow.data-model-one-table-per/scratch/remaining-work.md`,
  `questions.md`, `naming.md` (Jack, 2026-09-30: autonomous landing; merge, not
  rebase; one machine, one Home `~/.lf`; toss history; three migrations;
  bind write-once and prospective).
- `wave/infrastructure/MEMORY.md` § Data model and performance decisions
  (reconciled 2026-09-30): three owners, loop passes are lenses, no `RunId`.
- `wave/product/MEMORY.md` § Workspace redesign decisions (2026-09-26): the
  original control-room, bind, two-Flow-view and dark-mode decisions.
- LOO-353: `loopflow.growth-thoughts/scratch/growth-thoughts.md`, `intent.md`
  (Jack verbatim), `questions.md`; branch diff of `wave/product/MEMORY.md`
  records control room superseded and waveless deferral superseded. Unit 1's
  first cut (Rust checkout association, paged directory browser) is implemented
  and not shipped.
- Merged PRs 2026-09-29/30: #1353 (typed command/skill/Flow discovery), #1355
  (managed account identity), #1357 (v0.12.26), #1358 (resource recovery),
  #1359 (PR publication continuity), #1360 (resident Waves, lfd and chat removed,
  −35,333 lines), #1361 (hosted CI deferred while scratch exists), #1362 (launch
  context budgets), #1363 (failed reads no longer block handoff).
- Open PRs: #1296 (LOO-298, conflicting), #1354 (LOO-321 Linear Task lookup),
  #1356 (LOO-338 CLI owner tree), #1364 (deepest cuts first), #1365 (account
  status per Flow step).
- Relevant open Tasks from `lf roadmap --all`: Product: LOO-291, LOO-293 (Watch,
  blocked), LOO-327 (file browser, later), LOO-332 (progress to landing without
  a watcher, blocked), LOO-353. Infrastructure: LOO-298, "Desktop performance after the data model"
  (LOO-300 per product memory), LOO-342 (one main Home), LOO-297 (prototyping paths).
  Intelligence: LOO-347 (re-evaluate usage attribution after bind).
- This branch's own work: `git log origin/jack-heart/data-model-one-table-per..HEAD`,
  33 files, +2,323/−329 in Swift, Rust and docs.
