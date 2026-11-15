-- Every turn and tool boundary reads its own Session's steers. Each lookup
-- below scanned its table on a Machine with ten thousand batches.
-- A Session's reconcile asks whether another Session was already handed a
-- shared steer; the primary key leads with session_id, so look up by steer.
CREATE INDEX steer_deliveries_steer ON steer_deliveries(steer_id);
-- Cancelling or excerpting a batch finds its deliveries.
CREATE INDEX steer_deliveries_batch ON steer_deliveries(batch_id);
-- Confirmation and recovery find a Session's prepared and uncertain batches.
CREATE INDEX steer_batches_session ON steer_batches(session_id, evidence, created_at);
-- Reconcile finds the steers saved for the Session, its Tasks, its Waves and
-- its repository. A steer has exactly one owner, so each owner column is
-- mostly NULL; ANALYZE counts those NULLs as one key and rates a full index
-- useless. Partial indexes hold only the owned rows, so the planner keeps
-- them whether or not the store was analyzed.
CREATE INDEX steers_session ON steers(session_id) WHERE session_id IS NOT NULL;
CREATE INDEX steers_repo ON steers(repo) WHERE repo IS NOT NULL;
DROP INDEX steers_task;
CREATE INDEX steers_task ON steers(task_id, seq) WHERE task_id IS NOT NULL;
DROP INDEX steers_wave;
CREATE INDEX steers_wave ON steers(wave_id, seq) WHERE wave_id IS NOT NULL;
