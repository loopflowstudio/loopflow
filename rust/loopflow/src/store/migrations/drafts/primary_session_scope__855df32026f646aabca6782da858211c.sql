-- name: primary_session_scope
-- id: 855df32026f646aabca6782da858211c
-- depends_on: 

-- A primary Session is an ordinary conversation that is the one ongoing
-- conversation of its scope. The scope's identity stays in the row's own
-- columns; completed predecessors remain history.
ALTER TABLE agent_sessions ADD COLUMN primary_scope TEXT CHECK (primary_scope IN ('repository', 'wave', 'task'));
CREATE UNIQUE INDEX agent_sessions_primary_wave ON agent_sessions(wave_id)
    WHERE primary_scope = 'wave' AND completed_at IS NULL;
