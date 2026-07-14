DROP TABLE IF EXISTS repo_edges;
DROP TABLE IF EXISTS repos;

CREATE TABLE IF NOT EXISTS repos (
    path TEXT PRIMARY KEY,
    repo_id TEXT NOT NULL,
    name TEXT NOT NULL,
    added_at BIGINT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_repos_repo_id ON repos(repo_id);

CREATE TABLE IF NOT EXISTS repo_edges (
    parent_repo_id TEXT NOT NULL REFERENCES repos(repo_id) ON DELETE CASCADE,
    child_repo_id TEXT NOT NULL REFERENCES repos(repo_id) ON DELETE CASCADE,
    PRIMARY KEY (parent_repo_id, child_repo_id)
);
