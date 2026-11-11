-- A Session's reconcile asks whether another Session was already handed a
-- shared steer; the primary key leads with session_id, so look up by steer.
CREATE INDEX steer_deliveries_steer ON steer_deliveries(steer_id);
