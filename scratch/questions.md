# Open questions — W2-130

## RESUMED 2026-07-14 after the rollout; this slice is submitted

Rebased onto `origin/main` (poisoned base gone), schema re-authored as a forward
migration, first supervisor-safety slice landed and verified. What shipped, and
what is deliberately left:

**Shipped:** pinned execution context (+ unpinned Sessions refuse to launch);
durable abandon intent written with the command; `supervisor_restart_bar` honored
at every launch gate; submitted-with-open-PR barred from supervisor restart with
the W2-129 sequence as a regression; interrupt never launches; `BEGIN IMMEDIATE`
on child-session writes; debug builds default to `~/.lf-dev`.

**Corrections to the pause notes below:** the migration namespace is **0.10.002**,
not 0.11 — `scripts/new_migration.py` namespaces by the *package* version (0.10.1)
and the convention forbids an id ahead of it. And the fear that the schema change
would force every database to recreate is **dead**: verified against a real
pre-migration copy of the live registry, it upgrades and gains all five columns.

**The live-registry breakage was ours, and it is now provably prevented.** While
this branch was under test, its own `cargo test` run applied the unreleased
`0.10.002` to `~/.lf/loopflow.db` at 19:41:20 — a worktree build writing the real
ledger, which is failure #2 of this very design reproducing inside its own fix.
The `~/.lf-dev` default closes it; re-running the store suite now leaves the live
db untouched (verified: migration count unchanged).

**Still open, in order** (each is its own slice, none blocks this PR):

1. Move 5's remainder: `task run` finishes partial creation; BUSY vs UNIQUE split.
2. Move 6: one-writer lease — launch-gate liveness probe, generation reaping,
   harness process-group kill, no `self.child` overwrite. This is the W2-132
   two-writer race and is the largest remaining piece.
3. Move 7: base placement on `origin/main`; `lf pr open` range check.
4. Move 4: honest reads (`lf status` batched tmux probe).
5. The 4 live Task Sessions predate the pin, so they carry no execution context and
   will refuse to relaunch with an actionable message. That is correct and
   deliberate — but it means **they must be abandoned and re-run, not resumed.**

## PAUSED 2026-07-14 for the Loopflow 0.11 binary/database rollout (historical)

Paused at a durable boundary by the Wave. Worktree, branch, and provider history
preserved; no PR opened. `cargo build --all-targets`, `cargo clippy`, `cargo fmt`,
and all 948 lib tests are green at the pause commit — the resume starts from a
working tree, not a broken one.

**Why the pause matters to this Task specifically:** W2-130's whole subject is
which binary and which database a Session runs against. The 0.11 rollout changes
both underneath it. The pinned-context columns (`lf_bin`, `db_path`, `lf_home`)
record *absolute paths captured at Session creation* — so any Session created
before the rollout pins the pre-0.11 binary. That is correct behavior (the
Session reproduces the context it was born with), but it means **the three live
Task Sessions will keep relaunching the old `lf` until they are recreated.**
Verify that against the rollout before resuming; it may want a note in the
release, not a code change.

### The schema decision is now WRONG and must be redone on resume

Checked against `origin/main` during the pause, and this is the single most
important thing on this page.

`main` has already landed the migration rollout. The store no longer has one
editable baseline: it has **release-scoped migrations**
(`rust/loopflow/src/store/MIGRATIONS.md`, `migrations.rs`), ids of the form
`{major}.{minor}.{ordinal:03}`, and the baseline has been *renamed* to
`0.10.001_initial.sql`. The convention states one rule:

> **A shipped migration is never edited, renamed, or deleted.** Repair a shipped
> schema with a new forward migration.

`check_migrations.py` enforces it in CI *and* in `lf release check` / `lf release
run`, by diffing every migration against the last release tag.

This branch does the exact thing that rule forbids. Its base (`4fbc980f`) predates
the rollout, so it still has the old `001_initial.sql`, and W2-130 **edited it in
place** to add `lf_bin`/`db_path`/`lf_home`/`abandon_*`. The design's decision
"Edit the baseline, don't add a migration" was correct for the tree it was written
against and is now false. On resume it becomes:

- Rebase onto `origin/main` (needed anyway — see the contamination note below).
- Move the five columns into a **new forward migration** in the 0.11 namespace,
  authored with `uv run python scripts/new_migration.py session_execution_context`
  and registered in `MIGRATIONS` in id order.
- Leave `0.10.001_initial.sql` untouched. Delete the in-place edit entirely.
- Drop the "edit the baseline" decision from the design doc; it is superseded.

The recreate-the-database consequence I feared at the pause **evaporates** under
the new scheme — a forward migration is exactly how existing `loopflow.db` files
are supposed to gain columns. The rollout does not conflict with W2-130; it
*rescues* it.

### This branch is probably what is breaking the live registry right now

The wave thread reports `lf` opening an "incompatible `~/.lf/loopflow.db`" and
then claiming no owning Wave registry. That is very likely **this branch's doing**:
`~/.local/bin/lf` is a symlink to `/Users/jack/src/loopflow/local-bin/lf`, a
mutable binary in the repo. Any build of it carrying this branch's edited baseline
will fail `validate_baseline` against the real ledger and hard-fail every command
that opens the store — which is precisely the class of failure W2-130 exists to
kill, reproduced by W2-130.

Do not "repair" the store. Redoing the schema change as a forward migration
(above) removes the cause. Until then, `lf` built from this worktree must not be
pointed at `~/.lf`.

### Landed at the pause commit

- **Move 1 — pinned execution context.** `ChildExecutionContext {lf_bin, db_path,
  lf_home}` on both Session types + baseline columns; `resolve_pinned_lf_binary()`
  refuses a bare `lf`; both launch paths take argv[0] and `LF_DB_PATH`/`LF_HOME`
  from the Session, never from the calling process.
- **Move 3 (partial) — terminal intent.** `AbandonIntent` on both Sessions;
  `supervisor_restart_bar()` is the single gate, checked in `ChildSession::launch`
  and `wake_project_session`. `queue_command` rejects non-Abandon commands once
  abandonment is requested. Interrupt no longer launches a process in *any* form
  (bare or with replacement), and records an observable status + event when it
  lands on a Session whose process is already gone.
- **Failure 9 — delivered work does not restart.** `Submitted` + an open PR is now
  a supervisor restart bar (W2-129's generation 2 back into `task_clarify` with
  PR #878 already open). `LaunchIntent::ExplicitResume` keeps a human
  `lf task resume` able to answer review; only the supervisor is barred.
- **Move 5 (partial) — `BEGIN IMMEDIATE`** on all 17 child-session write
  transactions, fixing the un-waitable read→write upgrade behind
  `database is locked`.

### Next on resume, in order

1. Rebase onto `origin/main` and re-author the schema change as a **forward
   migration in the 0.11 namespace** (see above). This gates everything else, and
   it also clears the poisoned base and probably the live-registry breakage.
2. Write `abandon_intent` at Abandon-queue time in the same transaction as the
   command — the column and every gate reading it exist, but the *writer* does
   not, so the intent gate is currently dead code. **This is the one piece that
   makes moves 3 and the failure-9 gate actually fire; do it first after (1).**
3. Move 5's remainder: `run` finishes partial creation; BUSY vs UNIQUE split.
4. Move 6: one-writer lease (launch-gate liveness probe, generation reaping,
   harness process-group kill + no `self.child` overwrite).
5. Move 7: base placement on `origin/main`; `lf pr open` range check.
6. Move 4: honest reads (`lf status` batched tmux probe).
7. The regression suite in `tests/session_recovery.rs`, including the W2-129
   event sequence: `pull_request_opened #878` → `submitted` → a supervisor wake
   must NOT produce generation 2 / `running`.

## Assumed, proceeding

- **`lf commit: wave_pursue` committed to canonical `main`.** That commit
  (`4fbc980f`, `wave/infrastructure/MEMORY.md`) is the source of the task-base
  contamination in failure 8, and it contradicts `ensure_clean_main`'s own error
  text ("Wave and Project turns never edit repository files"). W2-130 makes it
  *harmless* by basing Tasks on `origin/main`; it does not stop a wave turn writing
  to `main`. **Assumption:** that is a separate task under Loopflow API, not W2-130
  scope. Folding it in would double the blast radius of a recovery change.

- **This branch is itself contaminated.** `jack-heart/w2-130`'s base is `4fbc980f`,
  which is in no remote branch but `origin/jack-heart/w2-132`. It must be rebased
  onto `origin/main` before the PR opens, or W2-130 ships the bug it fixes.

- **Debug-build guard (move 2) is a hard error.** If it turns out to break tests that
  rely on the default `~/.lf/loopflow.db` path, it downgrades to a `lf doctor` check
  rather than blocking the PR.
