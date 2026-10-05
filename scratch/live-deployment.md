Jack Heart wants the live installation of Loopflow on his Mac to look polished and behave coherently after the recent Task, Session and Flow API changes. The October 5 screenshots show done and abandoned Tasks in the working sidebar, missing-checkout dead ends, and blocked labels whose underlying work may already have landed. Own the integration and operational cleanup that makes the installed app pleasant to use, including reconciling the live Home, planning records, release/install state and visible workspace. Jack requested prioritizing database cleanup over inventing product changes for obsolete execution records.

Stack this Task on [LOO-353 · Task workspace and primary Sessions](https://linear.app/loopflow/issue/LOO-353/focus-on-your-own-work-the-task-workspace-and-primary-sessions), preserving its implementation and review boundary. Jack's accepted navigation direction: after LOO-353, clicking a Task opens that Task's Session directly; no intermediate Session list is needed. Verify this integrated behavior and repair remaining gaps without introducing another navigation layer.

Acceptance:
- The normal sidebar contains the open, started working set. Completed and abandoned Tasks no longer appear as actionable work or lead to missing-checkout errors; explicit history remains accessible. Reconcile disagreement between local terminal state and planning state, and preserve active conversations and unfinished work.
- Clicking a Task opens its Task Session directly. Preserve conversation identity, drafts, focus and history across navigation; avoid duplicate Sessions and an intermediate Session picker/list.
- Inspect every currently blocked Task in the live repository. Retire obsolete Flow/review/landing records through supported operations when the evidence supports it; continue genuinely unfinished work through its existing owner. Any remaining blocker has a current concrete reason, next action and owner. Never manufacture review approval or mark unfinished scope complete merely to clear a badge.
- Verify the installed Desktop and CLI contain the intended landed changes and read the same Home. Reconcile merged deliveries and completed Tasks. Preserve history and recoverable work; do not hand-edit SQLite, run draft migrations against the live Home, or disturb active drivers.
- Check the actual installed sidebar, Task opening, conversations, loading/error states and layout on Jack's existing workload. Fix remaining visible rough edges demonstrated there, and retain concise before/after evidence and any unresolved real blockers. A green PR alone does not establish that the live deployment is fixed.

Initial evidence: LOO-343 is locally done and LOO-329 abandoned while planning still reports unstarted; LOO-375 retained its old landing failure after PR #1427 merged. LOO-326 repair PR #1435 and LOO-376 optimization PR #1436 have merged; verify current installation and settlement instead of assuming either remains pending. CLI observed October 5: 0.13.3. Screenshots: /Users/jack/Desktop/Screenshot 2026-10-05 at 7.37.02 AM.png, 7.36.52 AM.png and 7.36.48 AM.png (filenames use a narrow nonbreaking space before AM).

Coordinate existing owners rather than duplicate their feature work: [LOO-376 · Startup](https://linear.app/loopflow/issue/LOO-376), [LOO-378 · Session connection](https://linear.app/loopflow/issue/LOO-378), [LOO-371 · Direct Task opening](https://linear.app/loopflow/issue/LOO-371), [LOO-304 · Steady-state responsiveness](https://linear.app/loopflow/issue/LOO-304), and [LOO-373 · Landing reconciliation](https://linear.app/loopflow/issue/LOO-373). This Task owns their coherent live-deployment result and the residual cleanup/polish, not a competing Flow controller or a redesign prerequisite.

## Execution handoff, October 5

Jack Heart explicitly requested starting this Task. The stack is recorded against LOO-353 PR #1439, base e10e2add4210cf2e0849ed84580743ad63988f2d. Preserve its pending demo review; do not claim Jack approved it or merge around it. Work in this child checkout. The parent implementation is a partial runtime cut: do not assume primary Task Sessions, Waiting or every planned Desktop interaction already exists. Inspect the branch and integrate the direct Task-to-Session experience required by this brief.

Begin with fresh installed-runtime facts and supported data reconciliation. Installed lf was 0.13.3 at handoff; older 0.13.0 errors are historical. Use lf for mutations, preserve active drivers, drafts and history, and report exact missing cleanup capabilities rather than editing the live SQLite store. Favor clearing demonstrated stale data over redesigning product behavior. The working-set projection in the inspected source retained a Task for any open Session or started plus planning-not-completed, so inspect both Session membership and disagreement between planning and local terminal states.

Screenshot evidence is copied into scratch/live-deployment/{7.37.02,7.36.52,7.36.48}.png. These show LOO-343 done and LOO-329 abandoned opening missing-checkout states, plus blocked labels for LOO-375 and LOO-367. Verify current records before disposition. LOO-326 PR #1435 and LOO-376 PR #1436 are merged; installed-version and settlement facts require fresh readback. Do not complete unfinished Product scope merely because a slice merged. Coordinate independent active work instead of launching duplicate drivers. Preserve useful conclusions outside scratch before shipping.

## Reconciliation result, October 5

Installed state: `/Applications/Loopflow.app` 0.13.3 with bundled `lf` 0.13.3; shell `lf` 0.13.3; both report Home `home_39860354aaca640c2ccb50bf6ca609d8`. #1418, #1419, #1427, #1435 and #1436 are ancestors of v0.13.3.

Applied through supported operations on the live Home:

| Task | Before | Operation | After |
|---|---|---|---|
| LOO-343, 354, 358, 362, 364 | local done, Linear unstarted, `complete_task` writeback pending on stale transient errors | `lf task complete <issue> --summary …` (retry) | Linear completed, writeback current |
| LOO-329, LOO-309 | local abandoned, Linear unstarted; each thread holds "Abandoned by Jack Heart, 2026-09-30" naming its successor | `lf task abandon <issue>` (retry) | Linear canceled; backup tags intact |
| LOO-375 | Blocked: `pr land -c` exit 1, though #1427 merged | `lf flow end 197c157d…` | idle, "PR merged; continue the Task on its next PR"; Task left open for its production-timing scope |
| LOO-312 | Blocked: "repeat at step 6 requires a decision" since Sept 27; worktree and branch gone locally and on origin | `lf flow end 34b35eab…` | idle; checkout still missing |

Source change: the sidebar working set excludes locally done/abandoned Work even when planning has not caught up (`WorkspaceProjection.inWorkingSet`); the Wave plan is unchanged. Direct Task opening already exists in the LOO-353 parent (`SessionsView.openTask` → `enterTask`, covered by `TaskMonitorProofTests`); nothing was added.

Check: `swift test --filter "TaskHistoryFilterTests|startedWorkingSet|manyStarted"` — 9 tests passed.

Remaining, each with owner and next action:

- LOO-367 stays Blocked for a real reason: loop-decide exited 1 at iteration 5, and its notes say the Session handoff waits on LOO-353. Owner: LOO-367's own Flow. Next: `lf --task LOO-367 flow start --reason …` once LOO-353's contract settles. Not restarted here.
- LOO-343's `ship` Flow still reads `current`: `lf flow end` and completion cleanup both fail with "changed under its driver" (a dead worker's claim). Harmless now that the Task is completed in Linear. Owner: LOO-353, whose migration removes claims.
- Intelligence's chapter Project reads `backlog` in Linear, so the Wave reports "no In Progress Project" and its Tasks are unavailable. `lf task sweep` reported this before LOO-362's writeback, so the retry did not cause it; the earlier roadmap read was a stale snapshot. Owner: Jack Heart (planning) / LOO-366.
- LOO-312's checkout is missing with no branch to restore. Owner: Growth. Next: `lf task checkout LOO-312` when selected.
- The five retried completions carry today's Linear completion date, not their real one.
- About thirty `ship` Waves plus `context` and `discord-demo`, all in temp directories, are registered in the live Home; installed `lf wave` has no removal command. They slow `roadmap --all`. Unowned.
- KAT-4, KAT-14, KAT-16, KAT-20 are completed in Linear but locally ready. Outside this repository; untouched.
- `~/Applications/Loopflow.app` is 0.10.0 and a `Loopflow Dev.app` process (PID 811) is running beside the installed app. Untouched.
- Until this branch ships behind LOO-353, the installed sidebar relies on the data fixes above rather than the projection change.
- No rendered check of the installed sidebar, Task opening or layout was possible headless; that verification remains with Jack's demo.
