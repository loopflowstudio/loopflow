-- Bound identity validation during statement preparation. SQLite otherwise
-- substitutes the nested CASE expressions at every reference while holding the
-- writer reservation. Materialization retains the same canonical checks.

DROP TRIGGER canonical_agent_sessions_insert;
CREATE TRIGGER canonical_agent_sessions_insert BEFORE INSERT ON agent_sessions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: agent_sessions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.transport_lf_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_linear_team_checks_insert;
CREATE TRIGGER canonical_linear_team_checks_insert BEFORE INSERT ON linear_team_checks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: linear_team_checks') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.team_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_machines_insert;
CREATE TRIGGER canonical_machines_insert BEFORE INSERT ON machines
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: machines') FROM (
 WITH incoming AS (
SELECT 'home_' AS domain,NEW.id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_planning_peer_changes_insert;
CREATE TRIGGER canonical_planning_peer_changes_insert BEFORE INSERT ON planning_peer_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.edit.target' AND NEW.record_table='waves' THEN 'uuid' WHEN path='$.edit.target' AND NEW.record_table='projects' THEN 'proj_' WHEN path='$.edit.target' AND NEW.record_table='tasks' THEN 'task_' WHEN path='$.edit.target' AND NEW.record_table='task_comments' THEN 'uuid' WHEN parent='$.parents' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('comment_task') THEN 'task_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('task_identifier') THEN 'label' WHEN parent='$.edit.change.value' AND typeof(key)='integer' AND json_extract(doc,'$.edit.change.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.edit.change.value.machine_id' AND json_extract(doc,'$.edit.change.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.edit.change.value.repository_id' AND json_extract(doc,'$.edit.change.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.mutation AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.mutation) THEN NEW.mutation END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_planning_peer_conflicts_insert;
CREATE TRIGGER canonical_planning_peer_conflicts_insert BEFORE INSERT ON planning_peer_conflicts
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_conflicts') FROM (
 WITH incoming AS (
SELECT CASE NEW.kind WHEN 'waves' THEN 'uuid' WHEN 'projects' THEN 'proj_' WHEN 'tasks' THEN 'task_' WHEN 'task_comments' THEN 'uuid' END AS domain,NEW.object_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pm_issue_changes_insert;
CREATE TRIGGER canonical_pm_issue_changes_insert BEFORE INSERT ON pm_issue_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_issue_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.issue_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pm_items_insert;
CREATE TRIGGER canonical_pm_items_insert BEFORE INSERT ON pm_items
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_items') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.team_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.claimed_project_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pm_items_update;
CREATE TRIGGER canonical_pm_items_update BEFORE UPDATE OF id,project_id,team_id,assignee,claimed_project_id,identifier ON pm_items
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_items') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.team_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.claimed_project_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pm_project_name_cutover_insert;
CREATE TRIGGER canonical_pm_project_name_cutover_insert BEFORE INSERT ON pm_project_name_cutover
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_project_name_cutover') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN parent='$.initiative_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.team_ids' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.body AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.body) THEN NEW.body END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pm_project_name_cutover_update;
CREATE TRIGGER canonical_pm_project_name_cutover_update BEFORE UPDATE OF id,body ON pm_project_name_cutover
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_project_name_cutover') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN parent='$.initiative_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.team_ids' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.body AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.body) THEN NEW.body END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pm_projects_insert;
CREATE TRIGGER canonical_pm_projects_insert BEFORE INSERT ON pm_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.initiative_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.initiative_ids) THEN NEW.initiative_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.team_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.team_ids) THEN NEW.team_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.task_order_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.task_order_json) THEN NEW.task_order_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pm_projects_update;
CREATE TRIGGER canonical_pm_projects_update BEFORE UPDATE OF id,initiative_ids,team_ids,task_order_json ON pm_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.initiative_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.initiative_ids) THEN NEW.initiative_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.team_ids AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.team_ids) THEN NEW.team_ids END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.task_order_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.task_order_json) THEN NEW.task_order_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pm_wave_projects_insert;
CREATE TRIGGER canonical_pm_wave_projects_insert BEFORE INSERT ON pm_wave_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_wave_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pm_wave_sync_insert;
CREATE TRIGGER canonical_pm_wave_sync_insert BEFORE INSERT ON pm_wave_sync
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_wave_sync') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.initiative AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pm_wave_sync_update;
CREATE TRIGGER canonical_pm_wave_sync_update BEFORE UPDATE OF wave_id,initiative ON pm_wave_sync
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_wave_sync') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.initiative AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pr_landings_insert;
CREATE TRIGGER canonical_pr_landings_insert BEFORE INSERT ON pr_landings
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pr_landings') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.supervisor_machine_id AS original
UNION ALL
SELECT 'landing_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.home_id' THEN 'home_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.retired_supervisor_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.retired_supervisor_json) THEN NEW.retired_supervisor_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_pr_landings_update;
CREATE TRIGGER canonical_pr_landings_update BEFORE UPDATE OF task_id,supervisor_machine_id,id,retired_supervisor_json ON pr_landings
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pr_landings') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.supervisor_machine_id AS original
UNION ALL
SELECT 'landing_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.home_id' THEN 'home_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.retired_supervisor_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.retired_supervisor_json) THEN NEW.retired_supervisor_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_processes_insert;
CREATE TRIGGER canonical_processes_insert BEFORE INSERT ON processes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: processes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.trace_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.parent_lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.caller_agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attached_lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'session' AS domain,NEW.caller_session_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.agent_session_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_processes_update;
CREATE TRIGGER canonical_processes_update BEFORE UPDATE OF id,trace_id,parent_lf_process_id,caller_agent_process_id,attached_lf_process_id,attachment_token,caller_session_id,agent_session_id ON processes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: processes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.trace_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.parent_lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.caller_agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attached_lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'session' AS domain,NEW.caller_session_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.agent_session_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_project_changes_insert;
CREATE TRIGGER canonical_project_changes_insert BEFORE INSERT ON project_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.value_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.value_json) THEN NEW.value_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' WHEN parent='$.value' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.base_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.base_json) THEN NEW.base_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' WHEN parent='$.value' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflict_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflict_json) THEN NEW.conflict_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN (parent GLOB '$[[]*].before' OR parent GLOB '$[[]*].after') AND typeof(key)='integer' THEN 'task_' WHEN parent GLOB '$[[]*]' AND key='issue' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.order_effects_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.order_effects_json) THEN NEW.order_effects_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_project_changes_update;
CREATE TRIGGER canonical_project_changes_update BEFORE UPDATE OF id,project_id,value_json,base_json,conflict_json,order_effects_json ON project_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' WHEN parent='$' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.value_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.value_json) THEN NEW.value_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' WHEN parent='$.value' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.base_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.base_json) THEN NEW.base_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' WHEN parent='$.value' AND typeof(key)='integer' THEN 'task_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflict_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflict_json) THEN NEW.conflict_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN (parent GLOB '$[[]*].before' OR parent GLOB '$[[]*].after') AND typeof(key)='integer' THEN 'task_' WHEN parent GLOB '$[[]*]' AND key='issue' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.order_effects_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.order_effects_json) THEN NEW.order_effects_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_project_events_insert;
CREATE TRIGGER canonical_project_events_insert BEFORE INSERT ON project_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_events') FROM (
 WITH incoming AS (
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.event.pr_id' THEN 'pr_' WHEN path='$.event.steer_id' THEN 'uuid' WHEN path='$.event.invocation_id' THEN 'uuid' WHEN path='$.event.relation_id' THEN 'uuid' WHEN path='$.event.source_issue_id' THEN 'uuid' WHEN path='$.event.target_issue_id' THEN 'uuid' WHEN parent='$.event.lf_process_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.event.exec_ids' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.event.intent.issue_id' THEN 'task_or_uuid' WHEN path='$.event.intent.project_id' THEN 'project_or_uuid' WHEN path='$.event.intent.relation_id' THEN 'uuid' WHEN path='$.event.intent.team_id' THEN 'uuid' WHEN path='$.event.intent.state_id' THEN 'uuid' WHEN path='$.event.intent.identifier' THEN 'label' WHEN path='$.event.link.issue_id' THEN 'task_or_uuid' WHEN path='$.event.link.project_id' THEN 'project_or_uuid' WHEN path='$.event.link.relation_id' THEN 'uuid' WHEN path='$.event.link.team_id' THEN 'uuid' WHEN path='$.event.link.state_id' THEN 'uuid' WHEN path='$.event.link.identifier' THEN 'label' WHEN path='$.task_id' THEN 'task_' WHEN path='$.steer_id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.kind_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.kind_json) THEN NEW.kind_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_project_transition_items_insert;
CREATE TRIGGER canonical_project_transition_items_insert BEFORE INSERT ON project_transition_items
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_transition_items') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.successor_id AS original
UNION ALL
SELECT 'task_or_uuid' AS domain,NEW.issue_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_project_transitions_insert;
CREATE TRIGGER canonical_project_transitions_insert BEFORE INSERT ON project_transitions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_transitions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.successor_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.predecessor_id AS original
UNION ALL
SELECT CASE WHEN path='$.wave_id' THEN 'uuid' WHEN path='$.successor_id' THEN 'project_or_uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.local_plan_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.local_plan_json) THEN NEW.local_plan_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN path='$.initiative' THEN 'uuid' WHEN path='$.link_id' THEN 'uuid' WHEN path='$.input.id' THEN 'uuid' WHEN path='$.input.teamId' THEN 'uuid' WHEN path='$.input.projectId' THEN 'uuid' WHEN path='$.input.assigneeId' THEN 'uuid' WHEN parent='$.input.teamIds' AND typeof(key)='integer' THEN 'uuid' WHEN (parent GLOB '$.baseline[[]*].value') AND typeof(key)='integer' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*].value' AND key='machine_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN parent GLOB '$.baseline[[]*].value' AND key='repository_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field')='task_admission_authority' THEN 'repo_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_project','task_project') THEN 'proj_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('comment_task') THEN 'task_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('task_identifier') THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.export_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.export_json) THEN NEW.export_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_project_transitions_update;
CREATE TRIGGER canonical_project_transitions_update BEFORE UPDATE OF wave_id,successor_id,predecessor_id,local_plan_json,export_json ON project_transitions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_transitions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.successor_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.predecessor_id AS original
UNION ALL
SELECT CASE WHEN path='$.wave_id' THEN 'uuid' WHEN path='$.successor_id' THEN 'project_or_uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.local_plan_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.local_plan_json) THEN NEW.local_plan_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN path='$.initiative' THEN 'uuid' WHEN path='$.link_id' THEN 'uuid' WHEN path='$.input.id' THEN 'uuid' WHEN path='$.input.teamId' THEN 'uuid' WHEN path='$.input.projectId' THEN 'uuid' WHEN path='$.input.assigneeId' THEN 'uuid' WHEN parent='$.input.teamIds' AND typeof(key)='integer' THEN 'uuid' WHEN (parent GLOB '$.baseline[[]*].value') AND typeof(key)='integer' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*].value' AND key='machine_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN parent GLOB '$.baseline[[]*].value' AND key='repository_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field')='task_admission_authority' THEN 'repo_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_project','task_project') THEN 'proj_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('comment_task') THEN 'task_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('task_identifier') THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.export_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.export_json) THEN NEW.export_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_projects_insert;
CREATE TRIGGER canonical_projects_insert BEFORE INSERT ON projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_project_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.planning_initiatives AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.planning_initiatives) THEN NEW.planning_initiatives END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.planning_teams AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.planning_teams) THEN NEW.planning_teams END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_projects_update;
CREATE TRIGGER canonical_projects_update BEFORE UPDATE OF wave_id,external_project_id,id,planning_initiatives,planning_teams ON projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_project_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.planning_initiatives AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.planning_initiatives) THEN NEW.planning_initiatives END) WHERE type='text')
UNION ALL
SELECT CASE WHEN parent='$' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.planning_teams AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.planning_teams) THEN NEW.planning_teams END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_repository_plans_insert;
CREATE TRIGGER canonical_repository_plans_insert BEFORE INSERT ON repository_plans
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: repository_plans') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_session_activity_insert;
CREATE TRIGGER canonical_session_activity_insert BEFORE INSERT ON session_activity
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_activity') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_session_activity_update;
CREATE TRIGGER canonical_session_activity_update BEFORE UPDATE OF attachment_token,agent_process_id,session_id ON session_activity
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_activity') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_session_events_insert;
CREATE TRIGGER canonical_session_events_insert BEFORE INSERT ON session_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_events') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
UNION ALL
SELECT CASE WHEN path='$.flow_id' AND json_extract(doc,'$.type') IN ('legacy_flow','legacy_flow_review') THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.payload AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.payload) THEN NEW.payload END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_session_events_update;
CREATE TRIGGER canonical_session_events_update BEFORE UPDATE OF lf_process_id,wave_id,agent_process_id,task_id,session_id,payload ON session_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_events') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.lf_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
UNION ALL
SELECT CASE WHEN path='$.flow_id' AND json_extract(doc,'$.type') IN ('legacy_flow','legacy_flow_review') THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.payload AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.payload) THEN NEW.payload END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_steer_batches_insert;
CREATE TRIGGER canonical_steer_batches_insert BEFORE INSERT ON steer_batches
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steer_batches') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_steer_batches_update;
CREATE TRIGGER canonical_steer_batches_update BEFORE UPDATE OF id,agent_process_id,attachment_token,session_id ON steer_batches
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steer_batches') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_steers_insert;
CREATE TRIGGER canonical_steers_insert BEFORE INSERT ON steers
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steers') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_steers_update;
CREATE TRIGGER canonical_steers_update BEFORE UPDATE OF id,wave_id,task_id,project_id,session_id ON steers
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: steers') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_changes_insert;
CREATE TRIGGER canonical_task_changes_insert BEFORE INSERT ON task_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.value_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.value_json) THEN NEW.value_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.base_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.base_json) THEN NEW.base_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflict_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflict_json) THEN NEW.conflict_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_changes_update;
CREATE TRIGGER canonical_task_changes_update BEFORE UPDATE OF id,task_id,value_json,base_json,conflict_json ON task_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT CASE WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('comment_task') THEN 'task_' WHEN path='$.value' AND json_extract(doc,'$.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value' AND typeof(key)='integer' AND json_extract(doc,'$.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.machine_id' AND json_extract(doc,'$.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.repository_id' AND json_extract(doc,'$.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.value_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.value_json) THEN NEW.value_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.base_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.base_json) THEN NEW.base_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('comment_task') THEN 'task_' WHEN path='$.value.value' AND json_extract(doc,'$.value.field') IN ('task_identifier') THEN 'label' WHEN parent='$.value.value' AND typeof(key)='integer' AND json_extract(doc,'$.value.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.value.value.machine_id' AND json_extract(doc,'$.value.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.value.value.repository_id' AND json_extract(doc,'$.value.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflict_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflict_json) THEN NEW.conflict_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_comment_deliveries_insert;
CREATE TRIGGER canonical_task_comment_deliveries_insert BEFORE INSERT ON task_comment_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_comment_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.comment_id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.comment_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.comment_json) THEN NEW.comment_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflicting_comment_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflicting_comment_json) THEN NEW.conflicting_comment_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_comment_deliveries_update;
CREATE TRIGGER canonical_task_comment_deliveries_update BEFORE UPDATE OF comment_id,comment_json,conflicting_comment_json ON task_comment_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_comment_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.comment_id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.comment_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.comment_json) THEN NEW.comment_json END) WHERE type='text')
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.conflicting_comment_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.conflicting_comment_json) THEN NEW.conflicting_comment_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_comments_insert;
CREATE TRIGGER canonical_task_comments_insert BEFORE INSERT ON task_comments
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_comments') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_creation_intents_insert;
CREATE TRIGGER canonical_task_creation_intents_insert BEFORE INSERT ON task_creation_intents
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_creation_intents') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN path='$.initiative' THEN 'uuid' WHEN path='$.link_id' THEN 'uuid' WHEN path='$.input.id' THEN 'uuid' WHEN path='$.input.teamId' THEN 'uuid' WHEN path='$.input.projectId' THEN 'uuid' WHEN path='$.input.assigneeId' THEN 'uuid' WHEN parent='$.input.teamIds' AND typeof(key)='integer' THEN 'uuid' WHEN (parent GLOB '$.baseline[[]*].value') AND typeof(key)='integer' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*].value' AND key='machine_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN parent GLOB '$.baseline[[]*].value' AND key='repository_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field')='task_admission_authority' THEN 'repo_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_project','task_project') THEN 'proj_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('comment_task') THEN 'task_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('task_identifier') THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.export_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.export_json) THEN NEW.export_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_creation_intents_update;
CREATE TRIGGER canonical_task_creation_intents_update BEFORE UPDATE OF task_id,project_id,export_json,repository_id ON task_creation_intents
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_creation_intents') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN path='$.initiative' THEN 'uuid' WHEN path='$.link_id' THEN 'uuid' WHEN path='$.input.id' THEN 'uuid' WHEN path='$.input.teamId' THEN 'uuid' WHEN path='$.input.projectId' THEN 'uuid' WHEN path='$.input.assigneeId' THEN 'uuid' WHEN parent='$.input.teamIds' AND typeof(key)='integer' THEN 'uuid' WHEN (parent GLOB '$.baseline[[]*].value') AND typeof(key)='integer' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*].value' AND key='machine_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN parent GLOB '$.baseline[[]*].value' AND key='repository_id' AND json_extract(doc,substr(parent,1,length(parent)-6)||'.field')='task_admission_authority' THEN 'repo_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('wave_project','task_project') THEN 'proj_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('comment_task') THEN 'task_' WHEN parent GLOB '$.baseline[[]*]' AND key='value' AND json_extract(doc,parent||'.field') IN ('task_identifier') THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.export_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.export_json) THEN NEW.export_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_deletions_insert;
CREATE TRIGGER canonical_task_deletions_insert BEFORE INSERT ON task_deletions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_deletions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.issue_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_events_insert;
CREATE TRIGGER canonical_task_events_insert BEFORE INSERT ON task_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_events') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT CASE WHEN path='$.pr_id' THEN 'pr_' WHEN path='$.steer_id' THEN 'uuid' WHEN path='$.invocation_id' THEN 'uuid' WHEN path='$.relation_id' THEN 'uuid' WHEN path='$.source_issue_id' THEN 'uuid' WHEN path='$.target_issue_id' THEN 'uuid' WHEN parent='$.lf_process_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.exec_ids' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.intent.issue_id' THEN 'task_or_uuid' WHEN path='$.intent.project_id' THEN 'project_or_uuid' WHEN path='$.intent.relation_id' THEN 'uuid' WHEN path='$.intent.team_id' THEN 'uuid' WHEN path='$.intent.state_id' THEN 'uuid' WHEN path='$.intent.identifier' THEN 'label' WHEN path='$.link.issue_id' THEN 'task_or_uuid' WHEN path='$.link.project_id' THEN 'project_or_uuid' WHEN path='$.link.relation_id' THEN 'uuid' WHEN path='$.link.team_id' THEN 'uuid' WHEN path='$.link.state_id' THEN 'uuid' WHEN path='$.link.identifier' THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.kind_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.kind_json) THEN NEW.kind_json END) WHERE type='text')
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_issue_identities_insert;
CREATE TRIGGER canonical_task_issue_identities_insert BEFORE INSERT ON task_issue_identities
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_issue_identities') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.issue_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_linear_ingested_comments_insert;
CREATE TRIGGER canonical_task_linear_ingested_comments_insert BEFORE INSERT ON task_linear_ingested_comments
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_linear_ingested_comments') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.comment_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_prs_insert;
CREATE TRIGGER canonical_task_prs_insert BEFORE INSERT ON task_prs
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_prs') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.linear_attachment_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.linear_comment_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_prs_update;
CREATE TRIGGER canonical_task_prs_update BEFORE UPDATE OF linear_attachment_id,linear_comment_id,task_id,id,parent_pr_id ON task_prs
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_prs') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.linear_attachment_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.linear_comment_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_state_deliveries_insert;
CREATE TRIGGER canonical_task_state_deliveries_insert BEFORE INSERT ON task_state_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_state_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_task_state_deliveries_update;
CREATE TRIGGER canonical_task_state_deliveries_update BEFORE UPDATE OF id,task_id ON task_state_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_state_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_tasks_insert;
CREATE TRIGGER canonical_tasks_insert BEFORE INSERT ON tasks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tasks') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_issue_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_team_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.checkout_machine_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.primary_session_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.issue_identifier AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_tasks_update;
CREATE TRIGGER canonical_tasks_update BEFORE UPDATE OF external_issue_id,planning_assignee,planning_team_id,id,project_id,checkout_machine_id,parent_pr_id,primary_session_id,issue_identifier,repository_id ON tasks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tasks') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.repository_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.external_issue_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_assignee AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.planning_team_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT 'home_' AS domain,NEW.checkout_machine_id AS original
UNION ALL
SELECT 'pr_' AS domain,NEW.parent_pr_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.primary_session_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.issue_identifier AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_tool_responses_insert;
CREATE TRIGGER canonical_tool_responses_insert BEFORE INSERT ON tool_responses
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tool_responses') FROM (
 WITH incoming AS (
SELECT 'response_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE NEW.work_kind WHEN 'wave' THEN 'uuid' WHEN 'project' THEN 'proj_' WHEN 'task' THEN 'task_' END AS domain,NEW.work_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

DROP TRIGGER canonical_waves_insert;
CREATE TRIGGER canonical_waves_insert BEFORE INSERT ON waves
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: waves') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.parent_wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.superseded_by_wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_activation_lf_process_id AS original
UNION ALL
SELECT 'proj_' AS domain,NEW.current_project_id AS original
),decoded AS MATERIALIZED (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS MATERIALIZED (
 SELECT *,CASE WHEN length(suffix)=38 AND substr(suffix,1,1)='{' AND substr(suffix,-1)='}' THEN substr(suffix,2,36)
 WHEN length(suffix)=45 AND lower(substr(suffix,1,9))='urn:uuid:' THEN substr(suffix,10) ELSE suffix END AS uuid FROM decoded
)
 SELECT original,CASE
 WHEN domain='session' THEN CASE WHEN (length(original)=32 AND original NOT GLOB '*[^0-9a-fA-F]*') OR
 (length(original)=40 AND lower(substr(original,1,8))='session_' AND substr(original,9) NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original) ELSE original END
 WHEN domain='label' THEN CASE WHEN lower(substr(original,1,3))='lf-' AND length(original)>3 AND substr(original,4) NOT GLOB '*[^0-9a-fA-F]*' THEN lower(original)
 WHEN instr(original,'-')>1 AND substr(original,1,instr(original,'-')-1) NOT GLOB '*[^a-zA-Z0-9]*' AND length(substr(original,instr(original,'-')+1))>0 AND substr(original,instr(original,'-')+1) NOT GLOB '*[^0-9]*' THEN upper(original) ELSE original END
 WHEN (length(suffix)=32 AND suffix NOT GLOB '*[^0-9a-fA-F]*') OR (length(uuid)=36 AND substr(uuid,9,1)='-' AND substr(uuid,14,1)='-' AND substr(uuid,19,1)='-' AND substr(uuid,24,1)='-' AND length(replace(uuid,'-',''))=32 AND replace(uuid,'-','') NOT GLOB '*[^0-9a-fA-F]*') THEN lower(original)
 ELSE original END AS canonical FROM uuids WHERE domain IS NOT NULL
 ) WHERE original IS NOT canonical;
END;

