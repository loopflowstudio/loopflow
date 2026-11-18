-- A Process records the Task it belongs to once, when it is written: the Task
-- its launch names, else its caller's, else its parent's, else the deepest
-- checkout holding its directory. Membership then reads this column instead of
-- matching paths, so a moved repository or a reused checkout path changes no
-- Task's Processes.
ALTER TABLE processes ADD COLUMN task_id TEXT;
CREATE INDEX processes_task ON processes(task_id) WHERE task_id IS NOT NULL;

-- Relocation receipts are the only membership a path cannot recover.
UPDATE processes SET task_id=r.task_id
FROM relocated_task_processes r WHERE r.process_id=processes.id;

-- Existing rows take their Task in the Machine server's maintenance pass, a
-- batch per write transaction: resolving every row here would hold the write
-- lock for tens of seconds at upgrade. Released lf keeps inserting rows without
-- a Task until it exits; the same pass reaches them past this cursor. The
-- table goes once no released lf writes Processes without a Task.
CREATE TABLE process_task_backfill (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    after_rowid INTEGER NOT NULL
);
INSERT INTO process_task_backfill(id, after_rowid) VALUES(1, 0);

-- leaving: relocated_task_processes LOO-877
