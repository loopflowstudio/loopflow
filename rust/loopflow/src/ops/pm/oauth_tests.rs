use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use serde_json::json;
use tempfile::TempDir;
use tokio::sync::Barrier;
use tracing::instrument::WithSubscriber;

use super::{
    linear_refresh_lock, pm_show_async, resolve_local_pm_token, PmRefresh, PmShowOptions,
    PmTestContext, PM_TEST_CONTEXT,
};
use crate::durable::{Author, WorkRef};
use crate::id::WaveId;
use crate::ops::error::OpsResult;
use crate::ops::NullProgress;
use crate::planning::{LinearProjectId, ProjectPlan};
use crate::pm::test_server::{self, json_response, QueuedResponse};
use crate::pm::{PmProviderKind, PmSnapshot};
use crate::provider_auth::{LinearRefreshError, LINEAR_REFRESH_CONFIG, LINEAR_REFRESH_URL};
use crate::store::{
    open_ephemeral_store, CredentialType, PmSnapshotRow, ProviderToken, StorageConfig, Store,
};
use crate::work::project::{Project, ProjectId};
use crate::work::wave::Wave;

struct Fixture {
    directory: TempDir,
    database: PathBuf,
    store: Arc<Store>,
}

impl Fixture {
    async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("registry.db");
        let store = Arc::new(
            open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
                .await
                .unwrap(),
        );
        Self {
            directory,
            database,
            store,
        }
    }

    fn context(&self, graphql_url: &str) -> PmTestContext {
        PmTestContext {
            path: self.database.clone(),
            store: self.store.clone(),
            graphql_url: graphql_url.into(),
        }
    }

    async fn resolve(&self, url: &str) -> OpsResult<Option<String>> {
        scoped(
            self.context(""),
            url,
            resolve_local_pm_token(PmProviderKind::Linear),
        )
        .await
    }

    async fn seed(&self, expires_at: i64) -> ProviderToken {
        let token = token("A1", "R1", expires_at);
        self.store.upsert_provider_token(&token).await.unwrap();
        token
    }

    async fn assert_token(&self, expected: &ProviderToken) {
        let current = self.store.get_provider_token("linear").await.unwrap();
        assert!(
            current.as_ref() == Some(expected),
            "credential generation changed"
        );
    }

    async fn planning_repo(&self) -> (PathBuf, Wave) {
        let repo = self.directory.path().join("repo");
        std::fs::create_dir_all(repo.join(".lf")).unwrap();
        // Local fixture history only; no installed Home or remote is touched.
        for args in [
            vec!["init", "-q"],
            vec![
                "remote",
                "add",
                "origin",
                "https://github.com/loopflowstudio/fixture.git",
            ],
        ] {
            assert!(std::process::Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(
            repo.join(".lf/config.yaml"),
            "pm:\n  provider: linear\n  linear_team: team-1\n",
        )
        .unwrap();
        std::fs::create_dir_all(repo.join("wave/product")).unwrap();
        std::fs::write(
            repo.join("wave/product/GOAL.md"),
            "---\npm:\n  linear_initiative: initiative-1\n---\nKeep working.\n",
        )
        .unwrap();
        let repo = std::fs::canonicalize(repo).unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "product".into(),
            repo.to_string_lossy().into_owned(),
        );
        self.store.create_wave(&wave).await.unwrap();
        (repo, wave)
    }

    async fn seed_snapshot(&self, wave: &Wave) -> PmSnapshotRow {
        let row = PmSnapshotRow {
            wave_id: wave.id().clone(),
            provider: "linear".into(),
            initiative: "initiative-1".into(),
            synced_at: 1,
            payload: serde_json::to_string(&PmSnapshot {
                projects: vec![],
                items: vec![],
            })
            .unwrap(),
        };
        self.store.put_pm_snapshot(row.clone()).await.unwrap();
        row
    }
}

// Unlike expect_err, this cannot dump a credential on an unexpected success.
fn failure_message<T>(result: OpsResult<T>) -> String {
    match result {
        Err(error) => error.to_string(),
        Ok(_) => panic!("operation should fail"),
    }
}

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn token(access: &str, refresh: &str, expires_at: i64) -> ProviderToken {
    ProviderToken {
        provider: "linear".into(),
        access_token: access.into(),
        refresh_token: Some(refresh.into()),
        oauth_client_id: Some("fixture-client".into()),
        expires_at: Some(expires_at),
        login: Some("fixture".into()),
        updated_at: now(),
        credential_type: CredentialType::OAuth,
    }
}

async fn scoped<T>(context: PmTestContext, url: &str, future: impl Future<Output = T>) -> T {
    PM_TEST_CONTEXT
        .scope(context, LINEAR_REFRESH_URL.scope(url.to_string(), future))
        .await
}

fn rotated() -> QueuedResponse {
    json_response(
        StatusCode::OK,
        json!({ "access_token": "A2", "refresh_token": "R2", "expires_in": 86400 }),
    )
}

fn rejected(code: &str) -> QueuedResponse {
    json_response(
        StatusCode::BAD_REQUEST,
        json!({ "error": code, "error_description": "synthetic-secret-description" }),
    )
}

fn gated(mut response: QueuedResponse) -> (QueuedResponse, Arc<Barrier>, Arc<Barrier>) {
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    response.gate = Some((entered.clone(), release.clone()));
    (response, entered, release)
}

async fn forced_read(repo: &Path) -> OpsResult<super::PmShowResult> {
    pm_show_async(
        repo,
        &PmShowOptions {
            wave: Some("product".into()),
            project: None,
            refresh: PmRefresh::Force,
        },
        &NullProgress,
    )
    .await
}

#[tokio::test]
async fn pm_read_linear_oauth_recovers() {
    let fixture = Fixture::new().await;
    let (repo, wave) = fixture.planning_repo().await;
    let stored_wave =
        serde_json::to_value(fixture.store.get_wave(wave.id()).await.unwrap()).unwrap();
    let timestamp = time::OffsetDateTime::from_unix_timestamp(now()).unwrap();
    let project = Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            id: LinearProjectId::new("project-1").unwrap(),
            slug: "reliability".into(),
            name: "Reliability".into(),
            prompt_context: "Retain this planning history.".into(),
            pm_snapshot_synced_at: 1,
        },
        wave_id: wave.id().clone(),
        iteration: 3,
        abandon_intent: None,
        created_at: timestamp,
        updated_at: timestamp,
    };
    fixture.store.create_project(&project).await.unwrap();
    let work = WorkRef::Project(project.id.clone());
    let steer = fixture
        .store
        .append_steer(&work, Author::User, "Preserve the current direction.")
        .await
        .unwrap();
    let history = fixture
        .store
        .project_events_after(&project.id, 0)
        .await
        .unwrap();
    fixture.seed_snapshot(&wave).await;
    fixture.seed(now() - 1).await;
    let (oauth, exchanges) = test_server::spawn(vec![
        json_response(StatusCode::SERVICE_UNAVAILABLE, json!({})),
        rotated(),
    ])
    .await;
    let (graphql, requests) = test_server::spawn_authorized(vec![
        json_response(StatusCode::OK, json!({"data":{"teams":{"nodes":[{
            "id":"team-1", "name":"Fixture", "key":"FIX",
            "description":"<!-- loopflow-repository: loopflowstudio/fixture -->"
        }]}}})),
        json_response(StatusCode::OK, json!({"data":{"initiative":{"projects":{
            "nodes":[{"id":"project-1", "name":"Product — Reliability", "description":"",
            "content":"## Definition\n\nFresh definition.\n\n## KRs\n\n- [ ] Fresh proof",
            "initiatives":{"nodes":[{"id":"initiative-1"}]}, "teams":{"nodes":[{"id":"team-1"}]}}],
            "pageInfo":{"hasNextPage":false,"endCursor":null}
        }}}})),
        json_response(StatusCode::OK, json!({"data":{"project":{"issues":{
            "nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}
        }}}})),
    ], Some("Bearer A2".into())).await;
    let result = scoped(fixture.context(&graphql), &oauth, forced_read(&repo))
        .await
        .unwrap();
    assert_eq!(result.projects[0].definition, "Fresh definition.");
    assert_eq!(result.projects[0].krs[0].text, "Fresh proof");
    let row = fixture.store.pm_snapshot(wave.id()).await.unwrap().unwrap();
    let snapshot: PmSnapshot = serde_json::from_str(&row.payload).unwrap();
    assert_eq!(snapshot.projects, result.projects);
    assert_eq!(
        serde_json::to_value(fixture.store.get_wave(wave.id()).await.unwrap()).unwrap(),
        stored_wave
    );
    assert_eq!(
        fixture.store.get_project(&project.id).await.unwrap(),
        Some(project.clone())
    );
    assert_eq!(fixture.store.work_steers(&work).await.unwrap(), vec![steer]);
    assert_eq!(
        fixture
            .store
            .project_events_after(&project.id, 0)
            .await
            .unwrap(),
        history
    );
    assert_eq!(exchanges.lock().await.len(), 2);
    assert!(exchanges
        .lock()
        .await
        .iter()
        .all(|r| r.body.contains("refresh_token=R1")));
    assert!(requests
        .lock()
        .await
        .iter()
        .all(|r| r.authorization.as_deref() == Some("Bearer A2")));
    let current = fixture
        .store
        .get_provider_token("linear")
        .await
        .unwrap()
        .unwrap();
    assert!(current.access_token == "A2" && current.refresh_token.as_deref() == Some("R2"));
    let conn = rusqlite::Connection::open(&fixture.database).unwrap();
    let (access, refresh, encrypted): (String,String,bool) = conn.query_row(
        "SELECT access_token, refresh_token, encrypted FROM provider_tokens WHERE provider='linear'", [],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).unwrap();
    assert!(encrypted && access != "A2" && refresh != "R2");
}

#[tokio::test]
async fn pm_read_linear_oauth_invalid_grant() {
    for cached in [true, false] {
        let fixture = Fixture::new().await;
        let (repo, wave) = fixture.planning_repo().await;
        let previous = if cached {
            Some(fixture.seed_snapshot(&wave).await)
        } else {
            None
        };
        let original = fixture.seed(now() - 1).await;
        let (oauth, exchanges) = test_server::spawn(vec![rejected("invalid_grant")]).await;
        let (graphql, requests) = test_server::spawn(vec![]).await;
        let error =
            failure_message(scoped(fixture.context(&graphql), &oauth, forced_read(&repo)).await);
        assert!(error.contains("invalid_grant") && error.contains("doppler run -- lf auth linear"));
        assert!(!error.contains("synthetic-secret"));
        assert_eq!(exchanges.lock().await.len(), 1);
        assert!(requests.lock().await.is_empty());
        fixture.assert_token(&original).await;
        assert_eq!(
            fixture.store.pm_snapshot(wave.id()).await.unwrap(),
            previous
        );
    }
}

#[tokio::test]
async fn linear_oauth_classifies_failures_without_partial_writes() {
    let cases = [
        (
            json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({"error":"invalid_grant"}),
            ),
            false,
            2,
        ),
        (
            json_response(
                StatusCode::TOO_MANY_REQUESTS,
                json!({"error":"invalid_client"}),
            ),
            false,
            2,
        ),
        (rejected("invalid_client"), true, 1),
        (rejected("synthetic-secret-code"), false, 1),
        (json_response(StatusCode::UNAUTHORIZED, json!({})), false, 1),
        (
            json_response(
                StatusCode::OK,
                json!({"access_token":"A2","expires_in":86400}),
            ),
            false,
            2,
        ),
        (
            json_response(
                StatusCode::OK,
                json!({"access_token":"", "refresh_token":"R2","expires_in":86400}),
            ),
            false,
            2,
        ),
        (
            json_response(
                StatusCode::OK,
                json!({"access_token":"A2", "refresh_token":"  ","expires_in":86400}),
            ),
            false,
            2,
        ),
        (
            json_response(
                StatusCode::OK,
                json!({"access_token":"A2", "refresh_token":"R2","expires_in":0}),
            ),
            false,
            2,
        ),
        (
            json_response(
                StatusCode::OK,
                json!({"access_token":"A2", "refresh_token":"R2"}),
            ),
            false,
            2,
        ),
        (
            json_response(
                StatusCode::OK,
                json!({"access_token":"A2", "refresh_token":"R2","expires_in":-1}),
            ),
            false,
            2,
        ),
        (
            json_response(
                StatusCode::OK,
                json!({"access_token":"A2", "refresh_token":"R2","expires_in":i64::MAX}),
            ),
            false,
            2,
        ),
        (
            test_server::response(
                StatusCode::OK,
                vec![],
                "not json synthetic-secret-body".into(),
            ),
            false,
            2,
        ),
    ];
    let fixture = Fixture::new().await;
    for (response, reconnect, attempts) in cases {
        let original = fixture.seed(now() - 1).await;
        let (url, requests) = test_server::spawn(vec![response.clone(), response]).await;
        let error = failure_message(fixture.resolve(&url).await);
        assert_eq!(error.contains("doppler run -- lf auth linear"), reconnect);
        assert!(!error.contains("synthetic-secret"));
        assert_eq!(requests.lock().await.len(), attempts);
        fixture.assert_token(&original).await;
    }
}

#[tokio::test]
async fn linear_oauth_concurrent_alias_readers_share_one_rotation() {
    let fixture = Fixture::new().await;
    fixture.seed(now() - 1).await;
    let alias = fixture.directory.path().join("alias.db");
    std::os::unix::fs::symlink(&fixture.database, &alias).unwrap();
    let other = Arc::new(
        open_ephemeral_store(&StorageConfig::sqlite(alias.clone()))
            .await
            .unwrap(),
    );
    let (response, entered, release) = gated(rotated());
    let (url, requests) = test_server::spawn(vec![response]).await;
    let first_ctx = fixture.context("");
    let first_url = url.clone();
    let first = tokio::spawn(async move {
        scoped(
            first_ctx,
            &first_url,
            resolve_local_pm_token(PmProviderKind::Linear),
        )
        .await
    });
    entered.wait().await;
    let second_ctx = PmTestContext {
        path: alias,
        store: other,
        graphql_url: String::new(),
    };
    let second = scoped(
        second_ctx,
        &url,
        resolve_local_pm_token(PmProviderKind::Linear),
    );
    let (second, _) = tokio::join!(second, release.wait());
    assert!(first.await.unwrap().unwrap().as_deref() == Some("A2"));
    assert!(second.unwrap().as_deref() == Some("A2"));
    assert_eq!(requests.lock().await.len(), 1);
}

#[tokio::test]
async fn linear_oauth_interactive_connection_and_deletion_win_inflight_exchange() {
    let fixture = Fixture::new().await;
    for (response, delete) in [
        (rotated(), false),
        (rejected("invalid_grant"), false),
        (rotated(), true),
    ] {
        fixture.seed(now() - 1).await;
        let (response, entered, release) = gated(response);
        let (url, requests) = test_server::spawn(vec![response]).await;
        let ctx = fixture.context("");
        let resolver = tokio::spawn(async move {
            scoped(ctx, &url, resolve_local_pm_token(PmProviderKind::Linear)).await
        });
        entered.wait().await;
        let winner = token("interactive", "interactive-refresh", now() + 86400);
        if delete {
            fixture.store.delete_provider_token("linear").await.unwrap();
        } else {
            fixture.store.upsert_provider_token(&winner).await.unwrap();
        }
        release.wait().await;
        let result = resolver.await.unwrap().unwrap();
        if delete {
            assert!(result.is_none());
            assert!(fixture
                .store
                .get_provider_token("linear")
                .await
                .unwrap()
                .is_none());
        } else {
            assert!(result.as_deref() == Some("interactive"));
            fixture.assert_token(&winner).await;
        }
        assert_eq!(requests.lock().await.len(), 1);
    }
}

#[tokio::test]
async fn linear_oauth_recovers_after_rolled_back_rotation() {
    let fixture = Fixture::new().await;
    let conn = rusqlite::Connection::open(&fixture.database).unwrap();
    for expired in [true, false] {
        let original = fixture.seed(now() + if expired { -1 } else { 60 }).await;
        conn.execute_batch("CREATE TRIGGER reject_rotation BEFORE UPDATE ON provider_tokens BEGIN SELECT RAISE(ABORT, 'fixture rollback'); END").unwrap();
        let (url, requests) = test_server::spawn(vec![rotated(), rotated()]).await;
        let result = fixture.resolve(&url).await;
        if expired {
            let error = failure_message(result);
            assert!(error.contains("persist") && !error.contains("reconnect"));
        } else {
            assert!(result.unwrap().as_deref() == Some("A1"));
        }
        fixture.assert_token(&original).await;
        conn.execute_batch("DROP TRIGGER reject_rotation").unwrap();
        assert!(fixture.resolve(&url).await.unwrap().as_deref() == Some("A2"));
        assert!(requests
            .lock()
            .await
            .iter()
            .all(|r| r.body.contains("refresh_token=R1")));
    }
}

#[tokio::test]
async fn linear_oauth_cancelled_lock_waiter_never_exchanges() {
    let fixture = Fixture::new().await;
    let original = fixture.seed(now() - 1).await;
    let lock = linear_refresh_lock(&fixture.database, Instant::now() + Duration::from_secs(5))
        .await
        .unwrap();
    let (url, requests) = test_server::spawn(vec![rotated()]).await;
    let future = fixture.resolve(&url);
    assert!(tokio::time::timeout(Duration::from_millis(50), future)
        .await
        .is_err());
    drop(lock);
    assert!(fixture.resolve(&url).await.unwrap().as_deref() == Some("A2"));
    assert_eq!(requests.lock().await.len(), 1);
    assert!(original.access_token == "A1");
}

#[tokio::test]
async fn linear_oauth_optional_local_authority_and_legacy_guidance() {
    let fixture = Fixture::new().await;
    let (url, requests) = test_server::spawn(vec![]).await;
    assert!(fixture.resolve(&url).await.unwrap().is_none());
    let mut original = fixture.seed(now() - 1).await;
    original.oauth_client_id = None;
    fixture
        .store
        .upsert_provider_token(&original)
        .await
        .unwrap();
    for (reason, reconnect) in [
        (LinearRefreshError::ClientConfigurationUnavailable, true),
        (LinearRefreshError::ConfigurationLookupFailed, false),
    ] {
        let error = failure_message(
            LINEAR_REFRESH_CONFIG
                .scope(Err(reason), fixture.resolve(&url))
                .await,
        );
        assert_eq!(error.contains("doppler run -- lf auth linear"), reconnect);
        fixture.assert_token(&original).await;
    }
    assert!(requests.lock().await.is_empty());
}

#[tokio::test]
async fn linear_oauth_failed_refresh_checks_expiry_at_return() {
    let fixture = Fixture::new().await;
    for expires_during_request in [false, true] {
        let expires = now() + if expires_during_request { 1 } else { 60 };
        let original = fixture.seed(expires).await;
        let (response, entered, release) = gated(rejected("unknown"));
        let (url, _) = test_server::spawn(vec![response]).await;
        let ctx = fixture.context("");
        let resolver = tokio::spawn(async move {
            scoped(ctx, &url, resolve_local_pm_token(PmProviderKind::Linear)).await
        });
        entered.wait().await;
        if expires_during_request {
            let remaining = (expires as i128 * 1_000_000_000
                - time::OffsetDateTime::now_utc().unix_timestamp_nanos())
            .max(0);
            tokio::time::sleep(Duration::from_nanos(remaining as u64)).await;
        }
        release.wait().await;
        let result = resolver.await.unwrap();
        if expires_during_request {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().as_deref() == Some("A1"));
        }
        fixture.assert_token(&original).await;
    }
}

#[tokio::test]
async fn linear_oauth_transport_failure_preserves_grant() {
    let fixture = Fixture::new().await;
    let original = fixture.seed(now() - 1).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (socket, _) = listener.accept().await.unwrap();
            drop(socket);
        }
    });
    let error = failure_message(fixture.resolve(&url).await);
    assert!(error.contains("unavailable") && !error.contains("reconnect"));
    fixture.assert_token(&original).await;
    server.await.unwrap();
}

#[tokio::test]
async fn pm_read_linear_oauth_sqlite_contention_has_bounded_failure_and_recovers() {
    let fixture = Fixture::new().await;
    let (repo, wave) = fixture.planning_repo().await;
    let previous = fixture.seed_snapshot(&wave).await;
    let original = fixture.seed(now() - 1).await;
    let conn = rusqlite::Connection::open(&fixture.database).unwrap();
    conn.execute_batch("BEGIN IMMEDIATE").unwrap();
    let (url, requests) = test_server::spawn(vec![rotated(), rotated()]).await;
    let start = Instant::now();
    let result = scoped(fixture.context(""), &url, forced_read(&repo)).await;
    assert!(result.is_err());
    assert!(start.elapsed() < Duration::from_secs(6));
    assert_eq!(requests.lock().await.len(), 1);
    conn.execute_batch("ROLLBACK").unwrap();
    // Joining serialization proves any cancelled persistence closure has settled.
    let guard = linear_refresh_lock(&fixture.database, Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    fixture.assert_token(&original).await;
    assert_eq!(
        fixture.store.pm_snapshot(wave.id()).await.unwrap(),
        Some(previous)
    );
    drop(guard);
    assert!(fixture.resolve(&url).await.unwrap().as_deref() == Some("A2"));
}

#[tokio::test]
async fn linear_oauth_ssh_forwards_only_a_current_optional_bearer() {
    let fixture = Fixture::new().await;
    let (url, _) = test_server::spawn(vec![rotated(), rejected("invalid_grant")]).await;
    let forward = || crate::lf::commands::ssh::resolve_pm_token_for_test();
    assert!(scoped(fixture.context(""), &url, forward()).await.is_none());
    fixture.seed(now() - 1).await;
    assert!(
        scoped(fixture.context(""), &url, forward())
            .await
            .as_deref()
            == Some("A2")
    );
    let original = fixture.seed(now() - 1).await;
    assert!(scoped(fixture.context(""), &url, forward()).await.is_none());
    fixture.assert_token(&original).await;
}

#[derive(Clone)]
struct TraceBuffer(Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for TraceBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn linear_oauth_proactive_failure_tracing_is_secret_free() {
    let fixture = Fixture::new().await;
    let mut original = fixture.seed(now() + 60).await;
    original.access_token = "synthetic-secret-access".into();
    original.refresh_token = Some("synthetic-secret-refresh".into());
    fixture
        .store
        .upsert_provider_token(&original)
        .await
        .unwrap();
    let (url, _) = test_server::spawn(vec![rejected("synthetic-secret-code")]).await;
    let output = TraceBuffer(Arc::new(std::sync::Mutex::new(Vec::new())));
    let writer = output.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    let value = fixture
        .resolve(&url)
        .with_subscriber(subscriber)
        .await
        .unwrap();
    assert!(value.as_deref() == Some(original.access_token.as_str()));
    let captured = String::from_utf8(output.0.lock().unwrap().clone()).unwrap();
    assert!(captured.contains("proactive Linear refresh failed"));
    assert!(!captured.contains("synthetic-secret"));
}

#[tokio::test]
async fn linear_oauth_stalled_first_attempt_leaves_time_for_one_replay() {
    let fixture = Fixture::new().await;
    fixture.seed(now() - 1).await;
    let (response, entered, _release) = gated(rotated());
    let (url, requests) = test_server::spawn(vec![response, rotated()]).await;
    let ctx = fixture.context("");
    let start = Instant::now();
    let resolver = tokio::spawn(async move {
        scoped(ctx, &url, resolve_local_pm_token(PmProviderKind::Linear)).await
    });
    entered.wait().await;
    let value = resolver.await.unwrap().unwrap();
    assert!(value.as_deref() == Some("A2"));
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(requests.lock().await.len(), 2);
}

#[tokio::test]
async fn linear_oauth_stale_rejection_does_not_condemn_expired_winner() {
    let fixture = Fixture::new().await;
    fixture.seed(now() - 1).await;
    let (response, entered, release) = gated(rejected("invalid_grant"));
    let (url, _) = test_server::spawn(vec![response]).await;
    let ctx = fixture.context("");
    let resolver = tokio::spawn(async move {
        scoped(ctx, &url, resolve_local_pm_token(PmProviderKind::Linear)).await
    });
    entered.wait().await;
    let winner = token("newer-access", "newer-refresh", now() - 1);
    fixture.store.upsert_provider_token(&winner).await.unwrap();
    release.wait().await;
    let error = failure_message(resolver.await.unwrap());
    assert!(!error.contains("invalid_grant") && !error.contains("reconnect"));
    fixture.assert_token(&winner).await;
}
