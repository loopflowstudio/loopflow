-- draft: detached_owner_viewers
-- An execution owner's viewers: whether one is attached, and when the last one
-- left. Both stay NULL on owners from builds that never reported viewers.
ALTER TABLE processes ADD COLUMN viewed INTEGER CHECK(viewed IN (0,1));
ALTER TABLE processes ADD COLUMN detached_at INTEGER;

-- draft: rename_flow_to_graph
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

UPDATE task_events
SET kind_json = json_remove(
    json_set(kind_json, '$.kind', 'graph_finished', '$.graph', json_extract(kind_json, '$.flow')),
    '$.flow')
WHERE json_extract(kind_json, '$.kind') = 'flow_finished';

-- draft: saved_selections
CREATE TABLE dashboards (
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    expression_key TEXT NOT NULL,
    name TEXT,
    pinned INTEGER NOT NULL CHECK(pinned IN (0,1)),
    slots_json TEXT CHECK(slots_json IS NULL OR json_valid(slots_json)),
    PRIMARY KEY(repository_id, id),
    FOREIGN KEY(repository_id, id, expression_key)
        REFERENCES dashboard_arrangements(repository_id, dashboard_id, expression_key)
        DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE dashboard_arrangements (
    repository_id TEXT NOT NULL,
    dashboard_id TEXT NOT NULL,
    expression_key TEXT NOT NULL,
    selection_json TEXT NOT NULL CHECK(json_valid(selection_json)),
    arrangement_json TEXT NOT NULL CHECK(json_valid(arrangement_json)),
    PRIMARY KEY(repository_id, dashboard_id, expression_key),
    FOREIGN KEY(repository_id, dashboard_id) REFERENCES dashboards(repository_id,id) ON DELETE CASCADE
);
CREATE TABLE work_pane_layouts (
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    kind TEXT NOT NULL CHECK(kind IN ('repository','wave','task')),
    work_id TEXT NOT NULL,
    layout_json TEXT NOT NULL CHECK(json_valid(layout_json))
        CHECK((json_extract(layout_json, '$.revision') > 0) IS TRUE),
    PRIMARY KEY(repository_id, kind, work_id)
);
CREATE TABLE task_orders (
    repository_id TEXT PRIMARY KEY REFERENCES repository_plans(id),
    task_ids_json TEXT NOT NULL CHECK(json_valid(task_ids_json))
);
CREATE TRIGGER store_revision_dashboards_insert AFTER INSERT ON dashboards BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_dashboards_update AFTER UPDATE ON dashboards BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_dashboards_delete AFTER DELETE ON dashboards BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_dashboard_arrangements_insert AFTER INSERT ON dashboard_arrangements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_dashboard_arrangements_update AFTER UPDATE ON dashboard_arrangements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_dashboard_arrangements_delete AFTER DELETE ON dashboard_arrangements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_work_pane_layouts_insert AFTER INSERT ON work_pane_layouts BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_work_pane_layouts_update AFTER UPDATE ON work_pane_layouts BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_work_pane_layouts_delete AFTER DELETE ON work_pane_layouts BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_task_orders_insert AFTER INSERT ON task_orders BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_task_orders_update AFTER UPDATE ON task_orders BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_task_orders_delete AFTER DELETE ON task_orders BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;

-- draft: two_person_gaps
-- Wave Workflow definitions live only in committed `.lf/workflows/` files and
-- builtins, so every teammate reads the same definition. A Wave keeps only the
-- selected name (`waves.workflow`). Dropping the table drops its triggers.
DROP TABLE wave_workflows;

-- An ownership entry names its person by their key (lowercase Git
-- `user.email`) and carries the claimant Machine's public name, if any, so
-- teammates can read where it is. Names stay display only.
ALTER TABLE task_ownership ADD COLUMN person_key TEXT;
ALTER TABLE task_ownership ADD COLUMN machine_label TEXT;
DROP TRIGGER task_ownership_append_only;
CREATE TRIGGER task_ownership_append_only BEFORE UPDATE ON task_ownership
WHEN NOT (OLD.ordered_at IS NULL AND NEW.ordered_at IS NOT NULL
    AND NEW.id IS OLD.id AND NEW.task_id IS OLD.task_id AND NEW.kind IS OLD.kind
    AND NEW.person IS OLD.person AND NEW.machine_id IS OLD.machine_id
    AND NEW.at IS OLD.at AND NEW.written_here IS OLD.written_here
    AND NEW.person_key IS OLD.person_key AND NEW.machine_label IS OLD.machine_label)
BEGIN SELECT RAISE(ABORT, 'Task ownership entries are append-only'); END;

-- A Machine is private by default: its label stays local. `lf machine public`
-- sets the name teammates see on this Machine's ownership entries and
-- published crons. Only the local Machine's row carries one.
ALTER TABLE machines ADD COLUMN public_name TEXT;
