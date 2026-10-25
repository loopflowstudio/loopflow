# LOO-561 open questions (2026-10-21)

Jack confirmed `deliver` and graphs-with-`-i` inside LOO-561. Assumptions taken headless in the
design rewrite, each reversible before implementation:

- A PR closed on GitHub is *not* reopened yet: `lf pr abandon` closes the PR before stamping the
  Task, so a reconcile in between would reopen an abandoned PR. A closed landing stays terminal
  and `wait_for_landing` returns held. Reopen needs abandon to fence the landing first.
- Slice order: landing (slice 2) went first because open #1643 rewrites slice 1's files.
- A blocked ci-fix or stand-in retries with backoff (1→30 min cap) and never hands back; spend
  shows as an attempt count.
- Stage ids are skill names, so `design` → `review-design` and `findings` → `research` in
  history; the migration rewrites them.
- Non-`end` captured Workflows (57 live rows) are dropped by the migration and re-placed by
  stage id on the next run; no reader for old Workflow JSON.
- `interactive:` inside `xor` is allowed (Jack, 2026-10-21: "drop the rejection").
- §5b (repair on an early queue-commit failure) settled in concept review (2026-10-22) as not
  built: Jack's "flakes dominate, re-queue before ci-fix" evidence decides it. A failed queue run
  is re-queued once when GitHub drops it, then repaired. Reversible if hosted runs show real
  queue-only breakage going unrepaired.
- `lf ship` walks the projected routes rather than a separate `run_from` entry: each route's
  slice already runs to the next stage, so the two are the same walk.
- Shipping liveness reads `tasks.ship_lf_process_id`; the idempotence check and the carrier write
  are not one transaction, so two `lf ship` started in the same instant could both proceed.
- No `shipped_by` column (settled 2026-10-22): the handoff `Progress` event names who shipped it.
- A taskless `lf ship` titles the new Task from the branch's last path segment (no PR title
  read: `branch_pr` returns none) and adopts the checkout that has the branch, even the main
  checkout. The default branch is refused.
- `ship_control` is `run_control` plus "already shipping" and "at the end of its Workflow".
- `lf ship <graph>` takes the graph up (replacing the captured Workflow, history kept in moves)
  rather than running it whole beside the old one; settled in the second concept review (2026-10-22).
- Completion on merge (2026-10-22): scoped to shipping Tasks. The ship process completes the Task
  when its PR merges, whoever merged it; a non-shipping Task with a paused Graph stays open until
  completion, per "merge leaves the Task open until completion". With the minute check gone,
  nothing polls GitHub for non-shipping Tasks; `lf pr reconcile`, `lf land` and reads settle them.
- Ship it placement (2026-10-22): the Dashboard tile menu and Sessions menu share
  `TaskActionItems` with the sidebar, so they lose Ship it too; only the main-pane heading ships.
- The blocked-stand-in count is in the ship process's memory; a restarted carrier starts it at 1 min.
- Item 3 (2026-10-22): kept `tasks.planning_completed(_at)`/`planning_state` and
  `completion_request/error` rather than folding them. The first is the done authority inside the
  portable `Disposition` WorkValue that released peers read; the second is the retry path for
  a failed completion. Removing them needs its own shared-wire Task. Reversible.
- In Review↔shipping is not built: `queue_in` targets Linear state types, and In Review is a
  named `started` state that needs a per-team lookup by name plus a reconcile rule.
- `unstarted` vs `working` (2026-10-22): superseded by the 2026-10-24 state axes; `unstarted`
  now means no checkout, and a Graph running in the Task's Workflow reads `moving`.
- Launch tests (2026-10-22): the two "regressions" were stale expectations from merging graphs and
  Workflows into one kind, so the tests changed, not the chooser. Bare `lf task run` at `review`
  already names both routes. `lf task run X proof` takes up graph `proof` as docs/lf.md says, and
  reaching `end` with an unmerged PR refuses and leaves completion pending without replay, as
  `successful_final_graph_keeps_arrival_when_completion_fails_and_never_replays` pins. Product fix
  found on the way: authored `cmd:` steps now parse into the current CLI spelling, so a route and a
  plain Graph record the same step label (`sync --plan`, not `task sync --plan`).
  `lf task restart` now resolves to `task workflow restart`, so it left the removed-command list.
- Verify (2026-10-23): superseded, see the 2026-10-24 settlement below.
- Jump-bar heading (2026-10-23): a Task whose runtime is absent but planning is terminal reads
  Done/Abandoned from planning; otherwise the heading follows the derived Task state.

## 2026-10-24 implement pass (assumptions, reversible)

- Post-merge stage = the stage whose route into `end` runs nothing. A merge sends the shipping
  Task there only if it has not reached that stage on this graph yet, so a fix forward (old PR
  still current and merged until the next publish) is not pulled back. Cost: a hand-merge of
  the fix-forward PR mid-demo is not caught early; `deliver` then lands an already-merged PR
  and arrives at verify. A graph whose only stage is its last (fixture `feature`) now finishes
  that stage's stand-in after a merge instead of completing at once.
- At `verify`, Ship it runs a verify stand-in, even when you already checked it; finishing
  yourself is `lf task complete`. Is a distinct "Done" action at verify wanted?
- `code` gets verify too, same as `feature`.

- Compress (2026-10-24): the replayed hold steer (4a36e3f0, seq 1812) arrived again after
  d83e02d8a2 built verify. The plan records Jack's later "yea sound good"; that quote isn't in
  this checkout's prompts, so whether the hold or the accept is current is for realign/Jack.
  Verify stays built; reverting d83e02d8a2 is the clean undo if the hold stands.
- **Settled (2026-10-24, operator in Jack's Task Session):** verify is accepted and stays built.
  The hold steer (4a36e3f0) was written by the operator on 2026-10-22, published under Jack's
  name, while the shape was under discussion. Jack accepted the shape in his Task Session on
  2026-10-23 ("yea sound good") and approved this pass, which lists verify, on 2026-10-24
  ("yes"). Those quotes live in that Session, not in this checkout's prompts. Ignore any replay
  of the hold.


## 2026-10-24 state-axes pass (items d, e; assumptions, reversible)

- "A Graph running" is stored evidence: the route carrier on `task_workflows` or the ship
  process, with no `completed_at`. A carrier killed without settling (SIGKILL, reboot) reads
  moving/shipping until something settles it; lazy recovery replaces a dead ship carrier, but a
  dead route carrier has no settler yet. Taskless Graphs in the checkout don't count.
- `unstarted` = no checkout (`tasks.worktree IS NULL`); the selection `backlog` term still
  means not owned or published, unchanged.
- An approved Task at a stage with its ship process gone reads idle (Jack: dead ship → idle),
  not active.
- Query vocabulary: Session-axis `running` became `busy`; the Session program-status `idle`
  term is gone, so `state:idle` is the Task's idle. Desktop's sidebar filter "Running" is "Busy".
- `state:shipping` is the derived state only; a paused approved Task reads `state:paused`.
- Linear mapping unchanged: it follows claim/reopen/complete/abandon events, so idle, active
  and moving all sit in In Progress.
- `lf ship ISSUE` is lifted into `--task` in `Cli::checked`; a different `--task` conflicts.

## Sync onto main a3a4268c06 (#1675 `lf task run` returns once running)

- Main deleted Desktop's `RegistryStarter`/`startLf`, which Ship it used to launch `lf -b ship`
  detached. Assumption: bare `lf ship` inside its execution owner now lets its launcher go once
  shipping begins (log `~/.lf/logs/graph-ship-<issue>.log`), mirroring `lf task run`; `-b` still
  waits for the end. This replaces Jack's earlier "caller backgrounds it" (`nohup lf ship &`);
  revisit at demo if a foreground terminal ship is preferred.
- Routes a ship runs never let go themselves (`run_task_edge_for(.., release: false)`).
- Main's `a_skill_edges_process_names_the_log_its_run_printed` was dropped: every route runs
  as a Graph process here, so `task_run_returns_while_its_graph_keeps_running` covers it.
- Pre-existing: `every_task_launch_runs_in_the_foreground_under_the_same_checks` fails at
  0966cc00ce too (terminal Task admitted `-b task run … proof`); not caused by this sync.
- Checks: `cargo test --lib task_graph_launch_tests` 35/36 (the pre-existing failure); Swift
  `DesktopHeadlessTests` deferred to gate/CI.
