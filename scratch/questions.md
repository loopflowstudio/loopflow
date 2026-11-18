# Open choices (made headless, for review)

- Config spelling: `tracker: {linear: {team}}` (externally tagged) chosen over `tracker: linear` + sibling `linear: {team}`.
- `lf repo tracker` chosen over `lf repo sync`; no alias for `lf repo plan`.
- Wire: `linear_id` → `tracker_id`, `acquisition` → `tracker_state {state, synced_at}`; LOO-942 folded in.
- Table renames staged with bidirectional mirror triggers instead of views (old code upserts; SQLite refuses UPSERT on views).
- Left alone: `planning_*` / `pm_snapshot_synced_at` columns on `tasks`/`projects` (each rename is a staged rebuild of a hot table); `planning_user`; plan-writer modules `planning_write`/`planning_order`/`planning_reads`/`planning_sync`.
- Wave Project commands: `lf wave edit-plan`/`update-plan` → `edit-project`/`update-project` (`--plan` → `--file`); `lf wave plan` and its `bind`/`connect`/`reteam` keep "plan". Passed over: one `lf wave project edit|update|bind` group (moves bind out of the plan group for little gain).
- `LinearIssueEvidence` → `LinearIssueState` (concept review).
- Authored Wave files still say "chapter" and are left as written: `.lf/wave/growth/MEMORY.md:4`, `.lf/wave/infrastructure/GOAL.md:38,49`, `.lf/wave/infrastructure/MEMORY.md:653`, and GOAL/MEMORY of intelligence, product. GOAL.md lines are read by agents; rewording them is their author's call.
- Served routes follow the CLI: `repo.plan` → `repo.tracker`, `plan.answer` → `repo.tracker.answer` (write-docs; `rust/loopflow/src/server/README.md`, `docs/dev/planning.md`).
- `lf repo tracker` output keeps today's "Tracker: linear. 3 open · 1 lost." shape in `docs/work.md`; the plan's sketch (`tracker  linear (team …)` columns) was not adopted.
- Historical journal seeding keeps its original upgrade points through migration-local views. This avoids rescanning every Task when an already-seeded store changes table names.
- Released and candidate processes must lock the same `chapter-locks` directory before changing a Wave. Keep that path and the ignore/scan exclusions for historical `.lf/chapters` directories. The OAuth experiment in `tests/e2e/linear_oauth.py` seeds the retired `wave_chapters` and `pm_snapshots` model and asserts its old CLI output. Porting that Linux experiment needs a new fixture; a text substitution cannot update it.
- Task creation retries read serialized Tasks from SQLite. Store their released binding key and translate it on receipt reads so either CLI can retry the other’s request. The public wire uses `tracker_id` throughout.
