-- A headless run is history once its AgentProcess exits. Exits used to leave
-- Task, Wave and Graph runs open, so complete every headless Session with no
-- unfinished AgentProcess, at its last AgentProcess exit when one is recorded.
UPDATE agent_sessions SET completed_at=COALESCE(
    (SELECT p.completed_at FROM processes p WHERE p.id=agent_sessions.agent_process_id),
    created_at)
WHERE interactive=0 AND completed_at IS NULL
    AND id NOT IN(SELECT agent_session_id FROM processes INDEXED BY processes_unfinished
        WHERE completed_at IS NULL AND kind='agent' AND agent_session_id IS NOT NULL);
