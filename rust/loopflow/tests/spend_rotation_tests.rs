use std::sync::{Arc, Mutex};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Router,
};
use loopflow::spend::{
    rotation::{RotationOperation, RotationReceipt, RotationState},
    CredentialId, Inventory,
};
use loopflow::store::{open_ephemeral_store, StorageConfig};

struct Provider {
    old: String,
    candidate: String,
    created: bool,
    retired: bool,
    fail_retirement: bool,
}
async fn read(State(provider): State<Arc<Mutex<Provider>>>, headers: HeaderMap) -> StatusCode {
    let provider = provider.lock().unwrap();
    let value = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if (!provider.retired && value == format!("Bearer {}", provider.old))
        || (provider.created && value == format!("Bearer {}", provider.candidate))
    {
        StatusCode::OK
    } else {
        StatusCode::UNAUTHORIZED
    }
}
async fn create(State(provider): State<Arc<Mutex<Provider>>>) -> StatusCode {
    provider.lock().unwrap().created = true;
    StatusCode::CREATED
}
async fn old_secret(State(provider): State<Arc<Mutex<Provider>>>) -> String {
    provider.lock().unwrap().old.clone()
}
async fn candidate_secret(State(provider): State<Arc<Mutex<Provider>>>) -> String {
    provider.lock().unwrap().candidate.clone()
}
async fn retire(State(provider): State<Arc<Mutex<Provider>>>) -> StatusCode {
    let mut provider = provider.lock().unwrap();
    if provider.fail_retirement {
        provider.fail_retirement = false;
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        provider.retired = true;
        StatusCode::OK
    }
}

#[tokio::test]
async fn rotation_preserves_old_access_until_every_consumer_cuts_over_and_retirement_succeeds() {
    let provider = Arc::new(Mutex::new(Provider {
        old: uuid::Uuid::new_v4().to_string(),
        candidate: uuid::Uuid::new_v4().to_string(),
        created: false,
        retired: false,
        fail_retirement: true,
    }));
    let app = Router::new()
        .route("/read", get(read))
        .route("/create", post(create))
        .route("/doppler/old", get(old_secret))
        .route("/doppler/candidate", get(candidate_secret))
        .route("/retire", post(retire))
        .with_state(provider.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();
    // The fake Doppler endpoint provisions process-local values; no value enters the Store or CLI.
    let old = client
        .get(format!("{origin}/doppler/old"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let candidate = client
        .get(format!("{origin}/doppler/candidate"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let probe = |key: String| {
        let client = client.clone();
        let origin = origin.clone();
        async move {
            client
                .get(format!("{origin}/read"))
                .bearer_auth(key)
                .send()
                .await
                .unwrap()
                .status()
                .is_success()
        }
    };
    let directory = tempfile::tempdir().unwrap();
    let config = StorageConfig::sqlite(directory.path().join("loopflow.db"));
    let store = open_ephemeral_store(&config).await.unwrap();
    let mut inventory: Inventory = serde_json::from_str(include_str!(
        "../../../tests/fixtures/dto/spend/inventory.json"
    ))
    .unwrap();
    inventory.credentials[0].version = Some("old-version".into());
    let mut replacement = inventory.credentials[0].clone();
    replacement.id = CredentialId("replacement".into());
    replacement.reference.name = "REPLACEMENT".into();
    replacement.version = Some("candidate-version".into());
    inventory.credentials.push(replacement);
    inventory.environments[0].home_id = Some(store.local_home().await.unwrap().id);
    let mut second = inventory.environments[0].clone();
    second.id = loopflow::spend::EnvironmentId("agent".into());
    // Two required consumers in one test Home still require independent cutover receipts.
    inventory.environments.push(second);
    let mut requirement = inventory.requirements[0].clone();
    requirement.id = loopflow::spend::RequirementId("agent-key".into());
    requirement.environment = inventory.environments[1].id.clone();
    inventory.requirements.push(requirement);
    store
        .import_spend_inventory(inventory.clone())
        .await
        .unwrap();
    let rotation = store
        .begin_spend_rotation(
            CredentialId("key".into()),
            CredentialId("replacement".into()),
            Some("test provider has exactly these two consumers".into()),
        )
        .await
        .unwrap();
    let receipt =
        |operation: RotationOperation, index: Option<usize>, success: bool| RotationReceipt {
            operation,
            environment: index.map(|i| inventory.environments[i].id.clone()),
            executed_home: index.and_then(|i| inventory.environments[i].home_id.clone()),
            candidate_version: "candidate-version".into(),
            success,
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            evidence: "synthetic provider HTTP status".into(),
        };
    assert!(probe(old.clone()).await);
    let failed = probe(candidate.clone()).await;
    assert!(!failed);
    store
        .record_spend_rotation(
            rotation.id.clone(),
            receipt(RotationOperation::CandidateRead, Some(0), failed),
        )
        .await
        .unwrap();
    assert!(store
        .activate_spend_rotation(rotation.id.clone())
        .await
        .is_err());
    assert!(probe(old.clone()).await);
    assert!(client
        .post(format!("{origin}/create"))
        .send()
        .await
        .unwrap()
        .status()
        .is_success());
    for i in 0..2 {
        store
            .record_spend_rotation(
                rotation.id.clone(),
                receipt(
                    RotationOperation::CandidateRead,
                    Some(i),
                    probe(candidate.clone()).await,
                ),
            )
            .await
            .unwrap();
        if i == 0 {
            assert!(store
                .activate_spend_rotation(rotation.id.clone())
                .await
                .is_err());
        }
    }
    let active = store
        .activate_spend_rotation(rotation.id.clone())
        .await
        .unwrap();
    assert_eq!(active.state, RotationState::RetirementPending);
    // Restart after activation, before the consumers acknowledge using the new reference.
    drop(store);
    let store = open_ephemeral_store(&config).await.unwrap();
    let resumed = store
        .activate_spend_rotation(rotation.id.clone())
        .await
        .unwrap();
    assert_eq!(resumed.state, RotationState::RetirementPending);
    let access = store
        .spend_access(inventory.environments[0].id.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(access.credentials[0].reference.name, "REPLACEMENT");
    store
        .record_spend_rotation(
            rotation.id.clone(),
            receipt(
                RotationOperation::ConsumerCutover,
                Some(0),
                probe(candidate.clone()).await,
            ),
        )
        .await
        .unwrap();
    assert!(probe(old.clone()).await); // The second consumer has not reloaded yet.
    assert!(store
        .record_spend_rotation(
            rotation.id.clone(),
            receipt(RotationOperation::Retirement, None, true)
        )
        .await
        .is_err());
    store
        .record_spend_rotation(
            rotation.id.clone(),
            receipt(
                RotationOperation::ConsumerCutover,
                Some(1),
                probe(candidate.clone()).await,
            ),
        )
        .await
        .unwrap();
    let failed = client
        .post(format!("{origin}/retire"))
        .send()
        .await
        .unwrap()
        .status()
        .is_success();
    assert!(!failed);
    assert_eq!(
        store
            .record_spend_rotation(
                rotation.id.clone(),
                receipt(RotationOperation::Retirement, None, failed)
            )
            .await
            .unwrap()
            .state,
        RotationState::RetirementPending
    );
    assert!(probe(old.clone()).await);
    let retired = client
        .post(format!("{origin}/retire"))
        .send()
        .await
        .unwrap()
        .status()
        .is_success();
    assert!(retired);
    let result = store
        .record_spend_rotation(
            rotation.id.clone(),
            receipt(RotationOperation::Retirement, None, retired),
        )
        .await
        .unwrap();
    assert_eq!(result.state, RotationState::Retired);
    assert!(probe(candidate.clone()).await);
    assert!(!probe(old.clone()).await);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("LF_HOME", directory.path())
        .args(["auth", "access", "rotation", "show", &rotation.id.0])
        .output()
        .unwrap();
    assert!(output.status.success());
    for bytes in [
        &output.stdout,
        &output.stderr,
        &serde_json::to_vec(&result).unwrap(),
    ] {
        assert!(!bytes.windows(old.len()).any(|w| w == old.as_bytes()));
        assert!(!bytes
            .windows(candidate.len())
            .any(|w| w == candidate.as_bytes()));
    }
    for entry in std::fs::read_dir(directory.path()).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            let bytes = std::fs::read(entry.path()).unwrap();
            assert!(!bytes.windows(old.len()).any(|w| w == old.as_bytes()));
            assert!(!bytes
                .windows(candidate.len())
                .any(|w| w == candidate.as_bytes()));
        }
    }
    server.abort();
}
