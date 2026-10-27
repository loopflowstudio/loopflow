-- A Wave's or repository's new Tasks may take a machine from a pool or an
-- on-demand (one namespace) instead of one named Machine. A scope sets one or
-- the other; the nearest scope that sets either wins.
ALTER TABLE scope_defaults ADD COLUMN tasks_capacity TEXT
    CHECK (tasks_capacity IS NULL OR tasks_machine_id IS NULL);

-- Daytona and Modal sandboxes replace RunPod pods, which no published release
-- could rent.
DELETE FROM rentals WHERE provider='runpod';
DELETE FROM ondemands WHERE provider='runpod';
