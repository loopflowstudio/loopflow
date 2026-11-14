//! OpenCode keeps every conversation in one SQLite store under its home,
//! `session_message` rows in order of `seq`. Each row's `data`, with its `id`
//! and `type`, is the message `opencode session export` writes. The store is
//! opened read-only.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;

use super::live::TurnState;
use super::{text_of, Calls, Launch, RequestSource, Turn, Turns};

/// The store in OpenCode's home.
pub(crate) fn store(home: &Path) -> PathBuf {
    home.join("opencode.db")
}

fn open(store: &Path) -> Result<Connection> {
    let connection = Connection::open_with_flags(
        store,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("open {}", store.display()))?;
    connection.busy_timeout(std::time::Duration::from_millis(200))?;
    Ok(connection)
}

/// Conversation `id`'s messages, oldest first.
pub(super) fn messages(store: &Path, id: &str) -> Result<Vec<Value>> {
    let connection = open(store)?;
    let mut statement = connection
        .prepare("SELECT id, type, data FROM session_message WHERE session_id=?1 ORDER BY seq")?;
    let rows = statement.query_map(params![id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut messages = Vec::new();
    for row in rows {
        let (id, kind, data) = row?;
        let Ok(mut message) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        if let Some(fields) = message.as_object_mut() {
            fields.insert("id".into(), id.into());
            fields.insert("type".into(), kind.into());
            messages.push(message);
        }
    }
    Ok(messages)
}

/// Unix seconds of conversation `id`'s last write.
pub(super) fn last_written(store: &Path, id: &str) -> Option<i64> {
    let at: Option<i64> = open(store)
        .ok()?
        .query_row(
            "SELECT max(time_updated) FROM session_message WHERE session_id=?1",
            params![id],
            |row| row.get(0),
        )
        .ok()?;
    Some(at? / 1000)
}

/// The conversations last written in `cwd` since `since`, in Unix seconds,
/// newest first, with when.
pub(super) fn written_in(store: &Path, cwd: &Path, since: i64) -> Vec<(String, i64)> {
    let read = || -> Result<Vec<(String, i64)>> {
        let connection = open(store)?;
        let mut statement = connection.prepare(
            "SELECT id, time_updated FROM session_v2
             WHERE directory=?1 AND time_updated>=?2 ORDER BY time_updated DESC",
        )?;
        let rows = statement.query_map(
            params![cwd.to_string_lossy(), since.saturating_mul(1000)],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? / 1000)),
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    };
    read().unwrap_or_else(|error| {
        tracing::debug!(%error, "Could not read OpenCode conversations");
        Vec::new()
    })
}

/// A user message starts a turn, named by its id as Loopflow's own recording
/// names it; a `synthetic` one starts a turn once OpenCode answers it. Each
/// assistant message is one model call with its usage and cost; one that
/// completed with any `finish` but `tool-calls` hands back, and an `idle` row
/// says how the turn ended.
pub(crate) fn read(messages: &[Value]) -> Vec<Turn> {
    let mut turns = Turns::default();
    let mut calls = Calls::default();
    for message in messages {
        let id = message["id"].as_str().unwrap_or_default();
        let time = |key: &str| super::rfc3339(message["time"][key].as_i64()? / 1000);
        match message["type"].as_str() {
            Some("user") => {
                let text = message["text"].as_str().unwrap_or_default();
                turns.start(Some(id.to_owned()), time("created"));
                if Launch::parse(text).is_some() {
                    turns.request(time("created"), text.to_owned(), RequestSource::Injected);
                } else {
                    let typed = text.split("<lf:").next().unwrap_or_default();
                    if !typed.trim().is_empty() {
                        turns.request(time("created"), typed.to_owned(), RequestSource::Person);
                    }
                }
            }
            Some("synthetic") => turns.wake(Some(id.to_owned()), time("created")),
            Some("assistant") => {
                turns.current();
                parts(&mut turns, id, &message["content"]);
                let tokens = &message["tokens"];
                let count = |value: &Value| value.as_u64().unwrap_or(0);
                let turn = calls.add(
                    &mut turns,
                    "opencode",
                    [
                        count(&tokens["input"]),
                        count(&tokens["cache"]["read"]),
                        count(&tokens["cache"]["write"]),
                        count(&tokens["output"]),
                        count(&tokens["reasoning"]),
                    ],
                    message["cost"].as_f64(),
                    message["model"]["id"].as_str(),
                );
                if !message["error"].is_null() {
                    turn.receipt.errors.push(message["error"].clone());
                }
                if message["time"]["completed"].is_null() || message["finish"] == "tool-calls" {
                    continue;
                }
                let aborted = message["error"]["type"]
                    .as_str()
                    .is_some_and(|kind| kind.contains("abort"));
                let state = if aborted {
                    TurnState::Interrupted
                } else {
                    TurnState::Ended
                };
                turns.end(state, time("completed"));
            }
            Some("idle") if !turns.turns.is_empty() => {
                let state = match message["outcome"].as_str() {
                    Some("interrupted") => TurnState::Interrupted,
                    _ => TurnState::Ended,
                };
                turns.end(state, time("created"));
            }
            _ => {}
        }
    }
    turns.finish()
}

fn parts(turns: &mut Turns, id: &str, content: &Value) {
    for part in content.as_array().into_iter().flatten() {
        match part["type"].as_str() {
            Some("text") => turns.message(id, part["text"].as_str().unwrap_or("")),
            Some("reasoning") => turns.thought(id, part["text"].as_str().unwrap_or("")),
            Some("tool") => {
                let state = &part["state"];
                let call = part["id"].as_str().unwrap_or_default();
                turns.tool(Some(call), part["name"].as_str(), state["input"].clone());
                match state["status"].as_str() {
                    Some("error") => turns.resolve(call, true, text_of(&state["error"])),
                    Some("completed") => {
                        let output = match state["output"].as_str() {
                            Some(output) => output.to_owned(),
                            None => text_of(&state["content"]),
                        };
                        turns.resolve(call, false, output);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::live::TurnUsage;

    /// An OpenCode store holding `messages` of conversation `ses`, in order.
    fn store(messages: &[Value]) -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let connection = Connection::open(super::store(home.path())).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE session_v2(id TEXT PRIMARY KEY, directory TEXT, time_updated INTEGER);
                 CREATE TABLE session_message(id TEXT PRIMARY KEY, session_id TEXT, type TEXT,
                    seq INTEGER, time_created INTEGER, time_updated INTEGER, data TEXT);
                 INSERT INTO session_v2 VALUES('ses','/work',1794327909000);",
            )
            .unwrap();
        for (seq, message) in messages.iter().enumerate() {
            let mut data = message.clone();
            let fields = data.as_object_mut().unwrap();
            let id = fields.remove("id").unwrap();
            let kind = fields.remove("type").unwrap();
            connection
                .execute(
                    "INSERT INTO session_message VALUES(?1,'ses',?2,?3,0,?4,?5)",
                    params![
                        id.as_str(),
                        kind.as_str(),
                        seq as i64,
                        1_794_327_900_000 + seq as i64 * 1000,
                        data.to_string()
                    ],
                )
                .unwrap();
        }
        home
    }

    fn assistant(id: &str, finish: Option<&str>, completed: Option<i64>, input: u64) -> Value {
        serde_json::json!({"id":id,"type":"assistant",
            "time":{"created":1_794_327_901_000_i64,"completed":completed},
            "model":{"id":"glm-5.2","providerID":"opencode"},"finish":finish,"cost":0.01,
            "tokens":{"input":input,"output":50,"reasoning":5,"cache":{"read":1000,"write":0}},
            "content":[{"type":"text","text":"Working."}]})
    }

    #[test]
    fn an_opencode_turn_runs_through_its_tool_calls_and_ends_at_its_answer() {
        let user = serde_json::json!({"id":"msg_u1","type":"user","text":"Fix the build",
            "time":{"created":1_794_327_900_000_i64}});
        let calling = assistant("msg_a1", Some("tool-calls"), Some(1_794_327_903_000), 200);
        let home = store(&[user.clone(), calling.clone()]);
        let source = super::store(home.path());
        let turn = read(&messages(&source, "ses").unwrap()).pop().unwrap();
        assert_eq!(turn.id.as_deref(), Some("msg_u1"));
        assert_eq!(turn.state, TurnState::Running);
        assert_eq!(turn.started_at.as_deref(), Some("2026-11-10T16:25:00Z"));
        assert_eq!(last_written(&source, "ses"), Some(1_794_327_901));

        let answer = assistant("msg_a2", Some("stop"), Some(1_794_327_909_000), 100);
        let idle = serde_json::json!({"id":"msg_i","type":"idle","outcome":"succeeded",
            "time":{"created":1_794_327_910_000_i64}});
        let turn = read(&[user, calling, answer, idle]).pop().unwrap();
        assert_eq!(turn.state, TurnState::Ended);
        assert_eq!(turn.ended_at.as_deref(), Some("2026-11-10T16:25:10Z"));
        assert_eq!(turn.model.as_deref(), Some("glm-5.2"));
        assert_eq!(
            turn.usage,
            Some(TurnUsage {
                input_tokens: 2300,
                cached_input_tokens: 2000,
                output_tokens: 100,
            })
        );
        assert_eq!(turn.receipt.usage["provider"], "opencode");
        assert_eq!(turn.receipt.usage["usage"]["cost_usd"], 0.02);
    }

    #[test]
    fn an_opencode_conversation_is_found_by_its_directory_since_a_time() {
        let home = store(&[]);
        let source = super::store(home.path());
        let written = written_in(&source, Path::new("/work"), 1_794_327_000);
        assert_eq!(written, [("ses".to_owned(), 1_794_327_909)]);
        assert!(written_in(&source, Path::new("/work"), 1_794_328_000).is_empty());
        assert!(written_in(&source, Path::new("/elsewhere"), 0).is_empty());
    }
}
