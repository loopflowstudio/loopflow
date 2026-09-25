-- name: wave_chapters
-- id: 34ae9954950dea7dda43fbffd4e50bdd
-- depends_on: 

CREATE TABLE wave_chapters (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    chapter_id TEXT NOT NULL,
    project_id TEXT NOT NULL UNIQUE,
    current INTEGER NOT NULL CHECK (current IN (0, 1)),
    receipt TEXT NOT NULL CHECK (json_valid(receipt)),
    PRIMARY KEY (wave_id, chapter_id)
);
CREATE UNIQUE INDEX wave_current_chapter ON wave_chapters(wave_id) WHERE current = 1;
CREATE UNIQUE INDEX wave_pending_chapter ON wave_chapters(wave_id)
    WHERE json_extract(receipt, '$.phase') != 'complete';

-- A first execution remains a fact after a Flow position resets or settles.
INSERT INTO task_events(task_id, kind_json, created_at)
SELECT task_id, '{"kind":"started"}', updated_at FROM task_flow_positions
WHERE worker_generation > 0
AND NOT EXISTS(SELECT 1 FROM task_events e WHERE e.task_id=task_flow_positions.task_id
 AND json_extract(e.kind_json, '$.kind')='started');
CREATE TRIGGER task_chapter_started AFTER UPDATE OF worker_generation ON task_flow_positions
WHEN NEW.worker_generation > 0
BEGIN
    INSERT INTO task_events(task_id, kind_json, created_at)
    SELECT NEW.task_id, '{"kind":"started"}', NEW.updated_at
    WHERE NOT EXISTS(SELECT 1 FROM task_events WHERE task_id=NEW.task_id
      AND json_extract(kind_json, '$.kind')='started');
END;

-- The Wave already received these Task observations. Preserve any missing copy
-- before retiring the obsolete Project recipient queue.
INSERT OR IGNORE INTO observation_outbox(recipient_kind,recipient_id,source_kind,source_id,event_id,payload_json,created_at,delivered_at)
SELECT 'wave',p.wave_id,o.source_kind,o.source_id,o.event_id,o.payload_json,o.created_at,NULL
FROM observation_outbox o JOIN tasks t ON t.id=o.source_id JOIN projects p ON p.id=t.project_id
WHERE o.recipient_kind='project' AND o.source_kind='task' AND o.delivered_at IS NULL;
UPDATE observation_outbox SET delivered_at=unixepoch()
WHERE recipient_kind='project' AND delivered_at IS NULL;

-- Existing PM caches predate chapter targets. No old instrument target becomes
-- an implicit target for a new chapter. Provider refresh owns subsequent edits.
UPDATE pm_snapshots SET payload = json_set(payload, '$.projects', json(
    (SELECT json_group_array(json_set(json_remove(value, '$.definition'),
        '$.metric_targets', json('[]'))) FROM json_each(payload, '$.projects'))
));
