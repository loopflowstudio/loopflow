ALTER TABLE task_graphs ADD COLUMN breakpoint TEXT CHECK (breakpoint IS NULL OR (json_valid(breakpoint) AND lf_process_id IS NULL));
