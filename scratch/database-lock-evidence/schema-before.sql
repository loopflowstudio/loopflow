CREATE TABLE schema_migrations (
                version TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL
            , checksum TEXT, parent_history TEXT, build_provenance TEXT, source_identity TEXT, source_revision TEXT, package_version TEXT, reader_floor TEXT);
CREATE TABLE provider_tokens (
    provider TEXT PRIMARY KEY,
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    oauth_client_id TEXT,
    expires_at INTEGER,
    login TEXT,
    updated_at INTEGER NOT NULL,
    credential_type TEXT NOT NULL,
    encrypted INTEGER NOT NULL
);
CREATE TABLE blob_tokens (
    sha TEXT PRIMARY KEY,
    lines INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    tokens INTEGER NOT NULL
);
CREATE TABLE "provider_accounts" (
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    home TEXT,
    credential_state TEXT NOT NULL CHECK (
        credential_state IN ('connected', 'missing')
    ),
    utilization_percent INTEGER CHECK (
        utilization_percent IS NULL OR
        utilization_percent BETWEEN 0 AND 100
    ),
    cooldown_until INTEGER,
    cooldown_reason TEXT,
    last_selected_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, observed_email TEXT, observed_subject TEXT, observed_plan TEXT, observed_credential_digest TEXT, long_lived_token TEXT, long_lived_token_minted_at INTEGER,
    PRIMARY KEY (provider, account_id)
);
CREATE TABLE provider_account_limits (
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    window TEXT NOT NULL,
    used_percent INTEGER NOT NULL,
    resets_at INTEGER,
    plan TEXT,
    observed_at INTEGER NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('stream', 'poll')), model TEXT, limited_until INTEGER,
    PRIMARY KEY (provider, account_id, window)
);
CREATE TABLE "provider_session_accounts" (
    provider TEXT NOT NULL,
    provider_session_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    created_at INTEGER NOT NULL, isolated INTEGER NOT NULL DEFAULT 1
    CHECK (isolated IN (0, 1)),
    PRIMARY KEY (provider, provider_session_id),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id) ON DELETE CASCADE
);
CREATE TABLE "machines" (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    observed_at INTEGER NOT NULL
, public_name TEXT, pool TEXT REFERENCES pools(name), cost_usd TEXT, expires_at INTEGER, is_local INTEGER NOT NULL DEFAULT 0 CHECK (is_local IN (0,1)));
CREATE TABLE "task_linear_observations" (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    last_revision TEXT NOT NULL,
    last_title TEXT NOT NULL,
    last_description TEXT NOT NULL,
    last_success_at INTEGER NOT NULL,
    degraded_reason TEXT,
    updated_at INTEGER NOT NULL
);
CREATE TABLE "task_linear_ingested_comments" (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    comment_id TEXT NOT NULL,
    ingested_at INTEGER NOT NULL,
    PRIMARY KEY (task_id, comment_id)
);
CREATE TABLE task_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE project_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind_json TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE task_pr_repair_incidents (
    task_pr_id TEXT NOT NULL REFERENCES task_prs(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (
        kind IN ('avoidable_rebase_agent', 'manual_git_repair')
    ),
    occurred_at INTEGER NOT NULL,
    PRIMARY KEY (task_pr_id, kind)
);
CREATE TABLE metric_instruments (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    metric_id TEXT NOT NULL,
    instrument TEXT NOT NULL,
    registered_at INTEGER NOT NULL,
    PRIMARY KEY (wave_id, metric_id)
);
CREATE TABLE metric_observations (
    observation_id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    metric_id TEXT NOT NULL,
    contract_revision TEXT NOT NULL,
    instrument TEXT NOT NULL,
    source_time_seconds INTEGER NOT NULL,
    source_time_nanoseconds INTEGER NOT NULL,
    received_at INTEGER NOT NULL,
    graduation_qualifying INTEGER NOT NULL,
    payload TEXT NOT NULL
);
CREATE TABLE pr_landings (
    id TEXT PRIMARY KEY,
    repo TEXT NOT NULL,
    pr_number INTEGER NOT NULL CHECK (pr_number > 0),
    worktree TEXT NOT NULL,
    branch TEXT NOT NULL,
    task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
    requested_head_sha TEXT NOT NULL,
    observed_head_sha TEXT NOT NULL,
    merge_commit TEXT,
    after_merge TEXT CHECK (after_merge IN ('continue_task', 'complete_task')),
    next_slug TEXT,
    state TEXT NOT NULL CHECK (state IN ('watching', 'repairing', 'merged', 'closed', 'blocked')),
    generation INTEGER NOT NULL CHECK (generation > 0),
    supervisor_placement TEXT CHECK (supervisor_placement IN ('local', 'home')),
    supervisor_machine_id TEXT REFERENCES "machines"(id) ON DELETE RESTRICT,
    supervisor_process_id INTEGER,
    supervisor_heartbeat_at INTEGER,
    blocked_reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL, retired_supervisor_json TEXT
    CHECK (retired_supervisor_json IS NULL OR json_valid(retired_supervisor_json)),
    CHECK ((supervisor_placement IS NULL) = (supervisor_process_id IS NULL)),
    CHECK ((supervisor_placement IS NULL) = (supervisor_heartbeat_at IS NULL)),
    CHECK (supervisor_placement = 'home' OR supervisor_machine_id IS NULL),
    CHECK (supervisor_placement != 'home' OR supervisor_machine_id IS NOT NULL),
    CHECK ((state = 'blocked') = (blocked_reason IS NOT NULL)),
    CHECK (state != 'merged' OR merge_commit IS NOT NULL),
    CHECK (after_merge != 'complete_task' OR next_slug IS NULL),
    CHECK (task_id IS NOT NULL OR (after_merge IS NULL AND next_slug IS NULL))
);
CREATE TABLE "ci_incidents" (
    identity TEXT PRIMARY KEY,
    landing_id TEXT REFERENCES pr_landings(id) ON DELETE RESTRICT,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    pr_id TEXT REFERENCES task_prs(id) ON DELETE SET NULL,
    repo TEXT NOT NULL,
    pr_number INTEGER NOT NULL CHECK (pr_number > 0),
    failed_head_sha TEXT NOT NULL,
    failure_set_json TEXT NOT NULL,
    provider_completed_at INTEGER,
    poll_observed_at INTEGER,
    webhook_received_at INTEGER,
    claimed_landing_generation INTEGER,
    responded_at INTEGER,
    green_at INTEGER,
    merged_at INTEGER,
    blocked_at INTEGER,
    blocked_reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    repaired_head_sha TEXT, "repair_lf_process_id" TEXT REFERENCES "processes"("id"), repair_session_id TEXT REFERENCES agent_sessions(id), repair_retries INTEGER NOT NULL DEFAULT 0, repair_finished_at INTEGER, repair_error TEXT, repair_conclusion TEXT CHECK (repair_conclusion IN ('published','blocked')),
    CHECK (poll_observed_at IS NOT NULL OR webhook_received_at IS NOT NULL),
    CHECK ((blocked_at IS NULL) = (blocked_reason IS NULL)),
    CHECK (landing_id IS NULL OR claimed_landing_generation IS NULL OR claimed_landing_generation > 0)
);
CREATE TABLE tool_responses (
    id TEXT PRIMARY KEY,
    work_kind TEXT NOT NULL CHECK (work_kind IN ('wave', 'project', 'task')),
    work_id TEXT NOT NULL CHECK (length(trim(work_id)) > 0),
    request_id TEXT NOT NULL,
    choice TEXT NOT NULL CHECK (length(trim(choice)) > 0),
    responded_at INTEGER NOT NULL,
    UNIQUE (work_kind, work_id, request_id)
);
CREATE TABLE task_deletions (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    issue_id TEXT NOT NULL,
    identifier TEXT NOT NULL,
    confirmed_at INTEGER NOT NULL,
    PRIMARY KEY (wave_id, issue_id)
);
CREATE TABLE task_issue_identities (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    issue_id TEXT PRIMARY KEY NOT NULL,
    identifier TEXT NOT NULL
);
CREATE TABLE "access_profiles" (
    profile_id TEXT PRIMARY KEY,
    chrome_directory TEXT NOT NULL UNIQUE,
    expected_login TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE "processes" (
    "id" TEXT PRIMARY KEY,
    trace_id TEXT NOT NULL,
    "parent_lf_process_id" TEXT,
    via_agent INTEGER CHECK (via_agent IN (0, 1)),
    caller_session_id TEXT,
    caller_provider_generation INTEGER,
    command TEXT,
    repo TEXT,
    cwd TEXT,
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    outcome TEXT CHECK (outcome IN ('succeeded', 'failed', 'interrupted')),
    exit_code INTEGER,
    error TEXT,
    signal TEXT
, "pid" INTEGER, kind TEXT NOT NULL DEFAULT 'lf' CHECK(kind IN ('lf','agent')), agent_session_id TEXT REFERENCES agent_sessions(id), os_started_at INTEGER, endpoint TEXT, agent_provider TEXT, agent_interactive INTEGER CHECK(agent_interactive IN (0,1)), caller_agent_process_id TEXT, attached_lf_process_id TEXT, attachment_token TEXT, attachment_exit_seq INTEGER REFERENCES session_events(seq), spawn_state TEXT CHECK(spawn_state IN ('reserved','spawn_requested','spawn_failed','exited')), terminal_id TEXT, receipt_locator TEXT, journal_trace_locator TEXT, viewed INTEGER CHECK(viewed IN (0,1)), native_input TEXT, viewer_hold TEXT CHECK(viewer_hold IN ('viewed','absent')), viewer_hold_at INTEGER);
CREATE TABLE "session_events" (
    seq INTEGER PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    provider_thread TEXT,
    provider_turn TEXT,
    kind TEXT NOT NULL CHECK(kind IN ('started','usage','completed','observed','output','captured')),
    receipt_key TEXT NOT NULL,
    provider_generation INTEGER,
    "lf_process_id" TEXT REFERENCES "processes"("id"),
    task_id TEXT REFERENCES tasks(id),
    wave_id TEXT REFERENCES waves(id),
    observed_at INTEGER NOT NULL,
    payload TEXT NOT NULL,
    captured_event INTEGER REFERENCES session_events(seq)
, agent_process_id TEXT);
CREATE TABLE "agent_sessions" (
    id TEXT PRIMARY KEY NOT NULL,
    current_capture INTEGER UNIQUE REFERENCES session_events(seq),
    title TEXT NOT NULL,
    title_source TEXT NOT NULL CHECK (title_source IN ('generated', 'human')),
    completed_at INTEGER,
    created_at INTEGER NOT NULL,
    request TEXT,
    interactive INTEGER NOT NULL DEFAULT 1 CHECK (interactive IN (0, 1)),
    repo TEXT,
    provider_thread TEXT,
    task_id TEXT REFERENCES tasks(id),
    wave_id TEXT REFERENCES waves(id),
    work_source TEXT CHECK (work_source IN ('declared', 'checkout', 'inherited', 'bound')),
    bound_at INTEGER,
    input_published INTEGER NOT NULL CHECK(input_published IN (0,1)),
    cwd TEXT NOT NULL,
    skill TEXT,
    provider TEXT,
    model TEXT,
    primary_scope TEXT CHECK (primary_scope IN ('repository', 'wave')), agent_process_id TEXT REFERENCES processes(id), transport_lf_process_id TEXT REFERENCES processes(id), fence_locator TEXT, moved_to_machine_id TEXT);
CREATE TABLE pm_projects (
    repo TEXT NOT NULL,
    provider TEXT NOT NULL,
    id TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN (0,1)), membership_unresolved INTEGER NOT NULL DEFAULT 0 CHECK(membership_unresolved IN (0,1)), task_order_json TEXT CHECK(task_order_json IS NULL OR json_valid(task_order_json)), revision TEXT, slug TEXT, name TEXT, summary TEXT, status TEXT, krs TEXT, metric_targets TEXT, initiative_ids TEXT, team_ids TEXT, description TEXT, description_revision TEXT, description_observed_at INTEGER,
    PRIMARY KEY(repo, provider, id)
);
CREATE TABLE pm_items (
    repo TEXT NOT NULL,
    provider TEXT NOT NULL,
    id TEXT NOT NULL,
    identifier TEXT NOT NULL COLLATE NOCASE,
    project_id TEXT,
    observed_at INTEGER NOT NULL,
    needs_refresh INTEGER NOT NULL DEFAULT 0 CHECK(needs_refresh IN (0,1)),
    revision TEXT, branch_name TEXT, url TEXT, name TEXT, description TEXT, rank INTEGER, completed INTEGER, completed_at TEXT, due_date TEXT, state TEXT, team_id TEXT, assignee TEXT, completed_at_observed INTEGER NOT NULL DEFAULT 0, due_date_observed INTEGER NOT NULL DEFAULT 0, assignee_observed INTEGER NOT NULL DEFAULT 0, claimed_project_id TEXT, project_id_observed INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(repo, provider, id)
);
CREATE TABLE pm_wave_sync (
    wave_id TEXT NOT NULL PRIMARY KEY REFERENCES waves(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    initiative TEXT NOT NULL,
    synced_at INTEGER NOT NULL
);
CREATE TABLE pm_wave_projects (
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY(wave_id, project_id)
);
CREATE TABLE pm_issue_changes (
    issue_id TEXT PRIMARY KEY,
    revision_ns INTEGER,
    removed INTEGER NOT NULL CHECK(removed IN (0,1))
);
CREATE TABLE provider_account_switches (
    id INTEGER PRIMARY KEY,
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    switched_at INTEGER NOT NULL,
    cause TEXT NOT NULL CHECK (cause IN ('person', 'exhaustion'))
);
CREATE TABLE pm_project_name_cutover (
    repo TEXT NOT NULL,
    provider TEXT NOT NULL CHECK(provider = 'linear'),
    id TEXT NOT NULL,
    body TEXT NOT NULL,
    observed_at INTEGER NOT NULL,
    archived INTEGER NOT NULL,
    membership_unresolved INTEGER NOT NULL,
    converted_at INTEGER,
    PRIMARY KEY(repo, provider, id)
);
CREATE TABLE project_binding_imports (
    wave_id TEXT PRIMARY KEY REFERENCES waves(id),
    original_yaml TEXT,
    imported_at INTEGER NOT NULL
);
CREATE TABLE project_transitions (
    wave_id TEXT NOT NULL REFERENCES waves(id),
    successor_id TEXT NOT NULL,
    predecessor_id TEXT,
    reset_name TEXT,
    create_successor INTEGER CHECK(create_successor IN (0, 1)),
    created_at INTEGER NOT NULL,
    settled_at INTEGER, local_plan_json TEXT, export_json TEXT CHECK(export_json IS NULL OR json_valid(export_json)), export_attempted INTEGER NOT NULL DEFAULT 0, export_link_attempted INTEGER NOT NULL DEFAULT 0, export_error TEXT, export_observed_at INTEGER,
    PRIMARY KEY (wave_id, successor_id)
);
CREATE TABLE project_transition_items (
    wave_id TEXT NOT NULL,
    successor_id TEXT NOT NULL,
    issue_id TEXT NOT NULL,
    PRIMARY KEY (wave_id, successor_id, issue_id),
    FOREIGN KEY (wave_id, successor_id)
        REFERENCES project_transitions(wave_id, successor_id)
);
CREATE TABLE "graph_processes" (
    "lf_process_id" TEXT PRIMARY KEY REFERENCES "processes"("id"),
    graph TEXT NOT NULL,
    compiled TEXT NOT NULL CHECK (json_valid(compiled))
) STRICT;
CREATE TABLE "graph_process_steps" (
    seq INTEGER PRIMARY KEY,
    "graph_lf_process_id" TEXT NOT NULL REFERENCES "graph_processes"("lf_process_id"),
    "lf_process_id" TEXT NOT NULL UNIQUE REFERENCES "processes"("id"),
    node INTEGER NOT NULL,
    iterations TEXT NOT NULL CHECK (json_valid(iterations))
) STRICT;
CREATE TABLE session_activity (
    session_id TEXT PRIMARY KEY REFERENCES agent_sessions(id) ON DELETE CASCADE,
    observed_at INTEGER NOT NULL,
    open_tools INTEGER NOT NULL,
    pending_input INTEGER NOT NULL,
    yielded INTEGER NOT NULL CHECK (yielded IN (0, 1))
, program_status TEXT CHECK (program_status IS NULL OR json_valid(program_status)), status_stream TEXT, status_sequence INTEGER, attachment_token TEXT, agent_process_id TEXT, preview TEXT CHECK (preview IS NULL OR json_valid(preview)), cpu_sampled_at INTEGER, cpu_quiet_since INTEGER) STRICT;
CREATE TABLE "projects" (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    external_project_id TEXT UNIQUE,
    created_at INTEGER NOT NULL,
    project_slug TEXT,
    project_name TEXT,
    project_summary TEXT NOT NULL DEFAULT '',
    planning_rank INTEGER NOT NULL DEFAULT 0 CHECK(planning_rank >= 0),
    planning_provider_revision TEXT,
    planning_initiatives TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(planning_initiatives)),
    planning_teams TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(planning_teams)),
    description TEXT,
    pm_snapshot_synced_at INTEGER,
    updated_at INTEGER,
    work_state TEXT NOT NULL DEFAULT 'ready'
        CHECK (work_state IN ('ready', 'done', 'abandoned')),
    work_terminal_at INTEGER,
    iteration INTEGER NOT NULL DEFAULT 0 CHECK (iteration >= 0),
    status TEXT NOT NULL DEFAULT 'started'
        CHECK (status IN ('backlog','planned','started','paused','completed','canceled')),
    legacy_current INTEGER CHECK (legacy_current IN (-1, 0, 1))
, krs TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(krs)), metric_targets TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(metric_targets)));
CREATE TABLE task_comments (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    body TEXT NOT NULL,
    author TEXT NOT NULL CHECK(json_valid(author)),
    created_at TEXT,
    provider_revision TEXT
);
CREATE TABLE task_comment_deliveries (
    comment_id TEXT PRIMARY KEY REFERENCES task_comments(id) ON DELETE RESTRICT,
    comment_json TEXT NOT NULL CHECK(json_valid(comment_json)),
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1)),
    error TEXT,
    conflicting_comment_json TEXT CHECK(conflicting_comment_json IS NULL OR json_valid(conflicting_comment_json))
);
CREATE TABLE "task_prs" (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 1),
    slug TEXT NOT NULL,
    branch TEXT NOT NULL,
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
    parent_pr_id TEXT REFERENCES "task_prs"(id),
    github_observation TEXT,
    linear_attachment_id TEXT,
    linear_comment_id TEXT,
    linear_link_error TEXT,
    merge_mode TEXT CHECK (merge_mode IN ('user', 'auto')),
    merge_requested_at INTEGER,
    merge_head_sha TEXT CHECK (merge_head_sha IS NULL OR length(trim(merge_head_sha)) > 0),
    merged_at INTEGER,
    merge_tracking_complete INTEGER NOT NULL DEFAULT 0 CHECK(merge_tracking_complete IN (0,1)),
    repair_tracking_complete INTEGER NOT NULL DEFAULT 0 CHECK(repair_tracking_complete IN (0,1)),
    pr_title TEXT, pr_body TEXT, pr_copy_head_sha TEXT, historical INTEGER NOT NULL DEFAULT 0 CHECK(historical IN (0,1)),
    UNIQUE (task_id, sequence),
    CHECK ((github_number IS NULL) = (github_url IS NULL)),
    CHECK (github_number IS NULL OR publication_requested_at IS NOT NULL),
    CHECK (after_merge != 'complete_task' OR next_slug IS NULL),
    CHECK (merge_commit IS NULL OR github_number IS NOT NULL),
    CHECK (merge_commit IS NULL OR abandoned_at IS NULL),
    CHECK ((merge_mode IS NULL) = (merge_requested_at IS NULL)),
    CHECK ((merge_mode IS NULL) = (merge_head_sha IS NULL)),
    CHECK (merge_mode IS NULL OR github_number IS NOT NULL),
    CHECK (merge_mode IS NULL OR merge_head_sha = github_head_sha)
);
CREATE TABLE project_changes (
    order_effects_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(order_effects_json)),
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    field TEXT GENERATED ALWAYS AS (COALESCE(json_extract(value_json,'$.field'),'task_order')) VIRTUAL NOT NULL CHECK(field IN ('project_name','project_summary','project_description','workflow','krs','metric_targets','project_status','task_order')),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    base_json TEXT CHECK(base_json IS NULL OR json_valid(base_json)),
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json)),
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    error TEXT,
    acknowledged_revision TEXT,
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1))
);
CREATE TABLE task_changes (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    field TEXT GENERATED ALWAYS AS (COALESCE(json_extract(value_json,'$.field'),'deleted')) VIRTUAL NOT NULL CHECK(field IN ('task_title','task_description','task_assignee','task_project','deleted')),
    value_json TEXT NOT NULL CHECK(json_valid(value_json)),
    base_json TEXT CHECK(base_json IS NULL OR json_valid(base_json)),
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json)),
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    error TEXT,
    acknowledged_revision TEXT,
    acknowledged INTEGER NOT NULL DEFAULT 0 CHECK(acknowledged IN (0,1))
);
CREATE TABLE planning_peer_changes (
    id TEXT PRIMARY KEY,
    record_table TEXT NOT NULL CHECK(record_table IN ('waves','projects','tasks','task_comments')),
    mutation TEXT NOT NULL CHECK(json_valid(mutation)),
    object_id TEXT GENERATED ALWAYS AS (json_extract(mutation,'$.edit.target')) VIRTUAL NOT NULL,
    field TEXT GENERATED ALWAYS AS (json_extract(mutation,'$.edit.change.field')) VIRTUAL NOT NULL,
    clock INTEGER GENERATED ALWAYS AS (json_extract(mutation,'$.clock')) VIRTUAL NOT NULL CHECK(clock>=0)
);
CREATE TABLE planning_peer_observed (
    id TEXT PRIMARY KEY REFERENCES planning_peer_changes(id)
);
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
CREATE TABLE planning_peer_routes (
    mutation_id TEXT NOT NULL REFERENCES planning_peer_changes(id),
    repo TEXT NOT NULL,
    destination TEXT NOT NULL,
    PRIMARY KEY(repo,destination,mutation_id),
    FOREIGN KEY(repo,destination) REFERENCES planning_destinations(repo,id)
);
CREATE TABLE planning_peer_imports (
    repo TEXT NOT NULL,
    destination TEXT NOT NULL,
    revision TEXT NOT NULL,
    PRIMARY KEY(repo,destination)
);
CREATE TABLE planning_peer_conflicts (
    repo TEXT NOT NULL,
    kind TEXT NOT NULL,
    object_id TEXT NOT NULL,
    reason TEXT NOT NULL,
    active INTEGER NOT NULL CHECK(active IN (0,1)),
    PRIMARY KEY(kind,object_id,reason)
);
CREATE TABLE steer_adapters (
    agent_process_id TEXT PRIMARY KEY REFERENCES processes(id),
    capability TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL,
    activated_at INTEGER
);
CREATE TABLE steers (
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
CREATE TABLE session_steers (
    session_id TEXT NOT NULL REFERENCES agent_sessions(id),
    steer_id TEXT NOT NULL REFERENCES steers(id),
    attempt INTEGER NOT NULL DEFAULT 0,
    applicable INTEGER NOT NULL DEFAULT 1 CHECK(applicable IN (0,1)),
    PRIMARY KEY(session_id, steer_id)
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
CREATE TABLE steer_deliveries (
    session_id TEXT NOT NULL,
    steer_id TEXT NOT NULL,
    attempt INTEGER NOT NULL,
    batch_id TEXT NOT NULL REFERENCES steer_batches(id),
    excerpted INTEGER CHECK(excerpted IN (0,1)),
    PRIMARY KEY(session_id, steer_id, attempt),
    FOREIGN KEY(session_id, steer_id) REFERENCES session_steers(session_id, steer_id)
);
CREATE TABLE repository_plans (
    id TEXT PRIMARY KEY NOT NULL,
    repo TEXT NOT NULL,
    selected INTEGER NOT NULL CHECK (selected IN (0, 1))
);
CREATE TABLE linear_team_checks (
    team_id TEXT PRIMARY KEY,
    attempted_at INTEGER NOT NULL,
    synced_through TEXT
);
CREATE TABLE relocated_task_processes (
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    process_id TEXT NOT NULL REFERENCES processes(id) ON DELETE CASCADE,
    PRIMARY KEY (task_id, process_id)
);
CREATE TABLE "tasks" (
    id TEXT PRIMARY KEY,
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    external_issue_id TEXT UNIQUE,
    issue_identifier TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    issue_title TEXT,
    issue_description TEXT,
    pm_snapshot_synced_at INTEGER,
    worktree TEXT,
    workspace_slug TEXT,
    abandon_reason TEXT,
    updated_at INTEGER,
    abandoned_at INTEGER,
    primary_session_id TEXT REFERENCES agent_sessions(id) ON DELETE SET NULL,
    planning_revision INTEGER NOT NULL DEFAULT 0 CHECK (planning_revision >= 0),
    planning_rank INTEGER NOT NULL DEFAULT 0 CHECK (planning_rank >= 0),
    planning_assignee TEXT,
    planning_state TEXT,
    planning_completed_at TEXT,
    planning_provider_revision TEXT,
    planning_url TEXT,
    planning_branch_name TEXT,
    planning_team_id TEXT,
    planning_completed INTEGER NOT NULL DEFAULT 0 CHECK (planning_completed IN (0,1)),
    planning_deleted_at INTEGER
, branch TEXT NOT NULL DEFAULT '', base_commit TEXT NOT NULL DEFAULT '', parent_pr_id TEXT REFERENCES task_prs(id), completion_request INTEGER, completion_error TEXT, planning_due_date TEXT, checkout_machine_id TEXT REFERENCES machines(id), paused INTEGER NOT NULL DEFAULT 0 CHECK (paused IN (0,1)), shipped_at INTEGER, ship_lf_process_id TEXT REFERENCES processes(id), ship_restarts INTEGER NOT NULL DEFAULT 0, ship_approved_lf_process_id TEXT, ship_stopped_at INTEGER, ship_stopped_lf_process_id TEXT, regressed_in TEXT, ship_stopped_person TEXT);
CREATE TABLE "task_creation_intents" (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE RESTRICT,
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    export_json TEXT CHECK(export_json IS NULL OR json_valid(export_json)),
    export_attempted INTEGER NOT NULL DEFAULT 0,
    export_error TEXT
, due_date TEXT, export_observed_at INTEGER);
CREATE TABLE task_ownership (
    id TEXT PRIMARY KEY CHECK (id GLOB 'own_*'),
    -- Entries outlive their Task, so the log carries no foreign key.
    task_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('claim', 'handoff', 'unclaim', 'recorded')),
    person TEXT,
    machine_id TEXT,
    at INTEGER NOT NULL,
    -- Provider order in milliseconds, stamped once when the provider reports it.
    ordered_at INTEGER,
    -- Machine-local: whether this Machine appended the entry.
    written_here INTEGER NOT NULL CHECK (written_here IN (0, 1)), person_key TEXT, machine_label TEXT,
    CHECK (kind = 'recorded' OR machine_id IS NOT NULL)
);
CREATE TABLE dashboards (
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    expression_key TEXT NOT NULL,
    name TEXT,
    pinned INTEGER NOT NULL CHECK(pinned IN (0,1)),
    slots_json TEXT CHECK(slots_json IS NULL OR json_valid(slots_json)),
    PRIMARY KEY(repository_id, id),
    FOREIGN KEY(repository_id, id, expression_key)
        REFERENCES dashboard_arrangements(repository_id, dashboard_id, expression_key)
        DEFERRABLE INITIALLY DEFERRED
);
CREATE TABLE dashboard_arrangements (
    repository_id TEXT NOT NULL,
    dashboard_id TEXT NOT NULL,
    expression_key TEXT NOT NULL,
    selection_json TEXT NOT NULL CHECK(json_valid(selection_json)),
    arrangement_json TEXT NOT NULL CHECK(json_valid(arrangement_json)),
    PRIMARY KEY(repository_id, dashboard_id, expression_key),
    FOREIGN KEY(repository_id, dashboard_id) REFERENCES dashboards(repository_id,id) ON DELETE CASCADE
);
CREATE TABLE work_pane_layouts (
    repository_id TEXT NOT NULL REFERENCES repository_plans(id),
    kind TEXT NOT NULL CHECK(kind IN ('repository','wave','task')),
    work_id TEXT NOT NULL,
    layout_json TEXT NOT NULL CHECK(json_valid(layout_json))
        CHECK((json_extract(layout_json, '$.revision') > 0) IS TRUE),
    PRIMARY KEY(repository_id, kind, work_id)
);
CREATE TABLE task_orders (
    repository_id TEXT PRIMARY KEY REFERENCES repository_plans(id),
    task_ids_json TEXT NOT NULL CHECK(json_valid(task_ids_json))
);
CREATE TABLE pools (
  name TEXT PRIMARY KEY NOT NULL CHECK(length(trim(name))>0),
  created_at INTEGER NOT NULL
);
CREATE TABLE ondemands (
  name TEXT PRIMARY KEY NOT NULL CHECK(length(trim(name))>0),
  created_at INTEGER NOT NULL
);
CREATE TABLE rentals (
  id TEXT PRIMARY KEY NOT NULL CHECK(id GLOB 'rent_*'),
  -- Kept after the on-demand is removed: history outlives configuration.
  ondemand TEXT NOT NULL,
  provider TEXT NOT NULL,
  -- Sent with the create before any reply; finds an unconfirmed create.
  provider_name TEXT NOT NULL UNIQUE,
  provider_id TEXT UNIQUE,
  machine_id TEXT UNIQUE REFERENCES machines(id),
  by_hand INTEGER NOT NULL CHECK(by_hand IN (0,1)),
  hardware TEXT NOT NULL CHECK(json_valid(hardware)),
  usd_per_hour TEXT NOT NULL,
  cap_usd TEXT NOT NULL,
  deadline INTEGER NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('requested','unconfirmed','booting','running','ending','ended','lost')),
  created_at INTEGER NOT NULL,
  ended_at INTEGER,
  cost_usd TEXT
);
CREATE TABLE machine_records (
  origin_machine_id TEXT NOT NULL REFERENCES machines(id),
  kind TEXT NOT NULL CHECK(kind IN ('process','session','session_event','graph_process','graph_process_step')),
  key TEXT NOT NULL,
  row TEXT NOT NULL CHECK(json_valid(row)),
  observed_at INTEGER NOT NULL,
  PRIMARY KEY(origin_machine_id, kind, key)
);
CREATE TABLE github_people(
    email TEXT PRIMARY KEY,
    login TEXT,
    name TEXT,
    avatar_url TEXT,
    checked_at INTEGER NOT NULL,
    CHECK ((login IS NULL) = (avatar_url IS NULL))
);
CREATE INDEX idx_task_events_task ON task_events(task_id, id);
CREATE INDEX idx_project_events_project ON project_events(project_id, id);
CREATE INDEX metric_observations_identity_source
ON metric_observations (
    wave_id,
    metric_id,
    source_time_seconds,
    source_time_nanoseconds,
    observation_id
);
CREATE INDEX metric_observations_contract_evidence
ON metric_observations (
    wave_id,
    metric_id,
    contract_revision,
    graduation_qualifying
);
CREATE UNIQUE INDEX idx_pr_landings_active
    ON pr_landings(repo, pr_number)
    WHERE state IN ('watching', 'repairing');
CREATE INDEX idx_pr_landings_supervisor
    ON pr_landings(state, supervisor_heartbeat_at);
CREATE INDEX idx_ci_incidents_observed
    ON ci_incidents(poll_observed_at, webhook_received_at);
CREATE INDEX idx_ci_incidents_pr ON ci_incidents(pr_id, created_at);
CREATE INDEX idx_ci_incidents_landing ON ci_incidents(landing_id, created_at);
CREATE INDEX idx_ci_incidents_open ON ci_incidents(green_at, merged_at, updated_at);
CREATE INDEX open_sessions ON "agent_sessions"(created_at, id) WHERE completed_at IS NULL;
CREATE INDEX session_inventory ON "agent_sessions"(interactive, completed_at, title, id);
CREATE INDEX session_repo_inventory ON "agent_sessions"(repo, interactive, completed_at, title, id);
CREATE INDEX conversation_task_inventory ON agent_sessions(task_id,title,id);
CREATE UNIQUE INDEX session_event_receipt ON session_events(
    session_id, COALESCE(provider_thread,''), COALESCE(provider_turn,''), kind, receipt_key);
CREATE INDEX session_events_history ON session_events(session_id,seq);
CREATE INDEX session_events_turn ON session_events(session_id,provider_thread,provider_turn,seq);
CREATE INDEX session_event_input ON session_events(session_id,captured_event) WHERE kind='started';
CREATE INDEX session_input_membership ON session_events(
    session_id, captured_event, CASE WHEN json_valid(payload) THEN CASE WHEN json_extract(payload,'$.source')='manifest.json' AND json_extract(payload,'$.evidence.schema_version')=1 AND json_extract(payload,'$.evidence.artifact_key')=json_extract(payload,'$.input_id') AND receipt_key=json_extract(payload,'$.input_id')||':manifest.json' THEN json_extract(payload,'$.evidence.flow.kind') END END)
    WHERE kind='observed' AND substr(receipt_key,-14)=':manifest.json';
CREATE UNIQUE INDEX session_capture_key ON session_events(receipt_key) WHERE kind='captured';
CREATE INDEX pm_items_identifier ON pm_items(repo, provider, identifier);
CREATE INDEX pm_items_project ON pm_items(repo, provider, project_id);
CREATE UNIQUE INDEX agent_sessions_primary_repository ON agent_sessions(repo)
    WHERE primary_scope = 'repository' AND completed_at IS NULL;
CREATE UNIQUE INDEX agent_sessions_primary_wave ON agent_sessions(wave_id)
    WHERE primary_scope = 'wave' AND completed_at IS NULL;
CREATE INDEX idx_provider_account_switches_provider_time
    ON provider_account_switches(provider, switched_at);
CREATE UNIQUE INDEX project_transitions_pending
    ON project_transitions(wave_id) WHERE settled_at IS NULL;
CREATE INDEX processes_parent ON processes(parent_lf_process_id, started_at, id);
CREATE INDEX processes_trace ON processes(trace_id, started_at, id);
CREATE INDEX processes_recent ON processes(started_at DESC, id);
CREATE INDEX processes_unfinished ON processes(started_at, id) WHERE completed_at IS NULL;
CREATE INDEX session_events_process ON session_events(lf_process_id, session_id) WHERE lf_process_id IS NOT NULL;
CREATE INDEX session_process_membership ON session_events(session_id,lf_process_id) WHERE lf_process_id IS NOT NULL;
CREATE INDEX idx_projects_wave ON projects(wave_id, created_at);
CREATE INDEX idx_projects_wave_updated ON projects(wave_id, updated_at DESC);
CREATE INDEX idx_task_comments_task ON task_comments(task_id,created_at,id);
CREATE UNIQUE INDEX idx_task_prs_current ON task_prs(task_id) WHERE historical=0;
CREATE UNIQUE INDEX idx_task_prs_current_branch ON task_prs(branch) WHERE historical=0;
CREATE INDEX agent_process_session ON processes(agent_session_id) WHERE kind='agent';
CREATE INDEX agent_process_attachment ON processes(attached_lf_process_id) WHERE kind='agent';
CREATE INDEX session_observation_receipt ON session_events(session_id,receipt_key) WHERE kind='observed';
CREATE INDEX idx_project_changes_pending ON project_changes(project_id,field,seq);
CREATE INDEX idx_task_changes_pending ON task_changes(task_id,field,seq);
CREATE INDEX planning_peer_changes_object ON planning_peer_changes(object_id,field);
CREATE INDEX steers_task ON steers(task_id, seq);
CREATE INDEX steers_wave ON steers(wave_id, seq);
CREATE UNIQUE INDEX steer_batches_boundary ON steer_batches(agent_process_id,boundary) WHERE evidence != 'cancelled';
CREATE UNIQUE INDEX repository_selected_plan ON repository_plans(repo) WHERE selected=1;
CREATE INDEX task_prs_branch ON task_prs(branch);
CREATE INDEX relocated_process_tasks ON relocated_task_processes(process_id, task_id);
CREATE INDEX idx_tasks_project ON tasks(project_id, created_at);
CREATE INDEX idx_tasks_issue_identifier ON tasks(issue_identifier);
CREATE UNIQUE INDEX idx_tasks_worktree ON tasks(worktree);
CREATE INDEX idx_tasks_updated ON tasks(updated_at DESC);
CREATE INDEX tasks_branch ON tasks(branch);
CREATE INDEX idx_tasks_repository ON tasks(repository_id, project_id, planning_rank);
CREATE UNIQUE INDEX processes_receipt_locator ON processes(receipt_locator);
CREATE INDEX idx_task_ownership_task ON task_ownership(task_id);
CREATE INDEX graph_process_steps_graph ON graph_process_steps(graph_lf_process_id, seq);
CREATE INDEX idx_rentals_ondemand_live ON rentals(ondemand) WHERE state NOT IN ('ended','lost');
CREATE INDEX agent_sessions_transport ON agent_sessions(transport_lf_process_id)
    WHERE transport_lf_process_id IS NOT NULL;
CREATE INDEX session_turn_events ON session_events(
    session_id, provider_thread, provider_turn, kind, observed_at)
    WHERE kind IN ('started','usage','completed','output');
CREATE INDEX session_events_captured_event ON session_events(captured_event)
    WHERE captured_event IS NOT NULL;
CREATE INDEX agent_sessions_agent_process ON agent_sessions(agent_process_id)
    WHERE agent_process_id IS NOT NULL;
CREATE INDEX ci_incidents_repair_process ON ci_incidents(repair_lf_process_id)
    WHERE repair_lf_process_id IS NOT NULL;
CREATE INDEX tasks_ship_process ON tasks(ship_lf_process_id)
    WHERE ship_lf_process_id IS NOT NULL;
CREATE TABLE task_graphs (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    graph TEXT NOT NULL CHECK (json_valid(graph)),
    step TEXT NOT NULL,
    run_start TEXT CHECK (run_start IS NULL OR json_valid(run_start)),
    lf_process_id TEXT REFERENCES processes(id),
    updated_at INTEGER NOT NULL,
    CHECK ((run_start IS NULL) = (lf_process_id IS NULL))
) STRICT;
CREATE INDEX task_graphs_process ON task_graphs(lf_process_id)
    WHERE lf_process_id IS NOT NULL;
CREATE TABLE "task_state_deliveries" (
    seq INTEGER PRIMARY KEY,
    id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE RESTRICT,
    target TEXT NOT NULL CHECK(target IN ('completed','unstarted','started','canceled')),
    base_revision TEXT,
    base_state TEXT,
    attempted INTEGER NOT NULL DEFAULT 0 CHECK(attempted IN (0,1)),
    settled INTEGER NOT NULL DEFAULT 0 CHECK(settled IN (0,1)),
    error TEXT,
    conflict_json TEXT CHECK(conflict_json IS NULL OR json_valid(conflict_json))
);
CREATE INDEX task_state_deliveries_task ON task_state_deliveries(task_id,seq);
CREATE TABLE "waves" (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    repo TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    parent_wave_id TEXT REFERENCES "waves"(id) ON DELETE CASCADE,
    promoted_at INTEGER,
    retired_at INTEGER,
    superseded_by_wave_id TEXT REFERENCES "waves"(id) ON DELETE RESTRICT,
    retirement_reason TEXT,
    work_state TEXT NOT NULL DEFAULT 'ready'
        CHECK (work_state IN ('ready', 'done', 'abandoned')),
    work_terminal_at INTEGER,
    current_project_id TEXT REFERENCES projects(id),
    project_activation_lf_process_id TEXT REFERENCES processes(id),
    CHECK (
        retired_at IS NOT NULL
        OR (superseded_by_wave_id IS NULL AND retirement_reason IS NULL)
    ),
    CHECK (retirement_reason IS NULL OR length(trim(retirement_reason)) > 0)
);
CREATE INDEX idx_waves_parent
    ON waves(parent_wave_id) WHERE retired_at IS NULL;
CREATE INDEX idx_waves_superseded_by
    ON waves(superseded_by_wave_id) WHERE superseded_by_wave_id IS NOT NULL;
CREATE UNIQUE INDEX idx_waves_active_locator
    ON waves(repo,ifnull(parent_wave_id,''),name) WHERE retired_at IS NULL;
CREATE INDEX waves_project_activation_process ON waves(project_activation_lf_process_id)
    WHERE project_activation_lf_process_id IS NOT NULL;
CREATE VIEW wave_addresses AS
WITH RECURSIVE addresses(id,slug) AS (
    SELECT id,name FROM waves WHERE parent_wave_id IS NULL
    UNION ALL
    SELECT w.id,a.slug || '/' || w.name FROM waves w
    JOIN addresses a ON w.parent_wave_id=a.id
)
SELECT w.*,a.slug FROM waves w JOIN addresses a ON a.id=w.id;
CREATE TABLE wave_lifecycle_operations (
    id TEXT PRIMARY KEY,
    repo TEXT NOT NULL,
    wave_id TEXT NOT NULL REFERENCES waves(id),
    kind TEXT NOT NULL CHECK (kind IN ('remove', 'rename')),
    operation_json TEXT NOT NULL
);
CREATE INDEX wave_lifecycle_retry ON wave_lifecycle_operations(repo, kind);
CREATE UNIQUE INDEX idx_machines_local ON machines(is_local) WHERE is_local=1;
CREATE TABLE desktop_shells (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id),
    checkout TEXT NOT NULL,
    owner TEXT REFERENCES processes(id),
    created_at INTEGER NOT NULL,
    jobs_settled INTEGER NOT NULL DEFAULT 0,
    close_requested INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX desktop_shells_task ON desktop_shells(task_id);
CREATE UNIQUE INDEX desktop_shells_owner ON desktop_shells(owner);
CREATE TABLE task_retirements (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id),
    completion TEXT NOT NULL,
    checkout TEXT NOT NULL,
    branch TEXT NOT NULL,
    due_at INTEGER,
    phase TEXT NOT NULL CHECK(phase IN ('waiting','closing','retired')),
    reason TEXT,
    retry_at INTEGER,
    failures INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE agent_limit_recoveries (
    lf_process_id TEXT PRIMARY KEY REFERENCES processes(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    reason TEXT NOT NULL,
    retry_at INTEGER,
    agent TEXT,
    updated_at INTEGER NOT NULL
);
CREATE TABLE session_moves (
    session_id TEXT PRIMARY KEY,
    agent TEXT NOT NULL,
    requested_at INTEGER NOT NULL
);
CREATE TABLE creation_receipts (
    request_id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    run INTEGER NOT NULL,
    task_id TEXT,
    session_id TEXT,
    artifact_key TEXT,
    milestone TEXT NOT NULL,
    result TEXT,
    launch TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE store_revisions (
    domain TEXT PRIMARY KEY NOT NULL CHECK (domain IN ('planning','sessions','graphs','processes','usage','task_updates')),
    revision INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TRIGGER preserve_capture_delete BEFORE DELETE ON session_events WHEN OLD.kind='captured'
BEGIN SELECT RAISE(ABORT,'Captured Session evidence must be retained'); END;
CREATE TRIGGER validate_session_capture_insert BEFORE INSERT ON agent_sessions
WHEN NEW.current_capture IS NOT NULL AND NOT EXISTS(SELECT 1 FROM session_events WHERE seq=NEW.current_capture AND session_id=NEW.id AND kind='captured')
BEGIN SELECT RAISE(ABORT,'Captured event belongs to another Session'); END;
CREATE TRIGGER retain_task_agent_turn BEFORE DELETE ON session_events
WHEN OLD.kind='started' AND OLD.task_id IS NOT NULL
    AND NOT EXISTS(SELECT 1 FROM session_events WHERE kind='started' AND task_id=OLD.task_id AND seq!=OLD.seq)
BEGIN SELECT RAISE(ABORT,'Cannot remove the last work of a started Task'); END;
CREATE TRIGGER store_revision_pm_items_insert AFTER INSERT ON pm_items
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_items_update AFTER UPDATE ON pm_items
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_items_delete AFTER DELETE ON pm_items
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_projects_insert AFTER INSERT ON pm_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_projects_update AFTER UPDATE ON pm_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_projects_delete AFTER DELETE ON pm_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_projects_insert AFTER INSERT ON pm_wave_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_projects_update AFTER UPDATE ON pm_wave_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_projects_delete AFTER DELETE ON pm_wave_projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_sync_insert AFTER INSERT ON pm_wave_sync
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_sync_update AFTER UPDATE ON pm_wave_sync
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_wave_sync_delete AFTER DELETE ON pm_wave_sync
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_issue_changes_insert AFTER INSERT ON pm_issue_changes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_issue_changes_update AFTER UPDATE ON pm_issue_changes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pm_issue_changes_delete AFTER DELETE ON pm_issue_changes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_instruments_insert AFTER INSERT ON metric_instruments
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_instruments_update AFTER UPDATE ON metric_instruments
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_instruments_delete AFTER DELETE ON metric_instruments
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_observations_insert AFTER INSERT ON metric_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_observations_update AFTER UPDATE ON metric_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_metric_observations_delete AFTER DELETE ON metric_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_project_events_insert AFTER INSERT ON project_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_project_events_update AFTER UPDATE ON project_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_project_events_delete AFTER DELETE ON project_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_events_insert AFTER INSERT ON task_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_events_update AFTER UPDATE ON task_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_events_delete AFTER DELETE ON task_events
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_deletions_insert AFTER INSERT ON task_deletions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_deletions_update AFTER UPDATE ON task_deletions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_deletions_delete AFTER DELETE ON task_deletions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_issue_identities_insert AFTER INSERT ON task_issue_identities
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_issue_identities_update AFTER UPDATE ON task_issue_identities
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_issue_identities_delete AFTER DELETE ON task_issue_identities
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_linear_observations_insert AFTER INSERT ON task_linear_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_linear_observations_update AFTER UPDATE ON task_linear_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_linear_observations_delete AFTER DELETE ON task_linear_observations
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_pr_repair_incidents_insert AFTER INSERT ON task_pr_repair_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_pr_repair_incidents_update AFTER UPDATE ON task_pr_repair_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_pr_repair_incidents_delete AFTER DELETE ON task_pr_repair_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pr_landings_insert AFTER INSERT ON pr_landings
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pr_landings_update AFTER UPDATE ON pr_landings
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_pr_landings_delete AFTER DELETE ON pr_landings
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_ci_incidents_insert AFTER INSERT ON ci_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_ci_incidents_update AFTER UPDATE ON ci_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_ci_incidents_delete AFTER DELETE ON ci_incidents
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_agent_sessions_insert AFTER INSERT ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_agent_sessions_delete AFTER DELETE ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_project_transitions_insert AFTER INSERT ON project_transitions
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;
CREATE TRIGGER store_revision_project_transitions_update AFTER UPDATE ON project_transitions
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;
CREATE TRIGGER store_revision_project_transitions_delete AFTER DELETE ON project_transitions
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;
CREATE TRIGGER store_revision_project_transition_items_insert AFTER INSERT ON project_transition_items
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;
CREATE TRIGGER store_revision_project_transition_items_update AFTER UPDATE ON project_transition_items
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;
CREATE TRIGGER store_revision_project_transition_items_delete AFTER DELETE ON project_transition_items
BEGIN UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning'; END;
CREATE TRIGGER store_revision_session_activity_insert AFTER INSERT ON session_activity
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_session_activity_delete AFTER DELETE ON session_activity
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_processes_insert AFTER INSERT ON processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;
CREATE TRIGGER store_revision_processes_delete AFTER DELETE ON processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;
CREATE TRIGGER pr_landings_no_machine_insert
BEFORE INSERT ON pr_landings WHEN NEW.supervisor_placement='home'
BEGIN
    SELECT RAISE(ABORT, 'Remote machine landing supervision is retired');
END;
CREATE TRIGGER store_revision_projects_insert AFTER INSERT ON projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_projects_update AFTER UPDATE ON projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_projects_delete AFTER DELETE ON projects
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_comments_insert AFTER INSERT ON task_comments
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_comments_update AFTER UPDATE ON task_comments
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_comments_delete AFTER DELETE ON task_comments
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_comment_deliveries_insert AFTER INSERT ON task_comment_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_comment_deliveries_update AFTER UPDATE ON task_comment_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_comment_deliveries_delete AFTER DELETE ON task_comment_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER task_pr_repair_incidents_require_active_pr
BEFORE INSERT ON task_pr_repair_incidents
WHEN EXISTS (
    SELECT 1
    FROM task_prs
    WHERE id = NEW.task_pr_id
      AND (merge_commit IS NOT NULL OR abandoned_at IS NOT NULL)
)
BEGIN
    SELECT RAISE(ABORT, 'repair incident requires an active Task PR');
END;
CREATE TRIGGER task_prs_enable_performance_tracking
AFTER INSERT ON task_prs
BEGIN
    UPDATE task_prs
    SET merge_tracking_complete = 1,
        repair_tracking_complete = 1
    WHERE id = NEW.id;
END;
CREATE TRIGGER store_revision_task_prs_insert AFTER INSERT ON task_prs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_prs_update AFTER UPDATE ON task_prs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_prs_delete AFTER DELETE ON task_prs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER task_pr_history_no_delete BEFORE DELETE ON task_prs WHEN OLD.historical=1
BEGIN SELECT RAISE(ABORT, 'historical Task PRs are read-only'); END;
CREATE TRIGGER task_pr_history_no_insert BEFORE INSERT ON task_prs WHEN NEW.historical=1
BEGIN SELECT RAISE(ABORT, 'only migration can create historical Task PRs'); END;
CREATE TRIGGER task_pr_placement_insert BEFORE INSERT ON task_prs WHEN NEW.historical=0
AND NOT EXISTS(SELECT 1 FROM tasks t WHERE t.id=NEW.task_id AND t.branch=NEW.branch
 AND t.base_commit=NEW.base_commit AND t.parent_pr_id IS NEW.parent_pr_id)
BEGIN SELECT RAISE(ABORT, 'Task PR placement differs from Task'); END;
CREATE TRIGGER store_revision_project_changes_insert AFTER INSERT ON project_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_project_changes_update AFTER UPDATE ON project_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_project_changes_delete AFTER DELETE ON project_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_changes_insert AFTER INSERT ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_changes_update AFTER UPDATE ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_changes_delete AFTER DELETE ON task_changes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_destinations_insert AFTER INSERT ON planning_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_destinations_update AFTER UPDATE ON planning_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_destinations_delete AFTER DELETE ON planning_destinations
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_routes_insert AFTER INSERT ON planning_peer_routes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_routes_update AFTER UPDATE ON planning_peer_routes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_routes_delete AFTER DELETE ON planning_peer_routes
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_imports_insert AFTER INSERT ON planning_peer_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_imports_update AFTER UPDATE ON planning_peer_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_imports_delete AFTER DELETE ON planning_peer_imports
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_conflicts_insert AFTER INSERT ON planning_peer_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_conflicts_update AFTER UPDATE ON planning_peer_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_planning_peer_conflicts_delete AFTER DELETE ON planning_peer_conflicts
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_steers_insert AFTER INSERT ON steers
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_steers_update AFTER UPDATE ON steers
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_steers_delete AFTER DELETE ON steers
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_repository_plans_insert AFTER INSERT ON repository_plans
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_repository_plans_update AFTER UPDATE ON repository_plans
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_repository_plans_delete AFTER DELETE ON repository_plans
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_relocated_task_processes_insert AFTER INSERT ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;
CREATE TRIGGER store_revision_relocated_task_processes_update AFTER UPDATE ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;
CREATE TRIGGER store_revision_relocated_task_processes_delete AFTER DELETE ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;
CREATE TRIGGER store_revision_tasks_insert AFTER INSERT ON tasks
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tasks_update AFTER UPDATE ON tasks
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_tasks_delete AFTER DELETE ON tasks
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_creation_intents_insert AFTER INSERT ON task_creation_intents
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_creation_intents_update AFTER UPDATE ON task_creation_intents
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_creation_intents_delete AFTER DELETE ON task_creation_intents
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER validate_task_repository_insert BEFORE INSERT ON tasks
WHEN NEW.project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects p JOIN waves w ON w.id=p.wave_id
    JOIN repository_plans r ON r.repo=w.repo WHERE p.id=NEW.project_id AND r.id=NEW.repository_id
)
BEGIN SELECT RAISE(ABORT, 'Task Project must belong to its repository'); END;
CREATE TRIGGER validate_conversation_ancestry_insert BEFORE INSERT ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t LEFT JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id IS NEW.wave_id
    ) THEN RAISE(ABORT,'Session Task and Wave disagree') END;
END;
CREATE TRIGGER store_revision_session_activity_update AFTER UPDATE ON session_activity
WHEN NEW.attachment_token IS NOT OLD.attachment_token
    OR NEW.agent_process_id IS NOT OLD.agent_process_id
    OR NEW.preview IS NOT OLD.preview
    OR NEW.program_status IS NOT OLD.program_status
    OR (NEW.pending_input > 0) IS NOT (OLD.pending_input > 0)
    OR (NEW.open_tools = 0) IS NOT (OLD.open_tools = 0)
    OR NEW.yielded IS NOT OLD.yielded
    OR (OLD.open_tools = 0 AND NEW.observed_at - OLD.observed_at >= 120)
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER session_preview_agent_session_update
AFTER UPDATE OF provider_thread,agent_process_id ON agent_sessions
WHEN NEW.provider_thread IS NOT OLD.provider_thread OR NEW.agent_process_id IS NOT OLD.agent_process_id
BEGIN
    UPDATE session_activity SET preview=NULL WHERE session_id=NEW.id;
END;
CREATE TRIGGER session_fence_locator_insert AFTER INSERT ON agent_sessions
WHEN NEW.fence_locator IS NULL
BEGIN
    UPDATE agent_sessions SET fence_locator=NEW.id WHERE id=NEW.id;
END;
CREATE TRIGGER session_fence_locator_immutable BEFORE UPDATE OF fence_locator ON agent_sessions
WHEN OLD.fence_locator IS NOT NULL AND NEW.fence_locator IS NOT OLD.fence_locator
BEGIN
    SELECT RAISE(ABORT, 'Session fence locator is immutable');
END;
CREATE TRIGGER process_locators_insert AFTER INSERT ON processes
BEGIN
    UPDATE processes SET receipt_locator=COALESCE(NEW.receipt_locator,NEW.id),
        journal_trace_locator=COALESCE(NEW.journal_trace_locator,
            (SELECT journal_trace_locator FROM processes WHERE trace_id=NEW.trace_id
             AND id!=NEW.id ORDER BY started_at,id LIMIT 1),NEW.trace_id)
    WHERE id=NEW.id;
END;
CREATE TRIGGER process_locators_immutable BEFORE UPDATE OF receipt_locator,journal_trace_locator ON processes
WHEN (OLD.receipt_locator IS NOT NULL AND NEW.receipt_locator IS NOT OLD.receipt_locator)
    OR (OLD.journal_trace_locator IS NOT NULL AND NEW.journal_trace_locator IS NOT OLD.journal_trace_locator)
BEGIN
    SELECT RAISE(ABORT, 'Process filesystem locators are immutable');
END;
CREATE TRIGGER store_revision_agent_sessions_update AFTER UPDATE ON agent_sessions
WHEN OLD.fence_locator IS NOT NULL
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_processes_update AFTER UPDATE ON processes
WHEN OLD.receipt_locator IS NOT NULL
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'processes';
END;
CREATE TRIGGER store_revision_agent_process_update AFTER UPDATE ON processes
WHEN NEW.kind='agent' AND OLD.receipt_locator IS NOT NULL
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER task_pr_repair_incidents_are_immutable
BEFORE UPDATE ON task_pr_repair_incidents
BEGIN
    SELECT RAISE(ABORT, 'Task PR repair incidents are immutable');
END;
CREATE TRIGGER preserve_capture_update BEFORE UPDATE ON session_events WHEN OLD.kind='captured'
BEGIN SELECT RAISE(ABORT,'Captured Session evidence is immutable'); END;
CREATE TRIGGER validate_session_capture BEFORE UPDATE OF current_capture ON agent_sessions
WHEN NEW.current_capture IS NOT NULL AND NOT EXISTS(SELECT 1 FROM session_events WHERE seq=NEW.current_capture AND session_id=NEW.id AND kind='captured')
BEGIN SELECT RAISE(ABORT,'Captured event belongs to another Session'); END;
CREATE TRIGGER pr_landings_no_machine_update
BEFORE UPDATE OF supervisor_placement ON pr_landings WHEN NEW.supervisor_placement='home'
BEGIN
    SELECT RAISE(ABORT, 'Remote machine landing supervision is retired');
END;
CREATE TRIGGER selected_project_owner_update BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS(SELECT 1 FROM waves WHERE current_project_id=NEW.id AND id!=NEW.wave_id)
BEGIN SELECT RAISE(ABORT, 'selected Project cannot change Wave ownership'); END;
CREATE TRIGGER task_prs_preserve_merged_at
BEFORE UPDATE OF merged_at ON task_prs
WHEN OLD.merged_at IS NOT NULL
 AND NEW.merged_at IS NOT OLD.merged_at
BEGIN
    SELECT RAISE(ABORT, 'Task PR merge evidence is immutable');
END;
CREATE TRIGGER task_pr_history_immutable BEFORE UPDATE ON task_prs WHEN OLD.historical=1
BEGIN SELECT RAISE(ABORT, 'historical Task PRs are read-only'); END;
CREATE TRIGGER task_pr_placement_update BEFORE UPDATE OF branch,base_commit,parent_pr_id ON task_prs
WHEN NEW.historical=0 AND NOT EXISTS(SELECT 1 FROM tasks t WHERE t.id=NEW.task_id
 AND t.branch=NEW.branch AND t.base_commit=NEW.base_commit AND t.parent_pr_id IS NEW.parent_pr_id)
BEGIN SELECT RAISE(ABORT, 'Task PR placement differs from Task'); END;
CREATE TRIGGER task_pr_history_no_mark BEFORE UPDATE OF historical ON task_prs
WHEN NEW.historical<>OLD.historical AND NOT (
 OLD.historical=0 AND NEW.historical=1
 AND (OLD.merge_commit IS NOT NULL OR OLD.abandoned_at IS NOT NULL))
BEGIN SELECT RAISE(ABORT, 'only settled Task PRs can become history'); END;
CREATE TRIGGER task_placement_to_pr AFTER UPDATE OF branch,base_commit,parent_pr_id ON tasks
BEGIN
 UPDATE task_prs SET branch=NEW.branch, base_commit=NEW.base_commit,
 parent_pr_id=NEW.parent_pr_id WHERE task_id=NEW.id AND historical=0
 AND merge_commit IS NULL AND abandoned_at IS NULL;
END;
CREATE TRIGGER task_repository_is_fixed BEFORE UPDATE OF repository_id ON tasks
WHEN NEW.repository_id IS NOT OLD.repository_id
BEGIN SELECT RAISE(ABORT, 'Task repository identity cannot change'); END;
CREATE TRIGGER validate_task_repository_update BEFORE UPDATE OF project_id,repository_id ON tasks
WHEN NEW.project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects p JOIN waves w ON w.id=p.wave_id
    JOIN repository_plans r ON r.repo=w.repo WHERE p.id=NEW.project_id AND r.id=NEW.repository_id
)
BEGIN SELECT RAISE(ABORT, 'Task Project must belong to its repository'); END;
CREATE TRIGGER validate_project_task_repositories BEFORE UPDATE OF wave_id ON projects
WHEN EXISTS (SELECT 1 FROM tasks t JOIN repository_plans r ON r.id=t.repository_id
    WHERE t.project_id=NEW.id AND NOT EXISTS(SELECT 1 FROM waves w WHERE w.id=NEW.wave_id AND w.repo=r.repo))
BEGIN SELECT RAISE(ABORT, 'Project move would change Task repository'); END;
CREATE TRIGGER task_session_membership AFTER UPDATE OF project_id ON tasks BEGIN
    UPDATE agent_sessions SET wave_id=(SELECT wave_id FROM projects WHERE id=NEW.project_id) WHERE task_id=NEW.id;
END;
CREATE TRIGGER project_task_session_membership AFTER UPDATE OF wave_id ON projects BEGIN
    UPDATE agent_sessions SET wave_id=NEW.wave_id WHERE task_id IN (SELECT id FROM tasks WHERE project_id=NEW.id);
END;
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_linear_team_checks_insert BEFORE INSERT ON linear_team_checks
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: linear_team_checks') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.team_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_machines_insert BEFORE INSERT ON machines
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: machines') FROM (
 WITH incoming AS (
SELECT 'home_' AS domain,NEW.id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_planning_peer_changes_insert BEFORE INSERT ON planning_peer_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.edit.target' AND NEW.record_table='waves' THEN 'uuid' WHEN path='$.edit.target' AND NEW.record_table='projects' THEN 'proj_' WHEN path='$.edit.target' AND NEW.record_table='tasks' THEN 'task_' WHEN path='$.edit.target' AND NEW.record_table='task_comments' THEN 'uuid' WHEN parent='$.parents' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_parent','project_wave','project_linear_id','task_linear_id','task_assignee','task_team') THEN 'uuid' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('wave_project','task_project') THEN 'proj_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('comment_task') THEN 'task_' WHEN path='$.edit.change.value' AND json_extract(doc,'$.edit.change.field') IN ('task_identifier') THEN 'label' WHEN parent='$.edit.change.value' AND typeof(key)='integer' AND json_extract(doc,'$.edit.change.field') IN ('project_initiatives','project_teams') THEN 'uuid' WHEN path='$.edit.change.value.machine_id' AND json_extract(doc,'$.edit.change.field') IN ('wave_delegation','project_delegation','task_delegation','task_admission_authority') THEN 'home_' WHEN path='$.edit.change.value.repository_id' AND json_extract(doc,'$.edit.change.field')='task_admission_authority' THEN 'repo_' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.mutation AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.mutation) THEN NEW.mutation END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_planning_peer_conflicts_insert BEFORE INSERT ON planning_peer_conflicts
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: planning_peer_conflicts') FROM (
 WITH incoming AS (
SELECT CASE NEW.kind WHEN 'waves' THEN 'uuid' WHEN 'projects' THEN 'proj_' WHEN 'tasks' THEN 'task_' WHEN 'task_comments' THEN 'uuid' END AS domain,NEW.object_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_pm_issue_changes_insert BEFORE INSERT ON pm_issue_changes
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_issue_changes') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.issue_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_pm_project_name_cutover_insert BEFORE INSERT ON pm_project_name_cutover
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_project_name_cutover') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN parent='$.initiative_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.team_ids' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.body AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.body) THEN NEW.body END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_pm_project_name_cutover_update BEFORE UPDATE OF id,body ON pm_project_name_cutover
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_project_name_cutover') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT CASE WHEN path='$.id' THEN 'uuid' WHEN parent='$.initiative_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.team_ids' AND typeof(key)='integer' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.body AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.body) THEN NEW.body END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_pm_wave_projects_insert BEFORE INSERT ON pm_wave_projects
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_wave_projects') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.project_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_pm_wave_sync_insert BEFORE INSERT ON pm_wave_sync
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_wave_sync') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.initiative AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_pm_wave_sync_update BEFORE UPDATE OF wave_id,initiative ON pm_wave_sync
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: pm_wave_sync') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.initiative AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_project_events_insert BEFORE INSERT ON project_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_events') FROM (
 WITH incoming AS (
SELECT 'proj_' AS domain,NEW.project_id AS original
UNION ALL
SELECT CASE WHEN path='$.event.pr_id' THEN 'pr_' WHEN path='$.event.steer_id' THEN 'uuid' WHEN path='$.event.invocation_id' THEN 'uuid' WHEN path='$.event.relation_id' THEN 'uuid' WHEN path='$.event.source_issue_id' THEN 'uuid' WHEN path='$.event.target_issue_id' THEN 'uuid' WHEN parent='$.event.lf_process_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.event.exec_ids' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.event.intent.issue_id' THEN 'task_or_uuid' WHEN path='$.event.intent.project_id' THEN 'project_or_uuid' WHEN path='$.event.intent.relation_id' THEN 'uuid' WHEN path='$.event.intent.team_id' THEN 'uuid' WHEN path='$.event.intent.state_id' THEN 'uuid' WHEN path='$.event.intent.identifier' THEN 'label' WHEN path='$.event.link.issue_id' THEN 'task_or_uuid' WHEN path='$.event.link.project_id' THEN 'project_or_uuid' WHEN path='$.event.link.relation_id' THEN 'uuid' WHEN path='$.event.link.team_id' THEN 'uuid' WHEN path='$.event.link.state_id' THEN 'uuid' WHEN path='$.event.link.identifier' THEN 'label' WHEN path='$.task_id' THEN 'task_' WHEN path='$.steer_id' THEN 'uuid' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.kind_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.kind_json) THEN NEW.kind_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_project_transition_items_insert BEFORE INSERT ON project_transition_items
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: project_transition_items') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'project_or_uuid' AS domain,NEW.successor_id AS original
UNION ALL
SELECT 'task_or_uuid' AS domain,NEW.issue_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_repository_plans_insert BEFORE INSERT ON repository_plans
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: repository_plans') FROM (
 WITH incoming AS (
SELECT 'repo_' AS domain,NEW.id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_session_activity_insert BEFORE INSERT ON session_activity
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_activity') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_session_activity_update BEFORE UPDATE OF attachment_token,agent_process_id,session_id ON session_activity
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: session_activity') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.attachment_token AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.agent_process_id AS original
UNION ALL
SELECT 'session' AS domain,NEW.session_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_task_comments_insert BEFORE INSERT ON task_comments
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_comments') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_task_deletions_insert BEFORE INSERT ON task_deletions
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_deletions') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.issue_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_task_events_insert BEFORE INSERT ON task_events
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_events') FROM (
 WITH incoming AS (
SELECT 'task_' AS domain,NEW.task_id AS original
UNION ALL
SELECT CASE WHEN path='$.pr_id' THEN 'pr_' WHEN path='$.steer_id' THEN 'uuid' WHEN path='$.invocation_id' THEN 'uuid' WHEN path='$.relation_id' THEN 'uuid' WHEN path='$.source_issue_id' THEN 'uuid' WHEN path='$.target_issue_id' THEN 'uuid' WHEN parent='$.lf_process_ids' AND typeof(key)='integer' THEN 'uuid' WHEN parent='$.exec_ids' AND typeof(key)='integer' THEN 'uuid' WHEN path='$.intent.issue_id' THEN 'task_or_uuid' WHEN path='$.intent.project_id' THEN 'project_or_uuid' WHEN path='$.intent.relation_id' THEN 'uuid' WHEN path='$.intent.team_id' THEN 'uuid' WHEN path='$.intent.state_id' THEN 'uuid' WHEN path='$.intent.identifier' THEN 'label' WHEN path='$.link.issue_id' THEN 'task_or_uuid' WHEN path='$.link.project_id' THEN 'project_or_uuid' WHEN path='$.link.relation_id' THEN 'uuid' WHEN path='$.link.team_id' THEN 'uuid' WHEN path='$.link.state_id' THEN 'uuid' WHEN path='$.link.identifier' THEN 'label' END AS domain,atom AS original FROM (
 SELECT replace(fullkey,'"','') AS path,replace(path,'"','') AS parent,key,atom,NEW.kind_json AS doc
 FROM json_tree(CASE WHEN json_valid(NEW.kind_json) THEN NEW.kind_json END) WHERE type='text')
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_task_issue_identities_insert BEFORE INSERT ON task_issue_identities
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_issue_identities') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.wave_id AS original
UNION ALL
SELECT 'uuid' AS domain,NEW.issue_id AS original
UNION ALL
SELECT 'label' AS domain,NEW.identifier AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_task_linear_ingested_comments_insert BEFORE INSERT ON task_linear_ingested_comments
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_linear_ingested_comments') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.comment_id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_tool_responses_insert BEFORE INSERT ON tool_responses
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: tool_responses') FROM (
 WITH incoming AS (
SELECT 'response_' AS domain,NEW.id AS original
UNION ALL
SELECT CASE NEW.work_kind WHEN 'wave' THEN 'uuid' WHEN 'project' THEN 'proj_' WHEN 'task' THEN 'task_' END AS domain,NEW.work_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER task_ownership_retained BEFORE DELETE ON task_ownership
BEGIN SELECT RAISE(ABORT, 'Task ownership entries are retained'); END;
CREATE TRIGGER store_revision_task_ownership_insert AFTER INSERT ON task_ownership
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_ownership_update AFTER UPDATE ON task_ownership
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER validate_graph_process_step BEFORE INSERT ON graph_process_steps BEGIN
    SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM processes step
        WHERE step.id=NEW.lf_process_id AND step.parent_lf_process_id=NEW.graph_lf_process_id
    ) THEN RAISE(ABORT,'A Graph step is a process its Graph process started') END;
END;
CREATE TRIGGER graph_processes_are_append_only BEFORE UPDATE ON graph_processes BEGIN
    SELECT RAISE(ABORT,'A Graph record is append-only');
END;
CREATE TRIGGER graph_process_steps_are_append_only BEFORE UPDATE ON graph_process_steps BEGIN
    SELECT RAISE(ABORT,'A Graph record is append-only');
END;
CREATE TRIGGER store_revision_graph_processes_insert AFTER INSERT ON graph_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_processes_update AFTER UPDATE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_processes_delete AFTER DELETE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_process_steps_insert AFTER INSERT ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_process_steps_update AFTER UPDATE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_graph_process_steps_delete AFTER DELETE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'graphs';
END;
CREATE TRIGGER store_revision_dashboards_insert AFTER INSERT ON dashboards BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_dashboards_update AFTER UPDATE ON dashboards BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_dashboards_delete AFTER DELETE ON dashboards BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_dashboard_arrangements_insert AFTER INSERT ON dashboard_arrangements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_dashboard_arrangements_update AFTER UPDATE ON dashboard_arrangements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_dashboard_arrangements_delete AFTER DELETE ON dashboard_arrangements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_work_pane_layouts_insert AFTER INSERT ON work_pane_layouts BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_work_pane_layouts_update AFTER UPDATE ON work_pane_layouts BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_work_pane_layouts_delete AFTER DELETE ON work_pane_layouts BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_task_orders_insert AFTER INSERT ON task_orders BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_task_orders_update AFTER UPDATE ON task_orders BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER store_revision_task_orders_delete AFTER DELETE ON task_orders BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='planning';
END;
CREATE TRIGGER task_ownership_append_only BEFORE UPDATE ON task_ownership
WHEN NOT (OLD.ordered_at IS NULL AND NEW.ordered_at IS NOT NULL
    AND NEW.id IS OLD.id AND NEW.task_id IS OLD.task_id AND NEW.kind IS OLD.kind
    AND NEW.person IS OLD.person AND NEW.machine_id IS OLD.machine_id
    AND NEW.at IS OLD.at AND NEW.written_here IS OLD.written_here
    AND NEW.person_key IS OLD.person_key AND NEW.machine_label IS OLD.machine_label)
BEGIN SELECT RAISE(ABORT, 'Task ownership entries are append-only'); END;
CREATE TRIGGER store_revision_session_events_insert AFTER INSERT ON session_events
WHEN NEW.kind != 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_session_events_update AFTER UPDATE ON session_events
WHEN NEW.kind != 'usage' OR OLD.kind != 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_session_events_delete AFTER DELETE ON session_events
WHEN OLD.kind != 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'sessions';
END;
CREATE TRIGGER store_revision_usage_insert AFTER INSERT ON session_events
WHEN NEW.kind = 'usage'
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'usage';
END;
CREATE TRIGGER store_revision_task_state_deliveries_insert AFTER INSERT ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_state_deliveries_update AFTER UPDATE ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER store_revision_task_state_deliveries_delete AFTER DELETE ON task_state_deliveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='planning'; END;
CREATE TRIGGER canonical_task_state_deliveries_insert BEFORE INSERT ON task_state_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_state_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER canonical_task_state_deliveries_update BEFORE UPDATE OF id,task_id ON task_state_deliveries
BEGIN
 SELECT RAISE(ABORT,'canonical identity required: task_state_deliveries') FROM (
 WITH incoming AS (
SELECT 'uuid' AS domain,NEW.id AS original
UNION ALL
SELECT 'task_' AS domain,NEW.task_id AS original
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER store_revision_task_graphs_insert AFTER INSERT ON task_graphs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_graphs_update AFTER UPDATE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_task_graphs_delete AFTER DELETE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER task_graph_is_fixed BEFORE UPDATE OF graph ON task_graphs BEGIN
    SELECT RAISE(ABORT,'A Task Graph is fixed when taken up; take up another to replace it');
END;
CREATE TRIGGER store_revision_waves_insert AFTER INSERT ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_waves_update AFTER UPDATE ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER store_revision_waves_delete AFTER DELETE ON waves
BEGIN
    UPDATE store_revisions SET revision = revision + 1 WHERE domain = 'planning';
END;
CREATE TRIGGER selected_project_wave_update BEFORE UPDATE OF current_project_id ON waves
WHEN NEW.current_project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM projects WHERE id=NEW.current_project_id AND wave_id=NEW.id
) BEGIN SELECT RAISE(ABORT, 'selected Project belongs to another Wave'); END;
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
),decoded AS (
 SELECT domain,original,CASE
 WHEN domain IN ('uuid','label','session') THEN original
 WHEN domain='task_or_uuid' AND lower(substr(original,1,5))='task_' THEN substr(original,6)
 WHEN domain='project_or_uuid' AND lower(substr(original,1,5))='proj_' THEN substr(original,6)
 WHEN domain IN ('task_or_uuid','project_or_uuid') THEN original
 WHEN lower(substr(original,1,length(domain)))=domain THEN substr(original,length(domain)+1)
 END AS suffix FROM incoming
), uuids AS (
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
CREATE TRIGGER validate_conversation_ancestry_update BEFORE UPDATE OF task_id,wave_id ON agent_sessions BEGIN
    SELECT CASE WHEN NEW.task_id IS NOT NULL AND NOT EXISTS(
        SELECT 1 FROM tasks t LEFT JOIN projects p ON p.id=t.project_id
        WHERE t.id=NEW.task_id AND p.wave_id IS NEW.wave_id
    ) THEN RAISE(ABORT,'Session Task and Wave disagree') END;
    SELECT CASE WHEN OLD.task_id IS NOT NULL AND NEW.task_id IS NOT OLD.task_id
        THEN RAISE(ABORT,'Session Task binding is permanent') END;
    SELECT CASE WHEN OLD.task_id IS NULL AND OLD.wave_id IS NOT NULL AND NEW.wave_id IS NOT OLD.wave_id
        AND NOT (OLD.primary_scope IS 'wave' AND NEW.primary_scope IS NULL AND NEW.wave_id IS NULL)
        THEN RAISE(ABORT,'Wave Session binding is permanent') END;
END;
CREATE TRIGGER store_revision_desktop_shells_insert AFTER INSERT ON desktop_shells BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_desktop_shells_update AFTER UPDATE ON desktop_shells BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_desktop_shells_delete AFTER DELETE ON desktop_shells BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_task_retirements_insert AFTER INSERT ON task_retirements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_task_retirements_update AFTER UPDATE ON task_retirements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER store_revision_task_retirements_delete AFTER DELETE ON task_retirements BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='processes';
END;
CREATE TRIGGER cancel_task_retirement AFTER UPDATE OF planning_completed,planning_completed_at,worktree,branch ON tasks
WHEN NEW.planning_completed=0 OR NEW.planning_completed_at IS NOT OLD.planning_completed_at
  OR NEW.worktree IS NOT OLD.worktree OR NEW.branch IS NOT OLD.branch
BEGIN
    DELETE FROM task_retirements WHERE task_id=NEW.id;
END;
CREATE TRIGGER store_revision_agent_limit_recoveries_insert AFTER INSERT ON agent_limit_recoveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='processes'; END;
CREATE TRIGGER store_revision_agent_limit_recoveries_update AFTER UPDATE ON agent_limit_recoveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='processes'; END;
CREATE TRIGGER store_revision_agent_limit_recoveries_delete AFTER DELETE ON agent_limit_recoveries
BEGIN UPDATE store_revisions SET revision=revision+1 WHERE domain='processes'; END;
CREATE TRIGGER task_update_task_graphs_insert AFTER INSERT ON task_graphs
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_task_graphs_update AFTER UPDATE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_task_graphs_delete AFTER DELETE ON task_graphs
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_graph_processes_insert AFTER INSERT ON graph_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_graph_processes_update AFTER UPDATE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_graph_processes_delete AFTER DELETE ON graph_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_graph_process_steps_insert AFTER INSERT ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_graph_process_steps_update AFTER UPDATE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_graph_process_steps_delete AFTER DELETE ON graph_process_steps
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_task_comments_insert AFTER INSERT ON task_comments
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_task_comments_update AFTER UPDATE ON task_comments
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_task_comments_delete AFTER DELETE ON task_comments
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_relocated_task_processes_insert AFTER INSERT ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_relocated_task_processes_update AFTER UPDATE ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_relocated_task_processes_delete AFTER DELETE ON relocated_task_processes
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_tasks_insert AFTER INSERT ON tasks
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_tasks_delete AFTER DELETE ON tasks
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_tasks_update AFTER UPDATE OF issue_identifier,worktree,checkout_machine_id ON tasks
WHEN NEW.issue_identifier IS NOT OLD.issue_identifier OR NEW.worktree IS NOT OLD.worktree OR NEW.checkout_machine_id IS NOT OLD.checkout_machine_id
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_agent_sessions_insert AFTER INSERT ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_agent_sessions_delete AFTER DELETE ON agent_sessions
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_agent_sessions_update AFTER UPDATE OF task_id,primary_scope,cwd,interactive,completed_at ON agent_sessions
WHEN NEW.task_id IS NOT OLD.task_id OR NEW.primary_scope IS NOT OLD.primary_scope OR NEW.cwd IS NOT OLD.cwd OR NEW.interactive IS NOT OLD.interactive OR NEW.completed_at IS NOT OLD.completed_at
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_machines_insert AFTER INSERT ON machines
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_machines_delete AFTER DELETE ON machines
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_machines_update AFTER UPDATE OF is_local ON machines
WHEN NEW.is_local IS NOT OLD.is_local
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_processes_insert AFTER INSERT ON processes
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.id)
    OR EXISTS(SELECT 1 FROM graph_process_steps WHERE lf_process_id=NEW.id)
    OR EXISTS(SELECT 1 FROM task_graphs WHERE lf_process_id=NEW.id)
    OR (NEW.kind='agent' AND EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.attached_lf_process_id)))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_processes_update AFTER UPDATE ON processes
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.id)
    OR EXISTS(SELECT 1 FROM graph_process_steps WHERE lf_process_id=OLD.id)
    OR EXISTS(SELECT 1 FROM task_graphs WHERE lf_process_id=OLD.id)
    OR (OLD.kind='agent' AND EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.attached_lf_process_id))) OR (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.id)
    OR EXISTS(SELECT 1 FROM graph_process_steps WHERE lf_process_id=NEW.id)
    OR EXISTS(SELECT 1 FROM task_graphs WHERE lf_process_id=NEW.id)
    OR (NEW.kind='agent' AND EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.attached_lf_process_id)))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_processes_delete AFTER DELETE ON processes
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.id)
    OR EXISTS(SELECT 1 FROM graph_process_steps WHERE lf_process_id=OLD.id)
    OR EXISTS(SELECT 1 FROM task_graphs WHERE lf_process_id=OLD.id)
    OR (OLD.kind='agent' AND EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.attached_lf_process_id)))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_session_events_insert AFTER INSERT ON session_events
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.lf_process_id)
    OR (CASE WHEN NEW.kind='observed' AND json_valid(NEW.payload)
        THEN json_extract(NEW.payload,'$.source')='task_update'
         AND json_extract(NEW.payload,'$.state') IN ('baseline','dispatched') END))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_session_events_update AFTER UPDATE ON session_events
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.lf_process_id)
    OR (CASE WHEN OLD.kind='observed' AND json_valid(OLD.payload)
        THEN json_extract(OLD.payload,'$.source')='task_update'
         AND json_extract(OLD.payload,'$.state') IN ('baseline','dispatched') END)) OR (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=NEW.lf_process_id)
    OR (CASE WHEN NEW.kind='observed' AND json_valid(NEW.payload)
        THEN json_extract(NEW.payload,'$.source')='task_update'
         AND json_extract(NEW.payload,'$.state') IN ('baseline','dispatched') END))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
CREATE TRIGGER task_update_session_events_delete AFTER DELETE ON session_events
WHEN (EXISTS(SELECT 1 FROM graph_processes WHERE lf_process_id=OLD.lf_process_id)
    OR (CASE WHEN OLD.kind='observed' AND json_valid(OLD.payload)
        THEN json_extract(OLD.payload,'$.source')='task_update'
         AND json_extract(OLD.payload,'$.state') IN ('baseline','dispatched') END))
BEGIN
    UPDATE store_revisions SET revision=revision+1 WHERE domain='task_updates';
END;
