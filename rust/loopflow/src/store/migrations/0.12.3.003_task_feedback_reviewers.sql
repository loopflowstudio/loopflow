-- Reviewer names authority directly; it does not suppress the authored step.

ALTER TABLE tasks RENAME COLUMN iterate_interaction_policy TO iterate_reviewer;
ALTER TABLE tasks RENAME COLUMN kickoff_interaction_policy TO kickoff_reviewer;
ALTER TABLE tasks RENAME COLUMN gate_interaction_policy TO gate_reviewer;

UPDATE tasks
SET iterate_reviewer = CASE iterate_reviewer
        WHEN 'require' THEN 'user'
        WHEN 'defer' THEN 'parent'
    END,
    kickoff_reviewer = CASE kickoff_reviewer
        WHEN 'require' THEN 'user'
        WHEN 'defer' THEN 'parent'
    END,
    gate_reviewer = CASE gate_reviewer
        WHEN 'require' THEN 'user'
        WHEN 'defer' THEN 'parent'
    END;
