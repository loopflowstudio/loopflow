ALTER TABLE pending_activations ADD COLUMN target_branch TEXT NOT NULL DEFAULT 'main';
ALTER TABLE wave_runs ADD COLUMN target_branch TEXT NOT NULL DEFAULT 'main';
