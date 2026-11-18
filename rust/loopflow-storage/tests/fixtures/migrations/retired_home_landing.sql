INSERT INTO pr_landings (
    id, repo, pr_number, worktree, branch, requested_head_sha,
    observed_head_sha, merge_commit, state, generation,
    supervisor_placement, supervisor_home_id, supervisor_process_id,
    supervisor_heartbeat_at, blocked_reason, created_at, updated_at,
    task_id, after_merge, next_slug
) VALUES (
    ?1, 'owner/repo', ?2, '/tmp/landing', 'repair', 'requested',
    'observed', ?3, ?4, 7, ?5, ?6, 123, 30, ?7, 10, 40,
    'landing_task', 'continue_task', 'follow-up'
);
