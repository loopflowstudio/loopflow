# W2-156 — blockers & control-plane defects

## BLOCKER (2026-07-15): shared control-plane store bricked by a cross-wave 0.11.004 migration collision

**Symptom.** Every `lf` store command on this machine fails:
```
database migration 0.11.004_context_launch_work is unknown to lf 0.11.1
(latest known 0.11.003_child_body_lease); this database needs a newer release
or the matching divergent local build
```
`lf task acknowledge`, `lf pm`, `lf memory`, `lf ls` all error; `lf status`
returns "no wave registry". So directive v2 could **not** be acknowledged
through the store, and PR2 cannot be landed via `lf`.

**Root cause — the documented hazard, recurred on the 0.11 control store.**
Two waves independently minted the same migration ordinal `0.11.004`:
- product (this task): `0.11.004_task_pr_ci_state` — **merged to main** in PR #916.
- Context Lab: `0.11.004_context_launch_work` — commit `06025c3eb` on branch
  `jack-heart/context-lab`, **not on main**.

A Context-Lab-built `lf` worker ran on this machine and migrated the shared
`~/.lf/loopflow.db` (pointed to by `LF_CONTROL_DB_PATH`) to
`0.11.004_context_launch_work`. Now the release `lf 0.11.1` and any product-built
`lf` (which knows `0.11.004_task_pr_ci_state`, not `context_launch_work`) reject
the store as incompatible. Only a Context-Lab build can read it.

Both branches saw `0.11.003` as the latest free ordinal and both picked
`0.11.004` — exactly the "migration numbers collide across branches" learning
already in wave memory, now on the isolated 0.11 store instead of `lfd.db`.

**Why I did not fix it (deliberate).** Repairing the shared store is authoritative
recovery + cross-wave coordination, not a W2-156 code change:
- A shipped migration is never edited; product's `0.11.004_task_pr_ci_state` is
  already merged, so it cannot be renumbered.
- Deleting/rebuilding `~/.lf/loopflow.db` is a control-plane operation that
  destroys other waves' local session state — a worker must not do it.
- Doctrine: on LF store-context divergence, do not mutate from the broken view;
  wait for authoritative recovery.

Self-obscuring: the brick also blocks `lf memory add`, so this defect can't be
written to wave memory from here — hence it lives in git until the store recovers.

**Recommended recovery (for a human / recovery context):**
1. Context Lab renumbers `context_launch_work` to the next free ordinal *after*
   `task_pr_ci_state` (i.e. `0.11.005`) — it is unmerged, so this is cheap.
   `validate_set` will force this anyway when context-lab rebases onto main
   (two `(0,11,4)` ids are rejected), so main is self-healing on rebase.
2. Rebuild the local `~/.lf/loopflow.db` (it is a read-model registry) with a
   build that knows the reconciled migration set, or restore from a pre-collision
   backup. Local session rows are the cost.
3. Adopt the governance fix already proposed in wave memory: **per-wave migration
   ordinal ranges** (so two waves never pick the same 0.11.004), or the
   `LF_HOME=~/.lf-dev` dev-store isolation so in-flight schema can't touch the
   real ledger.

## PR2 status

Design re-established in `scratch/wake-a-waiting-task-into.md` (the pre-land doc
died at land; recovered from wave memory). Implementation and `cargo` tests are
computable now (they use temp DBs, unaffected by the shared-store brick), but the
PR cannot be acknowledged/opened/landed via `lf` until the store recovers. Pause
PR2 landing on the recovery above; the design is ready to build the moment `lf`
store ops work again.

## Pre-existing (carried from PR1)

`pr_tests.rs::github_failure_leaves_publication_intent_observable` fails on this
sandbox at the pre-change baseline too — a git-in-sandbox environment issue, not
a regression.
</content>
