# Direction (Jack, 2026-10-29)

Once the catalog of existing worktrees is done, free disk space ASAP: remove the worktrees the catalog shows are dead (no started, unfinished Task and no live cwd), largest first. Push or record any unpushed commits before removing a worktree. Then keep reclaiming as much space as possible, including build output (target/, .build/, node_modules) in dead or idle checkouts. Report how many GB were freed.

## Busy worktrees (Jack, 2026-10-29): close conversations, delete worktrees

Done from the Task conversation. 24 of LOO-669's 36 busy worktrees had their idle conversations closed and were removed, freeing 119 GB → 268 GB. Uncommitted changes in 5 of them are kept under `refs/prune-backup/<dir>`; the list is in scratch/loo-669-cleanup/busy-removed.log. Kept: manabot eval/benchmark trees that may be mid-run (19d240, bb4160, e2826b, llm-eval, mtg-188, mtg-212), trees serving or running Python (etude.challenge-an-identified-trained-bot, mtg175-runtime, two-seat-report), and the repository and Wave conversations' checkouts (loopflow.repo, manabot.repo, loopflow.wave-6155f18a…). The real-world cases worth turning into prune rules and tests: a live but idle conversation does not protect a dead worktree, and repository and Wave conversation checkouts are persistent.
