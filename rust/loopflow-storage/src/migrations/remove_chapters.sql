-- A Wave no longer selects one of its Projects (LOO-995): it has exactly
-- one, and a Task in the Wave is in it. Released lf still reads these
-- members, so they stay, unwritten, until a contract drops them with their
-- idle triggers and rebuilds `waves`.
-- leaving: waves.current_project_id LOO-995
-- leaving: waves.project_activation_lf_process_id LOO-995
-- leaving: waves.project_activation_started_at LOO-995
-- leaving: waves.project_activation_pid LOO-995
-- leaving: waves.project_activation_os_started_at LOO-995
-- leaving: waves.project_activation_ended_at LOO-995
-- leaving: waves.project_activation_exit_code LOO-995
-- leaving: waves.project_activation_error LOO-995
-- Nothing wrote a successor rollover; its row also marked a Project as made
-- on this Machine, which `plan_stamps` now answers.
-- leaving: project_transitions LOO-995
-- KRs and metric targets are no longer sent to a tracker; an unsent edit of
-- either is retired, and their columns stay unwritten until the contract.
UPDATE tracker_actions SET phase='retired',error='KRs and metric targets are no longer sent'
WHERE subject IN ('krs','metric_targets') AND phase IN ('queued','sent','conflicted');
-- leaving: projects.krs LOO-995
-- leaving: projects.metric_targets LOO-995
-- leaving: tracker_projects.krs LOO-995
-- leaving: tracker_projects.metric_targets LOO-995
-- leaving: pm_projects.krs LOO-995
-- leaving: pm_projects.metric_targets LOO-995
-- Nothing reads or writes these since chapters ended.
-- leaving: projects.legacy_current LOO-995
-- leaving: project_events LOO-995
-- leaving: projects.iteration LOO-995
-- Live metrics are gone: no contract, instrument or observation is read.
-- leaving: metric_instruments LOO-995
-- leaving: metric_observations LOO-995
-- A Wave is exactly one Project. A Wave with several keeps its newest
-- started open one, else its newest; the others go with their unsent edits
-- and history. Their Tasks join the kept Project locally. Only existing
-- delivery requests can change tracker membership.
CREATE TEMP TABLE one_project_kept AS
SELECT w.wave_id, (SELECT k.id FROM projects k WHERE k.wave_id=w.wave_id
    ORDER BY (k.status='started' AND k.work_state='ready') DESC, k.created_at DESC, k.id DESC
    LIMIT 1) AS kept
FROM (SELECT DISTINCT wave_id FROM projects) w;
CREATE TEMP TABLE one_project_dropped AS
SELECT p.id, p.external_project_id AS external, k.kept FROM projects p JOIN one_project_kept k ON k.wave_id=p.wave_id
WHERE p.id!=k.kept;
CREATE UNIQUE INDEX one_project_dropped_id ON one_project_dropped(id);

-- Creates keep their identity through a lost reply. Change just the destination
-- a retry would send, leaving the rest of the saved fields intact.
WITH destinations AS (
    SELECT a.seq, f.fullkey || '.value' AS path, d.kept
    FROM tracker_actions a JOIN json_each(a.value_json) f
    JOIN one_project_dropped d ON d.id=json_extract(f.value,'$.value')
    WHERE a.task_id IS NOT NULL AND a.kind='create' AND a.phase IN ('queued','sent')
      AND json_extract(f.value,'$.field')='task_project'
)
UPDATE tracker_actions SET value_json=json_set(value_json,d.path,d.kept)
FROM destinations d WHERE tracker_actions.seq=d.seq;

-- A move has a desired destination and, until the next send, may also have
-- an earlier destination whose reply was lost. Compare that send and saved
-- observations by external ID once the local Project record is gone.
UPDATE tracker_actions AS a SET value_json=json_set(value_json,'$.value',d.kept),
    base_json=CASE WHEN phase='sent' AND d.external IS NOT NULL
        THEN json_set(COALESCE(base_json,'{}'),'$.sent_project',d.external) ELSE base_json END
FROM one_project_dropped d WHERE a.task_id IS NOT NULL AND a.subject='task_project'
    AND a.kind='field' AND a.phase IN ('queued','sent','conflicted')
    AND json_extract(a.value_json,'$.value')=d.id;
UPDATE tracker_actions AS a SET base_json=json_set(base_json,'$.value.value',d.external)
FROM one_project_dropped d WHERE a.task_id IS NOT NULL AND a.subject='task_project'
    AND a.phase IN ('queued','sent','conflicted') AND d.external IS NOT NULL
    AND json_extract(a.base_json,'$.value.value')=d.id;
UPDATE tracker_actions AS a SET conflict_json=json_set(conflict_json,'$.value.value',d.external)
FROM one_project_dropped d WHERE a.task_id IS NOT NULL AND a.subject='task_project'
    AND a.phase='conflicted' AND d.external IS NOT NULL
    AND json_extract(a.conflict_json,'$.value.value')=d.id;

UPDATE tasks SET project_id=(SELECT d.kept FROM one_project_dropped d WHERE d.id=tasks.project_id)
WHERE project_id IN (SELECT id FROM one_project_dropped);
UPDATE task_creation_intents SET project_id=(SELECT d.kept FROM one_project_dropped d
    WHERE d.id=task_creation_intents.project_id)
WHERE project_id IN (SELECT id FROM one_project_dropped);
UPDATE steers SET project_id=(SELECT d.kept FROM one_project_dropped d WHERE d.id=steers.project_id)
WHERE project_id IN (SELECT id FROM one_project_dropped);
UPDATE waves SET current_project_id=(SELECT d.kept FROM one_project_dropped d
    WHERE d.id=waves.current_project_id)
WHERE current_project_id IN (SELECT id FROM one_project_dropped);
DELETE FROM tracker_actions WHERE project_id IN (SELECT id FROM one_project_dropped);
DELETE FROM project_changes WHERE project_id IN (SELECT id FROM one_project_dropped);
DELETE FROM project_events WHERE project_id IN (SELECT id FROM one_project_dropped);
DELETE FROM plan_stamps WHERE owner_id IN (SELECT id FROM one_project_dropped);
DELETE FROM projects WHERE id IN (SELECT id FROM one_project_dropped);
DROP TABLE one_project_dropped;
DROP TABLE one_project_kept;
-- A live Wave with none gets one named for it. Its id is the Wave's UUID, so
-- every Machine that runs this, or later makes a missing one, makes the same
-- record; a historical Wave ID that is no hyphenated UUID gets a fresh one.
INSERT INTO projects(id,wave_id,created_at,updated_at,description,project_name,project_slug,status)
SELECT 'proj_'||CASE WHEN length(w.id)=36 AND replace(w.id,'-','') NOT GLOB '*[^0-9a-f]*'
        AND length(replace(w.id,'-',''))=32
    THEN replace(w.id,'-','') ELSE lower(hex(randomblob(16))) END,
    w.id,w.created_at,w.created_at,'',w.name,w.name,'started'
FROM waves w
WHERE w.retired_at IS NULL AND NOT EXISTS(SELECT 1 FROM projects p WHERE p.wave_id=w.id);
DROP INDEX idx_projects_wave;
CREATE UNIQUE INDEX projects_wave ON projects(wave_id);
-- A Wave's tracker Project is the one its Project links
-- (`projects.external_project_id`); no Initiative decides it, and nothing
-- orders a Wave's Projects. A queued attachment of a Project to an
-- Initiative goes.
-- leaving: tracker_wave_projects LOO-995
UPDATE tracker_actions SET phase='retired',error='Projects no longer join an Initiative'
WHERE project_id IS NOT NULL AND kind='link' AND phase IN ('queued','sent','conflicted');
-- leaving: tracker_wave_sync.initiative LOO-995
-- leaving: tracker_projects.initiative_ids LOO-995
-- leaving: tracker_projects.membership_unresolved LOO-995
-- leaving: projects.planning_initiatives LOO-995
-- A Wave's header links its tracker Project, so a read keeps the Project's URL.
ALTER TABLE tracker_projects ADD COLUMN url TEXT;
DROP TRIGGER store_revision_tracker_projects_update;
CREATE TRIGGER store_revision_tracker_projects_update AFTER UPDATE ON tracker_projects
WHEN NEW.repo IS NOT OLD.repo OR NEW.tracker IS NOT OLD.tracker OR
    NEW.id IS NOT OLD.id OR NEW.archived IS NOT OLD.archived OR
    NEW.membership_unresolved IS NOT OLD.membership_unresolved OR
    NEW.task_order_json IS NOT OLD.task_order_json OR
    NEW.revision IS NOT OLD.revision OR NEW.slug IS NOT OLD.slug OR
    NEW.name IS NOT OLD.name OR NEW.summary IS NOT OLD.summary OR
    NEW.status IS NOT OLD.status OR NEW.krs IS NOT OLD.krs OR
    NEW.metric_targets IS NOT OLD.metric_targets OR
    NEW.initiative_ids IS NOT OLD.initiative_ids OR
    NEW.team_ids IS NOT OLD.team_ids OR NEW.description IS NOT OLD.description OR
    NEW.description_revision IS NOT OLD.description_revision OR NEW.url IS NOT OLD.url OR
    (NEW.observed_at IS NULL) IS NOT (OLD.observed_at IS NULL) OR
    (NEW.description_observed_at IS NULL) IS NOT (OLD.description_observed_at IS NULL)
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
