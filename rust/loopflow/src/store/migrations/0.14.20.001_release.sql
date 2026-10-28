-- draft: task_graphs
-- A Task's Graph, one row per Task: the graph it took up, compiled then and
-- never edited, and where the Task stands on it. `step` is `start`, `end`, or
-- the interactive step it waits at, where a person takes part in the Task
-- conversation. While `lf task run` carries it, `run_start` is the cursor that
-- run began from and `lf_process_id` that Task run; whether it still runs is
-- that Process's. Taking up a graph replaces the row. Other Graphs in the
-- Task's checkout never touch it.
CREATE TABLE task_graphs (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    graph TEXT NOT NULL CHECK (json_valid(graph)),
    step TEXT NOT NULL,
    run_start TEXT CHECK (run_start IS NULL OR json_valid(run_start)),
    lf_process_id TEXT REFERENCES processes(id),
    updated_at INTEGER NOT NULL,
    CHECK ((run_start IS NULL) = (lf_process_id IS NULL))
) STRICT;
-- Process forgetting probes this reference on every exit.
CREATE INDEX task_graphs_process ON task_graphs(lf_process_id)
    WHERE lf_process_id IS NOT NULL;
INSERT INTO task_graphs(task_id,graph,step,updated_at)
SELECT task_id,graph,node,updated_at FROM task_workflows;
DROP TABLE task_workflows;
-- State deliveries named the Workflow move that caused them; the column goes
-- with the history, so the table is rebuilt with its index and triggers.
CREATE TABLE task_state_deliveries_graphs (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    target TEXT NOT NULL CHECK(target IN ('completed','unstarted','started','canceled')),
    base_revision TEXT,
    base_state TEXT,
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    settled INTEGER NOT NULL DEFAULT 0 CHECK(settled IN (0,1)),
    error TEXT,
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json))
);
INSERT INTO task_state_deliveries_graphs(seq,id,task_id,target,base_revision,base_state,attempted,settled,error,conflict_json)
SELECT seq,id,task_id,target,base_revision,base_state,attempted,settled,error,conflict_json FROM task_state_deliveries;
DROP TABLE task_state_deliveries;
ALTER TABLE task_state_deliveries_graphs RENAME TO task_state_deliveries;
CREATE INDEX task_state_deliveries_task ON task_state_deliveries(task_id,seq);
CREATE TRIGGER store_revision_task_state_deliveries_insert AFTER INSERT ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_state_deliveries_update AFTER UPDATE ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_state_deliveries_delete AFTER DELETE ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER canonical_task_state_deliveries_insert BEFORE INSERT ON task_state_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_state_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;
CREATE TRIGGER canonical_task_state_deliveries_update BEFORE UPDATE OF id,task_id ON task_state_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_state_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

-- Graph processes and Task events keep the history; moves had no reader left.
DROP TABLE task_workflow_moves;
CREATE TRIGGER store_revision_task_graphs_insert AFTER INSERT ON task_graphs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_graphs_update AFTER UPDATE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_graphs_delete AFTER DELETE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER task_graph_is_fixed BEFORE UPDATE OF graph ON task_graphs BEGIN
    SELECT RAISE(ABORT,'A Task Graph is fixed when taken up; take up another to replace it');
END;

-- A Wave's default is a graph like any other.
ALTER TABLE waves RENAME COLUMN workflow TO graph;
