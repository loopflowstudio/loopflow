-- ci_incident_repaired_head
-- Record the head a ci-fix body actually shipped for an incident.
--
-- Settlement judges head advancement against the authoritative remote head. When
-- the repair moved the head, the incident carries the repaired head so the
-- failure-to-response evidence names both the failed head and the head that
-- settled it. Written once (COALESCE), never overwritten by a later push.
ALTER TABLE ci_incidents ADD COLUMN repaired_head_sha TEXT;
