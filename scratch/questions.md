# Assumptions

- `PmRefresh::Auto` treats snapshots up to 15 minutes old as fresh, permits a
  cached fallback up to 24 hours old when refresh fails, and rejects older
  snapshots. The design requires bounded fresh/soft-stale/hard-stale behavior
  but does not specify durations; these fixed values avoid a new config knob.
- The existing Linear marker in `task start` is the idempotency receipt for the
  current provider API. If task creation commits but snapshot refresh fails, a
  retry finds the same marked issue before attempting another create.

# Compress-pass recovery (2026-07-10)

- The tree arrived non-compiling: the `lfd/executor` module had been replaced
  by `lfd/session_supervisor.rs` + `lfd/session_support.rs` and
  `engine/process.rs`, but the last import danglers (a `PathBuf` and a few
  moved-symbol paths) were never fixed. The compress pass finished that split,
  then collapsed the twice-copied active-run predicate onto
  `RunStatus::is_active()` and routed `wave::registry`'s private tmux probe to
  the canonical `engine::process::tmux_session_exists`. Lib (1059) + bin tests
  green, clippy clean.

# Rebase onto origin/main (2026-07-10)

- 33 commits replayed. Recurring conflict theme: main independently landed the
  native-hierarchy PM contract (bounded-staleness `load_show_snapshot`,
  `--sync`/`--no-sync`, `pm project archive`), while this branch carried older
  duplicates of the same work. Resolution: took **main's** PM code, docs, skills,
  and CLI flags wholesale wherever the branch only re-did what main already has
  (verified the branch tip added no unique symbols to those files first). This is
  the "rebase onto the landed PM contract" step `scratch/infrastructure.md`
  already called for.
- `wave/*/GOAL.md`: dropped the branch's re-added `pm.linear_project`; kept
  main's `pm.linear_initiative` (native hierarchy is the settled anchor).
- `wave/product/projects/wave-chat.md`: accepted main's deletion (Linear owns PM;
  `wave/*/projects/` is retired per `scratch/pm-linear-source.md`).
- `ops/pm.rs` `wave_pursue` commit: **combined** both sides — kept main's
  `PmProjectArchive*` types AND the branch's new `PmResolvedProject` /
  `pm_create_project` (both survive to the branch tip).
- Skill docs in `Benchmark Wave-to-Task steering`: took **theirs** (branch) —
  the branch deliberately reworded them for the Task Session model, matching the
  branch tip.
- Post-rebase compile fixes (task-session seam adapted to main's PM API, which
  lacks the branch's `PmSnapshotWrite`/`refresh_warning`):
  - `pm.rs`: `pm_show_async`/`pm_update_async` made `pub(crate)` for `task_pm`.
  - `task_pm.rs`: dropped `PmSnapshotWrite` handling — main's mutations refresh
    internally and return `Err` on failure. `TODO(product-pm)` left where the
    committed-but-refresh-pending distinction should return once the PM mutation
    API surfaces it.
  - `task.rs`: `pm_snapshot_warning` set to `None` at launch — main's
    `load_show_snapshot` logs the soft-stale fallback to progress but does not
    return it on `PmShowResult`. `TODO(product-pm)` to restore.
- Verified: `cargo build`/`clippy --all-targets -D warnings`/`fmt --check` clean;
  `cargo test -p loopflow` all green. Swift PM-read conflicts resolved to main's
  `definition`-shaped `WaveProject`; no branch Swift references the dropped
  `summary`/`refresh_warning`.

# Implement-pass concurrency (2026-07-10)

- Two pre-existing Codex processes had this worktree as their cwd while the
  steering slice was implemented. They contributed compatible review, docs,
  and migration-test edits during the pass. Those edits were preserved and
  reconciled; no git or rebase operation ran while the tree had multiple
  writers.
