-- name: opaque_steer_run_provenance
-- id: fe43b04cca774a8cb13b1a57813d4e34
-- depends_on:

CREATE TABLE steers_next (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL,
    rev INTEGER NOT NULL,
    author_kind TEXT NOT NULL CHECK (author_kind IN ('user', 'run')),
    author_run_id TEXT,
    text TEXT NOT NULL CHECK (length(trim(text)) > 0),
    issued_at INTEGER NOT NULL,
    CHECK ((author_kind = 'user') = (author_run_id IS NULL)),
    UNIQUE (epoch_id, rev),
    FOREIGN KEY (epoch_id, rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE CASCADE
);

INSERT INTO steers_next (
    id, epoch_id, rev, author_kind, author_run_id, text, issued_at
)
SELECT id, epoch_id, rev, author_kind, author_run_id, text, issued_at
FROM steers;

DROP TABLE steers;
ALTER TABLE steers_next RENAME TO steers;
CREATE INDEX idx_steers_epoch_revision ON steers(epoch_id, rev);
