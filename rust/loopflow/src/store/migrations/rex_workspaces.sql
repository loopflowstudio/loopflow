-- Semantic defaults describe future materialization; native hosts own active layouts.
CREATE TABLE wave_task_layouts (
    repository_id TEXT NOT NULL,
    wave_id TEXT NOT NULL,
    layout_json TEXT NOT NULL,
    PRIMARY KEY (repository_id, wave_id)
);

-- Locators survive native renames; pending writes prevent duplicate creation.
-- These records establish neither host retention nor ownership of native layouts.
CREATE TABLE host_workspaces (
    id TEXT PRIMARY KEY,
    repository_id TEXT NOT NULL,
    host TEXT NOT NULL,
    server TEXT NOT NULL,
    selection_json TEXT NOT NULL,
    copy_name TEXT,
    container_id TEXT,
    creation_pending INTEGER NOT NULL CHECK (creation_pending IN (0,1)),
    updated_at INTEGER NOT NULL
);
CREATE TABLE host_work_occurrences (
    id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL REFERENCES host_workspaces(id),
    task_id TEXT NOT NULL,
    destination_id TEXT,
    creation TEXT NOT NULL CHECK (creation IN ('pending', 'passive', 'tools_pending', 'complete'))
);

CREATE TRIGGER store_revision_wave_task_layouts_insert AFTER INSERT ON wave_task_layouts
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_wave_task_layouts_update AFTER UPDATE ON wave_task_layouts
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;

CREATE TRIGGER store_revision_wave_task_layouts_delete AFTER DELETE ON wave_task_layouts
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;
