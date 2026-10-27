-- A Graph step's `human` flag is now `interactive`. Recorded Graphs keep the old
-- key, which no longer reads, so it is renamed in every step, XOR paths included.
-- Inside JSON strings a quote is escaped, so `,"human":` matches only the key.
-- The record is append-only, so its trigger steps aside for the rewrite.
DROP TRIGGER graph_processes_are_append_only;
UPDATE graph_processes SET compiled=replace(compiled,',"human":',',"interactive":')
WHERE instr(compiled,',"human":')>0;
CREATE TRIGGER graph_processes_are_append_only BEFORE UPDATE ON graph_processes BEGIN
    SELECT RAISE(ABORT,'A Graph record is append-only');
END;
