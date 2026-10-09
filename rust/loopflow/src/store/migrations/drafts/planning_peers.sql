-- depends_on: local_planning
-- One mutation journal on the common planning writer. No execution is exported.
CREATE TABLE planning_peer_context (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    importing INTEGER NOT NULL DEFAULT 0 CHECK(importing IN (0,1)),
    observation TEXT CHECK(observation IS NULL OR json_valid(observation)),
    fields TEXT CHECK(fields IS NULL OR json_valid(fields))
);
INSERT INTO planning_peer_context(singleton) VALUES(1);
CREATE TABLE planning_peer_changes (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK(kind IN ('wave','project','task','comment')),
    object_id TEXT NOT NULL,
    field TEXT NOT NULL,
    value TEXT NOT NULL CHECK(json_valid(value)),
    clock INTEGER NOT NULL CHECK(clock>=0),
    linear TEXT CHECK(linear IS NULL OR json_valid(linear)),
    parents TEXT NOT NULL CHECK(json_valid(parents))
);
CREATE INDEX planning_peer_changes_object ON planning_peer_changes(kind,object_id,field);
CREATE INDEX planning_peer_changes_clock ON planning_peer_changes(clock);
CREATE TABLE planning_peer_heads (
    id TEXT PRIMARY KEY REFERENCES planning_peer_changes(id),
    kind TEXT NOT NULL,
    object_id TEXT NOT NULL,
    field TEXT NOT NULL
);
CREATE INDEX planning_peer_heads_object ON planning_peer_heads(kind,object_id,field);
CREATE TRIGGER planning_peer_advance AFTER INSERT ON planning_peer_changes BEGIN
    DELETE FROM planning_peer_heads WHERE id IN (SELECT value FROM json_each(NEW.parents));
    INSERT INTO planning_peer_heads(id,kind,object_id,field)
        VALUES(NEW.id,NEW.kind,NEW.object_id,NEW.field);
END;
-- Selection is local routing, not a second planner or a replication payload.
CREATE TABLE planning_user (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1),
    user_key TEXT NOT NULL UNIQUE
);
CREATE TABLE planning_destinations (
    repo TEXT NOT NULL,
    id TEXT NOT NULL,
    endpoint TEXT NOT NULL,
    reference TEXT NOT NULL,
    fetched_revision TEXT,
    acquisition_error TEXT,
    publication_revision TEXT,
    publication_state TEXT CHECK(publication_state IN ('pending','unconfirmed','confirmed')),
    publication_digest TEXT,
    publication_error TEXT,
    PRIMARY KEY(repo,id)
);
CREATE TABLE planning_active (
    repo TEXT PRIMARY KEY,
    destination TEXT NOT NULL,
    FOREIGN KEY(repo,destination) REFERENCES planning_destinations(repo,id)
);
CREATE TABLE planning_members (
    kind TEXT NOT NULL CHECK(kind IN ('wave','project','task','comment')),
    object_id TEXT NOT NULL,
    repo TEXT NOT NULL,
    destination TEXT NOT NULL,
    PRIMARY KEY(kind,object_id),
    FOREIGN KEY(repo,destination) REFERENCES planning_destinations(repo,id)
);
CREATE INDEX planning_members_destination ON planning_members(repo,destination);
-- Only new descendants inherit a selected plan. Binding never sweeps existing
-- local records into a joined destination.
CREATE TRIGGER peer_wave_membership AFTER INSERT ON waves BEGIN
    INSERT INTO planning_members(kind,object_id,repo,destination)
    SELECT 'wave',NEW.id,repo,destination FROM planning_members
    WHERE kind='wave' AND object_id=NEW.parent_wave_id
    AND (SELECT importing FROM planning_peer_context)=0;
    INSERT INTO planning_members(kind,object_id,repo,destination)
    SELECT 'wave',NEW.id,repo,destination FROM planning_active
    WHERE repo=NEW.repo AND NEW.parent_wave_id IS NULL
    AND (SELECT importing FROM planning_peer_context)=0;
END;
CREATE TRIGGER peer_project_membership AFTER INSERT ON projects BEGIN
    INSERT INTO planning_members(kind,object_id,repo,destination)
    SELECT 'project',NEW.id,repo,destination FROM planning_members
    WHERE kind='wave' AND object_id=NEW.wave_id
    AND (SELECT importing FROM planning_peer_context)=0;
END;
CREATE TRIGGER peer_task_membership AFTER INSERT ON tasks BEGIN
    INSERT INTO planning_members(kind,object_id,repo,destination)
    SELECT 'task',NEW.id,repo,destination FROM planning_members
    WHERE kind='project' AND object_id=NEW.project_id
    AND (SELECT importing FROM planning_peer_context)=0;
END;
CREATE TRIGGER peer_comment_membership AFTER INSERT ON task_comments BEGIN
    INSERT INTO planning_members(kind,object_id,repo,destination)
    SELECT 'comment',NEW.id,repo,destination FROM planning_members
    WHERE kind='task' AND object_id=NEW.task_id
    AND (SELECT importing FROM planning_peer_context)=0;
END;

CREATE TABLE planning_peer_imports (
    repo TEXT NOT NULL,
    destination TEXT NOT NULL,
    revision TEXT NOT NULL,
    PRIMARY KEY(repo,destination)
);

-- Retain rejected projection evidence independently of the planning row: a
-- duplicate provider mapping may prevent that row from existing at all.
CREATE TABLE planning_peer_conflicts (
    repo TEXT NOT NULL,
    kind TEXT NOT NULL,
    object_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    active INTEGER NOT NULL CHECK(active IN (0,1)),
    PRIMARY KEY(kind,object_id,reason)
);

CREATE TRIGGER store_revision_planning_peer_conflicts_insert AFTER INSERT ON planning_peer_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_conflicts_update AFTER UPDATE ON planning_peer_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_conflicts_delete AFTER DELETE ON planning_peer_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER peer_wave_insert AFTER INSERT ON waves
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'wave',NEW.id,j.key,
        CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),
        (SELECT CASE WHEN j.key IN (SELECT key FROM json_each(fields))
            AND j.value IS json_extract(fields, '$.' || j.key) THEN observation END FROM planning_peer_context),
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='wave' AND object_id=NEW.id AND field=j.key)
    FROM json_each(json_object('name',NEW.name,'parent_wave_id',NEW.parent_wave_id,'current_project_id',NEW.current_project_id)) j ;
END;

CREATE TRIGGER peer_wave_update AFTER UPDATE ON waves
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'wave',NEW.id,j.key,
        CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),
        (SELECT CASE WHEN j.key IN (SELECT key FROM json_each(fields))
            AND j.value IS json_extract(fields, '$.' || j.key) THEN observation END FROM planning_peer_context),
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='wave' AND object_id=NEW.id AND field=j.key)
    FROM json_each(json_object('name',NEW.name,'parent_wave_id',NEW.parent_wave_id,'current_project_id',NEW.current_project_id)) j WHERE j.value IS NOT json_extract(json_object('name',OLD.name,'parent_wave_id',OLD.parent_wave_id,'current_project_id',OLD.current_project_id), '$.' || j.key);
END;

INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'wave',r.id,j.key,
    CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
    0,NULL,'[]' FROM waves r, json_each(json_object('name',r.name,'parent_wave_id',r.parent_wave_id,'current_project_id',r.current_project_id)) j;

-- Semantic workflow/KR/target capture belongs to the common Rust content owner;
-- migration application seeds existing content in the same transaction.
CREATE TRIGGER peer_project_insert AFTER INSERT ON projects
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'project',NEW.id,j.key,
        CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),
        (SELECT CASE WHEN j.key IN (SELECT key FROM json_each(fields))
            AND j.value IS json_extract(fields, '$.' || j.key) THEN observation END FROM planning_peer_context),
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='project' AND object_id=NEW.id AND field=j.key)
    FROM json_each(json_object('wave_id',NEW.wave_id,'external_project_id',NEW.external_project_id,'project_slug',NEW.project_slug,'project_name',NEW.project_name,'project_summary',NEW.project_summary,'status',NEW.status,'planning_rank',NEW.planning_rank,'planning_initiatives',NEW.planning_initiatives,'planning_teams',NEW.planning_teams)) j ;
END;

CREATE TRIGGER peer_project_update AFTER UPDATE ON projects
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'project',NEW.id,j.key,
        CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),
        (SELECT CASE WHEN j.key IN (SELECT key FROM json_each(fields))
            AND j.value IS json_extract(fields, '$.' || j.key) THEN observation END FROM planning_peer_context),
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='project' AND object_id=NEW.id AND field=j.key)
    FROM json_each(json_object('wave_id',NEW.wave_id,'external_project_id',NEW.external_project_id,'project_slug',NEW.project_slug,'project_name',NEW.project_name,'project_summary',NEW.project_summary,'status',NEW.status,'planning_rank',NEW.planning_rank,'planning_initiatives',NEW.planning_initiatives,'planning_teams',NEW.planning_teams)) j WHERE j.value IS NOT json_extract(json_object('wave_id',OLD.wave_id,'external_project_id',OLD.external_project_id,'project_slug',OLD.project_slug,'project_name',OLD.project_name,'project_summary',OLD.project_summary,'status',OLD.status,'planning_rank',OLD.planning_rank,'planning_initiatives',OLD.planning_initiatives,'planning_teams',OLD.planning_teams), '$.' || j.key) OR ((SELECT observation FROM planning_peer_context) IS NOT NULL
        AND j.key IN (SELECT key FROM json_each((SELECT fields FROM planning_peer_context)))
        AND j.value IS json_extract((SELECT fields FROM planning_peer_context), '$.' || j.key)
        AND NOT EXISTS(SELECT 1 FROM planning_peer_heads h JOIN planning_peer_changes c ON c.id=h.id
            WHERE h.kind='project' AND h.object_id=NEW.id AND h.field=j.key AND c.linear IS NOT NULL
            AND json_extract(c.linear,'$.body.id') IS json_extract((SELECT observation FROM planning_peer_context),'$.body.id')
            AND json_extract(c.linear,'$.body.revision') IS json_extract((SELECT observation FROM planning_peer_context),'$.body.revision')));
END;

INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'project',r.id,j.key,
    CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
    0,NULL,'[]' FROM projects r, json_each(json_object('wave_id',r.wave_id,'external_project_id',r.external_project_id,'project_slug',r.project_slug,'project_name',r.project_name,'project_summary',r.project_summary,'status',r.status,'planning_rank',r.planning_rank,'planning_initiatives',r.planning_initiatives,'planning_teams',r.planning_teams)) j;

CREATE TRIGGER peer_task_insert AFTER INSERT ON tasks
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'task',NEW.id,j.key,
        CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),
        (SELECT CASE WHEN j.key IN (SELECT key FROM json_each(fields))
            AND j.value IS json_extract(fields, '$.' || j.key) THEN observation END FROM planning_peer_context),
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='task' AND object_id=NEW.id AND field=j.key)
    FROM json_each(json_object('project_id',NEW.project_id,'external_issue_id',NEW.external_issue_id,'issue_identifier',NEW.issue_identifier,'issue_title',NEW.issue_title,'issue_description',NEW.issue_description,'planning_rank',NEW.planning_rank,'planning_assignee',NEW.planning_assignee,'disposition',json_object('planning_completed',NEW.planning_completed,'planning_completed_at',NEW.planning_completed_at,'planning_state',NEW.planning_state),'planning_deleted_at',NEW.planning_deleted_at,'planning_url',NEW.planning_url,'planning_branch_name',NEW.planning_branch_name,'planning_team_id',NEW.planning_team_id)) j ;
END;

CREATE TRIGGER peer_task_update AFTER UPDATE ON tasks
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'task',NEW.id,j.key,
        CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),
        (SELECT CASE WHEN j.key IN (SELECT key FROM json_each(fields))
            AND j.value IS json_extract(fields, '$.' || j.key) THEN observation END FROM planning_peer_context),
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='task' AND object_id=NEW.id AND field=j.key)
    FROM json_each(json_object('project_id',NEW.project_id,'external_issue_id',NEW.external_issue_id,'issue_identifier',NEW.issue_identifier,'issue_title',NEW.issue_title,'issue_description',NEW.issue_description,'planning_rank',NEW.planning_rank,'planning_assignee',NEW.planning_assignee,'disposition',json_object('planning_completed',NEW.planning_completed,'planning_completed_at',NEW.planning_completed_at,'planning_state',NEW.planning_state),'planning_deleted_at',NEW.planning_deleted_at,'planning_url',NEW.planning_url,'planning_branch_name',NEW.planning_branch_name,'planning_team_id',NEW.planning_team_id)) j WHERE j.value IS NOT json_extract(json_object('project_id',OLD.project_id,'external_issue_id',OLD.external_issue_id,'issue_identifier',OLD.issue_identifier,'issue_title',OLD.issue_title,'issue_description',OLD.issue_description,'planning_rank',OLD.planning_rank,'planning_assignee',OLD.planning_assignee,'disposition',json_object('planning_completed',OLD.planning_completed,'planning_completed_at',OLD.planning_completed_at,'planning_state',OLD.planning_state),'planning_deleted_at',OLD.planning_deleted_at,'planning_url',OLD.planning_url,'planning_branch_name',OLD.planning_branch_name,'planning_team_id',OLD.planning_team_id), '$.' || j.key) OR ((SELECT observation FROM planning_peer_context) IS NOT NULL
        AND j.key IN (SELECT key FROM json_each((SELECT fields FROM planning_peer_context)))
        AND j.value IS json_extract((SELECT fields FROM planning_peer_context), '$.' || j.key)
        AND NOT EXISTS(SELECT 1 FROM planning_peer_heads h JOIN planning_peer_changes c ON c.id=h.id
            WHERE h.kind='task' AND h.object_id=NEW.id AND h.field=j.key AND c.linear IS NOT NULL
            AND json_extract(c.linear,'$.body.id') IS json_extract((SELECT observation FROM planning_peer_context),'$.body.id')
            AND json_extract(c.linear,'$.body.revision') IS json_extract((SELECT observation FROM planning_peer_context),'$.body.revision')));
END;

INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'task',r.id,j.key,
    CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
    0,NULL,'[]' FROM tasks r, json_each(json_object('project_id',r.project_id,'external_issue_id',r.external_issue_id,'issue_identifier',r.issue_identifier,'issue_title',r.issue_title,'issue_description',r.issue_description,'planning_rank',r.planning_rank,'planning_assignee',r.planning_assignee,'disposition',json_object('planning_completed',r.planning_completed,'planning_completed_at',r.planning_completed_at,'planning_state',r.planning_state),'planning_deleted_at',r.planning_deleted_at,'planning_url',r.planning_url,'planning_branch_name',r.planning_branch_name,'planning_team_id',r.planning_team_id)) j;

CREATE TRIGGER peer_comment_insert AFTER INSERT ON task_comments
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'comment',NEW.id,j.key,
        CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),
        (SELECT CASE WHEN j.key IN (SELECT key FROM json_each(fields))
            AND j.value IS json_extract(fields, '$.' || j.key) THEN observation END FROM planning_peer_context),
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='comment' AND object_id=NEW.id AND field=j.key)
    FROM json_each(json_object('task_id',NEW.task_id,'content',json_object('author',NEW.author,'body',NEW.body,'created_at',NEW.created_at))) j ;
END;

CREATE TRIGGER peer_comment_update AFTER UPDATE ON task_comments
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'comment',NEW.id,j.key,
        CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),
        (SELECT CASE WHEN j.key IN (SELECT key FROM json_each(fields))
            AND j.value IS json_extract(fields, '$.' || j.key) THEN observation END FROM planning_peer_context),
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='comment' AND object_id=NEW.id AND field=j.key)
    FROM json_each(json_object('task_id',NEW.task_id,'content',json_object('author',NEW.author,'body',NEW.body,'created_at',NEW.created_at))) j WHERE j.value IS NOT json_extract(json_object('task_id',OLD.task_id,'content',json_object('author',OLD.author,'body',OLD.body,'created_at',OLD.created_at)), '$.' || j.key) OR ((SELECT observation FROM planning_peer_context) IS NOT NULL
        AND j.key IN (SELECT key FROM json_each((SELECT fields FROM planning_peer_context)))
        AND j.value IS json_extract((SELECT fields FROM planning_peer_context), '$.' || j.key)
        AND NOT EXISTS(SELECT 1 FROM planning_peer_heads h JOIN planning_peer_changes c ON c.id=h.id
            WHERE h.kind='comment' AND h.object_id=NEW.id AND h.field=j.key AND c.linear IS NOT NULL
            AND json_extract(c.linear,'$.body.id') IS json_extract((SELECT observation FROM planning_peer_context),'$.body.id')
            AND json_extract(c.linear,'$.body.revision') IS json_extract((SELECT observation FROM planning_peer_context),'$.body.revision')));
END;

-- Existing provider comments wait for acquisition of their raw observation.
-- Exporting their normalized display body as an authored save would echo it.
INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'comment',r.id,j.key,
    CASE WHEN j.type IN ('object','array') THEN j.value ELSE json_quote(j.value) END,
    0,NULL,'[]' FROM task_comments r, json_each(json_object('task_id',r.task_id,'content',json_object('author',r.author,'body',r.body,'created_at',r.created_at))) j
    WHERE r.provider_revision IS NULL AND EXISTS(
        SELECT 1 FROM task_comment_deliveries d WHERE d.comment_id=r.id
            AND d.acknowledged=0 AND d.conflicting_comment_json IS NULL);

CREATE TRIGGER store_revision_planning_destinations_insert AFTER INSERT ON planning_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_destinations_update AFTER UPDATE ON planning_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_destinations_delete AFTER DELETE ON planning_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_members_insert AFTER INSERT ON planning_members
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_members_update AFTER UPDATE ON planning_members
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_members_delete AFTER DELETE ON planning_members
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_active_insert AFTER INSERT ON planning_active
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_active_update AFTER UPDATE ON planning_active
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_active_delete AFTER DELETE ON planning_active
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_peer_imports_insert AFTER INSERT ON planning_peer_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_peer_imports_update AFTER UPDATE ON planning_peer_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_planning_peer_imports_delete AFTER DELETE ON planning_peer_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- Project creation is planning, not a local rotation or activation receipt.
ALTER TABLE projects ADD COLUMN export_json TEXT CHECK(export_json IS NULL OR json_valid(export_json));
ALTER TABLE projects ADD COLUMN export_attempted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projects ADD COLUMN export_link_attempted INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projects ADD COLUMN export_error TEXT;
UPDATE projects SET (export_json,export_attempted,export_link_attempted,export_error)=(
    SELECT export_json,export_attempted,export_link_attempted,export_error FROM project_transitions c
    WHERE c.successor_id=projects.id AND c.wave_id=projects.wave_id)
WHERE EXISTS(SELECT 1 FROM project_transitions c WHERE c.successor_id=projects.id AND c.wave_id=projects.wave_id AND c.export_json IS NOT NULL);
ALTER TABLE project_transitions DROP COLUMN export_json;
ALTER TABLE project_transitions DROP COLUMN export_attempted;
ALTER TABLE project_transitions DROP COLUMN export_link_attempted;
ALTER TABLE project_transitions DROP COLUMN export_error;

-- Receipts are prepared by UPDATE after their planning row exists. Capture that
-- preparation and later attempts; import suppresses echo and migration seeds below.
UPDATE task_creation_intents SET export_json=json_remove(json_set(export_json,'$.parent',(SELECT p.id FROM projects p WHERE p.id=json_extract(task_creation_intents.export_json,'$.model.project_id') OR p.external_project_id=json_extract(task_creation_intents.export_json,'$.model.project_id')),'$.captured',json((
    SELECT json_group_array(id) FROM task_changes WHERE task_id=task_creation_intents.task_id
    AND seq<=json_extract(task_creation_intents.export_json,'$.through')))),'$.through')
WHERE export_json IS NOT NULL;
CREATE TRIGGER peer_task_creation_update AFTER UPDATE ON task_creation_intents
WHEN NEW.export_json IS NOT NULL AND (SELECT importing FROM planning_peer_context)=0 AND (NEW.export_json IS NOT OLD.export_json OR NEW.export_attempted IS NOT OLD.export_attempted OR NEW.export_error IS NOT OLD.export_error)
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'task',NEW.task_id,'creation',json_object('export',json(NEW.export_json),'attempted',json(CASE WHEN NEW.export_attempted THEN 'true' ELSE 'false' END),'link_attempted',json('false'),'error',NEW.export_error),
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='task' AND object_id=NEW.task_id AND field='creation');
END;
INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'task',NEW.task_id,'creation',json_object('export',json(NEW.export_json),'attempted',json(CASE WHEN NEW.export_attempted THEN 'true' ELSE 'false' END),'link_attempted',json('false'),'error',NEW.export_error),0,NULL,'[]'
FROM task_creation_intents AS NEW WHERE NEW.export_json IS NOT NULL;

UPDATE projects SET export_json=json_remove(json_set(export_json,'$.parent',wave_id,'$.captured',json((
    SELECT json_group_array(id) FROM project_changes WHERE project_id=projects.id
    AND seq<=json_extract(projects.export_json,'$.through')))),'$.through')
WHERE export_json IS NOT NULL;
CREATE TRIGGER peer_project_creation_update AFTER UPDATE ON projects
WHEN NEW.export_json IS NOT NULL AND (SELECT importing FROM planning_peer_context)=0 AND (NEW.export_json IS NOT OLD.export_json OR NEW.export_attempted IS NOT OLD.export_attempted OR NEW.export_error IS NOT OLD.export_error OR NEW.export_link_attempted IS NOT OLD.export_link_attempted)
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'project',NEW.id,'creation',json_object('export',json(NEW.export_json),'attempted',json(CASE WHEN NEW.export_attempted THEN 'true' ELSE 'false' END),'link_attempted',json(CASE WHEN NEW.export_link_attempted THEN 'true' ELSE 'false' END),'error',NEW.export_error),
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='project' AND object_id=NEW.id AND field='creation');
END;
INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'project',NEW.id,'creation',json_object('export',json(NEW.export_json),'attempted',json(CASE WHEN NEW.export_attempted THEN 'true' ELSE 'false' END),'link_attempted',json(CASE WHEN NEW.export_link_attempted THEN 'true' ELSE 'false' END),'error',NEW.export_error),0,NULL,'[]'
FROM projects AS NEW WHERE NEW.export_json IS NOT NULL;
