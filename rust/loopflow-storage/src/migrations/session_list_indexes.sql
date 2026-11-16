CREATE INDEX tasks_checkout_root ON tasks(checkout_machine_id, rtrim(worktree,'/'))
    WHERE worktree!='';
CREATE INDEX tasks_primary_session ON tasks(primary_session_id)
    WHERE primary_session_id IS NOT NULL;
CREATE INDEX agent_process_unfinished ON processes(id)
    WHERE kind='agent' AND completed_at IS NULL;
CREATE INDEX processes_cwd ON processes(cwd);
