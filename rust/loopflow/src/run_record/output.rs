//! Bounded passive reads. Cursors are reader state, never transcript authority.

mod native;

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::native_source::NativeSource;
use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};

// Configured Codex compaction records exceed 6 MiB; retain a finite 8 MiB page.
pub(crate) const PAGE_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const PAGE_RECORDS: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OutputSource {
    Journal,
    Claude,
    Codex,
    OpenCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputRecord {
    pub source_item_id: String,
    pub revision: String,
    pub event: ConversationEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputGap {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OutputCursor {
    pub source: OutputSource,
    pub session_id: Option<String>,
    pub location: String,
    pub identity: String,
    pub offset: u64,
    pub anchor: String,
    pub session_verified: bool,
    pub discarding_record: bool,
    pub journal_conversation: Option<bool>,
    pub pending_line: Option<(String, usize)>,
    pub parts: Option<native::PartCursor>,
}

#[derive(Debug)]
pub(crate) struct SourcePage {
    pub records: Vec<OutputRecord>,
    pub gaps: Vec<OutputGap>,
    pub cursor: OutputCursor,
    pub has_more: bool,
    pub reset: bool,
    record_bytes: usize,
}

impl SourcePage {
    // False leaves this record for the next page. An individually oversized
    // record is consumed with evidence so it cannot stall everything after it.
    fn push(&mut self, record: OutputRecord) -> bool {
        let bytes = serde_json::to_vec(&record)
            .expect("output record serializes")
            .len();
        if bytes > PAGE_BYTES {
            self.gaps.push(gap(
                "record_too_large",
                "A normalized output record exceeds the page payload bound and is omitted",
            ));
            return true;
        }
        if self.records.len() == PAGE_RECORDS || self.record_bytes + bytes > PAGE_BYTES {
            return false;
        }
        self.record_bytes += bytes;
        self.records.push(record);
        true
    }
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn gap(code: &str, message: impl Into<String>) -> OutputGap {
    OutputGap {
        code: code.into(),
        message: message.into(),
    }
}

fn file_identity(file: &File) -> io::Result<String> {
    let meta = file.metadata()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(format!("{}:{}", meta.dev(), meta.ino()))
    }
    #[cfg(not(unix))]
    {
        Ok(format!("{:?}", meta.created()?))
    }
}

fn anchor(file: &mut File, offset: u64) -> io::Result<String> {
    let start = offset.saturating_sub(128);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = vec![0; (offset - start) as usize];
    file.read_exact(&mut bytes)?;
    Ok(digest(&bytes))
}

/// Start a separate live reader without consuming its historical continuation.
pub(crate) fn tail_cursor(
    path: &Path,
    source: OutputSource,
    session: Option<&str>,
) -> io::Result<OutputCursor> {
    if source == OutputSource::OpenCode {
        let file = File::open(path)?;
        return Ok(OutputCursor {
            source,
            session_id: session.map(str::to_owned),
            location: path.to_string_lossy().into_owned(),
            identity: file_identity(&file)?,
            offset: 0,
            anchor: digest(&[]),
            session_verified: true,
            discarding_record: false,
            journal_conversation: None,
            pending_line: None,
            parts: Some(native::tail_parts(
                path,
                session.ok_or_else(|| io::Error::other("missing native Session identity"))?,
            )?),
        });
    }
    let head = read_source(path, source, session, None)?;
    if source != OutputSource::Journal && !head.cursor.session_verified {
        return Err(io::Error::other(
            "Native Session identity is not yet readable",
        ));
    }
    let mut cursor = head.cursor;
    cursor.pending_line = None;
    let mut file = File::open(path)?;
    if file_identity(&file)? != cursor.identity
        || anchor(&mut file, cursor.offset)? != cursor.anchor
    {
        return Err(io::Error::other(
            "Output source changed while starting its live cursor",
        ));
    }
    let end = file.metadata()?.len();
    let start = end.saturating_sub(PAGE_BYTES as u64);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.by_ref().take(end - start).read_to_end(&mut bytes)?;
    let complete = bytes.iter().rposition(|byte| *byte == b'\n').map(|i| i + 1);
    let Some(complete) = complete else {
        if start != 0 {
            return Err(io::Error::other(
                "Incomplete trailing output exceeds the live-start byte bound",
            ));
        }
        cursor.offset = 0;
        cursor.anchor = digest(&[]);
        return Ok(cursor);
    };
    if source == OutputSource::Journal {
        // Capture mode belongs to an attempt. Never carry the first attempt's
        // mode past an unseen retry or guess it from a trailing summary mirror.
        let first = if start == 0 {
            0
        } else {
            bytes
                .iter()
                .position(|byte| *byte == b'\n')
                .expect("complete line exists")
                + 1
        };
        let mut known_mode = start == 0;
        cursor.journal_conversation = None;
        for line in bytes[first..complete].split_inclusive(|byte| *byte == b'\n') {
            let envelope = match serde_json::from_slice::<super::EventEnvelope>(line) {
                Ok(envelope) if envelope.schema_version == super::SCHEMA_VERSION => envelope,
                _ => {
                    // An unreadable record may itself be an attempt boundary.
                    known_mode = false;
                    cursor.journal_conversation = None;
                    continue;
                }
            };
            if matches!(
                envelope.event,
                super::RunEvent::ProviderAttemptStarted { .. }
            ) {
                known_mode = true;
            } else if !known_mode && matches!(envelope.event, super::RunEvent::Conversation { .. })
            {
                // Positive canonical evidence establishes mode without scanning
                // back to a launch that may be outside this bounded window.
                known_mode = true;
                cursor.journal_conversation = Some(true);
            }
            if known_mode {
                journal_event(envelope.event, envelope.seq, &mut cursor);
            }
        }
        if !known_mode {
            return Err(io::Error::other(
                "Journal capture mode is unavailable in the live-start byte window; read history to continue",
            ));
        }
    }
    cursor.offset = start + complete as u64;
    cursor.discarding_record = false;
    cursor.anchor = anchor(&mut file, cursor.offset)?;
    Ok(cursor)
}

pub(crate) fn read_source(
    path: &Path,
    source: OutputSource,
    session: Option<&str>,
    previous: Option<&OutputCursor>,
) -> io::Result<SourcePage> {
    let mut file = File::open(path)?;
    let identity = file_identity(&file)?;
    let location = path.to_string_lossy().into_owned();
    let reset = previous.is_some_and(|old| {
        old.source != source
            || old.session_id.as_deref() != session
            || old.location != location
            || old.identity != identity
            || (source != OutputSource::OpenCode
                && (file.metadata().is_ok_and(|m| m.len() < old.offset)
                    || anchor(&mut file, old.offset).is_ok_and(|hash| hash != old.anchor)))
    });
    let cursor = previous
        .filter(|_| !reset)
        .cloned()
        .unwrap_or_else(|| OutputCursor {
            source,
            session_id: session.map(str::to_owned),
            location,
            identity,
            offset: 0,
            anchor: digest(&[]),
            session_verified: false,
            discarding_record: false,
            journal_conversation: None,
            pending_line: None,
            parts: None,
        });
    let mut page = SourcePage {
        records: Vec::new(),
        gaps: Vec::new(),
        cursor,
        has_more: false,
        reset,
        record_bytes: 0,
    };
    if reset {
        page.gaps.push(gap(
            "source_reset",
            "Output source was replaced or truncated; discard its previous items",
        ));
    }
    if source == OutputSource::OpenCode {
        native::read_parts(
            path,
            session.ok_or_else(|| io::Error::other("missing native Session identity"))?,
            &mut page,
        )?;
        return Ok(page);
    }
    file.seek(SeekFrom::Start(page.cursor.offset))?;
    let mut reader = BufReader::new(file.take(PAGE_BYTES as u64));
    let initial_offset = page.cursor.offset;
    let mut consumed = 0;
    let mut complete_records = 0;
    for _ in 0..PAGE_RECORDS {
        if page.records.len() == PAGE_RECORDS {
            break;
        }
        let start = page.cursor.offset;
        let mut bytes = Vec::new();
        let read = reader.read_until(b'\n', &mut bytes)?;
        consumed += read;
        if read == 0 {
            break;
        }
        if page.cursor.discarding_record {
            page.cursor.offset += read as u64;
            page.cursor.discarding_record = bytes.last() != Some(&b'\n');
            complete_records += usize::from(!page.cursor.discarding_record);
            continue;
        }
        if bytes.last() != Some(&b'\n') {
            if read == PAGE_BYTES {
                page.gaps.push(gap(
                    "record_too_large",
                    "A source record exceeds the bounded read size and is omitted",
                ));
                page.cursor.offset += read as u64;
                page.cursor.discarding_record = true;
            }
            break;
        }
        complete_records += 1;
        page.cursor.offset += read as u64;
        let revision = digest(&bytes);
        if page
            .cursor
            .pending_line
            .as_ref()
            .is_some_and(|(hash, _)| hash != &revision)
        {
            let mut replacement = read_source(path, source, session, None)?;
            replacement.reset = true;
            replacement.gaps.push(gap(
                "source_reset",
                "Output changed within a partially read message; discard its previous items",
            ));
            return Ok(replacement);
        }
        let value: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) => {
                page.gaps.push(gap(
                    "malformed_record",
                    format!("Malformed complete record at byte {start}"),
                ));
                continue;
            }
        };
        page.cursor
            .pending_line
            .get_or_insert((revision.clone(), 0));
        let complete = if source == OutputSource::Journal {
            match serde_json::from_value::<super::EventEnvelope>(value) {
                Ok(envelope) if envelope.schema_version == super::SCHEMA_VERSION => {
                    let event = journal_event(envelope.event, envelope.seq, &mut page.cursor);
                    if let Some(event) = event {
                        page.push(OutputRecord {
                            source_item_id: envelope.seq.to_string(),
                            revision,
                            event,
                        })
                    } else {
                        true
                    }
                }
                _ => {
                    page.gaps.push(gap(
                        "journal_schema",
                        format!("Unsupported Run record at byte {start}"),
                    ));
                    true
                }
            }
        } else {
            native::normalize_jsonl(source, session.unwrap_or(""), &value, start, &mut page)?
        };
        if !complete {
            page.cursor.offset = start;
            break;
        }
        page.cursor.pending_line = None;
    }
    let mut file = reader.into_inner().into_inner();
    page.has_more = page.cursor.offset < file.metadata()?.len()
        && (page.cursor.pending_line.is_some()
            || page.records.len() == PAGE_RECORDS
            || complete_records == PAGE_RECORDS
            || consumed >= PAGE_BYTES && page.cursor.offset > initial_offset);
    page.cursor.anchor = anchor(&mut file, page.cursor.offset)?;
    if source == OutputSource::Journal && page.cursor.journal_conversation == Some(false) {
        page.gaps.push(gap(
            "limited_capture",
            "This Run journal retains prose and tool summaries; complete tool results are not available in its normalized stream",
        ));
    }
    Ok(page)
}

fn journal_event(
    event: super::RunEvent,
    seq: u64,
    cursor: &mut OutputCursor,
) -> Option<ConversationEvent> {
    match event {
        super::RunEvent::ProviderAttemptStarted { .. } => {
            cursor.journal_conversation = None;
            None
        }
        super::RunEvent::Conversation { event } => {
            if !*cursor.journal_conversation.get_or_insert(true) {
                return None;
            }
            Some(*event)
        }
        super::RunEvent::Text { text } => {
            if *cursor.journal_conversation.get_or_insert(false) {
                return None;
            }
            Some(ConversationEvent::TextDelta {
                turn_id: "unassigned".into(),
                content: text,
            })
        }
        super::RunEvent::ToolUse { name, summary } => {
            if *cursor.journal_conversation.get_or_insert(false) {
                return None;
            }
            Some(ConversationEvent::ItemStarted {
                turn_id: "unassigned".into(),
                item: ConversationItem::Tool {
                    id: seq.to_string(),
                    name,
                    status: Lifecycle::Running,
                    input: Some(serde_json::Value::String(summary)),
                    output: None,
                },
            })
        }
        super::RunEvent::Result { outcome, .. } => {
            if *cursor.journal_conversation.get_or_insert(false) {
                return None;
            }
            Some(ConversationEvent::TurnCompleted {
                turn_id: "unassigned".into(),
                status: if outcome == "completed" {
                    Lifecycle::Completed
                } else {
                    Lifecycle::Failed
                },
            })
        }
        // Raw provider copies, usage, and launch bookkeeping are not another feed.
        _ => None,
    }
}

pub(crate) async fn resolve_native(
    store: &crate::store::SharedStore,
    manifest: &super::RunManifest,
    session: &super::ProviderSessionRef,
) -> io::Result<Option<NativeSource>> {
    if let Some(source) = &session.native_source {
        return Ok(Some(source.clone()));
    }
    let Some(account_id) = &session.account_id else {
        return Ok(None);
    };
    let accounts = store
        .list_provider_accounts(Some(&manifest.harness))
        .await
        .map_err(io::Error::other)?;
    let home = accounts
        .into_iter()
        .find(|account| &account.account_id == account_id)
        .and_then(|account| account.home);
    home.map(|home| {
        super::native_source::find_jsonl(&home, &manifest.harness, &session.provider_session_id)
    })
    .transpose()
    .map(Option::flatten)
}

#[cfg(test)]
mod tests {
    use super::{read_source, tail_cursor, OutputSource, PAGE_BYTES, PAGE_RECORDS};
    use crate::chat::types::ConversationEvent;
    use std::io::Write;

    fn claude(id: &str, text: &str) -> String {
        format!(
            "{}\n",
            serde_json::json!({"type":"assistant","sessionId":"session","uuid":id,"message":{"content":[{"type":"text","text":text}]}})
        )
    }

    #[test]
    fn claude_message_blocks_page_without_losing_tools_or_live_arrivals() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("native.jsonl");
        let mut blocks = (0..PAGE_RECORDS * 2 + 3)
            .map(|index| serde_json::json!({"type":"text","text":format!("block {index}")}))
            .collect::<Vec<_>>();
        blocks[PAGE_RECORDS - 1] = serde_json::json!({"type":"tool_use","id":"call","name":"bash","input":{"command":"echo proof"}});
        blocks[PAGE_RECORDS] =
            serde_json::json!({"type":"tool_result","tool_use_id":"call","content":"proof"});
        let line = serde_json::json!({"type":"assistant","sessionId":"session","uuid":"message","message":{"content":blocks}});
        std::fs::write(&path, format!("{line}\n")).unwrap();
        let live = tail_cursor(&path, OutputSource::Claude, Some("session")).unwrap();
        let first = read_source(&path, OutputSource::Claude, Some("session"), None).unwrap();
        assert_eq!(first.records.len(), PAGE_RECORDS);
        assert!(first.has_more);
        let mut records = first.records;
        let mut cursor = first.cursor;
        for _ in 0..2 {
            // Continuation survives the same serialization used by CLI reads.
            cursor = serde_json::from_slice(&serde_json::to_vec(&cursor).unwrap()).unwrap();
            let page =
                read_source(&path, OutputSource::Claude, Some("session"), Some(&cursor)).unwrap();
            assert!(page.records.len() <= PAGE_RECORDS);
            assert!(page.gaps.is_empty());
            records.extend(page.records);
            cursor = page.cursor;
        }
        assert_eq!(records.len(), blocks.len());
        for (index, record) in records.iter().enumerate() {
            assert_eq!(record.source_item_id, format!("message:{index}"));
        }
        for index in [PAGE_RECORDS - 1, PAGE_RECORDS] {
            assert!(
                matches!(&records[index].event, ConversationEvent::ItemCompleted {
                item: crate::chat::types::ConversationItem::Tool {id, ..}, ..
            } if id == "call")
            );
        }
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        write!(file, "{}", claude("arrival", "live arrival")).unwrap();
        for position in [live, cursor] {
            let page = read_source(
                &path,
                OutputSource::Claude,
                Some("session"),
                Some(&position),
            )
            .unwrap();
            assert_eq!(page.records.len(), 1);
            assert_eq!(page.records[0].source_item_id, "arrival:0");
            assert!(!page.has_more);
            assert!(read_source(
                &path,
                OutputSource::Claude,
                Some("session"),
                Some(&page.cursor)
            )
            .unwrap()
            .records
            .is_empty());
        }
        // Rewrite the still-pending first line in place, preserving inode/length
        // and the bytes preceding it. Its old block index is no longer valid.
        let before = read_source(&path, OutputSource::Claude, Some("session"), None).unwrap();
        std::fs::write(
            &path,
            format!("{}\n", line.to_string().replace("block", "fresh")),
        )
        .unwrap();
        let reset = read_source(
            &path,
            OutputSource::Claude,
            Some("session"),
            Some(&before.cursor),
        )
        .unwrap();
        assert!(reset.reset);
        assert!(reset.gaps.iter().any(|gap| gap.code == "source_reset"));
        assert_eq!(reset.records[0].source_item_id, "message:0");
        assert!(
            matches!(&reset.records[0].event, ConversationEvent::ItemCompleted {
            item: crate::chat::types::ConversationItem::Message {text, ..}, ..
        } if text == "fresh 0")
        );
    }

    #[test]
    fn native_normalized_payload_pages_and_skips_only_individually_oversized_records() {
        for source in [OutputSource::Claude, OutputSource::Codex] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("native.jsonl");
            let mut file = std::fs::File::create(&path).unwrap();
            if source == OutputSource::Claude {
                let blocks = (0..24)
                    .map(|index| serde_json::json!({"type":"text","text":format!("text {index}")}))
                    .collect::<Vec<_>>();
                writeln!(file, "{}", serde_json::json!({"type":"assistant","sessionId":"session","uuid":"message","message":{"id":"t".repeat(PAGE_BYTES / 16),"content":blocks}})).unwrap();
                write!(
                    file,
                    "{}",
                    claude(&"x".repeat(PAGE_BYTES / 2), "oversized identity")
                )
                .unwrap();
            } else {
                writeln!(
                    file,
                    "{}",
                    serde_json::json!({"type":"session_meta","payload":{"id":"session"}})
                )
                .unwrap();
                for index in 0..24 {
                    writeln!(file, "{}", serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","id":format!("{index:02}{}", "x".repeat(PAGE_BYTES / 32)),"content":format!("text {index}")}})).unwrap();
                }
                writeln!(file, "{}", serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","id":"x".repeat(PAGE_BYTES / 2 + 1),"content":"oversized identity"}})).unwrap();
            }
            let ending = if source == OutputSource::Claude {
                claude("ending", "reachable")
            } else {
                format!(
                    "{}\n",
                    serde_json::json!({"type":"response_item","payload":{"id":"ending","type":"message","role":"assistant","content":"reachable"}})
                )
            };
            write!(file, "{ending}").unwrap();
            let mut cursor = None;
            let mut seen = std::collections::BTreeSet::new();
            let mut gaps = Vec::new();
            let mut finished = false;
            for _ in 0..10 {
                let page = read_source(&path, source, Some("session"), cursor.as_ref()).unwrap();
                assert!(page.records.len() <= PAGE_RECORDS);
                assert!(
                    page.records
                        .iter()
                        .map(|record| serde_json::to_vec(record).unwrap().len())
                        .sum::<usize>()
                        <= PAGE_BYTES
                );
                for record in page.records {
                    assert!(
                        seen.insert(record.source_item_id),
                        "a paginated record must not replay"
                    );
                }
                gaps.extend(page.gaps);
                cursor = Some(page.cursor);
                if !page.has_more {
                    finished = true;
                    break;
                }
            }
            assert!(finished);
            assert_eq!(seen.len(), 25);
            assert_eq!(gaps.len(), 1);
            assert_eq!(gaps[0].code, "record_too_large");
            assert!(seen.iter().any(|id| id.starts_with("ending")));
            assert!(read_source(&path, source, Some("session"), cursor.as_ref())
                .unwrap()
                .records
                .is_empty());
        }
    }

    #[test]
    fn opencode_payload_paging_does_not_commit_an_unread_revision() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.db");
        let writer = rusqlite::Connection::open(&path).unwrap();
        writer.execute_batch("CREATE TABLE session(id TEXT PRIMARY KEY,time_created INTEGER); CREATE TABLE message(id TEXT PRIMARY KEY,session_id TEXT,data TEXT); CREATE TABLE part(id TEXT PRIMARY KEY,message_id TEXT,session_id TEXT,time_updated INTEGER,data TEXT); INSERT INTO session VALUES('session',1);").unwrap();
        let message = "m".repeat(PAGE_BYTES / 4);
        writer
            .execute(
                "INSERT INTO message VALUES(?1,'session','{\"role\":\"assistant\"}')",
                [&message],
            )
            .unwrap();
        for index in 0..6 {
            writer.execute("INSERT INTO part VALUES(?1,?2,'session',1,?3)", rusqlite::params![format!("part{index}"), message, serde_json::json!({"type":"text","text":format!("text {index}"),"time":{"end":1}}).to_string()]).unwrap();
        }
        let first = read_source(&path, OutputSource::OpenCode, Some("session"), None).unwrap();
        assert!(first.has_more);
        assert_eq!(first.records.len(), 3);
        let second = read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            Some(&first.cursor),
        )
        .unwrap();
        assert!(!second.has_more);
        assert_eq!(second.records.len(), 3);
        for (index, record) in first.records.iter().chain(&second.records).enumerate() {
            assert_eq!(record.source_item_id, format!("part{index}"));
        }
        for page in [&first, &second] {
            assert!(page.gaps.is_empty());
            assert!(
                page.records
                    .iter()
                    .map(|record| serde_json::to_vec(record).unwrap().len())
                    .sum::<usize>()
                    <= PAGE_BYTES
            );
        }
        assert!(read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            Some(&second.cursor)
        )
        .unwrap()
        .records
        .is_empty());
    }

    #[test]
    fn native_tail_receives_output_while_history_remains_unread() {
        for source in [OutputSource::Claude, OutputSource::Codex] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("native.jsonl");
            let mut file = std::fs::File::create(&path).unwrap();
            if source == OutputSource::Codex {
                writeln!(
                    file,
                    "{}",
                    serde_json::json!({"type":"session_meta","payload":{"id":"session"}})
                )
                .unwrap();
            }
            let record = |id: &str| {
                if source == OutputSource::Claude {
                    claude(id, id)
                } else {
                    format!(
                        "{}\n",
                        serde_json::json!({"type":"response_item","payload":{"id":id,"type":"message","role":"assistant","content":id}})
                    )
                }
            };
            for index in 0..PAGE_RECORDS * 3 {
                write!(file, "{}", record(&format!("history-{index}"))).unwrap();
            }
            let history = read_source(&path, source, Some("session"), None).unwrap();
            assert!(history.has_more);
            let unfinished = record("arriving");
            write!(file, "{}", &unfinished[..unfinished.len() - 1]).unwrap();
            let live = tail_cursor(&path, source, Some("session")).unwrap();
            assert!(read_source(&path, source, Some("session"), Some(&live))
                .unwrap()
                .records
                .is_empty());
            writeln!(file).unwrap();
            let output = read_source(&path, source, Some("session"), Some(&live)).unwrap();
            assert_eq!(output.records.len(), 1);
            assert!(output.records[0].source_item_id.starts_with("arriving"));
            let older = read_source(&path, source, Some("session"), Some(&history.cursor)).unwrap();
            assert!(older.has_more);
            assert!(older
                .records
                .iter()
                .all(|record| record.source_item_id.starts_with("history-")));
            assert!(
                read_source(&path, source, Some("session"), Some(&output.cursor))
                    .unwrap()
                    .records
                    .is_empty()
            );
            assert!(tail_cursor(&path, source, Some("wrong-session")).is_err());
            std::fs::write(&path, record("replacement")).unwrap();
            if source == OutputSource::Claude {
                let reset =
                    read_source(&path, source, Some("session"), Some(&output.cursor)).unwrap();
                assert!(reset.reset);
                assert_eq!(reset.records[0].source_item_id, "replacement:0");
            } else {
                assert!(read_source(&path, source, Some("session"), Some(&output.cursor)).is_err());
            }
        }
    }

    #[test]
    fn journal_tail_preserves_the_latest_attempt_capture_mode() {
        use crate::run_record::{EventEnvelope, RunEvent, SCHEMA_VERSION};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut file = std::fs::File::create(&path).unwrap();
        let mut seq = 0;
        let mut append = |event| {
            writeln!(
                file,
                "{}",
                serde_json::to_string(&EventEnvelope {
                    schema_version: SCHEMA_VERSION,
                    seq,
                    observed_at: time::OffsetDateTime::now_utc(),
                    event,
                })
                .unwrap()
            )
            .unwrap();
            seq += 1;
        };
        for _ in 0..PAGE_RECORDS * 2 {
            append(RunEvent::Text {
                text: "old summary capture".into(),
            });
        }
        append(RunEvent::ProviderAttemptStarted {
            provider: "codex".into(),
            model: None,
            account_id: None,
            attempt_key: "retry".into(),
        });
        append(RunEvent::ProviderOutput {
            stream: "stdout".into(),
            line: "x".repeat(PAGE_BYTES),
        });
        append(RunEvent::Conversation {
            event: Box::new(ConversationEvent::TextDelta {
                turn_id: "turn".into(),
                content: "normalized history".into(),
            }),
        });
        let live = tail_cursor(&path, OutputSource::Journal, None).unwrap();
        // Start between a normalized event and its summary mirror.
        append(RunEvent::Text {
            text: "mirror".into(),
        });
        append(RunEvent::Conversation {
            event: Box::new(ConversationEvent::TextDelta {
                turn_id: "turn".into(),
                content: "new output".into(),
            }),
        });
        let page = read_source(&path, OutputSource::Journal, None, Some(&live)).unwrap();
        assert_eq!(page.records.len(), 1);
        assert!(page.gaps.is_empty());
        assert!(
            matches!(&page.records[0].event, ConversationEvent::TextDelta {content, ..} if content == "new output")
        );
    }

    #[test]
    fn opencode_tail_retains_old_unfinished_parts_and_boundary_revisions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.db");
        let writer = rusqlite::Connection::open(&path).unwrap();
        writer.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE session(id TEXT PRIMARY KEY,time_created INTEGER); CREATE TABLE message(id TEXT PRIMARY KEY,session_id TEXT,data TEXT); CREATE TABLE part(id TEXT PRIMARY KEY,message_id TEXT,session_id TEXT,time_updated INTEGER,data TEXT); INSERT INTO session VALUES('session',1); INSERT INTO message VALUES('message','session','{\"role\":\"assistant\"}'); INSERT INTO part VALUES('unfinished','message','session',1,'{\"type\":\"text\",\"text\":\"in progress\"}');").unwrap();
        for index in 2..=400 {
            writer
                .execute(
                    "INSERT INTO part VALUES(?1,'message','session',?2,?3)",
                    rusqlite::params![
                        format!("part{index}"),
                        index,
                        serde_json::json!({"type":"text","text":"history","time":{"end":index}})
                            .to_string()
                    ],
                )
                .unwrap();
        }
        let history = read_source(&path, OutputSource::OpenCode, Some("session"), None).unwrap();
        assert!(history.has_more);
        let live = tail_cursor(&path, OutputSource::OpenCode, Some("session")).unwrap();
        writer.execute("UPDATE part SET data=?1 WHERE id='unfinished'", [serde_json::json!({"type":"text","text":"finished without a timestamp change","time":{"end":400}}).to_string()]).unwrap();
        let first =
            read_source(&path, OutputSource::OpenCode, Some("session"), Some(&live)).unwrap();
        assert_eq!(
            first
                .records
                .iter()
                .map(|record| record.source_item_id.as_str())
                .collect::<Vec<_>>(),
            ["unfinished", "part400"]
        );
        writer.execute("UPDATE part SET data=?1 WHERE id='part400'", [serde_json::json!({"type":"text","text":"same timestamp edit","time":{"end":400}}).to_string()]).unwrap();
        let changed = read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            Some(&first.cursor),
        )
        .unwrap();
        assert_eq!(changed.records.len(), 1);
        assert_eq!(changed.records[0].source_item_id, "part400");
        assert_ne!(changed.records[0].revision, first.records[1].revision);
        assert!(read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            Some(&changed.cursor)
        )
        .unwrap()
        .records
        .is_empty());
        assert!(
            read_source(
                &path,
                OutputSource::OpenCode,
                Some("session"),
                Some(&history.cursor)
            )
            .unwrap()
            .has_more
        );
    }

    #[test]
    fn native_tools_preserve_call_identity_across_pages_and_reversed_results() {
        use crate::chat::types::{ConversationItem, Lifecycle};
        use serde_json::json;

        for source in [OutputSource::Claude, OutputSource::Codex] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("native.jsonl");
            let mut file = std::fs::File::create(&path).unwrap();
            if source == OutputSource::Codex {
                writeln!(
                    file,
                    "{}",
                    json!({"type":"session_meta","payload":{"id":"session"}})
                )
                .unwrap();
            }
            for (index, call) in ["call-one", "call-two"].iter().enumerate() {
                let value = if source == OutputSource::Claude {
                    json!({"type":"assistant","sessionId":"session","uuid":format!("call-{index}"),"message":{"content":[{"type":"tool_use","id":call,"name":"shell","input":{"command":call}}]}})
                } else {
                    json!({"type":"response_item","payload":{"type":"function_call","call_id":call,"name":"shell","arguments":call}})
                };
                writeln!(file, "{value}").unwrap();
            }
            let calls = read_source(&path, source, Some("session"), None).unwrap();
            assert!(calls.gaps.is_empty());
            assert_eq!(calls.records.len(), 2);
            for (index, call) in ["call-two", "call-one"].iter().enumerate() {
                let value = if source == OutputSource::Claude {
                    json!({"type":"user","sessionId":"session","uuid":format!("result-{index}"),"message":{"content":[{"type":"tool_result","tool_use_id":call,"content":call}]}})
                } else {
                    json!({"type":"response_item","payload":{"type":"function_call_output","call_id":call,"output":call}})
                };
                writeln!(file, "{value}").unwrap();
            }
            let results = read_source(&path, source, Some("session"), Some(&calls.cursor)).unwrap();
            assert!(results.gaps.is_empty());
            assert_eq!(results.records.len(), 2);
            for (call, result) in calls.records.iter().zip(results.records.iter().rev()) {
                let ConversationEvent::ItemCompleted {
                    item: ConversationItem::Tool { id, status, .. },
                    ..
                } = &call.event
                else {
                    panic!("expected tool call")
                };
                let ConversationEvent::ItemCompleted {
                    item:
                        ConversationItem::Tool {
                            id: result_id,
                            output,
                            status: result_status,
                            ..
                        },
                    ..
                } = &result.event
                else {
                    panic!("expected tool result")
                };
                assert_eq!(id, result_id);
                assert_eq!(output.as_ref(), Some(id));
                assert_eq!(*status, Lifecycle::Running);
                assert_eq!(*result_status, Lifecycle::Completed);
                assert_ne!(call.source_item_id, result.source_item_id);
            }
            assert!(
                read_source(&path, source, Some("session"), Some(&results.cursor))
                    .unwrap()
                    .records
                    .is_empty()
            );
        }
    }

    #[test]
    fn oversized_record_reports_a_gap_without_stalling_later_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(&vec![b'x'; PAGE_BYTES + 7]).unwrap();
        writeln!(file).unwrap();
        write!(file, "{}", claude("after", "readable output")).unwrap();
        let page = read_source(&path, OutputSource::Claude, Some("session"), None).unwrap();
        assert_eq!(page.gaps[0].code, "record_too_large");
        assert!(page.has_more);
        let next = read_source(
            &path,
            OutputSource::Claude,
            Some("session"),
            Some(&page.cursor),
        )
        .unwrap();
        assert!(next.gaps.is_empty());
        assert_eq!(next.records.len(), 1);
        assert_eq!(next.records[0].source_item_id, "after:0");
        assert!(!next.has_more);
    }

    #[test]
    fn journal_outputs_one_encoding_per_attempt_and_never_raw_copies() {
        use crate::run_record::{EventEnvelope, RunEvent, SCHEMA_VERSION};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let mut file = std::fs::File::create(&path).unwrap();
        let events = [
            RunEvent::Conversation {
                event: Box::new(ConversationEvent::TextDelta {
                    turn_id: "turn".into(),
                    content: "normalized prose".into(),
                }),
            },
            RunEvent::Text {
                text: "summary mirror".into(),
            },
            RunEvent::ProviderOutput {
                stream: "stdout".into(),
                line: "raw mirror".into(),
            },
            RunEvent::ProviderAttemptStarted {
                provider: "claude".into(),
                model: None,
                account_id: None,
                attempt_key: "retry".into(),
            },
            RunEvent::Text {
                text: "retry prose".into(),
            },
            RunEvent::ToolUse {
                name: "shell".into(),
                summary: "pwd".into(),
            },
            RunEvent::Result {
                outcome: "failed".into(),
                duration_secs: Some(1.0),
            },
        ];
        for (seq, event) in events.into_iter().enumerate() {
            let envelope = EventEnvelope {
                schema_version: SCHEMA_VERSION,
                seq: seq as u64,
                observed_at: time::OffsetDateTime::now_utc(),
                event,
            };
            writeln!(file, "{}", serde_json::to_string(&envelope).unwrap()).unwrap();
        }
        let page = read_source(&path, OutputSource::Journal, None, None).unwrap();
        assert_eq!(page.gaps.len(), 1);
        assert_eq!(page.gaps[0].code, "limited_capture");
        assert_eq!(
            page.records
                .iter()
                .map(|r| r.source_item_id.as_str())
                .collect::<Vec<_>>(),
            ["0", "4", "5", "6"]
        );
        assert!(matches!(
            &page.records[3].event,
            ConversationEvent::TurnCompleted {
                status: crate::chat::types::Lifecycle::Failed,
                ..
            }
        ));
        assert!(
            read_source(&path, OutputSource::Journal, None, Some(&page.cursor))
                .unwrap()
                .records
                .is_empty()
        );
    }

    #[test]
    fn native_jsonl_waits_for_complete_lines_and_reports_gaps_and_resets() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.jsonl");
        let first = claude("one", "live prose");
        let second = claude("two", "more prose");
        std::fs::write(&path, format!("{first}{}", &second[..second.len() - 1])).unwrap();
        let page = read_source(&path, OutputSource::Claude, Some("session"), None).unwrap();
        assert_eq!(page.records.len(), 1);
        assert_eq!(page.cursor.offset, first.len() as u64);
        assert!(!page.has_more);
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writeln!(file).unwrap();
        writeln!(file, "malformed").unwrap();
        let next = read_source(
            &path,
            OutputSource::Claude,
            Some("session"),
            Some(&page.cursor),
        )
        .unwrap();
        assert_eq!(next.records.len(), 1);
        assert_eq!(next.records[0].source_item_id, "two:0");
        assert_eq!(next.gaps[0].code, "malformed_record");
        let quiet = read_source(
            &path,
            OutputSource::Claude,
            Some("session"),
            Some(&next.cursor),
        )
        .unwrap();
        assert!(quiet.records.is_empty());
        assert!(quiet.gaps.is_empty());
        std::fs::write(&path, claude("three", "replacement")).unwrap();
        let reset = read_source(
            &path,
            OutputSource::Claude,
            Some("session"),
            Some(&next.cursor),
        )
        .unwrap();
        assert!(reset.reset);
        assert_eq!(reset.records[0].source_item_id, "three:0");
        assert!(read_source(
            &path,
            OutputSource::Claude,
            Some("other-session"),
            Some(&reset.cursor)
        )
        .is_err());
    }

    #[test]
    fn native_jsonl_pages_without_rereading_or_rendering_codex_mirrors() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(
            file,
            "{}",
            serde_json::json!({"type":"session_meta","payload":{"id":"session"}})
        )
        .unwrap();
        for _ in 0..PAGE_RECORDS + 3 {
            writeln!(file, "{}", serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"prose"}]}})).unwrap();
        }
        for (kind, payload) in [
            (
                "event_msg",
                serde_json::json!({"type":"agent_message","message":"mirror"}),
            ),
            (
                "response_item",
                serde_json::json!({"type":"reasoning","encrypted_content":"private"}),
            ),
            (
                "response_item",
                serde_json::json!({"type":"message","role":"developer","content":"internal"}),
            ),
        ] {
            writeln!(
                file,
                "{}",
                serde_json::json!({"type":kind,"payload":payload})
            )
            .unwrap();
        }
        let page = read_source(&path, OutputSource::Codex, Some("session"), None).unwrap();
        assert!(page.has_more);
        assert_eq!(page.records.len(), PAGE_RECORDS - 1);
        let next = read_source(
            &path,
            OutputSource::Codex,
            Some("session"),
            Some(&page.cursor),
        )
        .unwrap();
        assert_eq!(next.records.len(), 4);
        assert!(!next.has_more);
        assert!(next.records.iter().all(|record| !page
            .records
            .iter()
            .any(|old| record.source_item_id == old.source_item_id)));
    }

    #[test]
    fn opencode_completed_history_does_not_accumulate_in_continuation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.db");
        let writer = rusqlite::Connection::open(&path).unwrap();
        writer.execute_batch("CREATE TABLE session(id TEXT PRIMARY KEY,time_created INTEGER); CREATE TABLE message(id TEXT PRIMARY KEY,session_id TEXT,data TEXT); CREATE TABLE part(id TEXT PRIMARY KEY,message_id TEXT,session_id TEXT,time_updated INTEGER,data TEXT); INSERT INTO session VALUES('session',1); INSERT INTO message VALUES('message','session','{\"role\":\"assistant\"}');").unwrap();
        for index in 1..=1000 {
            writer.execute("INSERT INTO part VALUES(?1,'message','session',?2,?3)", rusqlite::params![format!("part{index:04}"), index, serde_json::json!({"type":"text","text":format!("history {index}"),"time":{"end":index}}).to_string()]).unwrap();
        }
        let mut cursor = None;
        let mut seen = std::collections::BTreeSet::new();
        loop {
            let page = read_source(
                &path,
                OutputSource::OpenCode,
                Some("session"),
                cursor.as_ref(),
            )
            .unwrap();
            for record in page.records {
                assert!(
                    seen.insert(record.source_item_id),
                    "history must not replay"
                );
            }
            let encoded = serde_json::to_vec(&page.cursor).unwrap();
            assert!(
                encoded.len() < 2048,
                "completed history must not grow the cursor"
            );
            cursor = Some(serde_json::from_slice(&encoded).unwrap());
            if !page.has_more {
                break;
            }
        }
        assert_eq!(seen.len(), 1000);
        let quiet = read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            cursor.as_ref(),
        )
        .unwrap();
        assert!(quiet.records.is_empty());
        assert!(quiet.gaps.is_empty());
    }

    #[test]
    fn opencode_timestamp_only_update_does_not_replay_unchanged_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.db");
        let writer = rusqlite::Connection::open(&path).unwrap();
        writer.execute_batch("CREATE TABLE session(id TEXT PRIMARY KEY,time_created INTEGER); CREATE TABLE message(id TEXT PRIMARY KEY,session_id TEXT,data TEXT); CREATE TABLE part(id TEXT PRIMARY KEY,message_id TEXT,session_id TEXT,time_updated INTEGER,data TEXT); INSERT INTO session VALUES('session',1); INSERT INTO message VALUES('message','session','{\"role\":\"assistant\"}'); INSERT INTO part VALUES('part','message','session',100,'{\"type\":\"text\",\"text\":\"already shown\",\"time\":{\"end\":100}}');").unwrap();
        let first = read_source(&path, OutputSource::OpenCode, Some("session"), None).unwrap();
        assert_eq!(first.records.len(), 1);
        writer
            .execute("UPDATE part SET time_updated=101 WHERE id='part'", [])
            .unwrap();
        let next = read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            Some(&first.cursor),
        )
        .unwrap();
        assert!(next.records.is_empty());
        let quiet = read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            Some(&next.cursor),
        )
        .unwrap();
        assert!(
            quiet.records.is_empty(),
            "a timestamp change must not replay the same revision"
        );
        assert!(quiet.gaps.is_empty());
    }

    #[test]
    fn opencode_revisions_survive_equal_timestamps_paging_and_compaction() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opencode.db");
        let writer = rusqlite::Connection::open(&path).unwrap();
        writer.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE session(id TEXT PRIMARY KEY,time_created INTEGER); CREATE TABLE message(id TEXT PRIMARY KEY,session_id TEXT,data TEXT); CREATE TABLE part(id TEXT PRIMARY KEY,message_id TEXT,session_id TEXT,time_updated INTEGER,data TEXT); INSERT INTO session VALUES('session',1); INSERT INTO message VALUES('message','session','{\"role\":\"assistant\"}');").unwrap();
        for i in 0..PAGE_RECORDS + 3 {
            writer.execute("INSERT INTO part VALUES(?1,'message','session',100,?2)", rusqlite::params![format!("part{i:04}"), serde_json::json!({"type":"text","text":format!("live {i}"),"time":{"end":100}}).to_string()]).unwrap();
        }
        let first = read_source(&path, OutputSource::OpenCode, Some("session"), None).unwrap();
        assert_eq!(first.records.len(), PAGE_RECORDS);
        assert!(first.has_more);
        // Provider writes through a second connection while the reader retains its cursor.
        writer.execute("UPDATE part SET data=?1 WHERE id='part0000'", [serde_json::json!({"type":"text","text":"updated at identical timestamp","time":{"end":100}}).to_string()]).unwrap();
        writer
            .execute(
                "INSERT INTO part VALUES('new-part','message','session',101,?1)",
                [
                    serde_json::json!({"type":"text","text":"newer output","time":{"end":101}})
                        .to_string(),
                ],
            )
            .unwrap();
        let second = read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            Some(&first.cursor),
        )
        .unwrap();
        assert_eq!(second.records.len(), 3);
        assert!(second.has_more, "newer output waits for the next sweep");
        let third = read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            Some(&second.cursor),
        )
        .unwrap();
        assert!(third.has_more);
        let fourth = read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            Some(&third.cursor),
        )
        .unwrap();
        assert!(!fourth.has_more);
        assert_eq!(fourth.records.len(), 1);
        assert_eq!(fourth.records[0].source_item_id, "new-part");
        assert_eq!(third.records.len(), 1);
        assert_eq!(third.records[0].source_item_id, "part0000");
        assert_ne!(third.records[0].revision, first.records[0].revision);
        assert!(
            matches!(&third.records[0].event, ConversationEvent::ItemCompleted { item: crate::chat::types::ConversationItem::Message { text, .. }, .. } if text == "updated at identical timestamp")
        );
        writer
            .execute("DELETE FROM part WHERE id='part0000'", [])
            .unwrap();
        let reset = read_source(
            &path,
            OutputSource::OpenCode,
            Some("session"),
            Some(&fourth.cursor),
        )
        .unwrap();
        assert!(reset.reset);
        assert_eq!(reset.gaps[0].code, "source_reset");
        assert!(read_source(&path, OutputSource::OpenCode, Some("unknown"), None).is_err());
    }
}

#[cfg(test)]
mod configured_proof {
    use super::{read_source, resolve_native, OutputSource};
    use crate::chat::types::{ConversationEvent, ConversationItem};

    #[tokio::test]
    #[ignore = "requires LF_WATCH_RUN_DIR naming an active, already configured native Session"]
    async fn native_output_arrives_without_replacing_the_owned_client() {
        let dir = std::path::PathBuf::from(
            std::env::var_os("LF_WATCH_RUN_DIR").expect("configured Run directory"),
        );
        let manifest = crate::run_record::read_manifest(&dir).unwrap();
        let session = crate::run_record::read_provider_session(&dir)
            .unwrap()
            .unwrap();
        let registry = dir
            .ancestors()
            .nth(3)
            .expect("Run Home")
            .join("loopflow.db");
        let store = std::sync::Arc::new(crate::store::Store {
            sqlite: crate::store::sqlite::SqliteStore::open_run_ledger_read_only(&registry)
                .unwrap(),
        });
        let clients =
            crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness).unwrap();
        assert!(!clients.is_empty(), "original client must be active");
        let source = match manifest.harness.as_str() {
            "claude" => OutputSource::Claude,
            "codex" => OutputSource::Codex,
            "opencode" => OutputSource::OpenCode,
            _ => panic!("unsupported native provider"),
        };
        let location = resolve_native(&store, &manifest, &session)
            .await
            .unwrap()
            .expect("exact recorded account source");
        let mut cursor = None;
        loop {
            let page = read_source(
                location.path(),
                source,
                Some(&session.provider_session_id),
                cursor.as_ref(),
            )
            .unwrap();
            assert!(page.gaps.is_empty(), "source gaps: {:?}", page.gaps);
            cursor = Some(page.cursor);
            if !page.has_more {
                break;
            }
        }
        let before = cursor.as_ref().unwrap().offset;
        let mut prose = 0;
        let mut tools = 0;
        for _ in 0..45 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let page = read_source(
                location.path(),
                source,
                Some(&session.provider_session_id),
                cursor.as_ref(),
            )
            .unwrap();
            assert!(page.gaps.is_empty(), "source gaps: {:?}", page.gaps);
            for record in page.records {
                match record.event {
                    ConversationEvent::ItemCompleted {
                        item: ConversationItem::Message { .. },
                        ..
                    } => prose += 1,
                    ConversationEvent::ItemCompleted {
                        item: ConversationItem::Tool { .. },
                        ..
                    } => tools += 1,
                    _ => {}
                }
            }
            cursor = Some(page.cursor);
            if prose > 0 && tools > 0 {
                break;
            }
        }
        let remaining =
            crate::lf::commands::util::active_provider_clients(&dir, &manifest.harness).unwrap();
        assert_eq!(
            clients, remaining,
            "passive reads must preserve the exact owned clients"
        );
        eprintln!("native proof: provider={}, new_prose={}, new_tools={}, offset_before={}, offset_after={}, unchanged_owned_clients={}", manifest.harness, prose, tools, before, cursor.unwrap().offset, clients.len());
        assert!(
            prose > 0 && tools > 0,
            "need new prose AND tools while the original client remains active"
        );
    }
}
