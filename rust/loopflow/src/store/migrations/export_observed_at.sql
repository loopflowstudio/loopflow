-- When a pass last looked in the provider for an uncertain creation (attempted,
-- outcome unknown). Such creations are looked for once per budget window; a
-- creation's first attempt is never stamped, so a timed-out send gets one look
-- on the next pass.
ALTER TABLE task_creation_intents ADD COLUMN export_observed_at INTEGER;
ALTER TABLE project_transitions ADD COLUMN export_observed_at INTEGER;
