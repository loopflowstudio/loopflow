-- A Desktop shell runs in a held terminal. `owner` stays for shells an earlier
-- build started under `lf session serve`, until they end.
ALTER TABLE desktop_shells ADD COLUMN terminal_id TEXT REFERENCES held_terminals(id);
-- The `lf` that created the shell: what runs in it records that Process as
-- its parent, so Task cleanup treats it as the shell's.
ALTER TABLE desktop_shells ADD COLUMN launcher_lf_process_id TEXT REFERENCES processes(id);
CREATE UNIQUE INDEX desktop_shells_terminal ON desktop_shells(terminal_id) WHERE terminal_id IS NOT NULL;
