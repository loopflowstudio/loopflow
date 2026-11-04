-- Semantic defaults describe future materialization; native hosts own active layouts.
CREATE TABLE wave_task_layouts (
    repository_id TEXT NOT NULL,
    wave_id TEXT NOT NULL,
    layout_json TEXT NOT NULL,
    PRIMARY KEY (repository_id, wave_id)
);

-- Only native occurrence identity and observations, never active split geometry.
CREATE TABLE host_workspaces (
    id TEXT PRIMARY KEY,
    repository_id TEXT NOT NULL,
    host TEXT NOT NULL,
    server TEXT NOT NULL,
    selection_json TEXT NOT NULL,
    copy_name TEXT,
    container_id TEXT,
    observation TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE host_work_occurrences (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES host_workspaces(id),
    work_json TEXT NOT NULL,
    destination_id TEXT,
    observation TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TRIGGER store_revision_wave_task_layouts_insert AFTER INSERT ON wave_task_layouts
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_wave_task_layouts_update AFTER UPDATE ON wave_task_layouts
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_wave_task_layouts_delete AFTER DELETE ON wave_task_layouts
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_host_workspaces_insert AFTER INSERT ON host_workspaces
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_host_workspaces_update AFTER UPDATE ON host_workspaces
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_host_workspaces_delete AFTER DELETE ON host_workspaces
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_host_work_occurrences_insert AFTER INSERT ON host_work_occurrences
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_host_work_occurrences_update AFTER UPDATE ON host_work_occurrences
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_host_work_occurrences_delete AFTER DELETE ON host_work_occurrences
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;
