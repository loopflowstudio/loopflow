-- Rename the authority, retaining opaque IDs and all placement/history bytes.
ALTER TABLE homes RENAME TO machines;
ALTER TABLE work_placements RENAME COLUMN home_id TO machine_id;
ALTER TABLE pr_landings RENAME COLUMN supervisor_home_id TO supervisor_machine_id;

DROP INDEX idx_homes_route;
CREATE UNIQUE INDEX idx_machines_route ON machines(route);
DROP INDEX idx_work_placements_home;
CREATE INDEX idx_work_placements_machine ON work_placements(machine_id, placed_at);

-- The retired supervisor discriminator and JSON remain historical evidence.
-- Neither spelling may acquire remote supervision authority.
DROP TRIGGER pr_landings_no_home_insert;
DROP TRIGGER pr_landings_no_home_update;
CREATE TRIGGER pr_landings_no_machine_insert
BEFORE INSERT ON pr_landings WHEN NEW.supervisor_placement='home'
BEGIN
    SELECT RAISE(ABORT, 'Remote machine landing supervision is retired');
END;
CREATE TRIGGER pr_landings_no_machine_update
BEFORE UPDATE OF supervisor_placement ON pr_landings WHEN NEW.supervisor_placement='home'
BEGIN
    SELECT RAISE(ABORT, 'Remote machine landing supervision is retired');
END;
