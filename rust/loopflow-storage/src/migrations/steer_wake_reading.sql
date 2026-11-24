-- The newest steer the Machine server has read for Sessions to start. A
-- steer naming a stopped interactive Session, or a Task's steer for its
-- stopped primary conversation, starts that Session once, when the server
-- first reads past it. Steers saved before this migration start nothing.
CREATE TABLE steer_wake_reading (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    seq INTEGER NOT NULL
);
INSERT INTO steer_wake_reading(id, seq) SELECT 1, COALESCE(MAX(seq), 0) FROM steers;
