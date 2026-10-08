-- depends_on: rename_home_to_machine
-- Connection removal preserves the machine and every historical reference.
ALTER TABLE machines ADD COLUMN label TEXT;
ALTER TABLE machines ADD COLUMN repo TEXT CHECK ((label IS NULL) = (repo IS NULL));
CREATE UNIQUE INDEX idx_machines_label ON machines(label) WHERE label IS NOT NULL;
DROP INDEX idx_machines_route;
CREATE UNIQUE INDEX idx_machines_route ON machines(route)
WHERE route='local' OR label IS NOT NULL;
