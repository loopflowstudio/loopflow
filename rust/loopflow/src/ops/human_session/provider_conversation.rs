//! A provider's own conversation id names a Session wherever a Loopflow id
//! does. A conversation Loopflow started recorded that id; one the provider
//! started alone is found in its home and admitted when first connected.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};

use super::{local_capture_dir, lock_session_exec, publish_prepared_input};
use crate::provider_account::activation::native_home;
use crate::provider_auth::Provider;
use crate::session::{AgentSession, SessionKind, TitleSource};
use crate::store::{ProviderAccountId, SharedStore};

pub(super) async fn human_input_times<'a>(
    store: &SharedStore,
    sessions: impl IntoIterator<Item = &'a AgentSession>,
) -> Result<BTreeMap<String, i64>> {
    let mut codex = BTreeMap::<PathBuf, Vec<(String, String)>>::new();
    let mut claude = BTreeMap::new();
    let mut times = BTreeMap::new();
    for session in sessions {
        let provider = match session.provider.as_deref() {
            Some("codex") => Provider::Codex,
            Some("claude") => Provider::Claude,
            _ => continue,
        };
        let Some(reference) = store.sqlite.input_provider_session(&session.artifact_key)? else {
            continue;
        };
        let id = reference.provider_session_id;
        let isolated = store.provider_session_isolated(provider, &id).await?;
        let home = if isolated == Some(true) {
            let account = match reference.account_id {
                Some(account) => Some(account),
                None => store.provider_session_account(provider, &id).await?,
            };
            let Some(account) = account else {
                continue;
            };
            let Some(home) = store
                .get_provider_account(provider.as_str(), &account)
                .await?
                .and_then(|account| account.home)
            else {
                continue;
            };
            home
        } else {
            native_home(provider, None)
        };
        let home = fs::canonicalize(&home).unwrap_or(home);
        if provider == Provider::Codex {
            codex
                .entry(home)
                .or_default()
                .push((session.id.clone(), id));
        } else {
            let at = claude.entry((home.clone(), id.clone())).or_insert_with(|| {
                transcript(provider, &home, &id).and_then(|path| claude_input_time(&path, &id))
            });
            if let Some(at) = at {
                times.insert(session.id.clone(), *at);
            }
        }
    }
    for (home, sessions) in codex {
        let wanted = sessions.iter().map(|(_, id)| id.as_str()).collect();
        let native = codex_input_times(&home.join("history.jsonl"), &wanted);
        for (session, id) in sessions {
            if let Some(at) = native.get(&id) {
                times.insert(session, *at);
            }
        }
    }
    Ok(times)
}

fn json_lines(path: &Path) -> impl Iterator<Item = serde_json::Value> {
    fs::File::open(path).ok().into_iter().flat_map(|file| {
        BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok())
    })
}

fn codex_input_times(path: &Path, wanted: &BTreeSet<&str>) -> BTreeMap<String, i64> {
    let mut times = BTreeMap::<String, i64>::new();
    for row in json_lines(path) {
        let Some(id) = row["session_id"].as_str().filter(|id| wanted.contains(id)) else {
            continue;
        };
        let Some(at) = row["ts"]
            .as_i64()
            .filter(|at| *at >= 0)
            .and_then(|at| at.checked_mul(1000))
        else {
            continue;
        };
        times
            .entry(id.to_string())
            .and_modify(|saved| *saved = (*saved).max(at))
            .or_insert(at);
    }
    times
}

fn claude_input_time(path: &Path, id: &str) -> Option<i64> {
    json_lines(path)
        .filter_map(|row| {
            if row["type"] != "user"
                || row["userType"] != "external"
                || row["sessionId"].as_str() != Some(id)
                || ["isSidechain", "isMeta", "isCompactSummary", "isSynthetic"]
                    .iter()
                    .any(|key| row[key].as_bool() == Some(true))
                || ["sourceToolAssistantUUID", "agentId", "toolUseResult"]
                    .iter()
                    .any(|key| !row[key].is_null())
            {
                return None;
            }
            let content = &row["message"]["content"];
            let texts = if let Some(text) = content.as_str() {
                vec![text]
            } else {
                content
                    .as_array()?
                    .iter()
                    .filter(|block| block["type"] == "text")
                    .filter_map(|block| block["text"].as_str())
                    .collect::<Vec<_>>()
            };
            let image = content
                .as_array()
                .is_some_and(|blocks| blocks.iter().any(|block| block["type"] == "image"));
            if !(image || texts.iter().any(|text| !text.trim().is_empty()))
                || texts.iter().any(|text| {
                    [
                        "<lf:",
                        "<task-notification>",
                        "<local-command",
                        "<command-name>",
                        "<system-reminder>",
                        "<session-start-hook>",
                        "<tick>",
                        "<goal>",
                        "<ide_opened_file>",
                        "<ide_selection>",
                        "[Request interrupted by user",
                        "This session is being continued from a previous conversation",
                    ]
                    .iter()
                    .any(|marker| text.contains(marker))
                })
            {
                return None;
            }
            let at = time::OffsetDateTime::parse(
                row["timestamp"].as_str()?,
                &time::format_description::well_known::Rfc3339,
            )
            .ok()?;
            i64::try_from(at.unix_timestamp_nanos() / 1_000_000).ok()
        })
        .max()
}

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
        .create_session(AgentSession {
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
            flow_id: None,
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
        })
        .await?;
    let session = publish_prepared_input(
        &store.sqlite,
        &session,
        crate::session_record::SessionFlowMembership::Independent,
    )?;
    let dir = local_capture_dir(&session.artifact_key)
        .ok_or_else(|| anyhow!("Session {} has an invalid capture reference", session.id))?;
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

    #[tokio::test]
    async fn recency_reads_the_recorded_account_home_without_activating_it() {
        let directory = tempfile::tempdir().unwrap();
        let store: SharedStore = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                directory.path().join("store.db"),
            ))
            .await
            .unwrap(),
        );
        let account = crate::store::ProviderAccount {
            provider: "codex".into(),
            account_id: crate::store::ProviderAccountId::parse("resume-fixture").unwrap(),
            home: Some(directory.path().join("account")),
            login_email: None,
            observed_email: None,
            observed_subject: None,
            observed_credential_digest: None,
            observed_plan: None,
            credential_state: crate::store::CredentialState::Missing,
            routing_state: crate::store::RoutingState::Disabled,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        };
        store.upsert_provider_account(&account).await.unwrap();
        store
            .pin_provider_session_route(Provider::Codex, ID, &account.account_id, true)
            .await
            .unwrap();
        let home = account.home.as_ref().unwrap();
        fs::create_dir(home).unwrap();
        fs::write(
            home.join("history.jsonl"),
            format!("{{\"session_id\":\"{ID}\",\"ts\":42}}\n"),
        )
        .unwrap();
        let mut session = store
            .sqlite
            .test_session("recorded", &crate::session_record::new_artifact_key());
        session.provider = Some("codex".into());
        store.sqlite.retain_session_observation(&session, &crate::session::SessionObservation {
            artifact_key: session.artifact_key.clone(), source: "provider-session:fixture".into(),
            observed_at: 999, task_id: None, wave_id: None,
            payload: serde_json::json!({"source":"provider-session:fixture", "evidence":{
                "schema_version":1, "provider_session_id":ID, "account_id":account.account_id,
            }}),
        }).unwrap();
        let times = super::human_input_times(&store, [&session]).await.unwrap();
        assert_eq!(times.get(&session.id), Some(&42_000));
        let retained = store
            .get_provider_account("codex", &account.account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(retained.last_selected_at, None);
        assert_eq!(retained.credential_state, account.credential_state);
        assert_eq!(retained.routing_state, account.routing_state);
    }

    #[test]
    fn input_history_preserves_send_time_and_ignores_unrelated_or_broken_rows() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("history.jsonl");
        fs::write(
            &path,
            concat!(
                "{\"session_id\":\"a\",\"ts\":20}\n",
                "broken\n",
                "{\"session_id\":\"a\",\"ts\":10}\n",
                "{\"session_id\":\"unrelated\",\"ts\":99}\n",
                "{\"session_id\":\"a\",\"ts\":-1}\n",
                "{\"session_id\":\"a\",\"ts\":9223372036854775807}\n",
                "{\"session_id\":\"a\",\"ts\":"
            ),
        )
        .unwrap();
        assert_eq!(
            super::codex_input_times(&path, &std::collections::BTreeSet::from(["a"])),
            std::collections::BTreeMap::from([("a".into(), 20_000)])
        );
        assert!(super::codex_input_times(
            &home.path().join("missing"),
            &std::collections::BTreeSet::from(["a"])
        )
        .is_empty());
    }

    #[test]
    fn claude_recency_requires_main_conversation_human_input() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("claude.jsonl");
        let human = serde_json::json!({"type":"user", "userType":"external", "sessionId":ID,
            "timestamp":"2026-01-01T00:00:00.123Z", "message":{"content":"Continue here"}});
        let mut rows = vec![human.clone()];
        for (field, value) in [
            ("type", serde_json::json!("assistant")),
            ("userType", serde_json::Value::Null),
            ("sessionId", serde_json::json!("another")),
            ("isSidechain", serde_json::json!(true)),
            ("isMeta", serde_json::json!(true)),
            ("isCompactSummary", serde_json::json!(true)),
            ("isSynthetic", serde_json::json!(true)),
            ("sourceToolAssistantUUID", serde_json::json!("agent")),
            (
                "message",
                serde_json::json!({"content":[{"type":"tool_result","content":"done"}]}),
            ),
            (
                "message",
                serde_json::json!({"content":"<lf:steers>injected</lf:steers>"}),
            ),
            (
                "message",
                serde_json::json!({"content":"<task-notification>done</task-notification>"}),
            ),
            (
                "message",
                serde_json::json!({"content":"[Request interrupted by user]"}),
            ),
            (
                "message",
                serde_json::json!({"content":"<session-start-hook>injected</session-start-hook>"}),
            ),
        ] {
            let mut row = human.clone();
            row["timestamp"] = serde_json::json!("2026-01-02T00:00:00Z");
            row[field] = value;
            rows.push(row);
        }
        fs::write(
            &path,
            rows.iter()
                .map(|row| format!("{row}\n"))
                .collect::<String>(),
        )
        .unwrap();
        assert_eq!(super::claude_input_time(&path, ID), Some(1_767_225_600_123));
        let mut image = human;
        image["message"]["content"] =
            serde_json::json!([{"type":"image","source":{"type":"base64","data":"fixture"}}]);
        fs::write(&path, image.to_string()).unwrap();
        assert_eq!(super::claude_input_time(&path, ID), Some(1_767_225_600_123));
    }

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
