-- A Task's auto setting is one mark on its Graph: 'on' (`lf task auto on`,
-- `lf ship`, `lf -b task run`) or 'stopped' (`lf task auto off`), with when
-- and who (`Starter` JSON, resolved when it was set). NULL when nobody has set
-- it. `lf ship` is `lf task auto on`, then `lf task run`; no ship process
-- carries a Task, so its log and the shared handoff stamp leave.
ALTER TABLE task_graphs ADD COLUMN auto TEXT CHECK (auto IN ('on','stopped'));
ALTER TABLE task_graphs ADD COLUMN auto_at INTEGER;
ALTER TABLE task_graphs ADD COLUMN auto_by TEXT CHECK (auto_by IS NULL OR json_valid(auto_by));
-- A finished Task's last auto mark, in place of counting its log.
ALTER TABLE task_summaries ADD COLUMN auto TEXT CHECK (auto IN ('on','stopped'));

-- Each Task's latest approval or stop becomes its mark. A withdrawal (a
-- failure that ended shipping) reads as stopped.
UPDATE task_graphs SET (auto,auto_at,auto_by)=(
    SELECT CASE e.kind WHEN 'approved' THEN 'on' ELSE 'stopped' END,
        e.at,
        CASE WHEN e.caller_session_id IS NOT NULL THEN json_object(
                'kind','session','id',e.caller_session_id,
                'name',(SELECT NULLIF(title,'') FROM agent_sessions WHERE id=e.caller_session_id),
                'requester',e.person)
            ELSE json_object('kind','person','name',e.person) END
    FROM task_ship_events e
    WHERE e.task_id=task_graphs.task_id AND e.kind IN ('approved','stopped','withdrawn')
    ORDER BY e.id DESC LIMIT 1)
WHERE EXISTS(SELECT 1 FROM task_ship_events e
    WHERE e.task_id=task_graphs.task_id AND e.kind IN ('approved','stopped','withdrawn'));

-- A released lf still on this Machine reads a set `shipped_at` with no live
-- ship process as one to start again; cleared, it reads no Task as shipping.
UPDATE tasks SET shipped_at=NULL WHERE shipped_at IS NOT NULL;

-- leaving: task_ship_events LOO-971
-- leaving: tasks.shipped_at LOO-971
-- leaving: task_summaries.ship_events LOO-971
