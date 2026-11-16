-- draft: after_merge_continue_task
-- A non-terminal Task PR continues the Task; it does not create a review gate.

CREATE TABLE task_prs_next (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 1),
    slug TEXT NOT NULL,
    branch TEXT NOT NULL UNIQUE,
    base_commit TEXT NOT NULL,
    publication_requested_at INTEGER,
    after_merge TEXT CHECK (after_merge IN ('continue_task', 'complete_task')),
    next_slug TEXT,
    github_number INTEGER CHECK (github_number > 0),
    github_url TEXT,
    merge_commit TEXT,
    abandoned_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    github_head_sha TEXT,
    ci_observation TEXT,
    parent_pr_id TEXT REFERENCES task_prs_next(id),
    github_observation TEXT,
    linear_attachment_id TEXT,
    linear_comment_id TEXT,
    linear_link_error TEXT,
    UNIQUE (task_id, sequence),
    CHECK ((publication_requested_at IS NULL) = (after_merge IS NULL)),
    CHECK ((github_number IS NULL) = (github_url IS NULL)),
    CHECK (github_number IS NULL OR publication_requested_at IS NOT NULL),
    CHECK (after_merge != 'complete_task' OR next_slug IS NULL),
    CHECK (merge_commit IS NULL OR github_number IS NOT NULL),
    CHECK (merge_commit IS NULL OR abandoned_at IS NULL)
);

INSERT INTO task_prs_next (
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at, after_merge, next_slug,
    github_number, github_url, merge_commit, abandoned_at,
    created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
    github_observation, linear_attachment_id, linear_comment_id,
    linear_link_error
)
SELECT
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at,
    CASE after_merge WHEN 'review' THEN 'continue_task' ELSE after_merge END,
    next_slug, github_number, github_url, merge_commit, abandoned_at,
    created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
    github_observation, linear_attachment_id, linear_comment_id,
    linear_link_error
FROM task_prs;

DROP TABLE task_prs;
ALTER TABLE task_prs_next RENAME TO task_prs;
CREATE UNIQUE INDEX idx_task_prs_open
    ON task_prs(task_id) WHERE merge_commit IS NULL AND abandoned_at IS NULL;

-- draft: drop_agent_bus
DROP TABLE bus_cursors;
DROP TABLE bus_messages;

-- draft: repair_durable_input_timestamp_units
-- `0.11.031_durable_input_spine` copied legacy nanosecond timestamps into
-- columns read as Unix seconds. Preserve every row and normalize only values
-- that cannot already be represented as an OffsetDateTime Unix second.
UPDATE epoch_revisions
SET created_at = created_at / 1000000000
WHERE created_at < -377705116800 OR created_at > 253402300799;

UPDATE steers
SET issued_at = issued_at / 1000000000
WHERE issued_at < -377705116800 OR issued_at > 253402300799;

UPDATE tool_responses
SET responded_at = responded_at / 1000000000
WHERE responded_at < -377705116800 OR responded_at > 253402300799;

-- draft: task_feedback_reviewers
-- Reviewer names authority directly; it does not suppress the authored step.

ALTER TABLE tasks RENAME COLUMN iterate_interaction_policy TO iterate_reviewer;
ALTER TABLE tasks RENAME COLUMN kickoff_interaction_policy TO kickoff_reviewer;
ALTER TABLE tasks RENAME COLUMN gate_interaction_policy TO gate_reviewer;

UPDATE tasks
SET iterate_reviewer = CASE iterate_reviewer
        WHEN 'require' THEN 'user'
        WHEN 'defer' THEN 'parent'
    END,
    kickoff_reviewer = CASE kickoff_reviewer
        WHEN 'require' THEN 'user'
        WHEN 'defer' THEN 'parent'
    END,
    gate_reviewer = CASE gate_reviewer
        WHEN 'require' THEN 'user'
        WHEN 'defer' THEN 'parent'
    END;

-- draft: wave_promotion_occurrence
-- Promotion occurrence is distinct from chord ancestry. Existing parent links
-- remain ancestry-only and must not wake a child Wave after migration.

ALTER TABLE waves ADD COLUMN promoted_at INTEGER;

-- draft: explicit_pr_merge_requests
-- A published PR is ordinary Task continuity until a user or auto merge is
-- explicitly requested for one exact GitHub head.

CREATE TABLE task_prs_next (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 1),
    slug TEXT NOT NULL,
    branch TEXT NOT NULL UNIQUE,
    base_commit TEXT NOT NULL,
    publication_requested_at INTEGER,
    after_merge TEXT CHECK (after_merge IN ('continue_task', 'complete_task')),
    next_slug TEXT,
    github_number INTEGER CHECK (github_number > 0),
    github_url TEXT,
    merge_commit TEXT,
    abandoned_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    github_head_sha TEXT,
    ci_observation TEXT,
    parent_pr_id TEXT REFERENCES task_prs_next(id),
    github_observation TEXT,
    linear_attachment_id TEXT,
    linear_comment_id TEXT,
    linear_link_error TEXT,
    merge_mode TEXT CHECK (merge_mode IN ('user', 'auto')),
    merge_requested_at INTEGER,
    merge_head_sha TEXT CHECK (merge_head_sha IS NULL OR length(trim(merge_head_sha)) > 0),
    UNIQUE (task_id, sequence),
    CHECK ((github_number IS NULL) = (github_url IS NULL)),
    CHECK (github_number IS NULL OR publication_requested_at IS NOT NULL),
    CHECK (after_merge != 'complete_task' OR next_slug IS NULL),
    CHECK (merge_commit IS NULL OR github_number IS NOT NULL),
    CHECK (merge_commit IS NULL OR abandoned_at IS NULL),
    CHECK ((merge_mode IS NULL) = (merge_requested_at IS NULL)),
    CHECK ((merge_mode IS NULL) = (merge_head_sha IS NULL)),
    CHECK ((merge_mode IS NULL) = (after_merge IS NULL)),
    CHECK (next_slug IS NULL OR merge_mode IS NOT NULL),
    CHECK (merge_mode IS NULL OR github_number IS NOT NULL),
    CHECK (merge_mode IS NULL OR merge_head_sha = github_head_sha)
);

INSERT INTO task_prs_next (
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at, after_merge, next_slug,
    github_number, github_url, merge_commit, abandoned_at,
    created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
    github_observation, linear_attachment_id, linear_comment_id,
    linear_link_error, merge_mode, merge_requested_at, merge_head_sha
)
SELECT
    id, task_id, sequence, slug, branch, base_commit,
    publication_requested_at, NULL, NULL,
    github_number, github_url, merge_commit, abandoned_at,
    created_at, updated_at, github_head_sha, ci_observation, parent_pr_id,
    github_observation, linear_attachment_id, linear_comment_id,
    linear_link_error, NULL, NULL, NULL
FROM task_prs;

DROP TABLE task_prs;
ALTER TABLE task_prs_next RENAME TO task_prs;
CREATE UNIQUE INDEX idx_task_prs_open
    ON task_prs(task_id) WHERE merge_commit IS NULL AND abandoned_at IS NULL;

-- draft: run_owns_execution
-- Collapse runner authority and containment onto Run. The trace row records
-- one provider invocation and may point at its supervisor, but that relation
-- is provenance only.

-- Production migration wraps the full set with foreign keys disabled. Keep
-- this migration correct when applied directly by validation fixtures too,
-- because invocation ids and their Turn references change together below.
PRAGMA foreign_keys = OFF;

ALTER TABLE runs ADD COLUMN containment_kind TEXT CHECK (
    containment_kind IN ('process_group', 'tmux')
);
ALTER TABLE runs ADD COLUMN containment_id TEXT;
ALTER TABLE runs ADD COLUMN cwd TEXT;
ALTER TABLE runs ADD COLUMN started_at INTEGER;

-- Preserve the strongest containment already recorded by the former control
-- Launch. Historical ended Runs retain it; never-started reservations remain
-- empty.
UPDATE runs
SET containment_kind = (
        SELECT containment_kind FROM agent_launches
        WHERE product_run_id = runs.id AND containment_kind IS NOT NULL
        ORDER BY (launch_state != 'ended') DESC, started_at DESC, rowid DESC
        LIMIT 1
    ),
    containment_id = (
        SELECT containment_id FROM agent_launches
        WHERE product_run_id = runs.id AND containment_kind IS NOT NULL
        ORDER BY (launch_state != 'ended') DESC, started_at DESC, rowid DESC
        LIMIT 1
    ),
    cwd = (
        SELECT worktree FROM agent_launches
        WHERE product_run_id = runs.id AND containment_kind IS NOT NULL
        ORDER BY (launch_state != 'ended') DESC, started_at DESC, rowid DESC
        LIMIT 1
    ),
    started_at = (
        SELECT started_at FROM agent_launches
        WHERE product_run_id = runs.id AND containment_kind IS NOT NULL
        ORDER BY (launch_state != 'ended') DESC, started_at DESC, rowid DESC
        LIMIT 1
    )
WHERE EXISTS (
    SELECT 1 FROM agent_launches
    WHERE product_run_id = runs.id AND containment_kind IS NOT NULL
);

-- An imported controller without containable evidence cannot remain an
-- execution authority under the reduced model.
UPDATE runs
SET state = 'ended',
    ended_at = COALESCE(ended_at, unixepoch()),
    stop_reason = COALESCE(stop_reason, 'migration: missing Run containment'),
    containment_kind = NULL,
    containment_id = NULL,
    cwd = NULL,
    started_at = NULL
WHERE state IN ('active', 'stopping')
  AND (
      containment_kind IS NULL OR containment_id IS NULL OR cwd IS NULL
      OR started_at IS NULL
  );

-- A reservation has not acquired containment yet. Old control Launch rows may
-- have recorded a proposed containment while the Run was still reserved; do
-- not promote that proposal into acquired Run state.
UPDATE runs
SET containment_kind = NULL,
    containment_id = NULL,
    cwd = NULL,
    started_at = NULL
WHERE state = 'reserved';

-- Historical ended rows were not constrained as one checked group. Discard
-- partial evidence rather than importing an impossible containment identity.
UPDATE runs
SET containment_kind = NULL,
    containment_id = NULL,
    cwd = NULL,
    started_at = NULL
WHERE state = 'ended'
  AND (
      containment_kind IS NULL OR containment_id IS NULL OR cwd IS NULL
      OR started_at IS NULL
  );

UPDATE agent_launches
SET launch_state = 'ended',
    ended_at = COALESCE(ended_at, unixepoch()),
    outcome = CASE WHEN outcome = 'running' THEN 'failed' ELSE outcome END,
    handback_state = COALESCE(handback_state, 'unknown')
WHERE product_run_id IN (
    SELECT id FROM runs
    WHERE state = 'ended'
)
  AND ended_at IS NULL;

UPDATE agent_turns
SET status = 'failed',
    ended_at = COALESCE(ended_at, unixepoch())
WHERE status = 'running'
  AND launch_id IN (
      SELECT agent_launches.id
      FROM agent_launches
      JOIN runs ON runs.id = agent_launches.product_run_id
      WHERE runs.state = 'ended'
  );

CREATE TRIGGER runs_execution_shape_insert
BEFORE INSERT ON runs
BEGIN
    SELECT RAISE(ABORT, 'invalid Run execution shape')
    WHERE NOT (
        (NEW.state = 'reserved'
         AND NEW.containment_kind IS NULL AND NEW.containment_id IS NULL
         AND NEW.cwd IS NULL AND NEW.started_at IS NULL)
        OR
        (NEW.state IN ('active', 'stopping')
         AND NEW.containment_kind IS NOT NULL AND NEW.containment_id IS NOT NULL
         AND length(trim(NEW.containment_id)) > 0
         AND NEW.cwd IS NOT NULL AND length(trim(NEW.cwd)) > 0
         AND NEW.started_at IS NOT NULL)
        OR
        (NEW.state = 'ended'
         AND (
             (NEW.containment_kind IS NULL AND NEW.containment_id IS NULL
              AND NEW.cwd IS NULL AND NEW.started_at IS NULL)
             OR
             (NEW.containment_kind IS NOT NULL AND NEW.containment_id IS NOT NULL
              AND length(trim(NEW.containment_id)) > 0
              AND NEW.cwd IS NOT NULL AND length(trim(NEW.cwd)) > 0
              AND NEW.started_at IS NOT NULL)
         ))
    );
END;

CREATE TRIGGER runs_execution_shape_update
BEFORE UPDATE ON runs
BEGIN
    SELECT RAISE(ABORT, 'invalid Run execution shape')
    WHERE NOT (
        (NEW.state = 'reserved'
         AND NEW.containment_kind IS NULL AND NEW.containment_id IS NULL
         AND NEW.cwd IS NULL AND NEW.started_at IS NULL)
        OR
        (NEW.state IN ('active', 'stopping')
         AND NEW.containment_kind IS NOT NULL AND NEW.containment_id IS NOT NULL
         AND length(trim(NEW.containment_id)) > 0
         AND NEW.cwd IS NOT NULL AND length(trim(NEW.cwd)) > 0
         AND NEW.started_at IS NOT NULL)
        OR
        (NEW.state = 'ended'
         AND (
             (NEW.containment_kind IS NULL AND NEW.containment_id IS NULL
              AND NEW.cwd IS NULL AND NEW.started_at IS NULL)
             OR
             (NEW.containment_kind IS NOT NULL AND NEW.containment_id IS NOT NULL
              AND length(trim(NEW.containment_id)) > 0
              AND NEW.cwd IS NOT NULL AND length(trim(NEW.cwd)) > 0
              AND NEW.started_at IS NOT NULL)
         ))
    );
    SELECT RAISE(ABORT, 'Run containment is immutable once acquired')
    WHERE OLD.containment_kind IS NOT NULL
      AND (
          NEW.containment_kind IS NOT OLD.containment_kind
          OR NEW.containment_id IS NOT OLD.containment_id
          OR NEW.cwd IS NOT OLD.cwd
          OR NEW.started_at IS NOT OLD.started_at
      );
END;

DROP INDEX idx_agent_launches_one_control_live;
DROP INDEX idx_agent_launches_attention;
DROP INDEX idx_agent_launches_run;
DROP INDEX idx_agent_launches_process;
DROP INDEX idx_agent_launches_wave;
DROP INDEX idx_agent_launches_project;
DROP INDEX idx_agent_launches_task;

ALTER TABLE agent_launches RENAME TO agent_invocations;
ALTER TABLE agent_invocations RENAME COLUMN product_run_id TO supervising_run_id;
ALTER TABLE agent_invocations DROP COLUMN home_id;
ALTER TABLE agent_invocations DROP COLUMN launch_state;
ALTER TABLE agent_invocations DROP COLUMN containment_kind;
ALTER TABLE agent_invocations DROP COLUMN containment_id;
ALTER TABLE agent_invocations DROP COLUMN opaque_epoch_id;
ALTER TABLE agent_invocations DROP COLUMN opaque_basis_rev;

CREATE INDEX idx_agent_invocations_run
    ON agent_invocations(run_id, started_at);
CREATE INDEX idx_agent_invocations_process
    ON agent_invocations(process_id, started_at);
CREATE INDEX idx_agent_invocations_wave
    ON agent_invocations(wave, started_at);
CREATE INDEX idx_agent_invocations_project
    ON agent_invocations(project, started_at);
CREATE INDEX idx_agent_invocations_task
    ON agent_invocations(task, started_at);
CREATE INDEX idx_agent_invocations_supervisor
    ON agent_invocations(supervising_run_id, started_at)
    WHERE supervising_run_id IS NOT NULL;
CREATE INDEX idx_agent_invocations_attention
    ON agent_invocations(
        attention_kind, attention_work_kind, attention_work_id, attention_at
    )
    WHERE attention_kind IS NOT NULL;

DROP INDEX idx_agent_turns_launch;
ALTER TABLE agent_turns RENAME COLUMN launch_id TO invocation_id;

-- Legacy invocation ids were minted with the old type prefix. Rewrite every
-- matching trace identity and Turn reference once; unrelated imported ids are
-- untouched.
UPDATE agent_invocations
SET id = 'invocation_' || substr(id, 8)
WHERE id GLOB 'launch_*';
UPDATE agent_turns
SET invocation_id = 'invocation_' || substr(invocation_id, 8)
WHERE invocation_id GLOB 'launch_*';

CREATE INDEX idx_agent_turns_invocation
    ON agent_turns(invocation_id, ordinal);

PRAGMA foreign_keys = ON;

-- draft: durable_asks
-- A question is Turn-local tool I/O. Its Work and Basis are derived through
-- Turn -> AgentInvocation -> Run -> Epoch; only the answering route is stored.

CREATE TABLE ask_exchanges (
    id TEXT PRIMARY KEY,
    turn_id TEXT NOT NULL REFERENCES agent_turns(id) ON DELETE RESTRICT,
    route_kind TEXT NOT NULL CHECK (route_kind IN ('user', 'parent')),
    route_work_kind TEXT CHECK (
        route_work_kind IN ('wave', 'project', 'task')
    ),
    route_work_id TEXT,
    question TEXT NOT NULL CHECK (length(trim(question)) > 0),
    asked_at INTEGER NOT NULL,
    answer_author_kind TEXT CHECK (answer_author_kind IN ('user', 'run')),
    answer_author_id TEXT,
    answer_text TEXT,
    answered_at INTEGER,
    CHECK (
        (route_kind = 'user'
         AND route_work_kind IS NULL AND route_work_id IS NULL)
        OR
        (route_kind = 'parent'
         AND route_work_kind IS NOT NULL AND route_work_id IS NOT NULL
         AND length(trim(route_work_id)) > 0)
    ),
    CHECK (
        (answer_author_kind IS NULL AND answer_author_id IS NULL
         AND answer_text IS NULL AND answered_at IS NULL)
        OR
        (answer_author_kind = 'user' AND answer_author_id IS NULL
         AND answer_text IS NOT NULL AND length(trim(answer_text)) > 0
         AND answered_at IS NOT NULL)
        OR
        (answer_author_kind = 'run' AND answer_author_id IS NOT NULL
         AND length(trim(answer_author_id)) > 0
         AND answer_text IS NOT NULL AND length(trim(answer_text)) > 0
         AND answered_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_ask_exchanges_one_pending_turn
    ON ask_exchanges(turn_id) WHERE answered_at IS NULL;
CREATE INDEX idx_ask_exchanges_parent_pending
    ON ask_exchanges(route_work_kind, route_work_id, asked_at)
    WHERE route_kind = 'parent' AND answered_at IS NULL;
CREATE INDEX idx_ask_exchanges_user_pending
    ON ask_exchanges(asked_at)
    WHERE route_kind = 'user' AND answered_at IS NULL;

DROP INDEX idx_agent_invocations_attention;
ALTER TABLE agent_invocations DROP COLUMN attention_kind;
ALTER TABLE agent_invocations DROP COLUMN attention_work_kind;
ALTER TABLE agent_invocations DROP COLUMN attention_work_id;
ALTER TABLE agent_invocations DROP COLUMN attention_at;

ALTER TABLE work_flow_positions DROP COLUMN interactive;

ALTER TABLE tasks DROP COLUMN iterate_reviewer;
ALTER TABLE tasks DROP COLUMN kickoff_reviewer;
ALTER TABLE tasks DROP COLUMN gate_reviewer;

-- draft: ask_linear_comment_outbox
-- Ask and Answer commits enqueue their Linear write in the same transaction.
-- The provider call happens afterward; attempt state makes failures visible and
-- lets a later command reconcile a remotely-created comment without duplicating
-- it after a local process crash.
CREATE TABLE ask_linear_comment_outbox (
    ask_id TEXT NOT NULL REFERENCES ask_exchanges(id) ON DELETE RESTRICT,
    transition TEXT NOT NULL CHECK (transition IN ('ask', 'answer')),
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    issue_id TEXT NOT NULL CHECK (length(trim(issue_id)) > 0),
    body TEXT NOT NULL CHECK (length(trim(body)) > 0),
    created_at INTEGER NOT NULL,
    attempt_count INTEGER NOT NULL CHECK (attempt_count >= 0),
    attempt_started_at INTEGER,
    last_error TEXT,
    linear_comment_id TEXT,
    delivered_at INTEGER,
    PRIMARY KEY (ask_id, transition),
    CHECK (
        (linear_comment_id IS NULL AND delivered_at IS NULL)
        OR
        (linear_comment_id IS NOT NULL
         AND length(trim(linear_comment_id)) > 0
         AND delivered_at IS NOT NULL)
    )
);
CREATE INDEX idx_ask_linear_comment_outbox_pending
    ON ask_linear_comment_outbox(delivered_at, created_at, ask_id, transition);

-- draft: answer_invocations
-- Correlate a detached answer attempt to the exact durable Ask it serves.
-- The relation is purpose, not authority: only the supervising Run lease can
-- commit the Answer.
ALTER TABLE agent_invocations ADD COLUMN answer_ask_id TEXT
    REFERENCES ask_exchanges(id);

CREATE UNIQUE INDEX idx_agent_invocations_one_live_answer
    ON agent_invocations(answer_ask_id)
    WHERE answer_ask_id IS NOT NULL AND ended_at IS NULL;
