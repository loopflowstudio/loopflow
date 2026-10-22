-- Flow is renamed Graph (LOO-551). Tables, columns, index, triggers and the
-- revision domain follow; stored Workflow edges and Wave workflow YAML name the
-- Graph each edge runs as `graph`.

DROP TRIGGER store_revision_flow_processes_insert;
DROP TRIGGER store_revision_flow_processes_update;
DROP TRIGGER store_revision_flow_processes_delete;
DROP TRIGGER store_revision_flow_process_steps_insert;
DROP TRIGGER store_revision_flow_process_steps_update;
DROP TRIGGER store_revision_flow_process_steps_delete;
DROP TRIGGER validate_flow_process_step;
DROP TRIGGER flow_processes_are_append_only;
DROP TRIGGER flow_process_steps_are_append_only;
DROP INDEX flow_process_steps_flow;

ALTER TABLE flow_processes RENAME TO graph_processes;
ALTER TABLE graph_processes RENAME COLUMN graph TO compiled;
ALTER TABLE graph_processes RENAME COLUMN flow TO graph;
ALTER TABLE flow_process_steps RENAME TO graph_process_steps;
ALTER TABLE graph_process_steps RENAME COLUMN flow_lf_process_id TO graph_lf_process_id;

CREATE INDEX graph_process_steps_graph ON graph_process_steps(graph_lf_process_id, seq);

CREATE TRIGGER validate_graph_process_step BEFORE INSERT ON graph_process_steps BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM processes step
        WHERE step.id=NEW.lf_process_id AND step.parent_lf_process_id=NEW.graph_lf_process_id
    ) THEN RAISE(ABORT,'A Graph step is a process its Graph process started') END;
END;
CREATE TRIGGER graph_processes_are_append_only BEFORE UPDATE ON graph_processes BEGIN
    SELECT RAISE(ABORT,'A Graph record is append-only');
END;
CREATE TRIGGER graph_process_steps_are_append_only BEFORE UPDATE ON graph_process_steps BEGIN
    SELECT RAISE(ABORT,'A Graph record is append-only');
END;

CREATE TEMP TABLE store_revisions_before AS SELECT domain, revision FROM store_revisions;
DROP TABLE store_revisions;
CREATE TABLE store_revisions (
    domain TEXT PRIMARY KEY NOT NULL CHECK (domain IN ('planning', 'sessions', 'graphs', 'processes', 'usage')),
    revision INTEGER NOT NULL
) WITHOUT ROWID;
INSERT INTO store_revisions(domain, revision)
SELECT CASE domain WHEN 'flows' THEN 'graphs' ELSE domain END, revision FROM store_revisions_before;
DROP TABLE store_revisions_before;

CREATE TRIGGER store_revision_graph_processes_insert AFTER INSERT ON graph_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_processes_update AFTER UPDATE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_processes_delete AFTER DELETE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_process_steps_insert AFTER INSERT ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_process_steps_update AFTER UPDATE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_process_steps_delete AFTER DELETE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;

DROP TRIGGER task_workflow_graph_is_fixed;
UPDATE task_workflows SET graph = json_set(graph, '$.edges', (
    SELECT json_group_array(
        json_remove(json_set(edge.value, '$.graph', json_extract(edge.value, '$.flow')), '$.flow')
        ORDER BY edge.key)
    FROM json_each(task_workflows.graph, '$.edges') edge))
WHERE json_type(graph, '$.edges') = 'array';
CREATE TRIGGER task_workflow_graph_is_fixed BEFORE UPDATE OF graph ON task_workflows BEGIN
    SELECT RAISE(ABORT,'A Workflow graph is fixed when taken up');
END;

UPDATE wave_workflows
SET content = replace(replace(content, ' flow:', ' graph:'), '{flow:', '{graph:')
WHERE content LIKE '% flow:%' OR content LIKE '%{flow:%';
