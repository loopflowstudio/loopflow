//! A provider's own conversation id names a Session wherever a Loopflow id
//! does. A conversation Loopflow started recorded that id; one the provider
//! started alone is found in its home and admitted when first connected.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};

use super::{local_session_run_dir, lock_session_exec, publish_prepared_input};
use crate::provider_account::activation::native_home;
use crate::provider_auth::Provider;
use crate::session::{AgentSession, SessionKind, TitleSource};
use crate::store::{ProviderAccountId, SharedStore};

/// The Session that recorded `id` as its provider conversation.
pub(crate) async fn recorded(store: &SharedStore, id: &str) -> Result<Option<AgentSession>> {
    let sessions = store.sqlite.sessions_for_provider_thread(id)?;
    match sessions.as_slice() {
        [] => Ok(None),
        [session] => Ok(store.session(session).await?),
        sessions => bail!(
            "Provider conversation {id} is ambiguous: Sessions {}",
            sessions.join(", ")
        ),
    }
}

/// A conversation the provider can resume, in the home that holds it.
struct NativeConversation {
    provider: Provider,
    home: PathBuf,
    /// The stored account whose own home this is; absent in a native home.
    account: Option<ProviderAccountId>,
    cwd: Option<PathBuf>,
}

/// Admit the conversation a provider started on its own, attributed to no
/// Task. `None` when no provider home holds a conversation with this id.
pub(crate) async fn admit(store: &SharedStore, id: &str) -> Result<Option<AgentSession>> {
    // Provider conversation ids are UUIDs; nothing else names a transcript.
    if uuid::Uuid::parse_str(id).is_err() {
        return Ok(None);
    }
    let lock_id = format!("provider-conversation:{id}");
    let _admission = tokio::task::spawn_blocking(move || lock_session_exec(&lock_id)).await??;
    if let Some(session) = recorded(store, id).await? {
        return Ok(Some(session));
    }
    let found = find(store, id).await?;
    let conversation = match found.as_slice() {
        [] => return Ok(None),
        [conversation] => conversation,
        found => bail!(
            "Provider conversation {id} is ambiguous: {}",
            found
                .iter()
                .map(|found| format!("{} in {}", found.provider, found.home.display()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    let cwd = match conversation.cwd.clone().filter(|cwd| cwd.is_dir()) {
        Some(cwd) => cwd,
        None => crate::repo::working_directory()?,
    };
    let session = store
        .create_session(
            AgentSession {
                captured: None,
                id: format!("session_{}", uuid::Uuid::new_v4().simple()),
                artifact_key: crate::session_record::new_artifact_key(),
                caller_artifact_key: None,
                input_published: false,
                cwd,
                skill: None,
                provider: Some(conversation.provider.as_str().to_string()),
                model: None,
                node: None,
                iterations: None,
                task_id: None,
                wave_id: None,
                flow_session_id: None,
                work_source: None,
                bound_at: None,
                kind: SessionKind::Conversation,
                interactive: true,
                repo: None,
                title: format!("{} {}", conversation.provider, &id[..8]),
                title_source: TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: crate::store::rows::now_unix(),
            },
            None,
        )
        .await?;
    let session = publish_prepared_input(
        store,
        &session,
        crate::session_record::SessionFlowMembership::Independent,
    )?;
    let dir = local_session_run_dir(&session.artifact_key)
        .ok_or_else(|| anyhow!("Session {} has an invalid Run reference", session.id))?;
    crate::session_record::write_provider_session(&dir, id, conversation.account.clone())?;
    if let Some(account) = &conversation.account {
        // It resumes in the account home that holds its history.
        store
            .pin_provider_session_route(conversation.provider, id, account, true)
            .await?;
    }
    Ok(Some(session))
}

/// Every provider home holding a conversation with this id: each provider's
/// native home and each stored account's own.
async fn find(store: &SharedStore, id: &str) -> Result<Vec<NativeConversation>> {
    let accounts = store.list_provider_accounts(None).await?;
    let mut found: Vec<NativeConversation> = Vec::new();
    for provider in [Provider::Codex, Provider::Claude] {
        let stored = accounts
            .iter()
            .filter(|account| account.provider == provider.as_str())
            .filter_map(|account| Some((account.home.clone()?, Some(account.account_id.clone()))));
        for (home, account) in std::iter::once((native_home(provider, None), None)).chain(stored) {
            let home = fs::canonicalize(&home).unwrap_or(home);
            if found
                .iter()
                .any(|seen| seen.provider == provider && seen.home == home)
            {
                continue;
            }
            if let Some(transcript) = transcript(provider, &home, id) {
                found.push(NativeConversation {
                    provider,
                    cwd: recorded_cwd(&transcript),
                    home,
                    account,
                });
            }
        }
    }
    Ok(found)
}

/// Codex keeps `sessions/<year>/<month>/<day>/rollout-<time>-<id>.jsonl`;
/// Claude keeps `projects/<directory>/<id>.jsonl`.
fn transcript(provider: Provider, home: &Path, id: &str) -> Option<PathBuf> {
    match provider {
        Provider::Codex => {
            let suffix = format!("-{id}.jsonl");
            find_file(&home.join("sessions"), 3, &|name| {
                name.starts_with("rollout-") && name.ends_with(&suffix)
            })
        }
        _ => {
            let name = format!("{id}.jsonl");
            find_file(&home.join("projects"), 1, &|found| found == name)
        }
    }
}

fn find_file(directory: &Path, depth: usize, matches: &dyn Fn(&str) -> bool) -> Option<PathBuf> {
    fs::read_dir(directory).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        if depth > 0 {
            return find_file(&path, depth - 1, matches);
        }
        matches(entry.file_name().to_str()?).then_some(path)
    })
}

/// The directory the conversation ran in, from the head of its transcript.
fn recorded_cwd(transcript: &Path) -> Option<PathBuf> {
    BufReader::new(fs::File::open(transcript).ok()?)
        .lines()
        .take(32)
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(&line).ok())
        .find_map(|entry| {
            entry["cwd"]
                .as_str()
                .or_else(|| entry["payload"]["cwd"].as_str())
                .map(PathBuf::from)
        })
}

#[cfg(test)]
mod tests {
    use super::{recorded, recorded_cwd, transcript};
    use crate::provider_auth::Provider;
    use crate::session::AgentSession;
    use crate::store::SharedStore;
    use std::fs;
    use std::path::PathBuf;

    const ID: &str = "0199a213-81c0-7800-8aa1-bbab2a035a53";

    #[test]
    fn a_codex_rollout_is_found_by_its_conversation_id_with_its_directory() {
        let home = tempfile::tempdir().unwrap();
        let day = home.path().join("sessions/2026/10/02");
        fs::create_dir_all(&day).unwrap();
        fs::write(
            day.join(format!("rollout-2026-10-02T10-00-00-{ID}.jsonl")),
            r#"{"type":"session_meta","payload":{"id":"x","cwd":"/work/repo"}}"#,
        )
        .unwrap();

        let found = transcript(Provider::Codex, home.path(), ID).unwrap();
        assert_eq!(recorded_cwd(&found), Some(PathBuf::from("/work/repo")));
        assert!(transcript(Provider::Codex, home.path(), &ID.replace('3', "4")).is_none());
        assert!(transcript(Provider::Claude, home.path(), ID).is_none());
    }

    #[tokio::test]
    async fn a_provider_conversation_id_names_the_session_that_recorded_it() {
        let directory = tempfile::tempdir().unwrap();
        let store: SharedStore = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                directory.path().join("loopflow.db"),
            ))
            .await
            .unwrap(),
        );
        let input = crate::session_record::new_artifact_key();
        let session = store.sqlite.test_session("conversation", &input);
        let observe = |session: &AgentSession, source: &str| {
            store
                .sqlite
                .retain_session_observation(
                    session,
                    &crate::session::SessionObservation {
                        artifact_key: session.artifact_key.clone(),
                        source: source.into(),
                        observed_at: 1,
                        task_id: None,
                        wave_id: None,
                        payload: serde_json::json!({
                            "input_id": session.artifact_key,
                            "source": source,
                            "evidence": {"schema_version": 1, "provider_session_id": ID, "account_id": null},
                        }),
                    },
                )
                .unwrap();
        };
        assert!(recorded(&store, ID).await.unwrap().is_none());

        observe(&session, "provider-session:one");
        assert_eq!(recorded(&store, ID).await.unwrap().unwrap().id, session.id);
        let by_id = super::super::session_by_id(&store, ID).await.unwrap();
        assert_eq!(by_id.unwrap().id, session.id);

        let other = store
            .sqlite
            .test_session("other", &crate::session_record::new_artifact_key());
        observe(&other, "provider-session:two");
        let error = recorded(&store, ID).await.unwrap_err().to_string();
        assert!(
            error.contains("ambiguous")
                && error.contains("conversation")
                && error.contains("other")
        );
    }

    #[test]
    fn a_claude_transcript_is_found_by_its_conversation_id() {
        let home = tempfile::tempdir().unwrap();
        let project = home.path().join("projects/-work-repo");
        fs::create_dir_all(&project).unwrap();
        fs::write(
            project.join(format!("{ID}.jsonl")),
            "{\"type\":\"summary\"}\n{\"type\":\"user\",\"cwd\":\"/work/repo\"}\n",
        )
        .unwrap();

        let found = transcript(Provider::Claude, home.path(), ID).unwrap();
        assert_eq!(recorded_cwd(&found), Some(PathBuf::from("/work/repo")));
    }
}
