-- Typed CI control belongs to the exact Run that services the incident.
-- The command id was an intermediate wake ledger; current incident evidence
-- plus one active Run slot now selects and fences repair directly.
ALTER TABLE ci_incidents ADD COLUMN claimed_run_id TEXT REFERENCES runs(id) ON DELETE RESTRICT;
UPDATE ci_incidents
SET claimed_run_id = (
    SELECT r.id
    FROM child_commands cc
    JOIN runs r
      ON r.source_kind=cc.target_kind
     AND r.source_id=cc.session_id
     AND r.lease_generation=cc.claimed_by_generation
    WHERE cc.id=ci_incidents.trigger_command_id
    ORDER BY r.created_at DESC
    LIMIT 1
)
WHERE trigger_command_id IS NOT NULL;
ALTER TABLE ci_incidents DROP COLUMN trigger_command_id;
CREATE INDEX idx_ci_incidents_run ON ci_incidents(claimed_run_id, updated_at);
