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
