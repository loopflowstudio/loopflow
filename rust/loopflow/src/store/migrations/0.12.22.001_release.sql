-- draft: remove_landing_repair_counter
ALTER TABLE pr_landings DROP COLUMN repair_count;

-- draft: task_loop_review
ALTER TABLE task_flow_positions ADD COLUMN review_json TEXT;
