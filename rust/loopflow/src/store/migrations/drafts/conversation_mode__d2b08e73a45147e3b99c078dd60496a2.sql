-- name: conversation_mode
-- id: d2b08e73a45147e3b99c078dd60496a2
-- depends_on: record_execs

-- Existing Session rows were all interactive; prior headless Runs are imported
-- separately. Purpose and presentation mode no longer share one discriminator.
ALTER TABLE sessions ADD COLUMN interactive INTEGER NOT NULL DEFAULT 1
    CHECK (interactive IN (0, 1));
ALTER TABLE sessions ADD COLUMN purpose TEXT NOT NULL DEFAULT 'conversation'
    CHECK (purpose IN ('conversation', 'flow_review', 'ask'));
UPDATE sessions SET purpose=CASE kind WHEN 'interactive' THEN 'conversation' ELSE kind END;
ALTER TABLE sessions DROP COLUMN kind;
ALTER TABLE sessions RENAME COLUMN purpose TO kind;
CREATE INDEX session_inventory ON sessions(interactive, completed_at, title, id);

ALTER TABLE sessions ADD COLUMN repo TEXT;
UPDATE sessions SET repo=(SELECT w.repo FROM runs r JOIN waves w ON w.id=r.wave_id
    WHERE r.id=sessions.current_run_id);
CREATE INDEX session_repo_inventory ON sessions(repo, interactive, completed_at, title, id);
