-- name: record_flow_repository
-- id: 9a25318e934745ca8b87d6a57eb4e54c
-- depends_on: capture_session_events

-- Bound repository identity stays on Wave. Historical taskless captures lack
-- a recorded canonical repository; never turn their cwd into identity at read time.
ALTER TABLE flow_sessions ADD COLUMN unbound_repo TEXT;
CREATE TRIGGER validate_flow_repository_insert BEFORE INSERT ON flow_sessions
WHEN NEW.unbound_repo IS NOT NULL AND NEW.wave_id IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'Bound Flow repository belongs to its Wave'); END;
CREATE TRIGGER validate_flow_repository_update BEFORE UPDATE OF wave_id,unbound_repo ON flow_sessions
WHEN NEW.unbound_repo IS NOT NULL AND NEW.wave_id IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'Bound Flow repository belongs to its Wave'); END;
