-- name: work_enablement
-- id: 08a5cbd6d0b05c4561d10e21c15c903a
-- depends_on: 

ALTER TABLE work_placements
ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1));
