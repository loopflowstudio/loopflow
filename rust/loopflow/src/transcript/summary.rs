//! `lf history retention`: how much of your Session history the providers
//! still keep, and what they remove next. One SQL pass over Sessions and one
//! directory walk per provider home; nothing is stored.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{conversation_home, scan, ExpiryPolicy, Indexed, RetentionState};
use crate::harness::HarnessKind;
use crate::id::AgentSessionId;
use crate::provider_auth::Provider;
use crate::store::sqlite::SqliteStore;
use crate::store::ProviderAccountId;

const WEEK: i64 = 7 * 86_400;
const NEXT_EXPIRIES: usize = 20;

/// A Session as the summary reads it.
#[derive(Debug, Clone)]
pub struct RetainedSession {
    pub id: String,
    pub title: String,
    pub harness: String,
    pub thread: Option<AgentSessionId>,
    pub cwd: PathBuf,
    /// Its last recorded activity, Unix seconds.
    pub last_at: i64,
    pub account: Option<ProviderAccountId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetentionSummary {
    pub generated_at: i64,
    pub sessions: u64,
    pub harnesses: Vec<HarnessRetention>,
    pub groups: Vec<RetentionGroup>,
    /// The soonest removals still ahead.
    pub next_expiries: Vec<Expiring>,
    pub expiring_within_week: u64,
    pub homes: Vec<HomeRetention>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarnessRetention {
    pub harness: String,
    pub provider: u64,
    pub facts: u64,
    pub unknown: u64,
    pub next_expiry: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetentionGroup {
    pub harness: String,
    pub account: Option<ProviderAccountId>,
    pub age: AgeBucket,
    pub state: RetentionState,
    pub sessions: u64,
}

/// Time since a Session's transcript was last written, or its last activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgeBucket {
    UnderWeek,
    UnderMonth,
    UnderQuarter,
    Older,
}

impl AgeBucket {
    fn of(age: i64) -> Self {
        match age / 86_400 {
            ..7 => Self::UnderWeek,
            7..30 => Self::UnderMonth,
            30..90 => Self::UnderQuarter,
            _ => Self::Older,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::UnderWeek => "<7d",
            Self::UnderMonth => "7–30d",
            Self::UnderQuarter => "30–90d",
            Self::Older => ">90d",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expiring {
    pub session_id: String,
    pub title: String,
    pub harness: String,
    pub account: Option<ProviderAccountId>,
    pub after: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HomeRetention {
    pub harness: String,
    pub home: String,
    pub policy: ExpiryPolicy,
    /// Transcript files in the home, whether or not a Session recorded them.
    pub transcripts: u64,
}

struct Home {
    index: BTreeMap<String, Indexed>,
    policy: ExpiryPolicy,
}

pub fn summarize(store: &SqliteStore, now: i64) -> Result<RetentionSummary> {
    let mut homes: BTreeMap<(&'static str, PathBuf), Home> = BTreeMap::new();
    let mut groups: BTreeMap<(String, Option<ProviderAccountId>, AgeBucket, RetentionState), u64> =
        BTreeMap::new();
    let mut expiring = Vec::new();
    let sessions = store.retention_sessions()?;
    for session in &sessions {
        let Some(harness) = HarnessKind::parse(&session.harness) else {
            continue;
        };
        let (state, modified, after) = match (harness, &session.thread) {
            (_, None) => (RetentionState::Facts, None, None),
            (HarnessKind::OpenCode, Some(_)) => (RetentionState::Unknown, None, None),
            (HarnessKind::Pi, Some(thread)) => match super::modified(thread.as_str().as_ref()) {
                Some(modified) => (RetentionState::Provider, Some(modified), None),
                None => (RetentionState::Facts, None, None),
            },
            (HarnessKind::Claude | HarnessKind::Codex, Some(thread)) => {
                let provider = match harness {
                    HarnessKind::Claude => Provider::Claude,
                    _ => Provider::Codex,
                };
                match conversation_home(store, provider, thread, session.account.as_ref())? {
                    None => (RetentionState::Facts, None, None),
                    Some(home) => {
                        let home = homes
                            .entry((harness.as_str(), home.clone()))
                            .or_insert_with(|| Home {
                                index: scan(harness, &home),
                                policy: ExpiryPolicy::read(harness, Some(&home)),
                            });
                        match home.index.get(thread.as_str()) {
                            Some(found) => (
                                RetentionState::Provider,
                                Some(found.modified),
                                home.policy.after(found.modified),
                            ),
                            None => (RetentionState::Facts, None, None),
                        }
                    }
                }
            }
        };
        let age = AgeBucket::of(now - modified.unwrap_or(session.last_at));
        *groups
            .entry((
                harness.as_str().to_owned(),
                session.account.clone(),
                age,
                state,
            ))
            .or_default() += 1;
        if let Some(after) = after.filter(|after| *after >= now) {
            expiring.push(Expiring {
                session_id: session.id.clone(),
                title: session.title.clone(),
                harness: harness.as_str().to_owned(),
                account: session.account.clone(),
                after,
            });
        }
    }
    expiring.sort_by(|a, b| (a.after, &a.session_id).cmp(&(b.after, &b.session_id)));
    let mut harnesses: BTreeMap<String, HarnessRetention> = BTreeMap::new();
    for ((harness, _, _, state), count) in &groups {
        let entry = harnesses
            .entry(harness.clone())
            .or_insert_with(|| HarnessRetention {
                harness: harness.clone(),
                provider: 0,
                facts: 0,
                unknown: 0,
                next_expiry: None,
            });
        match state {
            RetentionState::Provider => entry.provider += count,
            RetentionState::Facts => entry.facts += count,
            RetentionState::Unknown => entry.unknown += count,
        }
    }
    for soonest in &expiring {
        let entry = harnesses
            .get_mut(&soonest.harness)
            .expect("an expiring Session was counted");
        entry.next_expiry.get_or_insert(soonest.after);
    }
    Ok(RetentionSummary {
        generated_at: now,
        sessions: groups.values().sum(),
        harnesses: harnesses.into_values().collect(),
        groups: groups
            .into_iter()
            .map(
                |((harness, account, age, state), sessions)| RetentionGroup {
                    harness,
                    account,
                    age,
                    state,
                    sessions,
                },
            )
            .collect(),
        expiring_within_week: expiring.iter().filter(|e| e.after < now + WEEK).count() as u64,
        next_expiries: expiring.into_iter().take(NEXT_EXPIRIES).collect(),
        homes: homes
            .into_iter()
            .map(|((harness, home), found)| HomeRetention {
                harness: harness.to_owned(),
                home: home.display().to_string(),
                policy: found.policy,
                transcripts: found.index.len() as u64,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionEventKind;
    use crate::transcript::session_transcript;
    use std::fs;
    use std::time::{Duration, SystemTime};

    const NOW: i64 = 1_793_000_000;
    const DAY: i64 = 86_400;
    const READABLE: &str = "0199a213-81c0-4800-8aa1-bbab2a035a01";
    const ROLLOUT: &str = "0199a213-81c0-7800-8aa1-bbab2a035a02";
    const GONE: &str = "0199a213-81c0-4800-8aa1-bbab2a035a03";

    fn account(store: &SqliteStore, provider: &str, name: &str, home: &std::path::Path) {
        store
            .upsert_provider_account(&crate::store::ProviderAccount {
                provider: provider.into(),
                account_id: ProviderAccountId::parse(name).unwrap(),
                home: Some(home.to_path_buf()),
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
            })
            .unwrap();
    }

    /// A Session on `provider` whose conversation `thread` ran on its account
    /// `name`, with one settled turn at `at`.
    fn session(store: &SqliteStore, id: &str, provider: &str, thread: Option<&str>, at: i64) {
        store.test_session(id, &crate::session_record::new_artifact_key());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET provider=?2,provider_thread=?3,created_at=?4 WHERE id=?1",
                rusqlite::params![id, provider, thread, at],
            )
            .unwrap();
        let Some(thread) = thread else { return };
        let route = match provider {
            "claude" => Provider::Claude,
            _ => Provider::Codex,
        };
        store
            .pin_provider_session_route(
                route,
                &thread.into(),
                &ProviderAccountId::parse(provider).unwrap(),
                true,
            )
            .unwrap();
        for (kind, payload) in [
            (SessionEventKind::Started, serde_json::json!({})),
            (
                SessionEventKind::Output,
                serde_json::json!({"text":"Shipped the fix."}),
            ),
            (
                SessionEventKind::Completed,
                serde_json::json!({"status":"completed"}),
            ),
        ] {
            store
                .record_session_event(id, &thread.into(), "turn-1", kind, &payload)
                .unwrap();
        }
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE session_events SET observed_at=?2 WHERE session_id=?1 AND kind<>'captured'",
                rusqlite::params![id, at],
            )
            .unwrap();
    }

    fn written(path: &std::path::Path, body: &str, at: i64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(at as u64))
            .unwrap();
    }

    #[test]
    fn retention_counts_what_providers_keep_and_explains_what_is_gone() {
        let root = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&root.path().join("store.db")).unwrap();
        let claude = root.path().join("claude");
        let codex = root.path().join("codex");
        account(&store, "claude", "claude", &claude);
        account(&store, "codex", "codex", &codex);
        session(&store, "readable", "claude", Some(READABLE), NOW - 10 * DAY);
        session(&store, "rollout", "codex", Some(ROLLOUT), NOW - 2 * DAY);
        session(&store, "gone", "claude", Some(GONE), NOW - 40 * DAY);
        session(&store, "threadless", "claude", None, NOW - DAY);
        let transcript = claude.join(format!("projects/-repo/{READABLE}.jsonl"));
        written(
            &transcript,
            &serde_json::json!({"type":"user","userType":"external",
                "timestamp":"2026-10-16T10:00:00Z","message":{"content":"Fix the parser"}})
            .to_string(),
            NOW - 10 * DAY,
        );
        written(
            &codex.join(format!(
                "sessions/2026/10/24/rollout-2026-10-24T10-00-00-{ROLLOUT}.jsonl"
            )),
            &serde_json::json!({"type":"response_item","payload":{"type":"message",
                "role":"assistant","content":[{"type":"output_text","text":"Rollout answer"}]}})
            .to_string(),
            NOW - 2 * DAY,
        );
        fs::create_dir_all(claude.join("projects")).unwrap();

        let summary = summarize(&store, NOW).unwrap();
        let claude_counts = &summary.harnesses[0];
        assert_eq!(
            (
                claude_counts.harness.as_str(),
                claude_counts.provider,
                claude_counts.facts
            ),
            ("claude", 1, 2)
        );
        assert_eq!(claude_counts.next_expiry, Some(NOW + 20 * DAY));
        assert_eq!(
            (
                summary.harnesses[1].provider,
                summary.harnesses[1].next_expiry
            ),
            (1, None)
        );
        assert_eq!(summary.next_expiries[0].session_id, "readable");
        assert_eq!(summary.expiring_within_week, 0);
        assert_eq!(summary.sessions, 4);

        let read = session_transcript(&store, "readable", NOW).unwrap();
        let conversation = &read.conversations[0];
        assert_eq!(conversation.retention.state, RetentionState::Provider);
        assert_eq!(
            conversation.account.as_ref().map(ProviderAccountId::as_str),
            Some("claude")
        );
        assert_eq!(
            conversation.turns[0].request.as_ref().unwrap().text,
            "Fix the parser"
        );

        let gone = session_transcript(&store, "gone", NOW).unwrap();
        let conversation = &gone.conversations[0];
        assert_eq!(conversation.retention.state, RetentionState::Facts);
        assert_eq!(conversation.retention.after, Some(NOW - 10 * DAY));
        let note = conversation.retention.note.as_deref().unwrap();
        assert!(note.contains("gone at claude; removed after"), "{note}");
        assert_eq!(
            conversation.facts[0].final_answer.as_deref(),
            Some("Shipped the fix.")
        );
        assert_eq!(conversation.facts[0].status.as_deref(), Some("completed"));
    }
}
