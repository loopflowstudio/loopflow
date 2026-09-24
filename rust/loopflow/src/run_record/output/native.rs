use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{digest, gap, OutputRecord, OutputSource, SourcePage, PAGE_BYTES, PAGE_RECORDS};
use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn content(value: &Value) -> String {
    if let Some(value) = value.as_str() {
        return value.into();
    }
    if let Some(parts) = value.as_array() {
        return parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
    }
    value.to_string()
}

fn message(id: String, body: String, phase: Option<String>) -> ConversationItem {
    ConversationItem::Message {
        id,
        text: body,
        phase,
    }
}

fn tool_id(value: &Value, key: &str, page: &mut SourcePage) -> Option<String> {
    let id = value
        .get(key)
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty());
    if id.is_none() {
        page.gaps.push(gap(
            "missing_item_identity",
            format!("Native tool record has no {key}"),
        ));
    }
    id.map(str::to_owned)
}

fn tool(
    id: String,
    name: String,
    input: Option<Value>,
    output: Option<String>,
    status: Lifecycle,
) -> ConversationItem {
    ConversationItem::Tool {
        id,
        name,
        status,
        input,
        output,
    }
}

fn push(page: &mut SourcePage, id: String, turn: String, item: ConversationItem) -> bool {
    let event = ConversationEvent::ItemCompleted {
        turn_id: turn,
        item,
    };
    let revision = digest(&serde_json::to_vec(&event).expect("conversation event serializes"));
    page.push(OutputRecord {
        source_item_id: id,
        revision,
        event,
    })
}

pub(super) fn normalize_jsonl(
    source: OutputSource,
    session: &str,
    value: &Value,
    offset: u64,
    page: &mut SourcePage,
) -> io::Result<bool> {
    match source {
        OutputSource::Claude => {
            if let Some(observed) = value.get("sessionId").and_then(Value::as_str) {
                if observed != session {
                    return Err(io::Error::other(
                        "Claude transcript Session does not match its receipt",
                    ));
                }
                page.cursor.session_verified = true;
            }
            let kind = text(value, "type");
            if !matches!(kind.as_str(), "assistant" | "user") {
                return Ok(true);
            }
            if !page.cursor.session_verified {
                return Err(io::Error::other(
                    "Claude transcript has no matching Session identity",
                ));
            }
            let uuid = text(value, "uuid");
            if uuid.is_empty() {
                page.gaps
                    .push(gap("missing_item_identity", "Claude message has no UUID"));
                return Ok(true);
            }
            let message = &value["message"];
            let turn = message
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(&uuid)
                .to_string();
            if let Some(body) = message["content"].as_str() {
                return Ok(push(
                    page,
                    uuid.clone(),
                    turn,
                    self::message(uuid, body.into(), Some(kind)),
                ));
            } else if let Some(blocks) = message["content"].as_array() {
                let first = page
                    .cursor
                    .pending_line
                    .as_ref()
                    .map_or(0, |(_, index)| *index);
                for (index, block) in blocks.iter().enumerate().skip(first).take(PAGE_RECORDS) {
                    page.cursor
                        .pending_line
                        .as_mut()
                        .expect("JSONL line is pending")
                        .1 = index + 1;
                    let id = format!("{uuid}:{index}");
                    let block_kind = text(block, "type");
                    let item_id = match block_kind.as_str() {
                        "tool_use" => tool_id(block, "id", page),
                        "tool_result" => tool_id(block, "tool_use_id", page),
                        _ => Some(id.clone()),
                    };
                    let Some(item_id) = item_id else { continue };
                    let item = match block_kind.as_str() {
                        "text" => {
                            self::message(id.clone(), text(block, "text"), Some(kind.clone()))
                        }
                        "tool_use" => tool(
                            item_id,
                            text(block, "name"),
                            block.get("input").cloned(),
                            None,
                            Lifecycle::Running,
                        ),
                        "tool_result" => tool(
                            item_id,
                            "tool_result".into(),
                            Some(serde_json::json!({"tool_use_id": block["tool_use_id"]})),
                            block.get("content").map(content),
                            if block["is_error"] == true {
                                Lifecycle::Failed
                            } else {
                                Lifecycle::Completed
                            },
                        ),
                        _ => continue,
                    };
                    if !push(page, id, turn.clone(), item) {
                        page.cursor
                            .pending_line
                            .as_mut()
                            .expect("JSONL line is pending")
                            .1 = index;
                        return Ok(false);
                    }
                }
                return Ok(first.saturating_add(PAGE_RECORDS) >= blocks.len());
            }
        }
        OutputSource::Codex => {
            let kind = text(value, "type");
            let payload = &value["payload"];
            if kind == "session_meta" {
                if payload["id"].as_str() != Some(session) {
                    return Err(io::Error::other(
                        "Codex transcript Session does not match its receipt",
                    ));
                }
                page.cursor.session_verified = true;
                return Ok(true);
            }
            // Response items are canonical. event_msg prose/tool mirrors and internal
            // reasoning/context records must never become another transcript copy.
            if kind != "response_item" && !(kind == "event_msg" && payload["type"] == "error") {
                return Ok(true);
            }
            if !page.cursor.session_verified {
                return Err(io::Error::other(
                    "Codex transcript has no matching Session metadata",
                ));
            }
            let id = payload
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("byte:{offset}"));
            let item_kind = text(payload, "type");
            let item_id = match item_kind.as_str() {
                "function_call"
                | "custom_tool_call"
                | "function_call_output"
                | "custom_tool_call_output" => tool_id(payload, "call_id", page),
                _ => Some(id.clone()),
            };
            let Some(item_id) = item_id else {
                return Ok(true);
            };
            let item = match item_kind.as_str() {
                "message" if matches!(payload["role"].as_str(), Some("assistant" | "user")) => {
                    message(
                        id.clone(),
                        content(&payload["content"]),
                        payload
                            .get("phase")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                            .or_else(|| payload["role"].as_str().map(str::to_owned)),
                    )
                }
                "function_call" | "custom_tool_call" => tool(
                    item_id,
                    text(payload, "name"),
                    payload
                        .get("arguments")
                        .or_else(|| payload.get("input"))
                        .cloned(),
                    None,
                    Lifecycle::Running,
                ),
                "function_call_output" | "custom_tool_call_output" => tool(
                    item_id,
                    "tool_result".into(),
                    Some(serde_json::json!({"call_id": payload["call_id"]})),
                    payload.get("output").map(content),
                    Lifecycle::Completed,
                ),
                "error" if kind == "event_msg" => {
                    return Ok(page.push(OutputRecord {
                        source_item_id: id,
                        revision: digest(&serde_json::to_vec(payload)?),
                        event: ConversationEvent::Error {
                            code: "provider_error".into(),
                            message: text(payload, "message"),
                            evidence: None,
                        },
                    }));
                }
                _ => return Ok(true),
            };
            return Ok(push(page, id, session.into(), item));
        }
        _ => {}
    }
    Ok(true)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PartCursor {
    session_created: i64,
    part_count: i64,
    watermark: i64,
    ceiling: i64,
    after: Option<(i64, String)>,
    seen: BTreeMap<String, PartRevision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PartRevision {
    updated: i64,
    hash: String,
    unfinished: bool,
}

pub(super) fn tail_parts(path: &Path, session: &str) -> io::Result<PartCursor> {
    (|| -> anyhow::Result<PartCursor> {
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.busy_timeout(std::time::Duration::from_millis(100))?;
        let tx = conn.transaction()?;
        let session_created = tx.query_row(
            "SELECT time_created FROM session WHERE id=?1",
            [session],
            |row| row.get(0),
        )?;
        let (part_count, watermark) = tx.query_row(
            "SELECT COUNT(*),COALESCE(MAX(time_updated),0) FROM part WHERE session_id=?1",
            [session],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        // Capture IDs in this same snapshot: a part may complete with an unchanged
        // timestamp before the first page reaches it. Its revision still belongs
        // to the live reader even when it leaves the unfinished SQL predicate.
        let mut statement = tx.prepare(
            "SELECT id,time_updated FROM part WHERE session_id=?1 AND
             CASE WHEN json_valid(data) THEN
               (json_extract(data,'$.type')='text' AND json_extract(data,'$.time.end') IS NULL)
               OR (json_extract(data,'$.type')='tool' AND
                   COALESCE(json_extract(data,'$.state.status'),'') NOT IN ('completed','error'))
             ELSE 1 END",
        )?;
        let seen = statement
            .query_map([session], |row| {
                Ok((
                    row.get(0)?,
                    PartRevision {
                        updated: row.get(1)?,
                        hash: String::new(),
                        unfinished: true,
                    },
                ))
            })?
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(PartCursor {
            session_created,
            part_count,
            watermark,
            ceiling: watermark,
            after: None,
            seen,
        })
    })()
    .map_err(io::Error::other)
}

pub(super) fn read_parts(path: &Path, session: &str, page: &mut SourcePage) -> io::Result<()> {
    let result = read_parts_in(path, session, page);
    result.map_err(io::Error::other)
}

fn read_parts_in(path: &Path, session: &str, page: &mut SourcePage) -> anyhow::Result<()> {
    let mut conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(std::time::Duration::from_millis(100))?;
    let tx = conn.transaction()?;
    let created: i64 = tx
        .query_row(
            "SELECT time_created FROM session WHERE id=?1",
            [session],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| anyhow::anyhow!("OpenCode Session is absent from its recorded database"))?;
    let part_count: i64 = tx.query_row(
        "SELECT COUNT(*) FROM part WHERE session_id=?1",
        [session],
        |row| row.get(0),
    )?;
    let mut cursor = page.cursor.parts.clone().unwrap_or(PartCursor {
        session_created: created,
        part_count,
        watermark: 0,
        ceiling: 0,
        after: None,
        seen: BTreeMap::new(),
    });
    if cursor.session_created != created || part_count < cursor.part_count {
        page.reset = true;
        page.gaps.push(gap(
            "source_reset",
            "OpenCode Session was replaced or compacted",
        ));
        cursor = PartCursor {
            session_created: created,
            part_count,
            watermark: 0,
            ceiling: 0,
            after: None,
            seen: BTreeMap::new(),
        };
    }
    // Detect deletion of retained boundary/unfinished parts. Do not turn a provider
    // compaction into silently healthy emptiness.
    for id in cursor.seen.keys() {
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM part WHERE session_id=?1 AND id=?2)",
            rusqlite::params![session, id],
            |row| row.get(0),
        )?;
        if !exists {
            page.reset = true;
            page.gaps.push(gap(
                "source_reset",
                "OpenCode retained parts were removed or compacted",
            ));
            cursor.watermark = 0;
            cursor.ceiling = 0;
            cursor.after = None;
            cursor.seen.clear();
            break;
        }
    }
    // Hold the upper timestamp for the entire paginated sweep. Advancing it
    // between pages could skip a same-millisecond edit behind the keyset cursor.
    if cursor.after.is_none() {
        cursor.ceiling = tx.query_row(
            "SELECT COALESCE(MAX(time_updated),0) FROM part WHERE session_id=?1",
            [session],
            |row| row.get(0),
        )?;
    }
    let unfinished = cursor
        .seen
        .iter()
        .filter(|(_, revision)| revision.unfinished)
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    let after = cursor.after.clone().unwrap_or((-1, String::new()));
    let mut statement = tx.prepare(
        "SELECT p.id, p.message_id, p.time_updated, substr(p.data,1,?6), substr(m.data,1,?6)
         FROM part p JOIN message m ON m.id=p.message_id AND m.session_id=p.session_id
         WHERE p.session_id=?1 AND (p.time_updated>=?2 OR p.id IN (SELECT value FROM json_each(?3)))
         AND (p.time_updated>?4 OR (p.time_updated=?4 AND p.id>?5))
         AND p.time_updated<=?8
         ORDER BY p.time_updated,p.id LIMIT ?7",
    )?;
    let mut rows = statement.query(rusqlite::params![
        session,
        cursor.watermark,
        serde_json::to_string(&unfinished)?,
        after.0,
        after.1,
        (PAGE_BYTES + 1) as i64,
        (PAGE_RECORDS + 1) as i64,
        cursor.ceiling
    ])?;
    let mut count = 0;
    let mut bytes = 0;
    while let Some(row) = rows.next()? {
        if count >= PAGE_RECORDS {
            page.has_more = true;
            break;
        }
        let id: String = row.get(0)?;
        let message_id: String = row.get(1)?;
        let updated: i64 = row.get(2)?;
        let data: String = row.get(3)?;
        let message: String = row.get(4)?;
        if bytes + data.len() + message.len() > PAGE_BYTES && count > 0 {
            page.has_more = true;
            break;
        }
        count += 1;
        bytes += data.len() + message.len();
        let previous_after = cursor.after.replace((updated, id.clone()));
        if data.len() + message.len() > PAGE_BYTES {
            page.gaps.push(gap(
                "record_too_large",
                format!("OpenCode part {id} exceeds the bounded read size"),
            ));
            continue;
        }
        let hash = digest(data.as_bytes());
        if let Some(old) = cursor.seen.get_mut(&id) {
            if old.hash == hash {
                // Retain the unchanged revision at its new inclusive boundary.
                old.updated = updated;
                continue;
            }
        }
        let (part, message) = match (
            serde_json::from_str::<Value>(&data),
            serde_json::from_str::<Value>(&message),
        ) {
            (Ok(part), Ok(message)) => (part, message),
            _ => {
                page.gaps.push(gap(
                    "malformed_record",
                    format!("Malformed OpenCode part {id}"),
                ));
                continue;
            }
        };
        let unfinished = match part["type"].as_str() {
            Some("text") => part["time"]["end"].is_null(),
            Some("tool") => !matches!(
                part["state"]["status"].as_str(),
                Some("completed" | "error")
            ),
            _ => false,
        };
        let item = if matches!(message["role"].as_str(), Some("assistant" | "user")) {
            match part["type"].as_str() {
                Some("text") => Some(self::message(
                    id.clone(),
                    text(&part, "text"),
                    message["role"].as_str().map(str::to_owned),
                )),
                Some("tool") => {
                    let state = &part["state"];
                    let status = match state["status"].as_str() {
                        Some("completed") => Lifecycle::Completed,
                        Some("error") => Lifecycle::Failed,
                        _ => Lifecycle::Running,
                    };
                    Some(tool(
                        id.clone(),
                        text(&part, "tool"),
                        state.get("input").cloned(),
                        state
                            .get("output")
                            .or_else(|| state.get("error"))
                            .map(content),
                        status,
                    ))
                }
                _ => None,
            }
        } else {
            None
        };
        if let Some(item) = item {
            if !push(page, id.clone(), message_id, item) {
                cursor.after = previous_after;
                page.has_more = true;
                break;
            }
        }
        cursor.seen.insert(
            id.clone(),
            PartRevision {
                updated,
                hash,
                unfinished,
            },
        );
    }
    if !page.has_more {
        cursor.watermark = cursor.ceiling;
        cursor.after = None;
        page.has_more = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM part WHERE session_id=?1 AND time_updated>?2)",
            rusqlite::params![session, cursor.ceiling],
            |row| row.get(0),
        )?;
    }
    // Keep both inclusive boundaries until this sweep ends, plus unfinished
    // parts. Interior completed history never needs to accumulate in the cursor.
    cursor.seen.retain(|_, revision| {
        revision.updated == cursor.watermark
            || revision.updated >= cursor.ceiling
            || revision.unfinished
    });
    cursor.part_count = part_count;
    page.cursor.parts = Some(cursor);
    page.cursor.session_verified = true;
    Ok(())
}
