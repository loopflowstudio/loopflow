-- Released metric evidence: two observations at the same source time.
INSERT INTO waves(id,name,repo,created_at) VALUES('CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC','Evidence Wave','/repo',1);
INSERT INTO metric_instruments VALUES('CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC','Case-Sensitive-Metric','retained-producer',1);
INSERT INTO metric_observations VALUES('f7a9c1b27437461a32749febfdf849f50a8e4061dad6b3d809c9e5170a979b24','CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC','Case-Sensitive-Metric','retained-revision','retained-producer',1792195200,0,1792195200,1,'{"kind":"observed","identity":{"wave_id":"CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC","metric_id":"Case-Sensitive-Metric"},"contract_revision":"retained-revision","instrument":"retained-producer","observation_id":"f7a9c1b27437461a32749febfdf849f50a8e4061dad6b3d809c9e5170a979b24","value":1.0,"source_window_start":"2026-10-10T00:00:00Z","source_window_end":"2026-10-17T00:00:00Z","complete":true}');
INSERT INTO metric_observations VALUES('e2daa5ef483874728aa623cb0e79c2235f61c6df2ff4340a5bafc17ca33a47b7','CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC','Case-Sensitive-Metric','retained-revision','retained-producer',1792195200,0,1792195200,0,'{"kind":"observed","identity":{"wave_id":"CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC","metric_id":"Case-Sensitive-Metric"},"contract_revision":"retained-revision","instrument":"retained-producer","observation_id":"e2daa5ef483874728aa623cb0e79c2235f61c6df2ff4340a5bafc17ca33a47b7","value":2.0,"source_window_start":"2026-10-10T00:00:00Z","source_window_end":"2026-10-17T00:00:00Z","complete":false}');
UPDATE agent_sessions SET wave_id='CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC'
WHERE id='SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA';
INSERT INTO session_events(session_id,kind,receipt_key,wave_id,observed_at,payload)
VALUES('SESSION_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA','captured','run_Opaque-Capture',
    'CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC',1,
    '{"artifact_key":"run_Opaque-Capture","unknown_id":"CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC"}');
