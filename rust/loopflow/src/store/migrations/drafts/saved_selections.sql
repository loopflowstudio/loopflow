CREATE TABLE dashboards (
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    expression_key TEXT NOT NULL,
    name TEXT,
    pinned INTEGER NOT NULL CHECK(pinned IN (0,1)),
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
