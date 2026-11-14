-- One row per Session and steer says whether that Session read it. A hook
-- claims the row before it prints and stamps delivered_at after, so a hook
-- that died between the two leaves a claim that lapses and the steer is
-- printed again. This replaces session_steers, steer_batches and
-- steer_deliveries, which nothing reads after this migration; a later
-- contract migration drops them.
CREATE TABLE steer_delivered (
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    steer_id TEXT NOT NULL REFERENCES steers(id),
    claimed_at INTEGER NOT NULL,
    delivered_at INTEGER,
    PRIMARY KEY(session_id, steer_id)
);
-- A Session started later asks whether another Session already read a shared
-- steer; the primary key leads with session_id, so look up by steer.
CREATE INDEX steer_delivered_steer ON steer_delivered(steer_id);
-- What each Session was handed at its current attempt stays delivered.
INSERT INTO steer_delivered(session_id, steer_id, claimed_at, delivered_at)
SELECT d.session_id, d.steer_id, min(b.created_at), min(b.created_at)
FROM steer_deliveries d
JOIN steer_batches b ON b.id = d.batch_id
JOIN session_steers s ON s.session_id = d.session_id AND s.steer_id = d.steer_id
    AND s.attempt = d.attempt
WHERE b.evidence IN ('uncertain', 'emitted', 'confirmed')
GROUP BY d.session_id, d.steer_id;
-- A Task update is a steer lf writes when it sees the Task changed. The last
-- reading says what the Task looked like when its updates were last written,
-- so the next reader writes only what moved since.
CREATE TABLE task_update_readings (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id),
    observation TEXT NOT NULL
);
