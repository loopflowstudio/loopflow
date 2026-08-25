-- name: ci_repair_attempt
-- id: fd0958718b2a0e54c1564e3621f8a17f
-- depends_on: 
ALTER TABLE ci_incidents ADD COLUMN repair_evidence_urls_json TEXT
    CHECK (
        repair_evidence_urls_json IS NULL
        OR (
            json_valid(repair_evidence_urls_json)
            AND json_type(repair_evidence_urls_json) = 'array'
        )
    );
ALTER TABLE ci_incidents ADD COLUMN repair_evidence_sha256 TEXT
    CHECK (
        repair_evidence_sha256 IS NULL
        OR (
            length(repair_evidence_sha256) = 64
            AND repair_evidence_sha256 NOT GLOB '*[^0-9a-f]*'
        )
    );
-- Keep the incident receipt readable even if its independently durable capture
-- ledger is unavailable during recovery.
ALTER TABLE ci_incidents ADD COLUMN repair_invocation_id TEXT
    CHECK (
        repair_invocation_id IS NULL
        OR (
            length(repair_invocation_id) = 43
            AND substr(repair_invocation_id, 1, 11) = 'invocation_'
            AND substr(repair_invocation_id, 12) NOT GLOB '*[^0-9a-f]*'
        )
    );
ALTER TABLE ci_incidents ADD COLUMN repair_deadline_at INTEGER;
ALTER TABLE ci_incidents ADD COLUMN repair_finished_at INTEGER
    CHECK (
        (
            repair_evidence_urls_json IS NULL
            AND repair_evidence_sha256 IS NULL
            AND repair_invocation_id IS NULL
            AND repair_deadline_at IS NULL
            AND repair_finished_at IS NULL
        )
        OR (
            repair_evidence_urls_json IS NOT NULL
            AND repair_evidence_sha256 IS NOT NULL
            AND repair_invocation_id IS NOT NULL
            AND responded_at IS NOT NULL
            AND repair_deadline_at > responded_at
            AND (repair_finished_at IS NULL OR repair_finished_at >= responded_at)
            AND (
                repair_finished_at IS NULL
                OR repaired_head_sha IS NOT NULL
                OR blocked_at IS NOT NULL
                OR green_at IS NOT NULL
                OR merged_at IS NOT NULL
            )
        )
    );
