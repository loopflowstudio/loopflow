-- A native request can stop shipping without being an OS Process. Preserve
-- the configured person's name at the stop; older unknown attribution stays unknown.
ALTER TABLE tasks ADD COLUMN ship_stopped_person TEXT;
