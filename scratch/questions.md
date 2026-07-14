# Open questions — W2-130

## PAUSED 2026-07-14 for the Loopflow 0.11 binary/database rollout

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

Also note: the schema change is an edit to the single baseline
(`001_initial.sql`), so every existing `loopflow.db` becomes incompatible and
hits the recreate path. That is by design and matches the store's own convention,
but it is a **destructive interaction with the 0.11 database rollout** and is the
first thing to reconcile on resume — not something to discover afterwards.

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

1. Reconcile the baseline-edit / recreate path with the 0.11 database rollout
   (above). This gates everything else.
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
