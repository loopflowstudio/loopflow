//! A Session's conversation: which provider home holds it, what it answered
//! and how long it is kept. `loopflow_harness::transcript` reads the text;
//! Loopflow stores none of it.

pub mod answer;
pub mod facts;
pub mod live;
pub mod raw;
mod session;
pub mod summary;
pub mod watch;

pub use session::threads;
pub use session::{
    session_transcript, Conversation, Retention, RetentionState, SessionTranscript, ThreadFacts,
    TurnFacts,
};

use std::path::{Path, PathBuf};

use anyhow::Result;
use loopflow_harness::transcript::{copy_between, transcript, Source};
use loopflow_harness::HarnessKind;
use loopflow_model::id::AgentSessionId;
use loopflow_storage::sqlite::SqliteStore;

use crate::provider_auth::Provider;

/// The provider's shared home on this Machine.
pub(crate) fn native_home(provider: Provider) -> PathBuf {
    let harness = crate::harness::from_provider(provider).expect("native provider has a harness");
    harness.harness().transcript_home()
}

/// Bring an older isolated transcript into the native home without touching auth.
pub(crate) fn adopt_native(provider: Provider, id: &AgentSessionId) -> Result<()> {
    let store = crate::store::open_sqlite(&crate::store::database_path_from_env()?)?;
    let to = native_home(provider);
    let from = conversation_home(&store, provider, id)?;
    if from.as_deref() == Some(to.as_path()) {
        return Ok(());
    }
    let harness = crate::harness::from_provider(provider).expect("native provider has a harness");
    if transcript(harness, &to, id.as_str()).is_none() {
        let copied = from
            .as_ref()
            .map(|from| copy_between(harness, id.as_str(), from, &to))
            .transpose()?
            .flatten();
        anyhow::ensure!(
            copied.is_some(),
            "cannot resume {provider} conversation {id}: its transcript is missing"
        );
    }
    store.record_native_conversation_home(provider, id)?;
    Ok(())
}

/// Whether `provider` can no longer resume conversation `id`: neither its
/// native home nor the home it ran in holds the transcript, as after a crash
/// that took the provider down before it wrote one. Only Claude and Codex
/// keep transcripts lf finds; another provider's thread is never lost.
pub fn thread_lost(store: &SqliteStore, provider: &str, id: &AgentSessionId) -> Result<bool> {
    let provider = match provider {
        "claude" => Provider::Claude,
        "codex" => Provider::Codex,
        _ => return Ok(false),
    };
    let harness = HarnessKind::parse(provider.as_str()).expect("native provider has a harness");
    if transcript(harness, &native_home(provider), id.as_str()).is_some() {
        return Ok(false);
    }
    Ok(conversation_home(store, provider, id)?
        .and_then(|home| transcript(harness, &home, id.as_str()))
        .is_none())
}

/// Why a recorded conversation has no readable transcript here.
#[derive(Debug, Clone, PartialEq)]
pub enum Gap {
    /// The account home it ran in, before every launch shared one, no longer
    /// exists.
    AccountHomeRemoved,
    /// The home is here and the file is not.
    FileMissing { home: PathBuf },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Located {
    /// `home` is the provider home whose settings decide how long it stays.
    Found {
        source: Source,
        home: Option<PathBuf>,
    },
    Gone(Gap),
}

/// The home that holds a recorded conversation: the provider's shared one,
/// unless it ran in an account's own home and has not resumed since. `None`
/// when that account is gone.
pub(crate) fn conversation_home(
    store: &SqliteStore,
    provider: Provider,
    id: &AgentSessionId,
) -> Result<Option<PathBuf>> {
    let Some(account) = store.provider_session_account_home(provider, id)? else {
        return Ok(Some(native_home(provider)));
    };
    Ok(store
        .provider_account_row(provider.as_str(), &account)?
        .and_then(|account| account.home))
}

/// Find the transcript of conversation `id`, which ran in `cwd`.
pub fn locate(
    store: &SqliteStore,
    harness: HarnessKind,
    id: &AgentSessionId,
    cwd: &Path,
) -> Result<Located> {
    crate::harness::launcher(harness).locate_transcript(store, id, cwd)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const ID: &str = "0199a213-81c0-7800-8aa1-bbab2a035a53";

    #[test]
    fn old_account_transcripts_resume_with_native_auth_without_copying_credentials() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let store = crate::store::open_sqlite(&ledger.home().join("loopflow.db")).unwrap();
        for provider in [Provider::Claude, Provider::Codex] {
            let native = ledger.home().join(format!("native-{provider}"));
            let old = ledger.home().join(format!("old-{provider}"));
            fs::create_dir_all(&native).unwrap();
            fs::create_dir_all(&old).unwrap();
            let _env = crate::config::environment::EnvVarGuard::set(
                if provider == Provider::Claude {
                    "CLAUDE_CONFIG_DIR"
                } else {
                    "CODEX_HOME"
                },
                native.to_str().unwrap(),
            );
            let credential = if provider == Provider::Claude {
                ".credentials.json"
            } else {
                "auth.json"
            };
            fs::write(native.join(credential), "native-A").unwrap();
            fs::write(old.join(credential), "saved-B").unwrap();
            let db = rusqlite::Connection::open(ledger.home().join("loopflow.db")).unwrap();
            db.execute("INSERT INTO provider_accounts(provider,account_id,home,credential_state,created_at,updated_at) VALUES(?1,'old',?2,'missing',1,1)",
                rusqlite::params![provider.as_str(), old.to_str().unwrap()]).unwrap();
            store.record_account_home_conversation(provider, ID, "old");
            let relative = if provider == Provider::Claude {
                format!("projects/-work-repo/{ID}.jsonl")
            } else {
                format!("sessions/2026/11/15/rollout-2026-11-15T10-00-00-{ID}.jsonl")
            };
            let lost = |id: &str| thread_lost(&store, provider.as_str(), &id.into()).unwrap();
            assert!(lost(ID), "no home holds it yet");
            let source = old.join(&relative);
            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(
                &source,
                format!(r#"{{"type":"session_meta","payload":{{"id":"{ID}"}}}}"#),
            )
            .unwrap();
            assert!(!lost(ID), "the home it ran in holds it");
            adopt_native(provider, &ID.into()).unwrap();
            assert!(!lost(ID), "the native home holds it");
            assert_eq!(
                fs::read(native.join(&relative)).unwrap(),
                fs::read(&source).unwrap()
            );
            assert_eq!(
                fs::read_to_string(native.join(credential)).unwrap(),
                "native-A"
            );
            assert_eq!(fs::read_to_string(old.join(credential)).unwrap(), "saved-B");
            assert_eq!(
                conversation_home(&store, provider, &ID.into()).unwrap(),
                Some(native.clone())
            );
            assert_eq!(
                store
                    .provider_session_account(provider, &ID.into())
                    .unwrap()
                    .unwrap()
                    .as_str(),
                "old"
            );
            // Existing native history wins even when the old copy changes later.
            db.execute("UPDATE provider_session_accounts SET isolated=1 WHERE provider=?1 AND provider_session_id=?2", rusqlite::params![provider.as_str(), ID]).unwrap();
            fs::write(&source, "older-home-change").unwrap();
            let retained = fs::read(native.join(&relative)).unwrap();
            adopt_native(provider, &ID.into()).unwrap();
            assert_eq!(fs::read(native.join(&relative)).unwrap(), retained);
            let missing = "0199a213-81c0-7800-8aa1-bbab2a035a54";
            store.record_account_home_conversation(provider, missing, "old");
            assert!(lost(missing));
            assert!(!thread_lost(&store, "pi", &missing.into()).unwrap());
            assert!(adopt_native(provider, &missing.into())
                .unwrap_err()
                .to_string()
                .contains("transcript is missing"));
        }
    }
}
