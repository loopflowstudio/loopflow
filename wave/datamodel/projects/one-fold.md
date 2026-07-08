# One fold

One canonical wave fold/query path. Audited 2026-07-08: the old M1/M2 gap
list is closed — `stream_events` single-owner (wave/subscription.rs:65),
`find_repo_root` one impl + shim, worktree naming one constructor
(engine/worktrees.rs:140), container-mode has no live path. What remains is
the real bet:

## KRs

- The two independent read models become one: today the journal fold
  (`fold_thread`/`fold_workers`, feeds the wave server) and the SQLite store
  interpretation (`lf ls`/`status`/`runs` + lfd HTTP routes via
  `WaveStateStore`) never share a fold. Concerto, CLI, and server read the
  same projection.
- Inert container residue swept when touched: redaction patterns
  (lfd/redaction.rs:78), dead `container_id` migration columns.
