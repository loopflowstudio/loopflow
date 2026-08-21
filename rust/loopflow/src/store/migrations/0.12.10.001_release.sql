-- draft: task_lifecycle_contract
ALTER TABLE tasks ADD COLUMN lifecycle_outcome TEXT NOT NULL DEFAULT 'delivery'
    CHECK (lifecycle_outcome IN ('delivery', 'design_only'));
