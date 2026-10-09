-- depends_on: local_planning
-- The migration data hook seeds retained provider invalidation/removal/archive evidence with unknown age.
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
-- Projection and effect acquisition consult the same retained provider claims.
-- Losing mappings and captured creation inputs still name their original object;
-- this view neither associates Work IDs nor changes sharing selection.
CREATE VIEW planning_peer_provider_claims AS
SELECT kind,object_id,
    CASE field WHEN 'creation' THEN json_extract(value,'$.export.id')
        ELSE json_extract(value,'$') END AS provider_id
FROM planning_peer_changes
WHERE (kind='task' AND field IN ('external_issue_id','creation'))
   OR (kind='project' AND field IN ('external_project_id','creation'));
-- Explicit local correspondence never merges Work or shares private selection.
-- An incoming journal identity resolves to an existing local execution owner.
CREATE TABLE planning_associations (
    kind TEXT NOT NULL CHECK(kind IN ('task','project')),
    origin_id TEXT NOT NULL,
    repo TEXT NOT NULL,
    provider_id TEXT NOT NULL,
    task_id TEXT REFERENCES tasks(id),
    project_id TEXT REFERENCES projects(id),
    PRIMARY KEY(kind,origin_id),
    CHECK((kind='task' AND task_id IS NOT NULL AND project_id IS NULL)
       OR (kind='project' AND project_id IS NOT NULL AND task_id IS NULL)),
    CHECK(origin_id != COALESCE(task_id,project_id))
);
CREATE TRIGGER store_revision_planning_associations_insert AFTER INSERT ON planning_associations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
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

-- Creation origin names the captured operation, not its local projection.
-- Multiple origins may retain attempts against one local Work without rewriting
-- their captured models, provider UUIDs, or private sharing membership.
CREATE TABLE planning_creations (
    kind TEXT NOT NULL CHECK(kind IN ('task','project')),
    origin_id TEXT NOT NULL,
    task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    export_json TEXT CHECK(export_json IS NULL OR json_valid(export_json)),
    export_attempted INTEGER NOT NULL DEFAULT 0 CHECK(export_attempted IN (0,1)),
    export_link_attempted INTEGER NOT NULL DEFAULT 0 CHECK(export_link_attempted IN (0,1)),
    export_error TEXT,
    export_acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(export_acknowledged IN (0,1)),
    PRIMARY KEY(kind,origin_id),
    CHECK((kind='task' AND task_id IS NOT NULL AND project_id IS NULL AND export_link_attempted=0)
       OR (kind='project' AND project_id IS NOT NULL AND task_id IS NULL)),
    CHECK(export_attempted=0 OR export_json IS NOT NULL),
    CHECK(export_link_attempted=0 OR export_attempted=1),
    CHECK(export_acknowledged=0 OR export_attempted=1)
);
CREATE INDEX planning_creations_task ON planning_creations(task_id);
CREATE INDEX planning_creations_project ON planning_creations(project_id);

-- Migrate the released owners directly into the final representation.
INSERT INTO planning_creations(kind,origin_id,task_id,export_json,export_attempted,export_error)
SELECT 'task',c.task_id,c.task_id,
    json_remove(json_set(c.export_json,'$.parent',(SELECT p.id FROM projects p WHERE
        p.id=json_extract(c.export_json,'$.model.project_id') OR p.external_project_id=json_extract(c.export_json,'$.model.project_id')),
        '$.captured',json((SELECT json_group_array(id) FROM task_changes WHERE task_id=c.task_id
            AND seq<=json_extract(c.export_json,'$.through')))),'$.through'),
    c.export_attempted,c.export_error
FROM task_creation_intents c WHERE c.export_json IS NOT NULL OR c.export_error IS NOT NULL;
INSERT INTO planning_creations(kind,origin_id,project_id,export_json,export_attempted,export_link_attempted,export_error)
SELECT 'project',p.id,p.id,
    json_remove(json_set(c.export_json,'$.parent',p.wave_id,'$.captured',json((
        SELECT json_group_array(id) FROM project_changes WHERE project_id=p.id
        AND seq<=json_extract(c.export_json,'$.through')))),'$.through'),
    c.export_attempted,c.export_link_attempted,c.export_error
FROM project_transitions c JOIN projects p ON p.id=c.successor_id AND p.wave_id=c.wave_id
WHERE c.export_json IS NOT NULL OR c.export_error IS NOT NULL;
ALTER TABLE task_creation_intents DROP COLUMN export_json;
ALTER TABLE task_creation_intents DROP COLUMN export_attempted;
ALTER TABLE task_creation_intents DROP COLUMN export_error;
ALTER TABLE project_transitions DROP COLUMN export_json;
ALTER TABLE project_transitions DROP COLUMN export_attempted;
ALTER TABLE project_transitions DROP COLUMN export_link_attempted;
ALTER TABLE project_transitions DROP COLUMN export_error;

-- One pending-creation reader for foreground delivery and presentation.
-- Keep each origin visible; a mapping is not acknowledgement.
CREATE VIEW planning_exports AS
SELECT 'project' AS kind,p.id,COALESCE(c.origin_id,p.id) AS origin_id,w.repo,
    COALESCE(json_extract(c.export_json,'$.input'),json_object('name',p.project_name)) AS input,
    COALESCE(c.export_attempted OR c.export_link_attempted,0) AS attempted,c.export_error AS error
FROM projects p JOIN waves w ON w.id=p.wave_id
LEFT JOIN planning_creations c ON c.project_id=p.id
WHERE (c.export_json IS NULL AND p.external_project_id IS NULL) OR (c.export_json IS NOT NULL AND c.export_acknowledged=0)
UNION ALL
SELECT 'task',t.id,COALESCE(c.origin_id,t.id),w.repo,
    COALESCE(json_extract(c.export_json,'$.input'),json_object('title',t.issue_title,'description',t.issue_description)),
    COALESCE(c.export_attempted,0),c.export_error
FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id
LEFT JOIN planning_creations c ON c.task_id=t.id
WHERE ((c.export_json IS NULL AND t.external_issue_id IS NULL) OR (c.export_json IS NOT NULL AND c.export_acknowledged=0))
AND (t.planning_deleted_at IS NULL OR c.export_attempted=1);

CREATE TRIGGER peer_creation_insert AFTER INSERT ON planning_creations
WHEN NEW.export_json IS NOT NULL AND (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),NEW.kind,NEW.origin_id,'creation',json_object('export',json(NEW.export_json),'attempted',json(CASE WHEN NEW.export_attempted THEN 'true' ELSE 'false' END),'link_attempted',json(CASE WHEN NEW.export_link_attempted THEN 'true' ELSE 'false' END),'error',NEW.export_error,'acknowledged',json(CASE WHEN NEW.export_acknowledged THEN 'true' ELSE 'false' END)),
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind=NEW.kind AND object_id=NEW.origin_id AND field='creation');
END;
CREATE TRIGGER peer_creation_update AFTER UPDATE ON planning_creations
WHEN NEW.export_json IS NOT NULL AND (SELECT importing FROM planning_peer_context)=0 AND (NEW.export_json IS NOT OLD.export_json OR NEW.export_attempted IS NOT OLD.export_attempted OR NEW.export_error IS NOT OLD.export_error OR NEW.export_link_attempted IS NOT OLD.export_link_attempted OR NEW.export_acknowledged IS NOT OLD.export_acknowledged)
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),NEW.kind,NEW.origin_id,'creation',json_object('export',json(NEW.export_json),'attempted',json(CASE WHEN NEW.export_attempted THEN 'true' ELSE 'false' END),'link_attempted',json(CASE WHEN NEW.export_link_attempted THEN 'true' ELSE 'false' END),'error',NEW.export_error,'acknowledged',json(CASE WHEN NEW.export_acknowledged THEN 'true' ELSE 'false' END)),
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind=NEW.kind AND object_id=NEW.origin_id AND field='creation');
END;
INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),NEW.kind,NEW.origin_id,'creation',json_object('export',json(NEW.export_json),'attempted',json(CASE WHEN NEW.export_attempted THEN 'true' ELSE 'false' END),'link_attempted',json(CASE WHEN NEW.export_link_attempted THEN 'true' ELSE 'false' END),'error',NEW.export_error,'acknowledged',json(CASE WHEN NEW.export_acknowledged THEN 'true' ELSE 'false' END)),0,NULL,'[]'
FROM planning_creations AS NEW WHERE NEW.export_json IS NOT NULL;
CREATE TRIGGER store_revision_planning_creations_insert AFTER INSERT ON planning_creations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_creations_update AFTER UPDATE ON planning_creations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_creations_delete AFTER DELETE ON planning_creations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

-- Capture the removal's original save time, independently of current visibility.
ALTER TABLE task_changes ADD COLUMN deletion_saved_at INTEGER;
UPDATE task_changes SET deletion_saved_at=(SELECT planning_deleted_at FROM tasks WHERE id=task_id)
WHERE field='deleted';

-- Each deletion receipt retains its own identity and causal history. A later save
-- cannot clock-select away an earlier uncertain provider effect.
CREATE TRIGGER peer_task_deletion_insert AFTER INSERT ON task_changes
WHEN NEW.field='deleted' AND (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'task',NEW.task_id,'deletion:'||NEW.id,json_object('deleted_at',NEW.deletion_saved_at,'base',json(NEW.base_json),'attempted',json(CASE WHEN NEW.attempted THEN 'true' ELSE 'false' END),'acknowledged',json(CASE WHEN NEW.acknowledged THEN 'true' ELSE 'false' END),'acknowledged_revision',NEW.acknowledged_revision,'conflict',json(NEW.conflict_json),'error',NEW.error),
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='task' AND object_id=NEW.task_id AND field='deletion:'||NEW.id);
END;
CREATE TRIGGER peer_task_deletion_update AFTER UPDATE ON task_changes
WHEN NEW.field='deleted' AND (SELECT importing FROM planning_peer_context)=0 AND (NEW.deletion_saved_at IS NOT OLD.deletion_saved_at OR NEW.base_json IS NOT OLD.base_json OR NEW.attempted IS NOT OLD.attempted OR NEW.acknowledged IS NOT OLD.acknowledged OR NEW.acknowledged_revision IS NOT OLD.acknowledged_revision OR NEW.conflict_json IS NOT OLD.conflict_json OR NEW.error IS NOT OLD.error)
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'task',NEW.task_id,'deletion:'||NEW.id,json_object('deleted_at',NEW.deletion_saved_at,'base',json(NEW.base_json),'attempted',json(CASE WHEN NEW.attempted THEN 'true' ELSE 'false' END),'acknowledged',json(CASE WHEN NEW.acknowledged THEN 'true' ELSE 'false' END),'acknowledged_revision',NEW.acknowledged_revision,'conflict',json(NEW.conflict_json),'error',NEW.error),
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='task' AND object_id=NEW.task_id AND field='deletion:'||NEW.id);
END;
INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'task',NEW.task_id,'deletion:'||NEW.id,json_object('deleted_at',NEW.deletion_saved_at,'base',json(NEW.base_json),'attempted',json(CASE WHEN NEW.attempted THEN 'true' ELSE 'false' END),'acknowledged',json(CASE WHEN NEW.acknowledged THEN 'true' ELSE 'false' END),'acknowledged_revision',NEW.acknowledged_revision,'conflict',json(NEW.conflict_json),'error',NEW.error),0,NULL,'[]'
FROM task_changes AS NEW WHERE NEW.field='deleted';

-- Ordering retains every move receipt. The first mutation's logical clock orders
-- intentions; later attempts/readbacks never make an old intention newest.
CREATE TRIGGER peer_project_order_insert AFTER INSERT ON project_changes
WHEN NEW.field='task_order' AND (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'project',NEW.project_id,'order:'||NEW.id,json_object('desired',json(NEW.value_json),'base',json(NEW.base_json),'effects',json(NEW.order_effects_json),'attempted',json(CASE WHEN NEW.attempted THEN 'true' ELSE 'false' END),'acknowledged',json(CASE WHEN NEW.acknowledged THEN 'true' ELSE 'false' END),'conflict',json(NEW.conflict_json),'error',NEW.error),
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='project' AND object_id=NEW.project_id AND field='order:'||NEW.id);
END;
CREATE TRIGGER peer_project_order_update AFTER UPDATE ON project_changes
WHEN NEW.field='task_order' AND (SELECT importing FROM planning_peer_context)=0 AND (NEW.value_json IS NOT OLD.value_json OR NEW.base_json IS NOT OLD.base_json OR NEW.order_effects_json IS NOT OLD.order_effects_json OR NEW.attempted IS NOT OLD.attempted OR NEW.acknowledged IS NOT OLD.acknowledged OR NEW.conflict_json IS NOT OLD.conflict_json OR NEW.error IS NOT OLD.error)
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    SELECT lower(hex(randomblob(16))),'project',NEW.project_id,'order:'||NEW.id,json_object('desired',json(NEW.value_json),'base',json(NEW.base_json),'effects',json(NEW.order_effects_json),'attempted',json(CASE WHEN NEW.attempted THEN 'true' ELSE 'false' END),'acknowledged',json(CASE WHEN NEW.acknowledged THEN 'true' ELSE 'false' END),'conflict',json(NEW.conflict_json),'error',NEW.error),
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_heads WHERE kind='project' AND object_id=NEW.project_id AND field='order:'||NEW.id);
END;
INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'project',NEW.project_id,'order:'||NEW.id,json_object('desired',json(NEW.value_json),'base',json(NEW.base_json),'effects',json(NEW.order_effects_json),'attempted',json(CASE WHEN NEW.attempted THEN 'true' ELSE 'false' END),'acknowledged',json(CASE WHEN NEW.acknowledged THEN 'true' ELSE 'false' END),'conflict',json(NEW.conflict_json),'error',NEW.error),NEW.seq,NULL,'[]'
FROM project_changes AS NEW WHERE NEW.field='task_order';

CREATE VIEW planning_order_current AS
SELECT project_id,id FROM (
    SELECT c.project_id,c.id,row_number() OVER (
        PARTITION BY c.project_id ORDER BY min(m.clock) DESC,c.id DESC) AS position
    FROM project_changes c JOIN planning_peer_changes m
        ON m.kind='project' AND m.object_id=c.project_id AND m.field='order:'||c.id
    WHERE c.field='task_order' GROUP BY c.project_id,c.id
) WHERE position=1;

-- Superseded intentions keep their receipts, but only an unresolved effect can
-- delay the selected save. Settled partial progress is not a new desired order.
CREATE VIEW planning_order_deliveries AS
SELECT id FROM project_changes WHERE field='task_order' AND acknowledged=0 AND conflict_json IS NULL
AND (id IN (SELECT id FROM planning_order_current)
    OR (attempted=1 AND json_extract(order_effects_json,'$[#-1].settled')=0));
