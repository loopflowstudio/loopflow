use std::collections::BTreeMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use loopflow::replay::{
    context_manifest_sha256, sha256_bytes, AgentConfigV1, ArtifactReferenceV1,
    ContextManifestIdentityV1, ConversationReferenceV1, ExecutionContractV1, LocalFileIdentityV1,
    ProcessConfigV1, ProviderExecutionV1, ReplayContractV1, ReplayTurnV1, RepositoryExecutionV1,
};
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{CredentialState, ProviderAccount, ProviderAccountId, RoutingState};
use loopflow::trace::{
    AgentInvocationRow, AgentTurnRow, ContextAssetRow, PreparedTurnContext,
    RecordedConversationEvent, RecordedConversationPayload, TRACE_SCHEMA_VERSION,
};
use rusqlite::Connection;
use tempfile::TempDir;
use time::OffsetDateTime;

const ELIGIBLE_ID: &str = "invocation_11111111111111111111111111111111";
const LEGACY_ID: &str = "invocation_22222222222222222222222222222222";
const TURN_ID: &str = "turn_11111111111111111111111111111111";
const AMBIGUOUS_ONE: &str = "invocation_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1";
const AMBIGUOUS_TWO: &str = "invocation_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2";

struct ReplayFixture {
    _root: TempDir,
    home: PathBuf,
    repo: PathBuf,
    bin: PathBuf,
    sentinel: PathBuf,
    conversation: PathBuf,
    immutable_artifacts: Vec<PathBuf>,
}

impl ReplayFixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let repo = root.path().join("repo");
        let bin = root.path().join("bin");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&bin).unwrap();
        _git(&repo, &["init", "-q"]);
        fs::write(repo.join("tracked.txt"), "recorded\n").unwrap();
        _git(&repo, &["add", "tracked.txt"]);
        _git(
            &repo,
            &[
                "-c",
                "user.name=Replay Test",
                "-c",
                "user.email=replay@example.com",
                "commit",
                "-qm",
                "record source",
            ],
        );
        let commit = String::from_utf8(
            Command::new("git")
                .args(["-C"])
                .arg(&repo)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();

        let sentinel = root.path().join("provider-launched");
        let trap = "#!/bin/sh\n: > \"$REPLAY_PROVIDER_SENTINEL\"\nexit 97\n";
        let codex = bin.join("codex");
        let claude = bin.join("claude");
        fs::write(&codex, trap).unwrap();
        fs::write(&claude, trap).unwrap();
        #[cfg(unix)]
        for path in [&codex, &claude] {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
        let config = root.path().join("provider-config.toml");
        fs::write(&config, "model = \"gpt-5\"\n").unwrap();

        let store = SqliteStore::new(&home.join("loopflow.db")).unwrap();
        let home_id = store.local_home().unwrap().id.to_string();
        let now = OffsetDateTime::now_utc().unix_timestamp();
        store
            .upsert_provider_account(&ProviderAccount {
                provider: "codex".to_string(),
                account_id: ProviderAccountId::parse("replay-test").unwrap(),
                home: None,
                login_email: None,
                credential_state: CredentialState::Connected,
                routing_state: RoutingState::Automatic,
                plan: None,
                paid_through: None,
                utilization_percent: None,
                cooldown_until: None,
                cooldown_reason: None,
                last_selected_at: None,
                created_at: now,
                updated_at: now,
            })
            .unwrap();

        let relative_dir = format!("run_test/process_test/{ELIGIBLE_ID}");
        let artifact_dir = home.join("traces").join(&relative_dir);
        let turns_dir = artifact_dir.join("turns");
        fs::create_dir_all(&turns_dir).unwrap();
        let system_path = format!("{relative_dir}/turns/0001-system.md");
        let task_path = format!("{relative_dir}/turns/0001-task.md");
        let conversation_path = format!("{relative_dir}/conversation.jsonl");
        let execution_path = format!("{relative_dir}/execution-contract.json");
        let replay_path = format!("{relative_dir}/replay-contract.json");
        let system = b"system prompt\n";
        let task = b"task prompt\n";
        fs::write(home.join("traces").join(&system_path), system).unwrap();
        fs::write(home.join("traces").join(&task_path), task).unwrap();

        let turn_id = loopflow::durable::TurnId::parse(TURN_ID).unwrap();
        let events = [
            RecordedConversationEvent {
                schema_version: TRACE_SCHEMA_VERSION,
                seq: 0,
                ts: OffsetDateTime::from_unix_timestamp(now).unwrap(),
                turn_id: Some(turn_id.clone()),
                payload: RecordedConversationPayload::UserInput {
                    op: "initial".to_string(),
                    text: String::from_utf8(task.to_vec()).unwrap(),
                },
            },
            RecordedConversationEvent {
                schema_version: TRACE_SCHEMA_VERSION,
                seq: 1,
                ts: OffsetDateTime::from_unix_timestamp(now).unwrap(),
                turn_id: Some(turn_id),
                payload: RecordedConversationPayload::Result {
                    status: "completed".to_string(),
                    duration_secs: Some(1.0),
                },
            },
        ];
        let mut conversation_bytes = Vec::new();
        for event in &events {
            serde_json::to_writer(&mut conversation_bytes, event).unwrap();
            conversation_bytes.push(b'\n');
        }
        let conversation = home.join("traces").join(&conversation_path);
        fs::write(&conversation, &conversation_bytes).unwrap();

        let prepared = PreparedTurnContext::from_prompts(
            std::str::from_utf8(system).unwrap(),
            std::str::from_utf8(task).unwrap(),
        );
        let context_assets = prepared.assets().cloned().collect::<Vec<_>>();
        let asset_rows = context_assets
            .iter()
            .cloned()
            .map(|asset| ContextAssetRow {
                turn_id: TURN_ID.to_string(),
                asset,
            })
            .collect::<Vec<_>>();
        let invocation = AgentInvocationRow {
            id: ELIGIBLE_ID.to_string(),
            run_id: "run_test".to_string(),
            answer_ask_id: None,
            process_id: "process_test".to_string(),
            started_at: now,
            ended_at: Some(now + 1),
            repo: repo.display().to_string(),
            worktree: repo.display().to_string(),
            wave: Some("intelligence".to_string()),
            flow: Some("task".to_string()),
            skill: Some("implement".to_string()),
            project: Some("trace".to_string()),
            task: Some("LOO-129".to_string()),
            provider: "codex".to_string(),
            model: Some("gpt-5".to_string()),
            surface: "headless".to_string(),
            capture_status: "complete".to_string(),
            incomplete_reason: None,
            outcome: "completed".to_string(),
            artifact_dir: relative_dir,
            conversation_path: conversation_path.clone(),
            provider_events_path: None,
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: events.len() as i64,
            conversation_bytes: conversation_bytes.len() as i64,
            supervision: None,
        };
        let turn = AgentTurnRow {
            id: TURN_ID.to_string(),
            invocation_id: ELIGIBLE_ID.to_string(),
            ordinal: 1,
            provider_turn_id: None,
            started_at: now,
            ended_at: Some(now + 1),
            status: "completed".to_string(),
            input_op: "initial".to_string(),
            context_coverage: "assembled".to_string(),
            tokenizer: "cl100k_base".to_string(),
            system_prompt_path: Some(system_path.clone()),
            task_prompt_path: task_path.clone(),
            system_tokens: prepared.system.as_ref().unwrap().tokens as i64,
            task_tokens: prepared.task.tokens as i64,
            supplied_context_tokens: prepared.total_tokens() as i64,
            usage: None,
            context_gather_ms: 1,
            context_render_ms: 1,
            context_persist_ms: 1,
            first_event_seq: Some(0),
            last_event_seq: Some(1),
            root_output: Some("done".to_string()),
            basis: None,
        };
        store
            .insert_trace_capture(&invocation, &turn, &asset_rows, &[])
            .unwrap();

        let replay_turn = ReplayTurnV1 {
            turn_id: TURN_ID.to_string(),
            ordinal: 1,
            input_op: "initial".to_string(),
            timing: "initial".to_string(),
            system_prompt: Some(ArtifactReferenceV1 {
                path: system_path,
                sha256: sha256_bytes(system),
            }),
            task_prompt: ArtifactReferenceV1 {
                path: task_path,
                sha256: sha256_bytes(task),
            },
            context_manifest: ContextManifestIdentityV1 {
                sha256: context_manifest_sha256(&context_assets),
                asset_count: context_assets.len() as u32,
            },
        };
        let execution = ExecutionContractV1 {
            schema_version: 1,
            invocation_id: ELIGIBLE_ID.to_string(),
            home_id: home_id.clone(),
            repository: RepositoryExecutionV1 {
                root: repo.display().to_string(),
                commit,
                clean: true,
            },
            provider: ProviderExecutionV1 {
                provider: "codex".to_string(),
                model: "gpt-5".to_string(),
                account_id: "replay-test".to_string(),
                binary: _file_identity(&codex),
                config_files: vec![_file_identity(&config)],
            },
            agent: AgentConfigV1 {
                agent: "codex:gpt-5".to_string(),
                max_turns: Some(1),
                cwd: repo.display().to_string(),
                run_context: "inherit".to_string(),
                permission_policy: "managed".to_string(),
                write_scope: "worktree".to_string(),
                writable_roots: Vec::new(),
                network_access: false,
                skip_permissions: false,
                structured_replies: Vec::new(),
                directive_relay: None,
            },
            process: ProcessConfigV1 {
                surface: "headless".to_string(),
                unattended: true,
                stream: true,
                stream_format: "raw".to_string(),
                timeout_ms: Some(60_000),
            },
            sanitized_argv: vec![codex.display().to_string(), "--json".to_string()],
            environment_selectors: BTreeMap::from([("NO_COLOR".to_string(), "1".to_string())]),
            initial_turn: replay_turn.clone(),
        };
        let execution_bytes = serde_json::to_vec_pretty(&execution).unwrap();
        let execution = home.join("traces").join(&execution_path);
        fs::write(&execution, &execution_bytes).unwrap();
        let replay = ReplayContractV1 {
            schema_version: 1,
            invocation_id: ELIGIBLE_ID.to_string(),
            home_id: home_id.clone(),
            wave: Some("intelligence".to_string()),
            project: Some("trace".to_string()),
            task: Some("LOO-129".to_string()),
            flow: Some("task".to_string()),
            skill: Some("implement".to_string()),
            execution_contract: ArtifactReferenceV1 {
                path: execution_path,
                sha256: sha256_bytes(&execution_bytes),
            },
            turns: vec![replay_turn],
            conversation: ConversationReferenceV1 {
                path: conversation_path,
                sha256: sha256_bytes(&conversation_bytes),
                trace_schema_version: TRACE_SCHEMA_VERSION,
                event_count: events.len() as u64,
                bytes: conversation_bytes.len() as u64,
            },
        };
        let replay_bytes = serde_json::to_vec_pretty(&replay).unwrap();
        let replay = home.join("traces").join(&replay_path);
        fs::write(&replay, &replay_bytes).unwrap();
        drop(store);

        let connection = Connection::open(home.join("loopflow.db")).unwrap();
        let replay_contracts_exist = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM sqlite_master
                    WHERE type='table' AND name='replay_contracts'
                 )",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        if !replay_contracts_exist {
            connection
                .execute_batch(&loopflow::store::migrations::migration_sql_for_test(
                    "replay_contracts",
                ))
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO replay_contracts (
                    invocation_id, schema_version, home_id, contract_path,
                    contract_sha256, captured_at
                 ) VALUES (?1, 1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    ELIGIBLE_ID,
                    home_id,
                    replay_path,
                    sha256_bytes(&replay_bytes),
                    now,
                ],
            )
            .unwrap();
        _insert_legacy(&connection, LEGACY_ID, None, "complete", &repo, 1);
        _insert_legacy(
            &connection,
            AMBIGUOUS_ONE,
            Some("gpt-5"),
            "prompt_only",
            &repo,
            now,
        );
        _insert_legacy(
            &connection,
            AMBIGUOUS_TWO,
            Some("gpt-5"),
            "prompt_only",
            &repo,
            now,
        );
        drop(connection);

        Self {
            _root: root,
            home: home.clone(),
            repo,
            bin,
            sentinel,
            conversation: conversation.clone(),
            immutable_artifacts: vec![
                home.join("loopflow.db"),
                replay,
                execution,
                conversation,
                artifact_dir.join("turns/0001-system.md"),
                artifact_dir.join("turns/0001-task.md"),
                config,
                codex,
            ],
        }
    }

    fn run(&self, args: &[&str]) -> Output {
        let path = format!(
            "{}:{}",
            self.bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .current_dir(&self.repo)
            .env("LF_HOME", &self.home)
            .env("LF_DB_PATH", self.home.join("loopflow.db"))
            .env("PATH", path)
            .env("REPLAY_PROVIDER_SENTINEL", &self.sentinel)
            .env("NO_COLOR", "1")
            .env_remove("LF_CONTROL_HOME")
            .env_remove("LF_CONTROL_DB_PATH")
            .env_remove("LF_RUN_ID")
            .env_remove("LF_PROCESS_ID")
            .env_remove("LF_AGENT_INVOCATION_ID")
            .output()
            .unwrap()
    }

    fn identities(&self) -> Vec<String> {
        self.immutable_artifacts
            .iter()
            .map(|path| sha256_bytes(&fs::read(path).unwrap()))
            .collect()
    }
}

#[test]
fn replay_check_classifies_real_contracts_without_launch_or_mutation() {
    let fixture = ReplayFixture::new();
    let before = fixture.identities();

    let eligible = fixture.run(&["replay", "check", ELIGIBLE_ID, "--json"]);
    assert!(
        eligible.status.success(),
        "{}",
        String::from_utf8_lossy(&eligible.stderr)
    );
    let eligible: serde_json::Value = serde_json::from_slice(&eligible.stdout).unwrap();
    assert_eq!(eligible["eligible"], true);
    assert_eq!(eligible["reasons"], serde_json::json!([]));

    let eligible_prefix = fixture.run(&["replay", "check", "invocation_11111111", "--json"]);
    assert!(eligible_prefix.status.success());
    let eligible_prefix: serde_json::Value =
        serde_json::from_slice(&eligible_prefix.stdout).unwrap();
    assert_eq!(eligible_prefix["invocation_id"], ELIGIBLE_ID);

    let refused = fixture.run(&["replay", "check", LEGACY_ID, "--json"]);
    assert_eq!(refused.status.code(), Some(1));
    let refused: serde_json::Value = serde_json::from_slice(&refused.stdout).unwrap();
    assert_eq!(
        _reason_codes(&refused),
        [
            "missing_effective_model",
            "missing_execution_contract",
            "missing_repository_revision",
        ]
    );

    let text = fixture.run(&["replay", "check", LEGACY_ID]);
    let text = String::from_utf8(text.stdout).unwrap();
    for code in [
        "missing_effective_model",
        "missing_execution_contract",
        "missing_repository_revision",
    ] {
        assert!(text.contains(code), "{text}");
    }

    let ambiguous = fixture.run(&["replay", "check", "invocation_aaaaaaaa", "--json"]);
    assert_eq!(ambiguous.status.code(), Some(1));
    let ambiguous: serde_json::Value = serde_json::from_slice(&ambiguous.stdout).unwrap();
    assert_eq!(ambiguous["invocation_id"], serde_json::Value::Null);
    assert_eq!(
        ambiguous["candidates"],
        serde_json::json!([AMBIGUOUS_ONE, AMBIGUOUS_TWO])
    );
    assert_eq!(_reason_codes(&ambiguous), ["ambiguous_address"]);

    let literal = fixture.run(&["replay", "check", "invocation_%", "--json"]);
    assert_eq!(literal.status.code(), Some(1));
    let literal: serde_json::Value = serde_json::from_slice(&literal.stdout).unwrap();
    assert_eq!(_reason_codes(&literal), ["not_found"]);

    assert!(!fixture.sentinel.exists(), "provider trap was executed");
    assert_eq!(fixture.identities(), before);
}

#[test]
fn replay_check_names_a_corrupt_conversation_without_launching() {
    let fixture = ReplayFixture::new();
    fs::write(&fixture.conversation, "changed\n").unwrap();

    let output = fixture.run(&["replay", "check", ELIGIBLE_ID, "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(_reason_codes(&result), ["artifact_hash_mismatch"]);
    assert!(!fixture.sentinel.exists(), "provider trap was executed");
}

#[test]
fn replay_check_never_searches_an_alternate_home() {
    let fixture = ReplayFixture::new();
    let connection = Connection::open(fixture.home.join("loopflow.db")).unwrap();
    let remote_home = "home_99999999999999999999999999999999";
    connection
        .execute(
            "INSERT INTO homes (id, route, created_at, observed_at)
             VALUES (?1, 'replay-test.example', 1, 1)",
            [remote_home],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE replay_contracts SET home_id=?2 WHERE invocation_id=?1",
            [ELIGIBLE_ID, remote_home],
        )
        .unwrap();
    drop(connection);

    let output = fixture.run(&["replay", "check", ELIGIBLE_ID, "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(_reason_codes(&result), ["artifact_authority_unavailable"]);
    assert!(!fixture.sentinel.exists(), "provider trap was executed");
}

fn _reason_codes(result: &serde_json::Value) -> Vec<&str> {
    result["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .map(|reason| reason["code"].as_str().unwrap())
        .collect()
}

fn _file_identity(path: &Path) -> LocalFileIdentityV1 {
    LocalFileIdentityV1 {
        path: path.display().to_string(),
        sha256: sha256_bytes(&fs::read(path).unwrap()),
    }
}

fn _git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-C"])
        .arg(repo)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn _insert_legacy(
    connection: &Connection,
    id: &str,
    model: Option<&str>,
    capture_status: &str,
    repo: &Path,
    now: i64,
) {
    connection
        .execute(
            "INSERT INTO agent_invocations (
                id, run_id, process_id, started_at, ended_at, repo, worktree,
                wave, flow, skill, provider, model, surface, capture_status,
                incomplete_reason, outcome, artifact_dir, conversation_path,
                provider_events_path, provider_session_id, provider_session_path,
                conversation_event_count, conversation_bytes, project, task
             ) VALUES (
                ?1, ?2, ?3, ?4, ?4, ?5, ?5,
                'intelligence', 'task', 'implement', 'codex', ?6, 'headless', ?7,
                NULL, 'completed', 'legacy', 'legacy/conversation.jsonl',
                NULL, NULL, NULL, 0, 0, 'trace', 'LOO-129'
             )",
            rusqlite::params![
                id,
                format!("run-{id}"),
                format!("process-{id}"),
                now,
                repo.display().to_string(),
                model,
                capture_status,
            ],
        )
        .unwrap();
}
