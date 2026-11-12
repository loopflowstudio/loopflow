-- Every local planning write takes the next Lamport clock while holding the
-- write lock, once per changed field. `clock` is a virtual column over the
-- mutation's JSON, so max(clock) parsed all 36,000 changes on each call.
CREATE INDEX planning_peer_changes_clock ON planning_peer_changes(clock);
-- A held terminal's lf processes, read by retirement and owner retention.
-- Fewer than one Process in a thousand names a terminal.
CREATE INDEX processes_held_terminal ON processes(held_terminal_id)
    WHERE held_terminal_id IS NOT NULL;
-- The retirement sweep and every viewer poll read the unfinished retirements;
-- finished ones accumulate with every Task.
CREATE INDEX task_retirements_unfinished ON task_retirements(task_id)
    WHERE phase!='retired';
-- The sweep also finds abandoned Tasks that still have a checkout.
CREATE INDEX tasks_abandoned_checkout ON tasks(checkout_machine_id)
    WHERE abandoned_at IS NOT NULL AND worktree IS NOT NULL;
-- Deleting a Session event checks that no Process names it as its attachment
-- exit. Without an index that check scanned every Process for each event the
-- retention compaction removed, while holding the write lock.
CREATE INDEX processes_attachment_exit ON processes(attachment_exit_seq)
    WHERE attachment_exit_seq IS NOT NULL;
-- Doctor and process pruning look for agents whose parent was never
-- recorded: a self-rooted trace that names a parent. Almost none qualify.
CREATE INDEX processes_unrecorded_ancestry ON processes(id)
    WHERE kind='agent' AND trace_id=id AND parent_lf_process_id IS NOT NULL;
-- A provider's conversation id finds its Sessions: by thread, and in the
-- `provider-session:` observations. The expression matches the query's.
CREATE INDEX agent_sessions_provider_thread ON agent_sessions(provider_thread)
    WHERE provider_thread IS NOT NULL;
CREATE INDEX session_events_provider_session ON session_events(
    CASE WHEN json_valid(payload) THEN json_extract(payload,'$.evidence.provider_session_id') END)
    WHERE kind='observed' AND instr(receipt_key,':provider-session:')>0;
-- An input selector is a prefix of a capture's key or of its caller's key.
CREATE INDEX session_capture_caller ON session_events(
    CASE WHEN json_valid(payload) THEN json_extract(payload,'$.caller_key') END)
    WHERE kind='captured';
-- Foreign-key children: deleting a parent checks each of these, which
-- otherwise scans the child table.
CREATE INDEX session_events_task ON session_events(task_id) WHERE task_id IS NOT NULL;
CREATE INDEX session_events_wave ON session_events(wave_id) WHERE wave_id IS NOT NULL;
CREATE INDEX session_steers_steer ON session_steers(steer_id);
CREATE INDEX planning_peer_routes_mutation ON planning_peer_routes(mutation_id);
