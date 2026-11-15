-- A database lifetime, independent of server restarts and Machine routing.
CREATE TABLE client_store_epoch (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    epoch TEXT NOT NULL
);
INSERT INTO client_store_epoch VALUES (1, lower(hex(randomblob(16))));

-- One receipt owner for creation milestones and durable native commands. Old
-- creation rows retain their payload fingerprints, results and recovery rules.
ALTER TABLE creation_receipts ADD COLUMN command TEXT;
ALTER TABLE creation_receipts ADD COLUMN delivery_state TEXT NOT NULL DEFAULT 'untracked'
    CHECK (delivery_state IN ('untracked','queued','dispatched','finished','uncertain'));
ALTER TABLE creation_receipts ADD COLUMN command_error TEXT;
ALTER TABLE creation_receipts ADD COLUMN commits TEXT NOT NULL DEFAULT '[]';
ALTER TABLE creation_receipts ADD COLUMN receipt_revision INTEGER NOT NULL DEFAULT 0;
CREATE TEMP TABLE client_operation_prior_revisions AS SELECT * FROM store_revisions;
DROP TABLE store_revisions;
CREATE TABLE store_revisions (
    domain TEXT PRIMARY KEY NOT NULL CHECK (domain IN ('planning','sessions','graphs','processes','usage','task_updates','operations')),
    revision INTEGER NOT NULL
) WITHOUT ROWID;
INSERT INTO store_revisions SELECT * FROM client_operation_prior_revisions;
DROP TABLE client_operation_prior_revisions;
INSERT INTO store_revisions(domain,revision) VALUES ('operations',0);
CREATE TRIGGER store_revision_creation_receipts_insert AFTER INSERT ON creation_receipts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='operations'; END;
CREATE TRIGGER store_revision_creation_receipts_update AFTER UPDATE ON creation_receipts
WHEN NEW.receipt_revision = OLD.receipt_revision
BEGIN
    UPDATE creation_receipts SET receipt_revision=OLD.receipt_revision+1 WHERE request_id=NEW.request_id;
    UPDATE store_revisions SET revision=revision+1 WHERE domain='operations';
END;
CREATE TRIGGER store_revision_creation_receipts_delete AFTER DELETE ON creation_receipts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='operations'; END;
