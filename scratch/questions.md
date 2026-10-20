# Open questions — LOO-521

- `task_workflows` is written with `INSERT OR REPLACE` (`store/sqlite/task_work.rs`).
  If a release adds a column there, an older writer's replace drops it. Documented
  in MIGRATIONS.md; not converted to an upsert here.
- Reader floors only help binaries that contain this change. lf ≤ 0.14.8 still
  refuses any newer store regardless of floor.
- New views and triggers count as breaking even when harmless; the design lists
  only tables, indexes and nullable/defaulted columns as additive.
