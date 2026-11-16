use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::store::{CredentialType, ProviderToken, ProviderTokenReplacement, Store};
use loopflow_storage::sqlite::SQLITE_WRITE_BUSY_TIMEOUT;

#[tokio::test]
async fn linear_oauth_cancelled_commit_retains_lock_until_blocking_work_settles() {
    let directory = tempfile::tempdir().unwrap();
    let sqlite =
        crate::store::opening::open_ephemeral(&directory.path().join("registry.db")).unwrap();
    let store = Arc::new(Store::from_sqlite_for_test(sqlite.clone()));
    let original = ProviderToken {
        provider: "linear".into(),
        access_token: "A1".into(),
        refresh_token: Some("R1".into()),
        oauth_client_id: Some("client".into()),
        expires_at: Some(1),
        login: None,
        updated_at: 1,
        credential_type: CredentialType::OAuth,
    };
    let replacement = ProviderToken {
        access_token: "A2".into(),
        refresh_token: Some("R2".into()),
        expires_at: Some(time::OffsetDateTime::now_utc().unix_timestamp() + 86400),
        ..original.clone()
    };
    store.upsert_provider_token(&original).await.unwrap();
    let lock_path = directory.path().join("refresh.lock");
    let lock = std::fs::File::create(&lock_path).unwrap();
    fs2::FileExt::try_lock_exclusive(&lock).unwrap();
    let observer = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&lock_path)
        .unwrap();
    // Hold the real connection mutex on a separate thread, across cancellation.
    let (held_tx, held_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let held_sqlite = sqlite.clone();
    let holder = std::thread::spawn(move || {
        let _connection = held_sqlite.conn.lock().unwrap();
        held_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    held_rx.await.unwrap();
    let deadline = Instant::now() + Duration::from_millis(50);
    let write = store.replace_provider_token(&original, &replacement, lock, deadline);
    assert!(tokio::time::timeout(Duration::from_millis(75), write)
        .await
        .is_err());
    assert!(fs2::FileExt::try_lock_exclusive(&observer).is_err());
    release_tx.send(()).unwrap();
    holder.join().unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while fs2::FileExt::try_lock_exclusive(&observer).is_err() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // The expired queued closure declined to write, and restored normal SQLite policy.
    assert!(store.get_provider_token("linear").await.unwrap().as_ref() == Some(&original));
    let busy: u32 = sqlite
        .conn
        .lock()
        .unwrap()
        .pragma_query_value(None, "busy_timeout", |r| r.get(0))
        .unwrap();
    assert_eq!(u128::from(busy), SQLITE_WRITE_BUSY_TIMEOUT.as_millis());
    let outcome = store
        .replace_provider_token(
            &original,
            &replacement,
            observer,
            Instant::now() + Duration::from_secs(2),
        )
        .await
        .unwrap();
    assert!(matches!(outcome, ProviderTokenReplacement::Replaced));
    assert!(store.get_provider_token("linear").await.unwrap().as_ref() == Some(&replacement));
}
