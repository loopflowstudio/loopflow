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
-- and history, and their Tasks join the kept one here only: nothing is sent
-- to a tracker for them.
CREATE TEMP TABLE one_project_kept AS
SELECT w.wave_id, (SELECT k.id FROM projects k WHERE k.wave_id=w.wave_id
    ORDER BY (k.status='started' AND k.work_state='ready') DESC, k.created_at DESC, k.id DESC
    LIMIT 1) AS kept
FROM (SELECT DISTINCT wave_id FROM projects) w;
CREATE TEMP TABLE one_project_dropped AS
SELECT p.id, k.kept FROM projects p JOIN one_project_kept k ON k.wave_id=p.wave_id
WHERE p.id!=k.kept;
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
-- Nothing orders a Wave's Projects.
-- leaving: tracker_wave_projects.position LOO-995
-- A Wave's tracker Project is the one its Project links; no Initiative
-- decides it. A link to any other Project goes, and so does a queued
-- attachment of a Project to an Initiative.
DELETE FROM tracker_wave_projects WHERE project_id NOT IN (
    SELECT p.external_project_id FROM projects p
    WHERE p.wave_id=tracker_wave_projects.wave_id AND p.external_project_id IS NOT NULL);
UPDATE tracker_actions SET phase='retired',error='Projects no longer join an Initiative'
WHERE project_id IS NOT NULL AND kind='link' AND phase IN ('queued','sent','conflicted');
-- leaving: tracker_wave_sync.initiative LOO-995
-- leaving: tracker_projects.initiative_ids LOO-995
-- leaving: tracker_projects.membership_unresolved LOO-995
-- leaving: projects.planning_initiatives LOO-995
