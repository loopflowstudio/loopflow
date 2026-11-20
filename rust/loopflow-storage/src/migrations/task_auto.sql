-- Auto is a setting on the Task: 'on' (`lf task auto on`, or `auto: on` in
-- configuration) or 'off' (`lf task auto off`), with when and who (`Starter`
-- JSON, resolved when it was set). A row is the mark; no row means nobody has
-- set it. A Task can hold one before it takes up a Graph. `lf task auto on`
-- turns it on, then runs the Task; no ship process carries a Task, so its log
-- and the shared handoff stamp leave.
CREATE TABLE task_auto (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK (state IN ('on','off')),
    set_at INTEGER NOT NULL,
    set_by TEXT NOT NULL CHECK (json_valid(set_by))
) STRICT;
-- A finished Task's last auto mark, in place of counting its log.
ALTER TABLE task_summaries ADD COLUMN auto TEXT CHECK (auto IN ('on','off'));

-- Each Task's latest approval or stop becomes its mark, whether or not it
-- took up a Graph. A withdrawal (a failure that ended shipping) reads as off.
INSERT INTO task_auto(task_id,state,set_at,set_by)
SELECT e.task_id,
    CASE e.kind WHEN 'approved' THEN 'on' ELSE 'off' END,
    e.at,
    CASE WHEN e.caller_session_id IS NOT NULL THEN json_object(
            'kind','session','id',e.caller_session_id,
            'name',(SELECT NULLIF(title,'') FROM agent_sessions WHERE id=e.caller_session_id),
            'requester',e.person)
        ELSE json_object('kind','person','name',e.person) END
FROM task_ship_events e
WHERE e.task_id IN (SELECT id FROM tasks)
    AND e.id=(SELECT max(l.id) FROM task_ship_events l
        WHERE l.task_id=e.task_id AND l.kind IN ('approved','stopped','withdrawn'));

CREATE TRIGGER store_revision_task_auto_insert AFTER INSERT ON task_auto
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_auto_update AFTER UPDATE ON task_auto
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_auto_delete AFTER DELETE ON task_auto
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- A released lf still on this Machine reads a set `shipped_at` with no live
-- ship process as one to start again; cleared, it reads no Task as shipping.
UPDATE tasks SET shipped_at=NULL WHERE shipped_at IS NOT NULL;

-- leaving: task_ship_events LOO-971
-- leaving: tasks.shipped_at LOO-971
-- leaving: task_summaries.ship_events LOO-971
