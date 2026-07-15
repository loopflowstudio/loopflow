//! W2-151: every `lf` command must resolve the ambient Wave the same way. The
//! bug was never that a resident exports `LF_WAVE_ID=<uuid>` — that is correct.
//! It was that consumers disagreed about how to read it: `status` handled a
//! UUID, `pm show` ignored the env entirely, others silently dropped a hand-set
//! name. This drives the ONE resolver directly across the seven environments,
//! then proves `lf status` and `lf pm show` — the original reproduction — agree
//! end to end from a resident wave's environment.

use std::path::Path;
use std::process::Command;

use loopflow::engine::wave_context::{resolve_managed_wave_name, WaveResolveError};
use loopflow::id::WaveId;
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{open_store, PmSnapshotRow, StorageConfig};
use loopflow::wave::Wave;

/// The command matrix at the resolver itself: one durable Wave, driven from
/// every ambient environment. Each cell resolves the SAME wave name or returns
/// the SAME classified error — the whole contract in one place.
#[tokio::test]
async fn resolver_matrix_agrees_across_every_environment() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let store = open_store(&StorageConfig::sqlite(tmp.path().join("loopflow.db")))
        .await
        .expect("open store");
    let wave = Wave::new(
        WaveId::new(),
        "product".to_string(),
        tmp.path().join("repo").display().to_string(),
    );
    store.create_wave(&wave).await.expect("register wave");
    let uuid = wave.id().as_str();

    // Wave / Project / Task processes all inherit the same durable UUID — the
    // Project/Task session ids never touch wave identity, so one cell covers
    // all three. UUID first: mapped to its registry name.
    assert_eq!(
        resolve_managed_wave_name(Some(&store), None, Some(uuid))
            .await
            .expect("uuid resolves"),
        "product"
    );

    // Hand-set name: the intentional fallback. `LF_WAVE_ID=product` resolves
    // even though it is not a UUID.
    assert_eq!(
        resolve_managed_wave_name(Some(&store), None, Some("product"))
            .await
            .expect("hand-set name resolves"),
        "product"
    );

    // Explicit `--wave` always wins, even over a (wrong) ambient id.
    assert_eq!(
        resolve_managed_wave_name(Some(&store), Some("product"), Some("something-else"))
            .await
            .expect("explicit wins"),
        "product"
    );

    // Stale identity: a real UUID the registry has never seen. Not silently
    // re-read as a name — a distinct, classified error.
    let stale = WaveId::new();
    assert_eq!(
        resolve_managed_wave_name(Some(&store), None, Some(stale.as_str())).await,
        Err(WaveResolveError::StaleIdentity(stale.as_str().to_string()))
    );

    // No context: neither `--wave` nor `LF_WAVE_ID` (empty counts as absent).
    assert_eq!(
        resolve_managed_wave_name(Some(&store), None, None).await,
        Err(WaveResolveError::NoContext)
    );
    assert_eq!(
        resolve_managed_wave_name(Some(&store), None, Some("   ")).await,
        Err(WaveResolveError::NoContext)
    );

    // An empty `--wave` is an explicit-but-unusable request, not "no context".
    assert!(matches!(
        resolve_managed_wave_name(Some(&store), Some("  "), None).await,
        Err(WaveResolveError::UnknownExplicit(_))
    ));

    // A hand-set name for a wave with no registry row still resolves to that
    // name — membership is each consumer's concern (PM keys files/snapshots by
    // name; status then reports it has no row). The resolver stays consistent.
    assert_eq!(
        resolve_managed_wave_name(Some(&store), None, Some("ghost"))
            .await
            .expect("unregistered name resolves to itself"),
        "ghost"
    );
}

/// Seed a machine home with one PM-linked wave and a fresh cache-only snapshot,
/// plus a separate repo directory the commands run inside.
fn seed(home: &Path, repo: &Path, wave_name: &str) -> Wave {
    std::fs::create_dir_all(home).expect("home");
    std::fs::create_dir_all(repo).expect("repo");
    let store = SqliteStore::new(&home.join("loopflow.db")).expect("open store");
    let wave = Wave::new(
        WaveId::new(),
        wave_name.to_string(),
        repo.display().to_string(),
    );
    store.create_wave(&wave).expect("register wave");
    let repo_key = std::fs::canonicalize(repo)
        .expect("canonicalize repo")
        .display()
        .to_string();
    store
        .put_pm_snapshot(&PmSnapshotRow {
            repo: repo_key,
            wave: wave_name.to_string(),
            provider: "linear".to_string(),
            initiative: "initiative-1".to_string(),
            synced_at: chrono::Utc::now().timestamp(),
            payload: r#"{"projects":[],"items":[]}"#.to_string(),
        })
        .expect("seed pm snapshot");
    wave
}

fn lf(home: &Path, repo: &Path, args: &[&str], wave_id: Option<&str>) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_RUN_ID")
        .env_remove("LF_CHANNEL")
        .env_remove("LF_WAVE_ID");
    if let Some(id) = wave_id {
        command.env("LF_WAVE_ID", id);
    }
    command.output().expect("lf runs")
}

fn wave_field(output: &std::process::Output) -> String {
    assert!(
        output.status.success(),
        "command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    // `status` nests the wave; `pm show` names it at the top level.
    json.get("wave")
        .and_then(|wave| wave.get("name").or(Some(wave)))
        .and_then(serde_json::Value::as_str)
        .expect("a wave name")
        .to_string()
}

/// The original reproduction: from a resident wave's environment
/// (`LF_WAVE_ID=<uuid>`, no `--wave`), both `lf pm show` and `lf status` resolve
/// the same wave. The Mac Project Session inherits the identical `LF_WAVE_ID`,
/// so this cell stands for both.
#[test]
fn pm_show_and_status_agree_from_a_resident_uuid() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let repo = tmp.path().join("repo");
    let wave = seed(&home, &repo, "product");
    let uuid = wave.id().as_str();

    let pm = lf(
        &home,
        &repo,
        &["pm", "show", "--no-sync", "--json"],
        Some(uuid),
    );
    let status = lf(&home, &repo, &["status", "--json"], Some(uuid));

    assert_eq!(wave_field(&pm), "product");
    assert_eq!(wave_field(&status), "product");
}

/// Explicit `--wave` beats a wrong ambient id in both commands; a hand-set name
/// resolves; missing and stale contexts are classified errors.
#[test]
fn pm_show_honors_the_shared_resolution_rules() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let repo = tmp.path().join("repo");
    let wave = seed(&home, &repo, "product");
    let uuid = wave.id().as_str();

    // Explicit override wins over a wrong ambient UUID.
    let overridden = lf(
        &home,
        &repo,
        &["pm", "show", "--wave", "product", "--no-sync", "--json"],
        Some(&WaveId::new().to_string()),
    );
    assert_eq!(wave_field(&overridden), "product");

    // Hand-set name resolves the same wave.
    let named = lf(
        &home,
        &repo,
        &["pm", "show", "--no-sync", "--json"],
        Some("product"),
    );
    assert_eq!(wave_field(&named), "product");

    // No context: the classified "pass --wave" error, not a UUID-as-name crash.
    let missing = lf(&home, &repo, &["pm", "show", "--no-sync", "--json"], None);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("determine wave"),
        "missing-context stderr: {}",
        String::from_utf8_lossy(&missing.stderr)
    );

    // Stale identity: a real UUID with no registry row is a distinct error that
    // names the stale id, never a silent fallback.
    let stale_id = WaveId::new().to_string();
    let stale = lf(
        &home,
        &repo,
        &["pm", "show", "--no-sync", "--json"],
        Some(&stale_id),
    );
    assert!(!stale.status.success());
    let stale_err = String::from_utf8_lossy(&stale.stderr);
    assert!(
        stale_err.contains("stale") && stale_err.contains(&stale_id),
        "stale stderr: {stale_err}"
    );
    let _ = uuid;
}
