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
- **Wave memory curated in compress** (2026-10-28): the compress step's
  context budget asked for it, so infrastructure MEMORY.md now replaces the
  LOO-344 draft section with "Migrations get ids at merge (LOO-656)", drops the
  draft-materialization gotcha and copy/validation-only references, and trims
  historical detail already kept behind git refs (15995/16000 tokens).
- **Reader catalog source** (2026-10-28): the package file comes from the built
  binary (`lf self install reader-catalog`, read from the `build.rs`-ordered
  catalog) rather than a second file `build.rs` writes, so the floors have one
  source. Each `lf-<target>` artifact carries an identical copy.
