INSERT INTO waves(id,name,repo,created_at) VALUES('w','product','/repo',1);
INSERT INTO projects(id,wave_id,created_at) VALUES('p','w',1);
INSERT INTO tasks(id,project_id,issue_identifier,workspace_slug,branch,base_commit,created_at,updated_at)
VALUES('t','p','TEST-1','work','work','base',1,1);
INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,
 github_number,github_url,github_head_sha,merge_commit,created_at,updated_at)
VALUES('first','t',1,'work','work','base',1,1,'https://example.com/pr/1','head','merged',1,1);
