-- A Wave's or repository's new Tasks may take a machine from a pool or an
-- on-demand (one namespace) instead of one named Machine. A scope sets one or
-- the other; the nearest scope that sets either wins.
ALTER TABLE scope_defaults ADD COLUMN tasks_capacity TEXT
    CHECK (tasks_capacity IS NULL OR tasks_machine_id IS NULL);

-- 0.14.16 rented RunPod pods from a template naming an image and recorded
-- hardware as a tagged CPU flavor or GPU; no current reader parses either, so
-- those rows go. Their pods' guardians already end them at their deadlines.
DELETE FROM rentals WHERE provider='runpod' AND json_extract(hardware,'$.kind') IS NOT NULL;
DELETE FROM ondemands WHERE provider='runpod' AND json_extract(template,'$.image') IS NOT NULL;
