-- Every turn and tool boundary reads its own Session's steers. Each lookup
-- below scanned its table on a Machine with ten thousand batches.
-- A Session's reconcile asks whether another Session was already handed a
-- shared steer; the primary key leads with session_id, so look up by steer.
CREATE INDEX steer_deliveries_steer ON steer_deliveries(steer_id);
-- Cancelling or excerpting a batch finds its deliveries.
CREATE INDEX steer_deliveries_batch ON steer_deliveries(batch_id);
-- Confirmation and recovery find a Session's prepared and uncertain batches.
CREATE INDEX steer_batches_session ON steer_batches(session_id, evidence, created_at);
-- Reconcile finds the steers saved for the Session itself and its repository;
-- Task and Wave steers already have their indexes.
CREATE INDEX steers_session ON steers(session_id);
CREATE INDEX steers_repo ON steers(repo);
