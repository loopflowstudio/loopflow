-- Released frontier: preserve local IDs, prose, criteria, selection, and uncertain completion.
INSERT INTO waves(id,name,repo,created_at) VALUES('retained-wave','proof','/repo',1);
INSERT INTO projects(id,wave_id,external_project_id,created_at,updated_at,project_slug,project_name,
    project_prompt_context,workflow,pm_snapshot_synced_at,iteration)
VALUES('retained-project','retained-wave','linear-project',1,7,'proof','Proof',
'Authored purpose.

workflow: obsolete-prose-selection
## Metric targets
```json
[{"metric_id":"recovery","target":{"kind":"at_least","value":1.0}}]
```
## KRs
This preamble is authored prose.
- [x] Retain this
  continued proof
- [ ] Check offline saves
## Notes
Keep these bytes.
','saved-selection',7,3);
INSERT INTO project_events(project_id,kind_json,created_at) VALUES('retained-project',
'{"kind":"iteration_completed","iteration":3,"summary":"Keep judgment"}',7);
INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,
    created_at,updated_at,worktree,workspace_slug,pm_writeback_json)
VALUES('retained-task','retained-project','linear-task','PROOF-1','Retain Task','Keep the brief',1,7,
    '/retained/task','proof','{"state":"pending","operation":"complete_task","error":"offline"}');
INSERT INTO task_workflows(task_id,graph,node,updated_at)
VALUES('retained-task','{"name":"review","nodes":[],"edges":[]}','review',7);
