CREATE TABLE saved_selections (
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    selection_json TEXT NOT NULL CHECK(json_valid(selection_json)),
    arrangement_json TEXT NOT NULL CHECK(json_valid(arrangement_json)),
    PRIMARY KEY(repository_id, id)
);
CREATE TRIGGER store_revision_saved_selections_insert AFTER INSERT ON saved_selections BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_saved_selections_update AFTER UPDATE ON saved_selections BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_saved_selections_delete AFTER DELETE ON saved_selections BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
