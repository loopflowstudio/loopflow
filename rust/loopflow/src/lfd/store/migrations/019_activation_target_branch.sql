ALTER TABLE pending_activations ADD COLUMN target_branch TEXT NOT NULL DEFAULT 'main';
ALTER TABLE runs ADD COLUMN target_branch TEXT NOT NULL DEFAULT 'main';
