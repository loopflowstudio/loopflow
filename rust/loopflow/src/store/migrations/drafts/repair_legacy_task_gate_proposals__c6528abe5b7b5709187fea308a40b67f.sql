-- name: repair_legacy_task_gate_proposals
-- id: c6528abe5b7b5709187fea308a40b67f
-- depends_on: 

-- TaskGateProposal replaced the Session-era terminal status with a boolean.
UPDATE tasks
SET gate_proposal_json = json_remove(
    json_set(
        gate_proposal_json,
        '$.done',
        json(
            CASE json_extract(gate_proposal_json, '$.status')
                WHEN 'completed' THEN 'true'
                WHEN 'waiting' THEN 'false'
                WHEN 'blocked' THEN 'false'
                WHEN 'failed' THEN 'false'
                WHEN 'abandoned' THEN 'false'
            END
        )
    ),
    '$.status'
)
WHERE gate_proposal_json IS NOT NULL
  AND json_type(gate_proposal_json, '$.done') IS NULL
  AND json_extract(gate_proposal_json, '$.status') IN (
    'completed', 'waiting', 'blocked', 'failed', 'abandoned'
);
