-- Frozen 0.12.8 store with an explicit Task PR merge request, captured before
-- 0.12.10 introduced reviewer-facing PR copy. This fixture is independent of
-- the current migration registry: do not regenerate it from MIGRATIONS.
-- `__LF_HOME__` is replaced with each test's isolated Home before loading.
PRAGMA foreign_keys=OFF;
BEGIN TRANSACTION;
CREATE TABLE schema_migrations (
                version TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL
            , checksum TEXT, parent_history TEXT, build_provenance TEXT, source_identity TEXT, source_revision TEXT, package_version TEXT);
INSERT INTO schema_migrations VALUES('0.10.001_initial',1787286317,'bfa974111e5a351c47d0b8a2575c85cfbff56abd89c9fda4b79ab8bd9435001b',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.10.002_session_execution_context',1787286317,'0148d95df8248cebe95f0f60e1100c4dfcf7f478381512c1aad21532fb9bf4ad',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.001_task_prs',1787286317,'88b42a585db93281a6e902962ae5f35a5b6b380e759c3fe9e05324717a3ccebc',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.002_project_session_successors',1787286317,'29b1dd339cfbdf5b69aa135e1e8e46df48c38a785f5bd9dd5baba3e3cd5f6e0e',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.003_child_body_lease',1787286317,'f4c2b66928ce0fd025cd4f1613cac2223593162326ecf9ed284586845f9df28f',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.004_task_pr_ci_state',1787286317,'75a7da762b7e5747f2197323a454675de619cc0aff17a41b223dde77afe39afb',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.005_provider_accounts',1787286317,'12efc42446edefe19c8f7ae3891af1832ee1d417984f6f4f1a7573af42aa31c7',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.006_context_launch_work',1787286317,'71d7175c1d69d6f9ad3748e74cc617b5f0b605b40d63d331ca254f737b065ef4',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.007_task_pr_parent',1787286317,'656a1d100f285485aa58be223064c2f3e64f96c900fab15c8a24e4306054effc',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.008_interactive_handoffs',1787286317,'2c295933a53b05c724a2148595d8141510ee0ce4c382da981aa499319820195a',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.009_context_pressure',1787286317,'3de7b68d28ec21b326b85eb58420daf16214e484918e05d9657f1d77a8262d9c',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.010_context_input_normalization',1787286317,'89fbc1f5c9dedd2a33f6b42380b29d4d2007e015fc94e8a579aba5b9a5386b20',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.011_profiles',1787286317,'bd141aabbe2e9eb194a6d6e7504e51df19cfc559fbf3a323328d0e1232f26211',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.012_provider_account_lifecycle',1787286317,'bd48a3073681e03985cc4444116297f21ab7ba3fe248e1e402d55945c144dc99',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.013_task_review_state',1787286317,'98e05968cfd1c710b97f66ff5be3e649a68405b63b716bfe27803ec401d51b3d',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.014_task_lifecycle',1787286317,'d58d3813cc105cf19305b594fa8a6d810f0bfcb14899cadda9e238483b17ce81',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.015_interaction_reviews',1787286317,'327d7f16511353e8d978d3c04f1bb7c54c604bd87256f6fce2c305f204dc0dd1',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.016_task_linear_observations',1787286317,'98583c55cedd2dee56a716c9eb81840e5bac487e9aad7a0690256a748c04ae02',NULL,NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.017_migration_provenance',1787286317,'af0a8c473bc34eee74fffcea6c7d4a5c3ba20bcb822924d3cfc68c8fc9b6e4d9','bac372f031c01dc844a91f4327aa0a280c1194b2375b3fb89af0d2e20542736b',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.018_session_body_provenance',1787286317,'94d09da3a31719cc9440886f9552bcf1bc4aef8d7ebe2224655f97e0064fc2e9','1dff763e05725066b54aa833e882340b0372b2877a08a217be7e4359e1cc7e69',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.019_task_pr_github_observation',1787286317,'e27f78e6410a246a97bc1980abad22de3137033fea4bcdd79556fa316de79fbe','48fbfbafd786ea94f5c3ace45fd8b30812bee4f643625b19a140b7db404f5f63',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.020_task_pr_linear_linkage',1787286318,'f81741ebdd59805cc4bff9306aaaf1ca5fd252937bf53dd571a8216d78950d2e','fcac71860d8b5b46ba2a53c0df67f9e4a889fb767c7210c752831927b84042d0',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.021_provider_deliveries',1787286318,'9b64cd09ab8d0f30b848123ae05221d5013b585f057c03b4d09398fcff905bbc','db37b2910f412da542e11750dc75f5f24df0a894573026460e7dd5896c8a2ef3',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.022_task_session_successors',1787286318,'8e8b55313d5a593f67c28a4ca02778cd3095999264dbe2358b43475e584e5415','2a1c8d1dd6f692aa4797d2065f2e0a600a0a640ed71e4e49f63cfa8539a19b81',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.023_capture_pruned_state',1787286318,'0d761406051205ed93a571edeb3a5f9b33f539d83e41c135fda20a75cad6af49','3dbbbac3911af4c1600f3a97f5d6d4b6da5a3c866beeda7924439fec14436493',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.024_ci_incidents',1787286318,'a4a7a61dbec70e00b833c21ebccd7739b3e5959df6c3a00c6f3080fd88e75be5','8d063360dbb79744859db5390035adefc919f50e88f836ca821fa3f4b71697a6',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.025_usage_deltas',1787286318,'b820cc8ceb13b302ea98763e75579c28e0ecf6e798d0e64cff7a7f06a15f159c','1040a777f4cc49496db64f0b72075f1787c4cab7601cd0c2991039a71a6184e3',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.026_lineage_boundary',1787286318,'3df84b280122f233a8af4cb2597a8af39a55acf241ae59e587b543670f312b62','02a1d373e4254ca4693a75e69b84a837c873866adbfd858b7bd1525960fc6c80',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.027_accounts_first',1787286318,'bfee9f726a92fbd8017eb323280d29ff1928ae7fa0a0956d87b154d851acbb98','a2fc02a37b2a385c4250cafa693a43b8fc685b36b9c1968e22c82c9777f33deb',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.029_ci_incident_repaired_head',1787286318,'36a038bfdc5bd9e50e91c8ca69ec44979eef3d52c9ce48dbbd1b2a6880454a3c','f8c18fab6f0128c35052a3c1c6f053bb671bc24953da3f9f5d9461340f285106',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.030_one_spend_grain',1787286318,'12e094503f39099d8f81a1c27efe5f724327590aa0c76e7b9bbe003ecef74012','21235e3f1b4e6ab82262e92f75fdfa427f162d88cf709de3468cddc922e11b05',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.031_durable_input_spine',1787286318,'9956af1ff3a3b4f9d04563ec044520fd222deb262ea7e97194a9f2d029924f99','c4062a7a17b0d161ad73b9bde6324d1b788906093c134154ea1019f8ab45dcc8',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.032_run_launch_attention',1787286318,'8e94f7ed1f2e0b2054af0b77747e22ab5b702b6321a03b43f4091558c8657c8f','3b30adb2d3c6df65891658a9b82aeae72e242883ba5dfb340d2cf92e6e754293',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.033_launch_attention_only',1787286318,'59d37770becdce0fc3b25826e17a11296b6ab5c83887258680f32bd2632c23db','6606cfd401d44885cc5319febd8edcf9ecf6bd665587541402e3b847767381c2',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.034_typed_ci_runs',1787286318,'a0ef365a7ee84a85edc5b2d0ea12bbd54ee2674288c7a827e4ea3c35b4dec0ce','e3d2440efce6752c21fc2b680066474e7acaa3bb8c043b2d8fa23004ea946f81',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.035_drop_child_commands',1787286318,'bde7048bafb3b36df7cb7620ad45c810d98bb9c44eb6e59893fbda2934b42e71','96626694ec7730811cd8a067a0e89e9abc52d260a8d2fa90fe3a9cb504523c33',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.036_delete_sessions',1787286318,'b9815039b4fc689302b60274a2b96797c42f9c8fd5e8b14d0157776dcc845794','83ef9afa6ffd8bd670dcf07f2763a60444c5419233130c77fc333c618da4269c',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.11.037_capture_terminal_states',1787286318,'7359e9056cf0ef89ee00e0f5b41a759123512ba5e90fb52c173ff3ee773a0f38','f169a9c3cf1f9a9beaf2b059eae2197b7be95842caed6701bdd26dc7d5a8b316',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.12.2.001_release',1787286318,'f4d1f5fa9d5f7ff04e1c6a9eb9039356abc1033a365751470e1a98b790d9591e','bb9aa8fb9e7933ffa69c809ef8273c888357a525dab08e4d611dc4cfe3ff8b90',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.12.3.001_release',1787286318,'672208c7877b34efcb4340ed4ea3b7c9847889177184956f950c6c15791dd8b1','152d7a0551404e75a6ae9c30ea3e599f8351b0a9402c7364a5e0a05c151477f3',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.12.4.001_release',1787286318,'6aa0076adfd1f115c8a473fd0403ede22d97d59d03a14bf234fad8170669a999','8d3f997b281a0a8a4d881e846d2e453eab353afb9220c7f6ed1a5b877c605b5c',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.12.5.001_release',1787286318,'0f72005e982144f6530604de155df6a048529c198bae13af8b195afe1c05ddcc','5e7ec5a1094431e7902a0ccb5683577b4594fbff0c77ef9c9ef71fd51d87da5d',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.12.7.001_release',1787286318,'5070c000d46bd81c66a28479734b826ec47da5e217c2ff6bd8a6b91229fc2eeb','ac692f7ff116cc2b85a336193ac583b61e71cc39f2af09e36486bf66b4dbd2ff',NULL,NULL,NULL,NULL);
INSERT INTO schema_migrations VALUES('0.12.8.001_release',1787286318,'12a7a9811d37fd2eb36519a7ec7307b79be9ad1038aedcd0c0b5a2bee923af25','603aeace9de39d1022365975050a89f1b29a13ea3226ebb6f4d37d2b90b7339d',NULL,NULL,NULL,NULL);
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
CREATE TABLE run_events (
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    parent_process_id TEXT,
    seq INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    repo TEXT,
    worktree TEXT,
    wave TEXT,
    node TEXT NOT NULL CHECK (node IN ('run', 'flow', 'skill')),
    event TEXT NOT NULL CHECK (event IN ('started', 'completed', 'errored', 'escalated')),
    command TEXT,
    flow TEXT,
    skill TEXT,
    step_index INTEGER,
    error TEXT);
INSERT INTO run_events VALUES('run-1','proc-lookup',NULL,0,1787286199,'__LF_HOME__/repo',NULL,'product','run','started','["lf","pm","sync"]',NULL,NULL,NULL,NULL);
INSERT INTO run_events VALUES('run-1','proc-lookup',NULL,1,1787286259,'__LF_HOME__/repo',NULL,'product','run','completed','["lf","pm","sync"]',NULL,NULL,NULL,NULL);
INSERT INTO run_events VALUES('run-flow','proc-flow',NULL,1,1787286269,'__LF_HOME__/repo',NULL,'product','flow','started','["lf","build"]','build',NULL,NULL,NULL);
INSERT INTO run_events VALUES('run-flow','proc-flow',NULL,2,1787286279,'__LF_HOME__/repo',NULL,'product','flow','completed','["lf","build"]','build',NULL,NULL,NULL);
INSERT INTO run_events VALUES('run-resident','proc-resident',NULL,0,1787286289,'__LF_HOME__/repo',NULL,'product','run','started','["lf","__resident"]',NULL,NULL,NULL,NULL);
INSERT INTO run_events VALUES('run-resident','proc-resident',NULL,1,1787286299,'__LF_HOME__/repo',NULL,'product','run','completed','["lf","__resident"]',NULL,NULL,NULL,NULL);
CREATE TABLE blob_tokens (
    sha TEXT PRIMARY KEY,
    lines INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    tokens INTEGER NOT NULL
);
CREATE TABLE context_assets (
    turn_id TEXT NOT NULL REFERENCES agent_turns(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    channel TEXT NOT NULL CHECK (channel IN ('system', 'task')),
    kind TEXT NOT NULL CHECK (kind IN (
        'operating_instructions', 'surface_instructions',
        'provider_instructions', 'repo_instructions', 'skill_instructions',
        'direction', 'goal', 'memory', 'chat', 'summary', 'document',
        'scratch', 'diff', 'clipboard', 'user_message', 'assembly'
    )),
    scope TEXT NOT NULL CHECK (scope IN (
        'global', 'provider', 'repo', 'wave', 'project', 'task', 'step', 'user'
    )),
    label TEXT NOT NULL,
    source_path TEXT,
    included_by TEXT NOT NULL,
    content_sha256 TEXT NOT NULL,
    byte_start INTEGER NOT NULL,
    byte_end INTEGER NOT NULL,
    bytes INTEGER NOT NULL,
    isolated_tokens INTEGER NOT NULL,
    attributed_tokens INTEGER NOT NULL,
    PRIMARY KEY (turn_id, position)
);
CREATE TABLE context_decisions (
    turn_id TEXT NOT NULL REFERENCES agent_turns(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN (
        'operating_instructions', 'surface_instructions',
        'provider_instructions', 'repo_instructions', 'skill_instructions',
        'direction', 'goal', 'memory', 'chat', 'summary', 'document',
        'scratch', 'diff', 'clipboard', 'user_message', 'assembly'
    )),
    scope TEXT NOT NULL CHECK (scope IN (
        'global', 'provider', 'repo', 'wave', 'project', 'task', 'step', 'user'
    )),
    label TEXT NOT NULL,
    source_path TEXT,
    decision TEXT NOT NULL CHECK (decision IN (
        'included', 'excluded', 'summarized', 'stat_only', 'truncated', 'deduplicated'
    )),
    reason TEXT NOT NULL,
    original_bytes INTEGER,
    original_tokens INTEGER,
    asset_position INTEGER,
    PRIMARY KEY (turn_id, position),
    FOREIGN KEY (turn_id, asset_position)
        REFERENCES context_assets(turn_id, position)
);
CREATE TABLE observation_outbox (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    recipient_kind TEXT NOT NULL CHECK (recipient_kind IN ('wave', 'project')),
    recipient_id TEXT NOT NULL,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('project', 'task')),
    source_id TEXT NOT NULL,
    event_id INTEGER NOT NULL,
    payload_json TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    delivered_at INTEGER,
    UNIQUE(recipient_kind, recipient_id, source_kind, source_id, event_id)
);
CREATE TABLE IF NOT EXISTS "provider_accounts" (
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    home TEXT,
    login_email TEXT,
    credential_state TEXT NOT NULL CHECK (
        credential_state IN ('connected', 'missing')
    ),
    routing_state TEXT NOT NULL CHECK (
        routing_state IN ('automatic', 'explicit_only', 'disabled')
    ),
    plan TEXT,
    paid_through INTEGER,
    utilization_percent INTEGER CHECK (
        utilization_percent IS NULL OR
        utilization_percent BETWEEN 0 AND 100
    ),
    cooldown_until INTEGER,
    cooldown_reason TEXT,
    last_selected_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (provider, account_id)
);
CREATE TABLE provider_deliveries (
    delivery_id   TEXT    NOT NULL,
    provider      TEXT    NOT NULL CHECK (provider IN ('linear', 'github')),
    -- "issue_edit" | "comment" | "ignored" | null (null only for unknown providers)
    event_kind    TEXT,
    -- "task_session" | null (null when no target Session resolved)
    target_kind   TEXT,
    target_id     TEXT,
    status        TEXT    NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending','processed','ignored','no_target','error')),
    -- JSON summary of the processing outcome, for ops inspection.
    outcome       TEXT,
    -- Unix milliseconds.
    received_at   INTEGER NOT NULL,
    processed_at  INTEGER,
    PRIMARY KEY (delivery_id, provider)
);
CREATE TABLE provider_account_limits (
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    window TEXT NOT NULL,
    used_percent INTEGER NOT NULL,
    resets_at INTEGER,
    plan TEXT,
    observed_at INTEGER NOT NULL,
    source TEXT NOT NULL CHECK (source IN ('stream', 'poll')),
    PRIMARY KEY (provider, account_id, window)
);
CREATE TABLE access_profiles (
    profile_id TEXT PRIMARY KEY,
    chrome_directory TEXT NOT NULL UNIQUE,
    expected_login TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE TABLE account_access_profiles (
    provider TEXT NOT NULL,
    account_id TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    profile_id TEXT NOT NULL,
    PRIMARY KEY (provider, account_id, position),
    UNIQUE (provider, account_id, profile_id),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id) ON DELETE CASCADE,
    FOREIGN KEY (profile_id)
        REFERENCES access_profiles(profile_id) ON DELETE RESTRICT
);
CREATE TABLE provider_routes (
    scope TEXT NOT NULL CHECK (scope IN ('repo', 'default')),
    scope_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    account_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (scope, scope_id, provider, position),
    UNIQUE (scope, scope_id, provider, account_id),
    CHECK (
        (scope = 'default' AND scope_id = '')
        OR (scope = 'repo' AND scope_id <> '')
    ),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id) ON DELETE RESTRICT
);
CREATE TABLE IF NOT EXISTS "provider_session_accounts" (
    provider TEXT NOT NULL,
    provider_session_id TEXT NOT NULL,
    account_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (provider, provider_session_id),
    FOREIGN KEY (provider, account_id)
        REFERENCES provider_accounts(provider, account_id) ON DELETE CASCADE
);
CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    wave_id TEXT NOT NULL REFERENCES waves(id) ON DELETE RESTRICT,
    external_project_id TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
, project_slug TEXT, project_name TEXT, project_prompt_context TEXT, pm_snapshot_synced_at INTEGER, iteration INTEGER, observation_cursor INTEGER, last_state_fingerprint TEXT, agent TEXT, provider TEXT, provider_session_id TEXT, abandon_requested_at INTEGER, abandon_reason TEXT, updated_at INTEGER);
INSERT INTO projects VALUES('proj_e972b70272fbb5e91c096ebe657f9f9b','9f599e30-8faa-4088-b2fc-d8d66ef90c4c','f56c583c-c360-4dc4-ba12-4b5a02268623',1787286319,'technical-architecture','Technical Architecture','Keep the system legible and minimally simple.',1787286318,1,0,NULL,'codex','codex',NULL,NULL,NULL,1787286319);
INSERT INTO projects VALUES('proj_4cef8988f7e2489e8cedd6d1ef8c3991','9f599e30-8faa-4088-b2fc-d8d66ef90c4c','95159066-9098-4d0b-8903-01459dc7ec14',1787286319,'auditability','Auditability','Every claim points to its receipt.',1787286319,1,0,NULL,'codex','codex',NULL,NULL,NULL,1787286319);
CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    external_issue_id TEXT NOT NULL UNIQUE,
    issue_identifier TEXT NOT NULL,
    created_at INTEGER NOT NULL
, issue_title TEXT, issue_description TEXT, pm_snapshot_synced_at INTEGER, pm_writeback_json TEXT, worktree TEXT, workspace_slug TEXT, agent TEXT, provider TEXT, provider_session_id TEXT, abandon_requested_at INTEGER, abandon_reason TEXT, iterate_flow TEXT, phase_cursor INTEGER, phase_iteration INTEGER, kickoff_flow TEXT, gate_flow TEXT, lifecycle_phase TEXT, phase_epoch INTEGER, gate_cycle INTEGER, gate_proposal_json TEXT, updated_at INTEGER);
INSERT INTO tasks VALUES('task_40fbeeaadfbca5367aa7391432ae84ff','proj_4cef8988f7e2489e8cedd6d1ef8c3991','task-prd-52','PRD-52',1787286319,'Expose one fleet snapshot from Wave to raw trace','This Task outlived its retired Linear Project.',1787286318,'{"state":"current"}','__LF_HOME__/repo.w2-127','w2-127','codex','codex',NULL,NULL,NULL,'slice',0,0,'task-design','ship','iterate',1,0,NULL,1787286319);
CREATE TABLE epochs (
    id TEXT PRIMARY KEY,
    number INTEGER NOT NULL CHECK (number > 0),
    wave_id TEXT REFERENCES waves(id) ON DELETE RESTRICT,
    project_id TEXT REFERENCES projects(id) ON DELETE RESTRICT,
    task_id TEXT REFERENCES tasks(id) ON DELETE RESTRICT,
    state TEXT NOT NULL CHECK (state IN ('open', 'done', 'abandoned')),
    current_rev INTEGER NOT NULL CHECK (current_rev >= 0),
    created_at INTEGER NOT NULL,
    terminal_at INTEGER,
    CHECK (
        (wave_id IS NOT NULL) +
        (project_id IS NOT NULL) +
        (task_id IS NOT NULL) = 1
    ),
    CHECK ((state = 'open') = (terminal_at IS NULL)),
    UNIQUE (wave_id, number),
    UNIQUE (project_id, number),
    UNIQUE (task_id, number)
);
INSERT INTO epochs VALUES('epoch_322f8f43d01d478c834dc7a44be6f2b3',1,'9f599e30-8faa-4088-b2fc-d8d66ef90c4c',NULL,NULL,'open',0,1787286319,NULL);
INSERT INTO epochs VALUES('epoch_ce8667a0f09e481dab15e22cae7a7d55',1,NULL,'proj_e972b70272fbb5e91c096ebe657f9f9b',NULL,'abandoned',0,1787286319,1787286319);
INSERT INTO epochs VALUES('epoch_a5f61d3c5ca54f7da239f5588e9eaf34',1,NULL,NULL,'task_40fbeeaadfbca5367aa7391432ae84ff','open',0,1787286319,NULL);
INSERT INTO epochs VALUES('epoch_a1738267f5c9457e8b8a016400f878e7',1,NULL,'proj_4cef8988f7e2489e8cedd6d1ef8c3991',NULL,'open',0,1787286319,NULL);
CREATE TABLE epoch_revisions (
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE CASCADE,
    rev INTEGER NOT NULL CHECK (rev >= 0),
    kind TEXT NOT NULL CHECK (kind IN (
        'truth', 'steer', 'tool_response', 'evidence'
    )),
    source_id TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (epoch_id, rev),
    UNIQUE (kind, source_id)
);
INSERT INTO epoch_revisions VALUES('epoch_322f8f43d01d478c834dc7a44be6f2b3',0,'truth','truth:epoch_322f8f43d01d478c834dc7a44be6f2b3:0',1787286319);
INSERT INTO epoch_revisions VALUES('epoch_ce8667a0f09e481dab15e22cae7a7d55',0,'truth','truth:epoch_ce8667a0f09e481dab15e22cae7a7d55:0',1787286319);
INSERT INTO epoch_revisions VALUES('epoch_a5f61d3c5ca54f7da239f5588e9eaf34',0,'truth','truth:epoch_a5f61d3c5ca54f7da239f5588e9eaf34:0',1787286319);
INSERT INTO epoch_revisions VALUES('epoch_a1738267f5c9457e8b8a016400f878e7',0,'truth','truth:epoch_a1738267f5c9457e8b8a016400f878e7:0',1787286319);
CREATE TABLE work_truth (
    epoch_id TEXT NOT NULL,
    rev INTEGER NOT NULL,
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
    created_at INTEGER NOT NULL,
    PRIMARY KEY (epoch_id, rev),
    FOREIGN KEY (epoch_id, rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE CASCADE
);
INSERT INTO work_truth VALUES('epoch_322f8f43d01d478c834dc7a44be6f2b3',0,'{"name":"product","repo":"__LF_HOME__/repo"}',1787286319);
INSERT INTO work_truth VALUES('epoch_ce8667a0f09e481dab15e22cae7a7d55',0,'{"external_project_id":"f56c583c-c360-4dc4-ba12-4b5a02268623","name":"Technical Architecture","pm_snapshot_synced_at":1787286318,"prompt_context":"Keep the system legible and minimally simple.","slug":"technical-architecture"}',1787286319);
INSERT INTO work_truth VALUES('epoch_a5f61d3c5ca54f7da239f5588e9eaf34',0,'{"description":"This Task outlived its retired Linear Project.","external_issue_id":"linear-task-w2-127","identifier":"W2-127","pm_snapshot_synced_at":1787286318,"title":"Preserve historical architecture evidence"}',1787286319);
INSERT INTO work_truth VALUES('epoch_a1738267f5c9457e8b8a016400f878e7',0,'{"external_project_id":"95159066-9098-4d0b-8903-01459dc7ec14","name":"Auditability","pm_snapshot_synced_at":1787286319,"prompt_context":"Every claim points to its receipt.","slug":"auditability"}',1787286319);
CREATE TABLE homes (
    id TEXT PRIMARY KEY,
    route TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    observed_at INTEGER NOT NULL
);
INSERT INTO homes VALUES('home_4a5da57ebe9ae0e50bb66a9ce23a818f','local',1787286318,1787286318);
CREATE TABLE runs (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    state TEXT NOT NULL CHECK (state IN ('reserved', 'active', 'stopping', 'ended')),
    trigger_json TEXT NOT NULL CHECK (json_valid(trigger_json)),
    retry_of TEXT REFERENCES runs(id) ON DELETE RESTRICT,
    lease_hash TEXT,
    lease_generation INTEGER,
    source_kind TEXT NOT NULL CHECK (source_kind IN ('wave', 'project', 'task', 'migration')),
    source_id TEXT,
    created_at INTEGER NOT NULL,
    ended_at INTEGER,
    stop_reason TEXT, containment_kind TEXT CHECK (
    containment_kind IN ('process_group', 'tmux')
), containment_id TEXT, cwd TEXT, started_at INTEGER, runtime_generation INTEGER, first_material_at INTEGER,
    CHECK ((state = 'ended') = (ended_at IS NOT NULL)),
    CHECK (state = 'ended' OR lease_hash IS NOT NULL)
);
INSERT INTO runs VALUES('run_109ff8339e454976a74b3d7c3ebe0977','epoch_322f8f43d01d478c834dc7a44be6f2b3','home_4a5da57ebe9ae0e50bb66a9ce23a818f','ended','{"kind":"user"}',NULL,'577d0738fb4ab84a9b966bcb1f9878b6446bbdcb07a86e94d2031f7f9b7651b3',NULL,'wave','9f599e30-8faa-4088-b2fc-d8d66ef90c4c',1787286319,1787286319,'{"kind":"requested"}','process_group','1','__LF_HOME__/repo',1787286319,NULL,NULL);
INSERT INTO runs VALUES('run_052a9ce9c4ed45838d35e315ce26fece','epoch_a1738267f5c9457e8b8a016400f878e7','home_4a5da57ebe9ae0e50bb66a9ce23a818f','active','{"kind":"user"}',NULL,'279002846a3eed9913356fcb8b585ec27a8e5674d26ad3db57328197b0083f37',NULL,'project','proj_4cef8988f7e2489e8cedd6d1ef8c3991',1787286319,NULL,NULL,'tmux','missing-current-project','__LF_HOME__/repo',1787286319,NULL,NULL);
CREATE TABLE waits (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    on_json TEXT NOT NULL CHECK (json_valid(on_json)),
    created_at INTEGER NOT NULL,
    resolved_at INTEGER
);
CREATE TABLE steers (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL,
    rev INTEGER NOT NULL,
    author_kind TEXT NOT NULL CHECK (author_kind IN ('user', 'run')),
    author_run_id TEXT REFERENCES runs(id) ON DELETE RESTRICT,
    text TEXT NOT NULL CHECK (length(trim(text)) > 0),
    issued_at INTEGER NOT NULL,
    CHECK ((author_kind = 'user') = (author_run_id IS NULL)),
    UNIQUE (epoch_id, rev),
    FOREIGN KEY (epoch_id, rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE CASCADE
);
CREATE TABLE tool_responses (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL,
    rev INTEGER NOT NULL,
    request_id TEXT NOT NULL,
    choice TEXT NOT NULL CHECK (length(trim(choice)) > 0),
    responded_at INTEGER NOT NULL,
    UNIQUE (epoch_id, request_id),
    UNIQUE (epoch_id, rev),
    FOREIGN KEY (epoch_id, rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE CASCADE
);
CREATE TABLE sends (
    id TEXT PRIMARY KEY,
    steer_id TEXT NOT NULL REFERENCES steers(id) ON DELETE CASCADE,
    turn_id TEXT NOT NULL REFERENCES agent_turns(id) ON DELETE CASCADE,
    via TEXT NOT NULL CHECK (via IN ('live', 'seed')),
    state TEXT NOT NULL CHECK (state IN ('sending', 'sent', 'failed', 'unknown')),
    provider_turn_id TEXT,
    reason TEXT,
    attempted_at INTEGER NOT NULL,
    finished_at INTEGER,
    CHECK ((state = 'sending') = (finished_at IS NULL)),
    UNIQUE (steer_id, turn_id, via)
);
CREATE TABLE work_flow_positions (
    epoch_id TEXT PRIMARY KEY REFERENCES epochs(id) ON DELETE CASCADE,
    flow TEXT NOT NULL CHECK (length(trim(flow)) > 0),
    step TEXT NOT NULL CHECK (length(trim(step)) > 0),
    step_index INTEGER NOT NULL CHECK (step_index >= 0),
    iteration INTEGER NOT NULL CHECK (iteration >= 0),
    updated_at INTEGER NOT NULL
, node_id TEXT, human INTEGER NOT NULL DEFAULT 0
    CHECK (
        human IN (0, 1)
        AND (human = 0 OR (node_id IS NOT NULL AND length(trim(node_id)) > 0))
    ));
CREATE TABLE done_proposals (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    basis_rev INTEGER NOT NULL,
    proposed_at INTEGER NOT NULL,
    UNIQUE (run_id, epoch_id, basis_rev),
    FOREIGN KEY (epoch_id, basis_rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE RESTRICT
);
CREATE TABLE work_placements (
    wave_id TEXT REFERENCES waves(id) ON DELETE CASCADE,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    placed_at INTEGER NOT NULL, enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    CHECK (
        (wave_id IS NOT NULL) +
        (project_id IS NOT NULL) +
        (task_id IS NOT NULL) = 1
    ),
    UNIQUE (wave_id),
    UNIQUE (project_id),
    UNIQUE (task_id)
);
INSERT INTO work_placements VALUES('9f599e30-8faa-4088-b2fc-d8d66ef90c4c',NULL,NULL,'home_4a5da57ebe9ae0e50bb66a9ce23a818f',1787286319,1);
INSERT INTO work_placements VALUES(NULL,'proj_e972b70272fbb5e91c096ebe657f9f9b',NULL,'home_4a5da57ebe9ae0e50bb66a9ce23a818f',1787286319,1);
INSERT INTO work_placements VALUES(NULL,NULL,'task_40fbeeaadfbca5367aa7391432ae84ff','home_4a5da57ebe9ae0e50bb66a9ce23a818f',1787286319,1);
INSERT INTO work_placements VALUES(NULL,'proj_4cef8988f7e2489e8cedd6d1ef8c3991',NULL,'home_4a5da57ebe9ae0e50bb66a9ce23a818f',1787286319,1);
CREATE TABLE IF NOT EXISTS "ci_incidents" (
    identity TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    pr_id TEXT NOT NULL REFERENCES task_prs(id) ON DELETE CASCADE,
    repo TEXT NOT NULL,
    pr_number INTEGER NOT NULL CHECK (pr_number > 0),
    failed_head_sha TEXT NOT NULL,
    failure_set_json TEXT NOT NULL,
    provider_completed_at INTEGER,
    poll_observed_at INTEGER,
    webhook_received_at INTEGER,
    claimed_run_id TEXT REFERENCES runs(id) ON DELETE RESTRICT,
    responded_at INTEGER,
    green_at INTEGER,
    merged_at INTEGER,
    blocked_at INTEGER,
    blocked_reason TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    repaired_head_sha TEXT,
    CHECK (poll_observed_at IS NOT NULL OR webhook_received_at IS NOT NULL),
    CHECK ((blocked_at IS NULL) = (blocked_reason IS NULL))
);
CREATE TABLE IF NOT EXISTS "task_linear_observations" (
    task_id TEXT PRIMARY KEY REFERENCES tasks(id) ON DELETE CASCADE,
    last_revision TEXT NOT NULL,
    last_title TEXT NOT NULL,
    last_description TEXT NOT NULL,
    last_success_at INTEGER NOT NULL,
    degraded_reason TEXT,
    updated_at INTEGER NOT NULL
);
INSERT INTO task_linear_observations VALUES('task_40fbeeaadfbca5367aa7391432ae84ff','','Preserve historical architecture evidence','This Task outlived its retired Linear Project.',1787286319,NULL,1787286319);
CREATE TABLE IF NOT EXISTS "task_linear_ingested_comments" (
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
CREATE TABLE IF NOT EXISTS "agent_invocations" (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    process_id TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    repo TEXT NOT NULL,
    worktree TEXT NOT NULL,
    wave TEXT,
    flow TEXT,
    skill TEXT,
    provider TEXT NOT NULL,
    model TEXT,
    surface TEXT NOT NULL,
    capture_status TEXT NOT NULL CHECK (
        capture_status IN (
            'capturing', 'complete', 'partial', 'prompt_only',
            'pruned', 'interrupted', 'lost'
        )
    ),
    incomplete_reason TEXT,
    outcome TEXT NOT NULL CHECK (
        outcome IN ('running', 'completed', 'failed', 'interrupted')
    ),
    artifact_dir TEXT NOT NULL,
    conversation_path TEXT NOT NULL,
    provider_events_path TEXT,
    provider_session_id TEXT,
    provider_session_path TEXT,
    conversation_event_count INTEGER NOT NULL,
    conversation_bytes INTEGER NOT NULL,
    project TEXT,
    task TEXT,
    supervising_run_id TEXT REFERENCES runs(id) ON DELETE RESTRICT,
    account_id TEXT,
    resume_token TEXT,
    handback_state TEXT CHECK (
        handback_state IN ('succeeded', 'failed', 'interrupted', 'unknown')
    )
, answer_ask_id TEXT
    REFERENCES ask_exchanges(id), ask_ready_at INTEGER, ask_presented_at INTEGER);
INSERT INTO agent_invocations VALUES('invocation_00000000000000000000000000000001','run-resident','proc-resident',1787286289,1787286299,'__LF_HOME__/repo','__LF_HOME__/repo','product','wave','wave/mutate','codex','gpt-5','headless','complete',NULL,'completed','traces/invocation-wave-mutate','traces/invocation-wave-mutate/conversation.jsonl',NULL,NULL,NULL,2,10,'auditability','W2-122','run_109ff8339e454976a74b3d7c3ebe0977',NULL,NULL,NULL,NULL,NULL,NULL);
CREATE TABLE IF NOT EXISTS "task_prs" (
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
    parent_pr_id TEXT REFERENCES "task_prs"(id),
    github_observation TEXT,
    linear_attachment_id TEXT,
    linear_comment_id TEXT,
    linear_link_error TEXT,
    merge_mode TEXT CHECK (merge_mode IN ('user', 'auto')),
    merge_requested_at INTEGER,
    merge_head_sha TEXT CHECK (merge_head_sha IS NULL OR length(trim(merge_head_sha)) > 0), merged_at INTEGER, merge_tracking_complete INTEGER NOT NULL DEFAULT 0
CHECK (merge_tracking_complete IN (0, 1)), repair_tracking_complete INTEGER NOT NULL DEFAULT 0
CHECK (repair_tracking_complete IN (0, 1)),
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
INSERT INTO task_prs VALUES('pr_30997b561ee443268a213758fdfad9ee','task_40fbeeaadfbca5367aa7391432ae84ff',1,'w2-127','jack-heart/w2-127','deadbeef',1787286319,'continue_task',NULL,240,'https://github.com/loopflowstudio/loopflow/pull/240',NULL,NULL,1787286319,1787286319,'head-240',NULL,NULL,NULL,NULL,NULL,NULL,'user',1787286319,'head-240',NULL,1,1);
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
CREATE TABLE home_runtime_generations (
    home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    generation INTEGER NOT NULL CHECK (generation > 0),
    build_version TEXT NOT NULL,
    source_revision TEXT NOT NULL,
    migration_frontier TEXT NOT NULL,
    activated_at INTEGER NOT NULL,
    PRIMARY KEY (home_id, generation)
);
CREATE TABLE home_upgrades (
    id TEXT PRIMARY KEY,
    home_id TEXT REFERENCES homes(id) ON DELETE RESTRICT,
    source_revision TEXT NOT NULL,
    source_identity TEXT NOT NULL,
    migration_authority TEXT NOT NULL
        CHECK (migration_authority IN ('published', 'validation_only')),
    package_version TEXT NOT NULL,
    build_version TEXT,
    latest_known_migration TEXT NOT NULL,
    prior_generation INTEGER NOT NULL CHECK (prior_generation >= 0),
    target_generation INTEGER NOT NULL CHECK (target_generation > prior_generation),
    phase TEXT NOT NULL CHECK (phase IN (
        'planned', 'draining', 'drained', 'migrating', 'restarting',
        'reconciling', 'completed', 'failed', 'rolled_back'
    )),
    keeper_mode TEXT NOT NULL CHECK (keeper_mode IN ('none', 'launchd', 'systemd')),
    cli_binary TEXT,
    cli_target TEXT,
    daemon_binary TEXT,
    daemon_target TEXT,
    app_source TEXT,
    app_target TEXT,
    app_superseded TEXT,
    legacy_app_target TEXT,
    migration_required INTEGER NOT NULL CHECK (migration_required IN (0, 1)),
    started_at INTEGER NOT NULL,
    completed_at INTEGER,
    artifacts_activated INTEGER NOT NULL CHECK (artifacts_activated IN (0, 1)),
    migration_applied INTEGER NOT NULL CHECK (migration_applied IN (0, 1)),
    daemon_restarted INTEGER NOT NULL CHECK (daemon_restarted IN (0, 1)),
    drain_timed_out INTEGER NOT NULL CHECK (drain_timed_out IN (0, 1)),
    coordinator_started_at INTEGER NOT NULL,
    recovery_pid INTEGER,
    error TEXT,
    CHECK (
        (cli_binary IS NULL AND cli_target IS NULL
            AND daemon_binary IS NULL AND daemon_target IS NULL)
        OR
        (cli_binary IS NOT NULL AND cli_target IS NOT NULL
            AND daemon_binary IS NOT NULL AND daemon_target IS NOT NULL)
    )
);
CREATE TABLE home_upgrade_work (
    upgrade_id TEXT NOT NULL REFERENCES home_upgrades(id) ON DELETE CASCADE,
    work_kind TEXT NOT NULL CHECK (work_kind IN ('wave', 'project', 'task')),
    work_id TEXT NOT NULL,
    enabled_before INTEGER NOT NULL CHECK (enabled_before IN (0, 1)),
    prior_run_id TEXT,
    resumed_run_id TEXT,
    containment_kind TEXT CHECK (containment_kind IN ('tmux', 'process_group')),
    containment_id TEXT,
    containment_observation TEXT NOT NULL
        CHECK (containment_observation IN ('absent', 'present', 'unprovable')),
    drain TEXT NOT NULL
        CHECK (drain IN ('pending', 'durable_only', 'interrupted', 'forced', 'failed')),
    reconciliation TEXT NOT NULL
        CHECK (reconciliation IN ('pending', 'resumed', 'skipped', 'failed')),
    error TEXT,
    PRIMARY KEY (upgrade_id, work_kind, work_id),
    CHECK ((containment_kind IS NULL) = (containment_id IS NULL))
);
CREATE TABLE IF NOT EXISTS "waves" (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    repo TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    parent_wave_id TEXT REFERENCES "waves"(id) ON DELETE CASCADE,
    promoted_at INTEGER,
    UNIQUE (repo, name)
);
INSERT INTO waves VALUES('9f599e30-8faa-4088-b2fc-d8d66ef90c4c','product','__LF_HOME__/repo',1787286319,NULL,NULL);
CREATE TABLE IF NOT EXISTS "pm_snapshots" (
    wave_id TEXT NOT NULL PRIMARY KEY REFERENCES waves(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    initiative TEXT NOT NULL,
    synced_at INTEGER NOT NULL,
    payload TEXT NOT NULL
);
INSERT INTO pm_snapshots VALUES('9f599e30-8faa-4088-b2fc-d8d66ef90c4c','linear','initiative-product',1787286319,'{"items":[{"assignee":null,"completed":false,"description":"Keep focused reads useful through stale Work.","id":"task-prd-52","identifier":"PRD-52","name":"Expose one fleet snapshot from Wave to raw trace","project":"auditability","project_id":"95159066-9098-4d0b-8903-01459dc7ec14","rank":1,"team_id":"team-product","url":"https://linear.app/loopflow/issue/PRD-52"}],"projects":[{"definition":"Every product surface shows enough truth to trust the system.","flows":{"finally":null,"first":null,"loop":null},"id":"95159066-9098-4d0b-8903-01459dc7ec14","initiative_ids":["initiative-product"],"krs":[{"holds":false,"text":"Every visible state carries its reason"}],"name":"Auditability","slug":"auditability","summary":"Every claim points to its receipt.","team_ids":["team-product"]}]}');
CREATE TABLE performance_evidence_authority (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    started_at INTEGER NOT NULL
);
INSERT INTO performance_evidence_authority VALUES(1,1787286318);
CREATE TABLE task_pr_repair_incidents (
    task_pr_id TEXT NOT NULL REFERENCES task_prs(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (
        kind IN ('avoidable_rebase_agent', 'manual_git_repair')
    ),
    occurred_at INTEGER NOT NULL,
    PRIMARY KEY (task_pr_id, kind)
);
CREATE TABLE IF NOT EXISTS "agent_turns" (
    id TEXT PRIMARY KEY,
    invocation_id TEXT NOT NULL REFERENCES agent_invocations(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    provider_turn_id TEXT,
    started_at INTEGER NOT NULL,
    ended_at INTEGER,
    status TEXT NOT NULL CHECK (
        status IN ('running', 'completed', 'failed', 'interrupted', 'partial')
    ),
    input_op TEXT NOT NULL CHECK (
        input_op IN ('initial', 'message', 'steer', 'queued')
    ),
    context_coverage TEXT NOT NULL CHECK (
        context_coverage IN ('assembled', 'provider_total_only', 'unknown')
    ),
    tokenizer TEXT NOT NULL,
    system_prompt_path TEXT,
    task_prompt_path TEXT NOT NULL,
    system_tokens INTEGER NOT NULL,
    task_tokens INTEGER NOT NULL,
    supplied_context_tokens INTEGER NOT NULL,
    context_gather_ms INTEGER NOT NULL,
    context_render_ms INTEGER NOT NULL,
    context_persist_ms INTEGER NOT NULL,
    first_event_seq INTEGER,
    last_event_seq INTEGER,
    root_output TEXT,
    epoch_id TEXT,
    basis_rev INTEGER,
    CHECK ((epoch_id IS NULL) = (basis_rev IS NULL)),
    UNIQUE (invocation_id, ordinal),
    FOREIGN KEY (epoch_id, basis_rev)
        REFERENCES epoch_revisions(epoch_id, rev) ON DELETE RESTRICT
);
INSERT INTO agent_turns VALUES('turn-wave-mutate','invocation_00000000000000000000000000000001',1,NULL,1787286289,1787286299,'completed','initial','assembled','o200k_base',NULL,'traces/invocation-wave-mutate/task.md',0,10,10,1,1,1,NULL,NULL,NULL,NULL,NULL);
CREATE TABLE turn_usage_samples (
    turn_id TEXT NOT NULL REFERENCES agent_turns(id) ON DELETE CASCADE,
    observed_at INTEGER NOT NULL,
    final_receipt INTEGER NOT NULL CHECK (final_receipt IN (0, 1)),
    input_tokens INTEGER CHECK (input_tokens IS NULL OR input_tokens >= 0),
    total_input_tokens INTEGER CHECK (
        total_input_tokens IS NULL OR total_input_tokens >= 0
    ),
    peak_input_tokens INTEGER CHECK (
        peak_input_tokens IS NULL OR peak_input_tokens >= 0
    ),
    context_window_tokens INTEGER CHECK (
        context_window_tokens IS NULL OR context_window_tokens > 0
    ),
    output_tokens INTEGER CHECK (output_tokens IS NULL OR output_tokens >= 0),
    reasoning_tokens INTEGER CHECK (
        reasoning_tokens IS NULL OR reasoning_tokens >= 0
    ),
    cache_read_tokens INTEGER CHECK (
        cache_read_tokens IS NULL OR cache_read_tokens >= 0
    ),
    cache_write_tokens INTEGER CHECK (
        cache_write_tokens IS NULL OR cache_write_tokens >= 0
    ),
    model TEXT,
    cost_usd REAL CHECK (cost_usd IS NULL OR cost_usd >= 0),
    PRIMARY KEY (turn_id, observed_at),
    CHECK (
        reasoning_tokens IS NULL OR output_tokens IS NULL
        OR reasoning_tokens <= output_tokens
    )
);
INSERT INTO turn_usage_samples VALUES('turn-wave-mutate',1787286299,1,10,10,10,100,5,NULL,0,NULL,NULL,0.0100000000000000002);
CREATE TABLE IF NOT EXISTS "ask_exchanges" (
    id TEXT PRIMARY KEY,
    epoch_id TEXT NOT NULL REFERENCES epochs(id) ON DELETE RESTRICT,
    origin_work_kind TEXT NOT NULL CHECK (
        origin_work_kind IN ('wave', 'project', 'task')
    ),
    origin_work_id TEXT NOT NULL CHECK (length(trim(origin_work_id)) > 0),
    origin_run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE RESTRICT,
    origin_turn_id TEXT REFERENCES agent_turns(id) ON DELETE RESTRICT,
    origin_invocation_id TEXT REFERENCES agent_invocations(id) ON DELETE RESTRICT,
    origin_home_id TEXT NOT NULL REFERENCES homes(id) ON DELETE RESTRICT,
    origin_cwd TEXT NOT NULL,
    target_kind TEXT NOT NULL CHECK (target_kind IN ('user', 'parent')),
    target_work_kind TEXT CHECK (
        target_work_kind IN ('wave', 'project', 'task')
    ),
    target_work_id TEXT,
    request_kind TEXT NOT NULL CHECK (request_kind IN ('intervention', 'flow_step')),
    request_prompt TEXT,
    request_flow TEXT,
    request_node_id TEXT,
    request_skill TEXT,
    request_iteration INTEGER CHECK (request_iteration >= 0),
    state TEXT NOT NULL CHECK (
        state IN ('queued', 'claimed', 'resolved', 'declined', 'cancelled')
    ),
    active_invocation_id TEXT REFERENCES agent_invocations(id) ON DELETE RESTRICT,
    result_kind TEXT CHECK (result_kind IN ('resolved', 'declined', 'cancelled')),
    result_text TEXT,
    terminal_author_kind TEXT CHECK (terminal_author_kind IN ('user', 'run')),
    terminal_author_id TEXT,
    asked_at INTEGER NOT NULL,
    terminal_at INTEGER,
    CHECK (
        (origin_turn_id IS NULL AND origin_invocation_id IS NULL)
        OR
        (origin_turn_id IS NOT NULL AND origin_invocation_id IS NOT NULL)
    ),
    CHECK (
        (target_kind = 'user'
         AND target_work_kind IS NULL AND target_work_id IS NULL)
        OR
        (target_kind = 'parent'
         AND target_work_kind IS NOT NULL AND target_work_id IS NOT NULL
         AND length(trim(target_work_id)) > 0)
    ),
    CHECK (
        (request_kind = 'intervention'
         AND request_prompt IS NOT NULL AND length(trim(request_prompt)) > 0
         AND request_flow IS NULL AND request_node_id IS NULL
         AND request_skill IS NULL AND request_iteration IS NULL)
        OR
        (request_kind = 'flow_step' AND request_prompt IS NULL
         AND request_flow IS NOT NULL AND length(trim(request_flow)) > 0
         AND request_node_id IS NOT NULL AND length(trim(request_node_id)) > 0
         AND request_skill IS NOT NULL AND length(trim(request_skill)) > 0
         AND request_iteration IS NOT NULL)
    ),
    CHECK (
        (state = 'queued' AND active_invocation_id IS NULL
         AND result_kind IS NULL AND result_text IS NULL AND terminal_at IS NULL)
        OR
        (state = 'claimed' AND active_invocation_id IS NOT NULL
         AND result_kind IS NULL AND result_text IS NULL AND terminal_at IS NULL)
        OR
        (state IN ('resolved', 'declined', 'cancelled')
         AND active_invocation_id IS NULL
         AND result_kind = state AND result_text IS NOT NULL
         AND length(trim(result_text)) > 0 AND terminal_at IS NOT NULL)
    ),
    CHECK (
        (terminal_author_kind IS NULL AND terminal_author_id IS NULL)
        OR
        (terminal_author_kind = 'user' AND terminal_author_id IS NULL)
        OR
        (terminal_author_kind = 'run' AND terminal_author_id IS NOT NULL
         AND length(trim(terminal_author_id)) > 0)
    ),
    CHECK (
        (state IN ('queued', 'claimed')
         AND terminal_author_kind IS NULL AND terminal_author_id IS NULL)
        OR
        (state IN ('resolved', 'declined')
         AND terminal_author_kind IS NOT NULL)
        OR state = 'cancelled'
    )
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
CREATE TRIGGER runs_preserve_first_material
BEFORE UPDATE OF first_material_at ON runs
WHEN OLD.first_material_at IS NOT NULL
 AND NEW.first_material_at IS NOT OLD.first_material_at
BEGIN
    SELECT RAISE(ABORT, 'Run first material evidence is immutable');
END;
CREATE TRIGGER task_prs_enable_performance_tracking
AFTER INSERT ON task_prs
BEGIN
    UPDATE task_prs
    SET merge_tracking_complete = 1,
        repair_tracking_complete = 1
    WHERE id = NEW.id;
END;
CREATE TRIGGER task_prs_preserve_merged_at
BEFORE UPDATE OF merged_at ON task_prs
WHEN OLD.merged_at IS NOT NULL
 AND NEW.merged_at IS NOT OLD.merged_at
BEGIN
    SELECT RAISE(ABORT, 'Task PR merge evidence is immutable');
END;
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
CREATE TRIGGER task_pr_repair_incidents_are_immutable
BEFORE UPDATE ON task_pr_repair_incidents
BEGIN
    SELECT RAISE(ABORT, 'Task PR repair incidents are immutable');
END;
CREATE INDEX idx_run_events_ts ON run_events(ts);
CREATE INDEX idx_run_events_run ON run_events(run_id);
CREATE INDEX idx_run_events_process ON run_events(process_id);
CREATE INDEX idx_context_assets_kind ON context_assets(kind);
CREATE INDEX idx_context_assets_hash ON context_assets(content_sha256);
CREATE INDEX idx_context_decisions_decision ON context_decisions(decision);
CREATE INDEX idx_observation_outbox_pending
    ON observation_outbox(recipient_kind, recipient_id, delivered_at, id);
CREATE UNIQUE INDEX idx_provider_accounts_login_email
ON provider_accounts(provider, login_email)
WHERE login_email IS NOT NULL;
CREATE INDEX idx_provider_deliveries_status ON provider_deliveries(status);
CREATE INDEX idx_provider_deliveries_received ON provider_deliveries(received_at);
CREATE INDEX idx_projects_wave ON projects(wave_id, created_at);
CREATE INDEX idx_tasks_project ON tasks(project_id, created_at);
CREATE UNIQUE INDEX idx_epochs_one_open_wave
    ON epochs(wave_id) WHERE state = 'open' AND wave_id IS NOT NULL;
CREATE UNIQUE INDEX idx_epochs_one_open_project
    ON epochs(project_id) WHERE state = 'open' AND project_id IS NOT NULL;
CREATE UNIQUE INDEX idx_epochs_one_open_task
    ON epochs(task_id) WHERE state = 'open' AND task_id IS NOT NULL;
CREATE UNIQUE INDEX idx_runs_one_active_epoch
    ON runs(epoch_id) WHERE state != 'ended';
CREATE UNIQUE INDEX idx_runs_lease_hash
    ON runs(lease_hash) WHERE lease_hash IS NOT NULL;
CREATE UNIQUE INDEX idx_runs_source_generation
    ON runs(source_kind, source_id, lease_generation)
    WHERE source_id IS NOT NULL AND lease_generation IS NOT NULL;
CREATE UNIQUE INDEX idx_waits_one_unresolved_epoch
    ON waits(epoch_id) WHERE resolved_at IS NULL;
CREATE INDEX idx_steers_epoch_revision ON steers(epoch_id, rev);
CREATE INDEX idx_sends_turn ON sends(turn_id, attempted_at);
CREATE UNIQUE INDEX idx_homes_route ON homes(route);
CREATE INDEX idx_projects_wave_updated ON projects(wave_id, updated_at DESC);
CREATE UNIQUE INDEX idx_tasks_issue_identifier ON tasks(issue_identifier);
CREATE UNIQUE INDEX idx_tasks_worktree ON tasks(worktree);
CREATE INDEX idx_tasks_updated ON tasks(updated_at DESC);
CREATE INDEX idx_work_placements_home
    ON work_placements(home_id, placed_at);
CREATE INDEX idx_ci_incidents_observed
    ON ci_incidents(poll_observed_at, webhook_received_at);
CREATE INDEX idx_ci_incidents_pr ON ci_incidents(pr_id, created_at);
CREATE INDEX idx_ci_incidents_open ON ci_incidents(green_at, merged_at, updated_at);
CREATE INDEX idx_ci_incidents_run ON ci_incidents(claimed_run_id, updated_at);
CREATE INDEX idx_task_events_task ON task_events(task_id, id);
CREATE INDEX idx_project_events_project ON project_events(project_id, id);
CREATE UNIQUE INDEX idx_task_prs_open
    ON task_prs(task_id) WHERE merge_commit IS NULL AND abandoned_at IS NULL;
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
CREATE INDEX idx_ask_linear_comment_outbox_pending
    ON ask_linear_comment_outbox(delivered_at, created_at, ask_id, transition);
CREATE UNIQUE INDEX idx_agent_invocations_one_live_answer
    ON agent_invocations(answer_ask_id)
    WHERE answer_ask_id IS NOT NULL AND ended_at IS NULL;
CREATE INDEX idx_home_upgrades_home_started
    ON home_upgrades(home_id, target_generation DESC, started_at DESC, id DESC);
CREATE INDEX idx_runs_runtime_generation
    ON runs(home_id, runtime_generation, state);
CREATE INDEX idx_waves_parent ON waves(parent_wave_id);
CREATE INDEX idx_agent_turns_invocation
    ON agent_turns(invocation_id, ordinal);
CREATE INDEX idx_agent_turns_started ON agent_turns(started_at);
CREATE INDEX idx_agent_turns_epoch_basis
    ON agent_turns(epoch_id, basis_rev, status);
CREATE INDEX idx_turn_usage_samples_observed
    ON turn_usage_samples(observed_at, turn_id);
CREATE INDEX idx_ask_exchanges_parent_pending
    ON ask_exchanges(target_work_kind, target_work_id, asked_at)
    WHERE target_kind = 'parent' AND state IN ('queued', 'claimed');
CREATE INDEX idx_ask_exchanges_user_pending
    ON ask_exchanges(asked_at)
    WHERE target_kind = 'user' AND state IN ('queued', 'claimed');
CREATE INDEX idx_ask_exchanges_epoch_pending
    ON ask_exchanges(epoch_id, asked_at)
    WHERE state IN ('queued', 'claimed');
COMMIT;
