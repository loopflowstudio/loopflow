-- The provider capability survives attachment handoff, never replacement.
CREATE TABLE steer_adapters (
    agent_process_id TEXT PRIMARY KEY REFERENCES processes(id),
    capability TEXT NOT NULL UNIQUE,
    provider_thread TEXT,
    created_at INTEGER NOT NULL,
    activated_at INTEGER
);

CREATE TABLE steer_publications (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    repo TEXT,
    wave_id TEXT REFERENCES waves(id),
    task_id TEXT REFERENCES tasks(id),
    project_id TEXT REFERENCES projects(id),
    session_id TEXT REFERENCES agent_sessions(id),
    source TEXT NOT NULL,
    revision TEXT NOT NULL,
    author TEXT NOT NULL,
    text TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    CHECK ((repo IS NOT NULL)+(wave_id IS NOT NULL)+(task_id IS NOT NULL)+
           (project_id IS NOT NULL)+(session_id IS NOT NULL)=1),
    UNIQUE(source, revision)
);
CREATE INDEX steer_publications_task ON steer_publications(task_id, seq);
CREATE INDEX steer_publications_wave ON steer_publications(wave_id, seq);
CREATE TABLE session_steer_subscriptions (
    session_id TEXT PRIMARY KEY REFERENCES agent_sessions(id),
    association_revision TEXT NOT NULL,
    activated_at INTEGER NOT NULL
);
CREATE TABLE session_steers (
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    publication_id TEXT NOT NULL REFERENCES steer_publications(id),
    attempt INTEGER NOT NULL DEFAULT 0,
    applicable INTEGER NOT NULL DEFAULT 1 CHECK(applicable IN (0,1)),
    PRIMARY KEY(session_id, publication_id)
);
CREATE TABLE steer_batches (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    agent_process_id TEXT NOT NULL REFERENCES processes(id),
    provider_thread TEXT,
    attachment_token TEXT NOT NULL,
    boundary TEXT NOT NULL,
    payload_hash TEXT,
    evidence TEXT NOT NULL CHECK(evidence IN ('prepared','uncertain','emitted','confirmed','cancelled')),
    created_at INTEGER NOT NULL
);
CREATE UNIQUE INDEX steer_batches_boundary ON steer_batches(agent_process_id,boundary) WHERE evidence != 'cancelled';
CREATE TABLE steer_deliveries (
    session_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    attempt INTEGER NOT NULL,
    batch_id TEXT NOT NULL REFERENCES steer_batches(id),
    excerpted INTEGER CHECK(excerpted IN (0,1)),
    PRIMARY KEY(session_id, publication_id, attempt),
    FOREIGN KEY(session_id, publication_id) REFERENCES session_steers(session_id, publication_id)
);
INSERT INTO steer_publications(id,task_id,source,revision,author,text,created_at)
SELECT lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-8'||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6))),task_id,'task-event-'||id,'1',
       json_extract(kind_json,'$.author'),json_extract(kind_json,'$.text'),created_at
FROM task_events WHERE json_extract(kind_json,'$.kind')='steer' ORDER BY id;
INSERT INTO steer_publications(id,project_id,source,revision,author,text,created_at)
SELECT lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-8'||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6))),project_id,'project-event-'||id,'1',
       json_extract(kind_json,'$.author'),json_extract(kind_json,'$.text'),created_at
FROM project_events WHERE json_extract(kind_json,'$.kind')='steer' ORDER BY id;
-- TaskObserved retains its event identity; orphaned historical copies retain
-- their complete source under the surviving Project rather than losing text.
INSERT INTO steer_publications(id,project_id,source,revision,author,text,created_at)
SELECT lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-4'||substr(lower(hex(randomblob(2))),2)||'-8'||substr(lower(hex(randomblob(2))),2)||'-'||lower(hex(randomblob(6))),e.project_id,'project-observed-'||e.id,'1',
       json_extract(e.kind_json,'$.event.author'),json_extract(e.kind_json,'$.event.text'),e.created_at
FROM project_events e
WHERE json_extract(e.kind_json,'$.kind')='task_observed'
  AND json_extract(e.kind_json,'$.event.kind')='steer'
  AND NOT EXISTS(SELECT 1 FROM steer_publications p
      WHERE p.source='task-event-'||json_extract(e.kind_json,'$.task_event_id'));
UPDATE project_events SET kind_json=json_set(kind_json,'$.event',json_object(
    'kind','steer','publication_id',COALESCE(
        (SELECT p.id FROM steer_publications p
         WHERE p.source='task-event-'||json_extract(project_events.kind_json,'$.task_event_id')),
        (SELECT p.id FROM steer_publications p WHERE p.source='project-observed-'||project_events.id))))
WHERE json_extract(kind_json,'$.kind')='task_observed'
  AND json_extract(kind_json,'$.event.kind')='steer';
UPDATE task_events SET kind_json=json_object('kind','steer','publication_id',(SELECT p.id FROM steer_publications p WHERE p.source='task-event-'||task_events.id))
WHERE json_extract(kind_json,'$.kind')='steer';
UPDATE project_events SET kind_json=json_object('kind','steer','publication_id',(SELECT p.id FROM steer_publications p WHERE p.source='project-event-'||project_events.id))
WHERE json_extract(kind_json,'$.kind')='steer';
