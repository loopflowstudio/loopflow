-- name: drop_flow_turn_caller
-- id: 0ccaa0f715a7407b846b1236e293ef68
-- depends_on: fold_flow_passes

-- Retain any historical caller token before deleting its unused runtime copy.
-- Selected Flow event payloads stay byte-for-byte unchanged.
INSERT INTO import_evidence(source,selector,payload)
SELECT 'exec_flow_turn',id,json_object('exec_id',id,'caller_flow_turn',caller_flow_turn)
FROM execs WHERE caller_flow_turn IS NOT NULL;
DROP INDEX flow_turn_caller;
ALTER TABLE execs DROP COLUMN caller_flow_turn;
