ALTER TABLE agent_sessions ADD COLUMN transport_lf_process_id TEXT REFERENCES processes(id);
