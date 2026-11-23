-- Complete planning and delivery facts for the cross-client Task scenario.
UPDATE projects SET project_slug='project',project_summary='',description='',krs='[]',metric_targets='[]',updated_at=1;
UPDATE tasks SET planning_state='unstarted' WHERE issue_identifier='FIX-2';
UPDATE tasks SET planning_state='canceled' WHERE issue_identifier='FIX-3';
UPDATE task_checkouts SET branch='fix-2',base_commit='base' WHERE task_id='task_22222222222222222222222222222222';
INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
VALUES('pr_22222222222222222222222222222222','task_22222222222222222222222222222222',1,'fix-2','fix-2','base',1,1);
