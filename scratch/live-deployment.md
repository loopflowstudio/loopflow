LOO-380's brief and acceptance live in its Linear directive. This branch stacks on LOO-353 PR #1439 (base e10e2add4); its demo review is still pending and is not Jack Heart's approval. Mutate the live Home only through `lf`; never hand-edit SQLite, run draft migrations there, or disturb active drivers. Before-state screenshots: `scratch/live-deployment/{7.37.02,7.36.52,7.36.48}.png` (LOO-343 done and LOO-329 abandoned opening missing-checkout states; blocked labels on LOO-375 and LOO-367).

## Reconciliation result, October 5

Installed state: `/Applications/Loopflow.app` 0.13.3 with bundled `lf` 0.13.3; shell `lf` 0.13.3; both report Home `home_39860354aaca640c2ccb50bf6ca609d8`. #1418, #1419, #1427, #1435 and #1436 are ancestors of v0.13.3.

Applied through supported operations on the live Home:

| Task | Before | Operation | After |
|---|---|---|---|
| LOO-343, 354, 358, 362, 364 | local done, Linear unstarted, `complete_task` writeback pending on stale transient errors | `lf task complete <issue> --summary …` (retry) | Linear completed, writeback current |
| LOO-329, LOO-309 | local abandoned, Linear unstarted; each thread holds "Abandoned by Jack Heart, 2026-09-30" naming its successor | `lf task abandon <issue>` (retry) | Linear canceled; backup tags intact |
| LOO-375 | Blocked: `pr land -c` exit 1, though #1427 merged | `lf flow end 197c157d…` | idle, "PR merged; continue the Task on its next PR"; Task left open for its production-timing scope |
| LOO-312 | Blocked: "repeat at step 6 requires a decision" since Sept 27; worktree and branch gone locally and on origin | `lf flow end 34b35eab…` | idle; checkout still missing |

Source change (unshipped, stacked behind LOO-353's review): the sidebar working set is started Work with unresolved execution (`WorkspaceTask.inWorkingSet`). A locally `ready` Task always counts as unresolved, so open started Work stays; locally done/abandoned Work leaves even when planning has not caught up, unless it holds an open Session, a non-idle latest Flow or observed unsettled files. The Wave plan and Show completed are unchanged.

Check: `swift test --filter "TaskHistoryFilterTests|startedWorkingSet|manyStarted"` — 9 tests passed.

## Acceptance against the directive

| Acceptance | Exists | Remains |
|---|---|---|
| Sidebar is the open, started working set | Projection change and test on this branch; live Home data reconciled so 0.13.3 already shows it for the nine Tasks above | Ships only after LOO-353 lands |
| Clicking a Task opens its Session directly | Inherited from the LOO-353 parent: `SessionsView.openTask` → `enterTask` focuses the retained pane's Session, else opens the current Flow-step Session, else the first open one (`TaskMonitorProofTests`) | A Task with no Session opens empty panes; nothing is created or picked by input recency. That is LOO-353's unimplemented Task primary selection / LOO-371, not added here. Draft, focus and history continuity unverified on the installed app |
| Every blocked Task inspected | LOO-375 and LOO-312 retired; LOO-367 kept with reason, next action and owner | LOO-343's stale `current` Flow until LOO-353's claim removal |
| Installed Desktop and CLI agree | Both 0.13.3 on one Home, containing the five named PRs | — |
| Rendered check of the installed app, after-state evidence | Before-state screenshots only | No headless rendering; Jack's demo |

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
