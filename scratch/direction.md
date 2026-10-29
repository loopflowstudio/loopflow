# 23217a plan (reconciled 2026-10-29)

## Done on this branch
- `lf wt prune` prints every checkout as `keep`/`remove` with its reason, in text and `--json`.
- Eligibility follows Task state: done/abandoned Task, merged/closed PR, or no Task/PR with HEAD idle 7 days. Changes under `scratch/` and `.lf/` never keep a checkout (Jack's decision).
- `--all` removes every checkout not held by an unfinished Task, a persistent (repo/Wave conversation) checkout, the default branch, the current checkout or a running process; unpushed commits and source changes are saved to `refs/lf/pruned/<branch>/<commit>` first.
- Demo evidence: on 2026-10-29 the branch build's `lf wt prune --dry-run` in loopflow kept 16 checkouts (main, current, 12 unfinished Tasks, 2 persistent, 1 touched today) and removed none — only main and active Task checkouts remain.

## Disk cleanup (Jack, 2026-10-29) — done by hand from the Task conversation
Direction: once the catalog of existing worktrees is done, free disk space ASAP: remove the worktrees the catalog shows are dead (no started, unfinished Task and no live cwd), largest first. Push or record any unpushed commits before removing a worktree. Then keep reclaiming as much space as possible, including build output (target/, .build/, node_modules) in dead or idle checkouts. Report how many GB were freed.

Dead worktrees were removed largest first; 24 of LOO-669's 36 busy worktrees had their idle conversations closed and were removed (119 GB freed → 268 GB free; 265 GB free at reconcile). Uncommitted changes from 5 are under `refs/prune-backup/<dir>`; see scratch/loo-669-cleanup/busy-removed.log. Kept: manabot eval/benchmark trees possibly mid-run (19d240, bb4160, e2826b, llm-eval, mtg-188, mtg-212), trees serving or running Python (etude.challenge-an-identified-trained-bot, mtg175-runtime, two-seat-report) and persistent conversation checkouts. Build output (2026-10-29, second pass): ~66 GB measured under ~/src. Removed only where no process other than the all-checkout `lf desktop plan --watch` watchers had a cwd or open file: loo-500 target/ + swift/.build, hootro/.venv, kata/scripts/omr/.venv, four small loopflow .venvs. `df /` before 264.3 GiB free, after 270.3 GiB (5.96 GiB freed; **~125 GB freed in total** across both passes). Kept because a build or conversation was live there: 5b3da9 (clippy), loo-618 (nextest), loo-669 (gate), loo-687 (cargo test), loo-673 (open Claude conversation), this checkout, and every manabot .venv (each tree had live processes). Those ~45 GB of target/ and .build become reclaimable once their builds finish. `uv cache prune` timed out on the lock held by live uv processes; not forced. sccache (10 GB) left as the shared compile cache.

## Remaining
- Jack's rule: a live but idle conversation does not protect a dead worktree. Prune still keeps any checkout with an `lsof` path inside it as "in use". Needs Session availability per holding process to tell an idle agent conversation from live work; substantial enough to stay explicit, not folded into this PR. No Task filed yet.
- Saved refs under `refs/lf/pruned/` never expire.

Check: `./target/debug/lf wt prune --dry-run` in loopflow — 0 to remove, 16 kept, reasons as above.
