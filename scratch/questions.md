# LOO-656 open questions and assumptions (headless implement, 2026-10-27)

- **Released lf ≤ 0.14.19 on a store with slug rows.** It parses ledger versions
  as numeric ids, so `task_graphs` makes it print "incompatible Loopflow
  database; delete loopflow.db" — wrong advice. `task_graphs` drops
  `task_workflows`, so 0.14.19 could not read the store anyway. Assumed
  acceptable (solo Machine, install switch backs up first); long-lived 0.14.19
  processes need a restart after `lf self install --main`.
- **Children of a worktree build on `~/.lf`** still resolve the installed entry
  (`resolve_lf_binary`), not the worktree binary. Kept as is; the top-level
  command runs its own code.
- **Sources without Git history** (crates.io package, container builds) treat every
  slug as unmerged unless `LOOPFLOW_MIGRATION_ORDER` names the order. Release tag
  builds check out full history and refuse to compile with an unmerged slug.
  Whether the crates.io package needs a generated order file is open.
- **`lf self install --main` installs the CLI only**; the Desktop app still ships
  with releases. It needs `gh` auth because Actions artifacts are private.
- **`~/.lf/copies/`** left by earlier draft builds: `disk-cleanup.sh` keeps
  reclaiming them after a week rather than leaving GBs behind.
- **One migration per Task** is guidance (`new_migration.py`), not a CI failure:
  merge-queue groups carry several Tasks' migrations at once.
- **Proposed Wave memory update** (not written; `.lf/wave/` is read-only here):
  infrastructure MEMORY.md's notes on draft migrations, release-cut ids,
  copy-on-draft and validation-only builds describe the retired model; replace
  them with "migrations get ids at merge; `lf self install --main` installs green
  main".
