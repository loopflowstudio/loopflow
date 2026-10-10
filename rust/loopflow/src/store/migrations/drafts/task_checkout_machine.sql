-- depends_on: local_planning, planning_peers

ALTER TABLE tasks ADD COLUMN checkout_machine_id TEXT REFERENCES machines(id);

-- Preserve the only recorded location evidence. Missing placement stays unknown.
UPDATE tasks SET checkout_machine_id = (
    SELECT machine_id FROM work_placements WHERE task_id = tasks.id
) WHERE worktree != '';

-- Historical assignments may be explicit or copied. Never infer that distinction
-- from equality with a parent's Machine. New children resolve their ancestry.
-- A portable assignment may name a Machine with no local connection. Keep its
-- identity without manufacturing a route; execution locations still require one.
CREATE TABLE delegated_placements (
    wave_id TEXT REFERENCES waves(id) ON DELETE CASCADE,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    machine_id TEXT NOT NULL,
    placed_at INTEGER NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    provenance TEXT NOT NULL DEFAULT 'legacy' CHECK (provenance IN ('explicit', 'legacy')),
    CHECK ((wave_id IS NOT NULL) + (project_id IS NOT NULL) + (task_id IS NOT NULL) = 1),
    UNIQUE (wave_id), UNIQUE (project_id), UNIQUE (task_id)
);
INSERT INTO delegated_placements
    SELECT wave_id,project_id,task_id,machine_id,placed_at,enabled,'legacy' FROM work_placements;
DROP TABLE work_placements;
ALTER TABLE delegated_placements RENAME TO work_placements;
CREATE INDEX idx_work_placements_machine ON work_placements(machine_id,placed_at);

-- The selected plan's repository identity is independent of its local locator.
-- Peer adoption supplies the same id explicitly; a code remote implies nothing.
CREATE TABLE repository_plans (
    id TEXT PRIMARY KEY NOT NULL,
    repo TEXT NOT NULL,
    selected INTEGER NOT NULL CHECK (selected IN (0, 1))
);
CREATE UNIQUE INDEX repository_selected_plan ON repository_plans(repo) WHERE selected=1;
INSERT INTO repository_plans(repo,id,selected)
    SELECT repo, 'repo_' || lower(hex(randomblob(16))), 1 FROM
        (SELECT DISTINCT repo FROM waves);

CREATE TRIGGER store_revision_work_placements_insert AFTER INSERT ON work_placements
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_work_placements_update AFTER UPDATE ON work_placements
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER store_revision_work_placements_delete AFTER DELETE ON work_placements
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;

CREATE TRIGGER peer_wave_delegation_insert AFTER INSERT ON waves
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    VALUES(lower(hex(randomblob(16))),'wave',NEW.id,'delegation','null',
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,'[]');
END;
INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'wave',r.id,'delegation',
    CASE WHEN p.machine_id IS NULL THEN 'null' ELSE json_object(
        'machine_id',p.machine_id,'placed_at',p.placed_at,'provenance',p.provenance) END,
    0,NULL,'[]' FROM waves r LEFT JOIN work_placements p ON p.wave_id=r.id;

CREATE TRIGGER peer_project_delegation_insert AFTER INSERT ON projects
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    VALUES(lower(hex(randomblob(16))),'project',NEW.id,'delegation','null',
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,'[]');
END;
INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'project',r.id,'delegation',
    CASE WHEN p.machine_id IS NULL THEN 'null' ELSE json_object(
        'machine_id',p.machine_id,'placed_at',p.placed_at,'provenance',p.provenance) END,
    0,NULL,'[]' FROM projects r LEFT JOIN work_placements p ON p.project_id=r.id;

CREATE TRIGGER peer_task_delegation_insert AFTER INSERT ON tasks
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    VALUES(lower(hex(randomblob(16))),'task',NEW.id,'delegation','null',
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,'[]');
END;
INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
SELECT lower(hex(randomblob(16))),'task',r.id,'delegation',
    CASE WHEN p.machine_id IS NULL THEN 'null' ELSE json_object(
        'machine_id',p.machine_id,'placed_at',p.placed_at,'provenance',p.provenance) END,
    0,NULL,'[]' FROM tasks r LEFT JOIN work_placements p ON p.task_id=r.id;

CREATE TRIGGER peer_delegation_insert AFTER INSERT ON work_placements
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    VALUES(lower(hex(randomblob(16))),CASE WHEN NEW.wave_id IS NOT NULL THEN 'wave' WHEN NEW.project_id IS NOT NULL THEN 'project' ELSE 'task' END,COALESCE(NEW.wave_id,NEW.project_id,NEW.task_id),'delegation',json_object('machine_id',NEW.machine_id,'placed_at',NEW.placed_at,'provenance',NEW.provenance),
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_capture_heads
            WHERE kind=(CASE WHEN NEW.wave_id IS NOT NULL THEN 'wave' WHEN NEW.project_id IS NOT NULL THEN 'project' ELSE 'task' END) AND object_id=COALESCE(NEW.wave_id,NEW.project_id,NEW.task_id) AND field='delegation'));
END;

CREATE TRIGGER peer_delegation_update AFTER UPDATE ON work_placements
WHEN (SELECT importing FROM planning_peer_context)=0
    AND (NEW.machine_id IS NOT OLD.machine_id OR NEW.placed_at IS NOT OLD.placed_at OR NEW.provenance IS NOT OLD.provenance)
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    VALUES(lower(hex(randomblob(16))),CASE WHEN NEW.wave_id IS NOT NULL THEN 'wave' WHEN NEW.project_id IS NOT NULL THEN 'project' ELSE 'task' END,COALESCE(NEW.wave_id,NEW.project_id,NEW.task_id),'delegation',json_object('machine_id',NEW.machine_id,'placed_at',NEW.placed_at,'provenance',NEW.provenance),
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_capture_heads
            WHERE kind=(CASE WHEN NEW.wave_id IS NOT NULL THEN 'wave' WHEN NEW.project_id IS NOT NULL THEN 'project' ELSE 'task' END) AND object_id=COALESCE(NEW.wave_id,NEW.project_id,NEW.task_id) AND field='delegation'));
END;

CREATE TRIGGER peer_delegation_delete AFTER DELETE ON work_placements
WHEN (SELECT importing FROM planning_peer_context)=0
BEGIN
    INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
    VALUES(lower(hex(randomblob(16))),CASE WHEN OLD.wave_id IS NOT NULL THEN 'wave' WHEN OLD.project_id IS NOT NULL THEN 'project' ELSE 'task' END,COALESCE(OLD.wave_id,OLD.project_id,OLD.task_id),'delegation','null',
        max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE((SELECT max(clock)+1 FROM planning_peer_changes),0)),NULL,
        (SELECT json_group_array(id) FROM planning_peer_capture_heads
            WHERE kind=(CASE WHEN OLD.wave_id IS NOT NULL THEN 'wave' WHEN OLD.project_id IS NOT NULL THEN 'project' ELSE 'task' END) AND object_id=COALESCE(OLD.wave_id,OLD.project_id,OLD.task_id) AND field='delegation'));
END;
