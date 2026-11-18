# Open choices (made headless, for review)

- Config spelling: `tracker: {linear: {team}}` (externally tagged) chosen over `tracker: linear` + sibling `linear: {team}`.
- `lf repo tracker` chosen over `lf repo sync`; no alias for `lf repo plan`.
- Wire: `linear_id` → `tracker_id`, `acquisition` → `tracker_state {state, synced_at}`; LOO-942 folded in.
- Table renames staged with bidirectional mirror triggers instead of views (old code upserts; SQLite refuses UPSERT on views).
- Left alone: `planning_*` / `pm_snapshot_synced_at` columns on `tasks`/`projects` (each rename is a staged rebuild of a hot table); `planning_user`; plan-writer modules `planning_write`/`planning_order`/`planning_reads`/`planning_sync`.
