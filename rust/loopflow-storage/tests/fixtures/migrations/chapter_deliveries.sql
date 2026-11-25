-- A released repository, with one saved create, one lost create reply and
-- three lost move replies. The caller supplies the repository plan.
WITH names(n,name) AS (VALUES (1,'billing'),(2,'refunds'))
INSERT INTO waves(id,name,repo,created_at)
SELECT printf('00000000-0000-4000-8000-%012x',n),name,repo,1
FROM names CROSS JOIN repository_plans;

WITH chapters(n,wave,external,at) AS (VALUES
    (1,'billing','november',1),(2,'billing','december',2),(3,'refunds','refunds',1))
INSERT INTO projects(id,wave_id,external_project_id,project_slug,project_name,project_summary,created_at,updated_at,description,status)
SELECT printf('proj_%032x',n),waves.id,external,external,external,'',at,at,'','started'
FROM chapters JOIN waves ON waves.name=chapters.wave;

WITH saved(n,title,destination,issue) AS (VALUES
    (1,'Queued','november',NULL),(2,'Arrived','november',NULL),
    (3,'Moving','refunds','moving'),(4,'Moved','november','moved'),(5,'Retry','november','retry'))
INSERT INTO tasks(id,repository_id,project_id,issue_identifier,external_issue_id,
    issue_title,issue_description,workspace_slug,created_at,updated_at)
SELECT printf('task_%032x',n),r.id,p.id,printf('lf-%04x',n),issue,title,'',lower(title),1,1
FROM saved JOIN projects p ON p.external_project_id=destination CROSS JOIN repository_plans r;

INSERT INTO tracker_actions(id,task_id,kind,phase,tracker,value_json)
SELECT lower(hex(randomblob(16))),id,'create',
    CASE issue_title WHEN 'Queued' THEN 'queued' ELSE 'sent' END,'linear',
    json_array(json_object('field','task_title','value',issue_title),
               json_object('field','task_description','value',''),
               json_object('field','task_project','value',project_id),
               json_object('field','future_field','value',42))
FROM tasks WHERE external_issue_id IS NULL;

INSERT INTO tracker_actions(id,task_id,subject,kind,phase,tracker,value_json,base_json)
SELECT lower(hex(randomblob(16))),t.id,'task_project','field','sent','linear',
    json_object('field','task_project','value',t.project_id),
    json_object('revision',NULL,'value',json_object('field','task_project','value',p.id))
FROM tasks t JOIN projects p ON p.external_project_id=CASE t.external_issue_id
    WHEN 'moving' THEN 'november' WHEN 'moved' THEN 'refunds' WHEN 'retry' THEN 'refunds' END;
