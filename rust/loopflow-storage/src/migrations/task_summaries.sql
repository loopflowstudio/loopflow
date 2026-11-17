-- Which text a skill run used: the SHA-256 of the resolved skill source, so
-- runs group by skill version.
ALTER TABLE agent_sessions ADD COLUMN skill_version TEXT;

-- One row per Task, written when it completes or is abandoned and replaced
-- when a reopened Task ends again. Everything in it is derived from Graph
-- processes, their steps, the Auto-Ship log and Sessions at that moment.
CREATE TABLE task_summaries (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    ended TEXT NOT NULL CHECK (ended IN ('done','abandoned')),
    ended_at INTEGER NOT NULL,
    -- The Graph the Task's last Graph process ran.
    graph TEXT,
    -- Times each loop decider sent its loop around again: {"<step>": n}.
    loop_passes TEXT NOT NULL CHECK (json_valid(loop_passes)),
    -- Auto-Ship log rows by kind: {"approved": n, "stopped": n, …}.
    ship_events TEXT NOT NULL CHECK (json_valid(ship_events)),
    -- From the first Graph process's start, else the Task's creation.
    wall_seconds INTEGER NOT NULL,
    -- First line the latest `reflect` step said last.
    reflect TEXT
) STRICT;
CREATE TRIGGER store_revision_task_summaries_insert AFTER INSERT ON task_summaries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_summaries_update AFTER UPDATE ON task_summaries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_summaries_delete AFTER DELETE ON task_summaries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
