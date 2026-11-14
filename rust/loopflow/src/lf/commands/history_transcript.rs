//! Reading conversations through `lf history`: a Session's turns as its
//! providers kept them, what is retained across Sessions, and search.

use std::collections::BTreeSet;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::chat::types::{ConversationItem, Lifecycle};
use crate::harness::HarnessKind;
use crate::lf::commands::WorkFilter;
use crate::store::sqlite::SqliteStore;
use crate::store::ProviderAccountId;
use crate::transcript::summary::{summarize, RetentionSummary};
use crate::transcript::{
    self, Conversation, Located, Request, RequestSource, RetentionState, SessionTranscript, Source,
    Turn, TurnItem,
};

const SNIPPET_CHARS: usize = 80;
const ANSWER_CHARS: usize = 160;

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn read_only_store() -> Result<Option<SqliteStore>> {
    let database = crate::store::database_path_from_env()?;
    if !database.exists() {
        return Ok(None);
    }
    Ok(Some(SqliteStore::open_processes_read_only(&database)?))
}

/// A Unix time as `YYYY-MM-DD HH:MM UTC`.
fn minute(at: i64) -> String {
    time::OffsetDateTime::from_unix_timestamp(at)
        .map(|at| format!("{} {:02}:{:02} UTC", at.date(), at.hour(), at.minute()))
        .unwrap_or_else(|_| at.to_string())
}

fn clip(text: &str, chars: usize) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    match line.char_indices().nth(chars) {
        Some((at, _)) => format!("{}…", &line[..at]),
        None => line,
    }
}

// -- lf history show --

/// The Session's conversation, read from its providers.
pub(crate) fn session(store: &SqliteStore, session: &str) -> Result<SessionTranscript> {
    transcript::session_transcript(store, session, now())
}

pub(crate) fn render(transcript: &SessionTranscript) -> String {
    let mut out = String::from("Conversation\n");
    if transcript.conversations.is_empty() {
        out.push_str("  no provider conversation recorded\n");
    }
    for conversation in &transcript.conversations {
        out.push('\n');
        render_conversation(&mut out, conversation);
    }
    out
}

fn render_conversation(out: &mut String, conversation: &Conversation) {
    let retention = &conversation.retention;
    out.push_str(&format!(
        "{} {}{}\n",
        conversation.harness,
        conversation.agent_session,
        conversation
            .account
            .as_ref()
            .map(|account| format!(" · account {account}"))
            .unwrap_or_default()
    ));
    if let Some(path) = &retention.path {
        out.push_str(&format!("  {path}\n"));
    }
    let policy = retention.policy.describe();
    match (retention.state, retention.after) {
        (RetentionState::Provider, Some(after)) => out.push_str(&format!(
            "  Kept by {} until after {} · {policy}\n",
            conversation.harness,
            transcript::date(after)
        )),
        (RetentionState::Provider, None) => out.push_str(&format!("  Retention: {policy}\n")),
        // A gone Claude conversation says how to keep the next one longer.
        (RetentionState::Facts, _)
            if matches!(
                retention.policy,
                transcript::ExpiryPolicy::ClaudeCleanup { .. }
            ) =>
        {
            out.push_str(&format!("  Retention: {policy}\n"))
        }
        _ => {}
    }
    if let Some(note) = &retention.note {
        out.push_str(&format!("  Transcript: {note}\n"));
    }
    if conversation.turns.is_empty() {
        render_facts(out, conversation);
        return;
    }
    for turn in &conversation.turns {
        render_turn(out, turn);
    }
}

fn render_facts(out: &mut String, conversation: &Conversation) {
    if conversation.facts.is_empty() {
        out.push_str("  No turns recorded.\n");
        return;
    }
    out.push_str("  Turns (facts):\n");
    for turn in &conversation.facts {
        let at = turn
            .started_at
            .or(turn.settled_at)
            .map(minute)
            .unwrap_or_else(|| "unknown time".into());
        let status = turn.status.as_deref().unwrap_or("unsettled");
        let answer = turn
            .final_answer
            .as_deref()
            .map(|answer| format!(" · final answer: \"{}\"", clip(answer, ANSWER_CHARS)))
            .unwrap_or_default();
        out.push_str(&format!("    {at} {status}{answer}\n"));
    }
}

/// One line for the turn (when it began, how it ended, its model and
/// tokens), then its requests and what the agent did, in order.
fn render_turn(out: &mut String, turn: &Turn) {
    out.push_str(&format!("\n── {}\n", turn_line(turn)));
    for item in &turn.items {
        match item {
            TurnItem::Request(request) => render_request(out, request),
            TurnItem::Agent(item) => render_item(out, item),
        }
    }
}

fn turn_line(turn: &Turn) -> String {
    use transcript::live::TurnState;
    let at = |at: &Option<String>| transcript::seconds(at).map(minute);
    let ended = at(&turn.ended_at).map(|at| format!(" {at}"));
    let state = match turn.state {
        TurnState::Running => "running".to_owned(),
        TurnState::Ended => format!("ended{}", ended.unwrap_or_default()),
        TurnState::Interrupted => format!("interrupted{}", ended.unwrap_or_default()),
    };
    [
        Some(turn.at().map_or_else(|| "turn".to_owned(), minute)),
        Some(state),
        turn.model.clone(),
        turn.usage.map(|usage| usage.describe()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

fn render_request(out: &mut String, request: &Request) {
    match request.from {
        RequestSource::Person => out.push_str(&format!("\nyou: {}\n", request.text.trim())),
        RequestSource::Injected => match transcript::Launch::parse(&request.text) {
            Some(launch) => {
                out.push_str("\ninjected:");
                if let Some((skill, _)) = &launch.skill {
                    out.push_str(&format!(" Skill {skill}"));
                }
                out.push('\n');
                if let Some(message) = launch.message.filter(|m| !m.is_empty()) {
                    out.push_str(&format!("{message}\n"));
                }
            }
            None => out.push_str(&format!(
                "\ninjected: ({} characters of context)\n",
                request.text.chars().count()
            )),
        },
    }
}

fn render_item(out: &mut String, item: &ConversationItem) {
    match item {
        ConversationItem::Message { text, .. } => {
            out.push_str(&format!("\n{}\n", text.trim()));
        }
        ConversationItem::Thought { text, .. } => {
            out.push_str(&format!("  thinking: {}\n", clip(text, SNIPPET_CHARS)));
        }
        ConversationItem::Tool {
            name,
            status,
            input,
            output,
            ..
        } => {
            let input = match input {
                Some(serde_json::Value::String(input)) => input.clone(),
                Some(input) => input.to_string(),
                None => String::new(),
            };
            out.push_str(&format!("  · {name} {}\n", clip(&input, SNIPPET_CHARS)));
            if *status == Lifecycle::Failed {
                out.push_str(&format!(
                    "    failed: {}\n",
                    clip(output.as_deref().unwrap_or_default(), SNIPPET_CHARS)
                ));
            }
        }
        _ => {}
    }
}

// -- lf history retention --

pub fn retention(json: bool) -> Result<()> {
    let Some(summary) = retention_summary()? else {
        println!("No Sessions recorded.");
        return Ok(());
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        print!("{}", render_retention(&summary));
    }
    Ok(())
}

/// What `lf history retention` reports; `None` without a store.
pub(crate) fn retention_summary() -> Result<Option<RetentionSummary>> {
    read_only_store()?
        .map(|store| summarize(&store, now()))
        .transpose()
}

fn render_retention(summary: &RetentionSummary) -> String {
    if summary.sessions == 0 {
        return "No Sessions recorded.\n".into();
    }
    let mut out = String::new();
    for harness in &summary.harnesses {
        let mut line = format!(
            "{:<9}{} readable · {} facts only",
            harness.harness, harness.provider, harness.facts
        );
        if harness.unknown > 0 {
            line.push_str(&format!(" · {} not checked", harness.unknown));
        }
        match harness.next_expiry {
            Some(after) => line.push_str(&format!(
                " · next removal after {}",
                transcript::date(after)
            )),
            None if harness.harness == HarnessKind::Claude.as_str() => {}
            None => line.push_str(" · no provider expiry"),
        }
        out.push_str(&line);
        out.push('\n');
    }
    out.push_str("\nBy account and age\n");
    let width = summary
        .groups
        .iter()
        .map(|group| account_name(group.account.as_ref()).len())
        .max()
        .unwrap_or(0)
        + 2;
    for group in &summary.groups {
        let state = match group.state {
            RetentionState::Provider => "readable",
            RetentionState::Facts => "facts only",
            RetentionState::Unknown => "not checked",
        };
        out.push_str(&format!(
            "  {:<9}{:<width$}{:<8}{:<12}{}\n",
            group.harness,
            account_name(group.account.as_ref()),
            group.age.label(),
            state,
            group.sessions
        ));
    }
    out.push_str(&format!(
        "\nExpiring in the next 7 days: {}\n",
        summary.expiring_within_week
    ));
    for expiring in &summary.next_expiries {
        out.push_str(&format!(
            "  after {}  {}  {} {}\n",
            transcript::date(expiring.after),
            expiring.session_id,
            expiring.harness,
            clip(&expiring.title, 60)
        ));
    }
    out.push_str("\nHomes\n");
    for home in &summary.homes {
        out.push_str(&format!(
            "  {} {} · {} transcripts · {}\n",
            home.harness,
            home.home,
            home.transcripts,
            home.policy.describe()
        ));
    }
    out
}

fn account_name(account: Option<&ProviderAccountId>) -> &str {
    account.map_or("native", ProviderAccountId::as_str)
}

// -- lf history search --

/// `lf history search TEXT --json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResults {
    pub query: String,
    pub since: i64,
    pub scanned_bytes: u64,
    pub hits: Vec<SearchHit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub session_id: String,
    pub harness: String,
    pub agent_session: String,
    pub at: Option<i64>,
    pub snippet: String,
}

pub(crate) fn search(
    query: &str,
    since: &str,
    filter: WorkFilter,
    limit: usize,
    json: bool,
) -> Result<()> {
    anyhow::ensure!(!query.trim().is_empty(), "Search text is empty");
    let since = super::util::parse_since(since, time::OffsetDateTime::now_utc())?.unix_timestamp();
    let mut results = SearchResults {
        query: query.to_owned(),
        since,
        scanned_bytes: 0,
        hits: Vec::new(),
    };
    if let Some(store) = read_only_store()? {
        let sessions = super::session_history::collect_history(filter, None, since)?
            .into_iter()
            .map(|history| history.session_id.to_string())
            .collect::<BTreeSet<_>>();
        let needle = query.to_lowercase();
        // A raw byte prefilter, so only transcripts that mention it are parsed.
        let prefilter = regex::bytes::RegexBuilder::new(&regex::escape(query))
            .case_insensitive(true)
            .build()?;
        'sessions: for session in sessions {
            for thread in transcript::threads(&store, &session)? {
                let Ok((harness, Located::Found { source, .. })) = thread.located else {
                    continue;
                };
                if let Source::File(path) = &source {
                    let Ok(bytes) = std::fs::read(path) else {
                        continue;
                    };
                    results.scanned_bytes += bytes.len() as u64;
                    if !prefilter.is_match(&bytes) {
                        continue;
                    }
                }
                let Ok(turns) = transcript::read(harness, &source) else {
                    continue;
                };
                for turn in &turns {
                    for text in texts(turn) {
                        if let Some(snippet) = snippet(text, &needle) {
                            results.hits.push(SearchHit {
                                session_id: session.clone(),
                                harness: harness.as_str().to_owned(),
                                agent_session: thread.facts.thread.as_str().to_owned(),
                                at: turn.at(),
                                snippet,
                            });
                            if results.hits.len() >= limit {
                                break 'sessions;
                            }
                        }
                    }
                }
            }
        }
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&results)?);
        return Ok(());
    }
    for hit in &results.hits {
        println!(
            "{} · {} · {}",
            hit.session_id,
            hit.at.map(minute).unwrap_or_else(|| "unknown time".into()),
            hit.snippet
        );
    }
    if results.hits.is_empty() {
        println!("No matches.");
    }
    println!(
        "Scanned {:.1} MB of transcripts",
        results.scanned_bytes as f64 / 1_000_000.0
    );
    Ok(())
}

/// The searchable text of a turn: its requests, messages and tool calls.
fn texts(turn: &Turn) -> Vec<&str> {
    let mut texts = Vec::new();
    for item in &turn.items {
        match item {
            TurnItem::Request(request) => texts.push(request.text.as_str()),
            TurnItem::Agent(
                ConversationItem::Message { text, .. } | ConversationItem::Thought { text, .. },
            ) => texts.push(text),
            TurnItem::Agent(ConversationItem::Tool { output, .. }) => {
                texts.extend(output.as_deref());
            }
            TurnItem::Agent(_) => {}
        }
    }
    texts
}

/// The words around the first match of `needle` (already lowercase).
fn snippet(text: &str, needle: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let at = lower.find(needle)?;
    // Lowercasing can change byte lengths; fall back to the whole text then.
    let at = if lower.len() == text.len() && text.is_char_boundary(at) {
        at
    } else {
        0
    };
    let start = text[..at]
        .char_indices()
        .rev()
        .nth(SNIPPET_CHARS / 2)
        .map_or(0, |(index, _)| index);
    let snippet = clip(&text[start..], SNIPPET_CHARS);
    Some(if start > 0 {
        format!("…{snippet}")
    } else {
        snippet
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_snippet_centers_on_the_match_and_ignores_case() {
        let text = format!(
            "{} the cleanupPeriodDays setting {}",
            "a ".repeat(100),
            "b ".repeat(100)
        );
        let found = snippet(&text, "cleanupperioddays").unwrap();
        assert!(found.starts_with('…') && found.contains("cleanupPeriodDays"));
        assert!(snippet("nothing here", "absent").is_none());
    }

    fn facts_only(harness: HarnessKind, note: &str) -> String {
        let conversation = Conversation {
            harness: harness.as_str().into(),
            agent_session: "thread-1".into(),
            account: None,
            retention: transcript::Retention {
                state: RetentionState::Facts,
                path: None,
                policy: transcript::ExpiryPolicy::read(harness, None),
                after: Some(0),
                note: Some(note.into()),
            },
            turns: Vec::new(),
            facts: Vec::new(),
        };
        let mut out = String::new();
        render_conversation(&mut out, &conversation);
        out
    }

    #[test]
    fn a_gone_claude_conversation_says_how_to_keep_more() {
        let claude = facts_only(
            HarnessKind::Claude,
            "gone at claude; removed after 1970-01-01",
        );
        assert!(claude.contains("removed after") && claude.contains("cleanupPeriodDays"));
        let codex = facts_only(HarnessKind::Codex, "missing at codex");
        assert!(!codex.contains("removed after") && !codex.contains("cleanupPeriodDays"));
    }

    #[test]
    fn a_turn_prints_one_line_then_its_requests_and_what_the_agent_did() {
        let turn: Turn = serde_json::from_value(serde_json::json!({
            "id": "u1",
            "state": "ended",
            "started_at": "2026-11-10T16:25:37Z",
            "ended_at": "2026-11-10T16:27:51Z",
            "model": "claude-opus-5-5",
            "usage": {"input_tokens": 41200, "cached_input_tokens": 38000, "output_tokens": 1100},
            "items": [
                {"request": {"at": "2026-11-10T16:25:37Z", "text": "Explain the build", "from": "person"}},
                {"agent": {"type": "message", "id": "a", "text": "It builds with cargo.", "phase": null}},
                {"request": {"at": null, "text": "<system-reminder>x", "from": "injected"}},
            ],
        }))
        .unwrap();
        let mut out = String::new();
        render_turn(&mut out, &turn);
        assert_eq!(
            out,
            "\n── 2026-11-10 16:25 UTC · ended 2026-11-10 16:27 UTC · claude-opus-5-5 · \
             41.2k in (38.0k cached) · 1.1k out\n\
             \nyou: Explain the build\n\
             \nIt builds with cargo.\n\
             \ninjected: (18 characters of context)\n"
        );
    }
}
