-- Wave Workflow definitions live only in committed `.lf/workflows/` files and
-- builtins, so every teammate reads the same definition. A Wave keeps only the
-- selected name (`waves.workflow`). Dropping the table drops its triggers.
DROP TABLE wave_workflows;

-- An ownership entry names its person by their key (lowercase Git
-- `user.email`) and carries the claimant Machine's public name, if any, so
-- teammates can read where it is. Names stay display only.
ALTER TABLE task_ownership ADD COLUMN person_key TEXT;
ALTER TABLE task_ownership ADD COLUMN machine_label TEXT;
DROP TRIGGER task_ownership_append_only;
CREATE TRIGGER task_ownership_append_only BEFORE UPDATE ON task_ownership
WHEN NOT (OLD.ordered_at IS NULL AND NEW.ordered_at IS NOT NULL
    AND NEW.id IS OLD.id AND NEW.task_id IS OLD.task_id AND NEW.kind IS OLD.kind
    AND NEW.person IS OLD.person AND NEW.machine_id IS OLD.machine_id
    AND NEW.at IS OLD.at AND NEW.written_here IS OLD.written_here
    AND NEW.person_key IS OLD.person_key AND NEW.machine_label IS OLD.machine_label)
BEGIN SELECT RAISE(ABORT, 'Task ownership entries are append-only'); END;

-- A Machine is private by default: its label stays local. `lf machine public`
-- sets the name teammates see on this Machine's ownership entries and
-- published crons. Only the local Machine's row carries one.
ALTER TABLE machines ADD COLUMN public_name TEXT;
