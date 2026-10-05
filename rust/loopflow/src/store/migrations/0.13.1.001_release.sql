-- draft: retire_home_landing_supervisors
-- Retain obsolete authority on its landing, without treating a Home PID as local.
ALTER TABLE pr_landings ADD COLUMN retired_supervisor_json TEXT
    CHECK (retired_supervisor_json IS NULL OR json_valid(retired_supervisor_json));

UPDATE pr_landings
SET retired_supervisor_json=json_object(
        'placement', supervisor_placement,
        'home_id', supervisor_home_id,
        'process_id', supervisor_process_id,
        'heartbeat_at', supervisor_heartbeat_at,
        'generation', generation),
    generation=generation+1,
    supervisor_placement=NULL,
    supervisor_home_id=NULL,
    supervisor_process_id=NULL,
    supervisor_heartbeat_at=NULL
WHERE supervisor_placement='home';

-- An older executable must not acquire a fresh Home claim after retirement.
CREATE TRIGGER pr_landings_no_home_insert
BEFORE INSERT ON pr_landings WHEN NEW.supervisor_placement='home'
BEGIN
    SELECT RAISE(ABORT, 'Home landing supervision is retired');
END;

CREATE TRIGGER pr_landings_no_home_update
BEFORE UPDATE OF supervisor_placement ON pr_landings WHEN NEW.supervisor_placement='home'
BEGIN
    SELECT RAISE(ABORT, 'Home landing supervision is retired');
END;
