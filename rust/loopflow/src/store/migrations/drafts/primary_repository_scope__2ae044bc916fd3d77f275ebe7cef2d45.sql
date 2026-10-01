-- name: primary_repository_scope
-- id: 2ae044bc916fd3d77f275ebe7cef2d45
-- depends_on: primary_session_scope

-- A repository has one ongoing conversation of its own, with or without Waves.
CREATE UNIQUE INDEX agent_sessions_primary_repository ON agent_sessions(repo)
    WHERE primary_scope = 'repository' AND completed_at IS NULL;
