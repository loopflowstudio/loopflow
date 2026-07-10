# Open questions / assumptions — rebase run

## Concurrent agent active in this worktree (observed 2026-07-10)

During the rebase onto `origin/main` (`05c3a551`), another process was live in
this same worktree:

- My initial `git log main..HEAD` showed HEAD at `734ccb86` (6 branch commits).
- By rebase time, HEAD had advanced to `56d37321` — a concurrent process had
  added two commits: `lf pm: connect infrastructure to linear` and
  `lf pm: connect intelligence to linear`.
- After the rebase completed cleanly, fresh **uncommitted** edits appeared in
  the tree: `rust/loopflow/src/lfd/pm/linear.rs`, `rust/loopflow/src/ops/pm.rs`.

**What I did:** rebased all 8 branch commits (6 original + 2 pm) onto
`05c3a551`. No conflicts. Net branch-vs-main diff is byte-identical to the
pre-rebase net diff, so the rebase preserved intent and added only main's one
new commit (`bytes` bump #859). The concurrent agent's uncommitted pm edits are
untouched by the rebase (they were written after it) and remain in the tree.

**Assumption / decision:** pushed with `--force-with-lease` so the push aborts
safely if the remote branch moved under me. The concurrent agent's in-flight
pm work is local and unpushed, so my push does not clobber it; when that agent
commits, it builds on the rebased HEAD. If the lease rejects the push, the
concurrent driver pushed first — do not re-force; reconcile by hand.

I did **not** stash-pop the earlier `.lf/metrics/ops.jsonl` snapshot
(`stash@{0}`) — it is a stale metrics artifact and the file has a live writer;
popping risks clobbering concurrent appends.

## update-wave run (2026-07-10)

Reconciled MEMORY.md to the two efforts on this branch (`lf radio` explicit
pub/sub; Linear-owned PM with `projects/*.md` deleted). Two calls made headless:

- **Filed loopflow-api task `8e77a60f`** for a real contract gap this branch
  introduced: `lf pm show --json` emits no `projects`/`synced_at`, but the new
  Swift `PmShowSnapshot`/`RegistryQuery.plan()` require both as non-optional
  Codable, so the Mac plan decode throws. Also `lf pm sync` still writes the
  `projects/*.md` markdown the Linear-owned design says shouldn't exist. Pick one
  source before trusting the plan render.
- **Removed the `wave/*/projects/` dirs** that `lf pm sync` regenerated while I
  inspected live state. The branch deleted them on purpose; they're regenerable
  by re-running sync, so restoring the branch's clean tree was the safe call.
