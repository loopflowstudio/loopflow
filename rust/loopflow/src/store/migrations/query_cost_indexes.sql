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
