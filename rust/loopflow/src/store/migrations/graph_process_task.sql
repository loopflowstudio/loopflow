-- A Graph records the Task it started for, once, in its launch row. Reading a
-- Task's Graphs then probes an index instead of walking every Graph's
-- directory ancestors and Session links.
ALTER TABLE graph_processes ADD COLUMN task_id TEXT REFERENCES tasks(id);

-- Earlier Graphs take the Task the read-time rule gave them: a relocation
-- receipt, else the deepest local checkout holding the Graph's directory, else
-- the Task bound to a Session the Graph's Process took part in. Materialized
-- first: a correlated UPDATE repeats the ancestor walk for every row.
CREATE TEMP TABLE graph_process_tasks (
    graph TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL
) WITHOUT ROWID;
INSERT INTO graph_process_tasks(graph, task_id)
WITH RECURSIVE
    f AS MATERIALIZED (SELECT e.id, e.cwd FROM graph_processes g JOIN processes e ON e.id=g.lf_process_id),
    up(graph, path) AS (SELECT id, cwd FROM f
        UNION ALL SELECT graph, rtrim(rtrim(path, replace(path,'/','')),'/') FROM up WHERE path!=''),
    checkouts AS MATERIALIZED (SELECT id, rtrim(worktree,'/') AS root FROM tasks
        WHERE checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) AND worktree!=''),
    links(session_id, graph) AS (
        SELECT se.session_id, se.lf_process_id FROM session_events se INDEXED BY session_events_process
            WHERE se.lf_process_id IN (SELECT id FROM f)
        UNION SELECT p.agent_session_id, p.attached_lf_process_id FROM processes p
            WHERE p.kind='agent' AND p.attached_lf_process_id IN (SELECT id FROM f) AND p.agent_session_id IS NOT NULL
        UNION SELECT p.agent_session_id, p.id FROM processes p
            WHERE p.kind='agent' AND p.id IN (SELECT id FROM f) AND p.agent_session_id IS NOT NULL),
    candidates(graph, task_id, rank, depth) AS (
        SELECT r.process_id, r.task_id, 0, 0 FROM relocated_task_processes r
            WHERE r.process_id IN (SELECT id FROM f)
        UNION ALL SELECT up.graph, c.id, 1, length(c.root) FROM up JOIN checkouts c ON c.root=up.path
        UNION ALL SELECT l.graph, a.task_id, 2, 0 FROM links l JOIN agent_sessions a ON a.id=l.session_id
            WHERE a.primary_scope IS NULL AND a.task_id IS NOT NULL)
SELECT graph, task_id FROM (
    SELECT graph, task_id, row_number() OVER (PARTITION BY graph ORDER BY rank, depth DESC, task_id) AS n
    FROM candidates)
WHERE n=1;

DROP TRIGGER graph_processes_are_append_only;
UPDATE graph_processes SET task_id=m.task_id
FROM graph_process_tasks m WHERE m.graph=graph_processes.lf_process_id;
CREATE TRIGGER graph_processes_are_append_only BEFORE UPDATE ON graph_processes BEGIN
    SELECT RAISE(ABORT,'A Graph record is append-only');
END;
DROP TABLE graph_process_tasks;

CREATE INDEX graph_processes_task ON graph_processes(task_id) WHERE task_id IS NOT NULL;
