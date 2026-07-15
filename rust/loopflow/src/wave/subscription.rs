use std::ops::ControlFlow;

use anyhow::Result;
use futures_util::StreamExt;

/// One SSE frame off the wire: the event name and its joined data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub event: String,
    pub data: String,
}

/// Incremental SSE parser, fed raw bytes. Comment lines (keep-alive pings)
/// drop; `event:` names the pending frame; `data:` lines accumulate joined by
/// `\n`; the blank line emits the frame when it carries data.
#[derive(Debug, Default)]
pub struct SseFrameParser {
    line: Vec<u8>,
    event: String,
    data: String,
}

impl SseFrameParser {
    pub fn push(&mut self, byte: u8) -> Option<Frame> {
        if byte != b'\n' {
            self.line.push(byte);
            return None;
        }
        if self.line.last() == Some(&b'\r') {
            self.line.pop();
        }
        let line = String::from_utf8_lossy(&self.line).into_owned();
        self.line.clear();
        self.consume_line(&line)
    }

    fn consume_line(&mut self, line: &str) -> Option<Frame> {
        if line.is_empty() {
            let frame = (!self.data.is_empty()).then(|| Frame {
                event: std::mem::take(&mut self.event),
                data: std::mem::take(&mut self.data),
            });
            self.event.clear();
            self.data.clear();
            return frame;
        }
        if line.starts_with(':') {
            return None;
        }
        if let Some(name) = line.strip_prefix("event:") {
            self.event = name.trim().to_string();
        } else if let Some(chunk) = line.strip_prefix("data:") {
            let chunk = chunk.strip_prefix(' ').unwrap_or(chunk);
            if !self.data.is_empty() {
                self.data.push('\n');
            }
            self.data.push_str(chunk);
        }
        None
    }
}

/// Follow one `/events` connection until it ends, handing every frame to
/// `on_frame`. `query` is `""` for the mind's thread, or `"?inbox=true"` for
/// the resident's scope. Returning [`ControlFlow::Break`] from `on_frame` closes
/// the connection early — the human thread uses it to reconnect on a `resync`
/// frame. Connection failure and non-2xx are errors.
pub async fn stream_events(
    endpoint: &str,
    query: &str,
    on_frame: &mut impl FnMut(Frame) -> ControlFlow<()>,
) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{endpoint}/events{query}"))
        .header("Accept", "text/event-stream")
        .send()
        .await?
        .error_for_status()?;
    let mut bytes = response.bytes_stream();
    let mut parser = SseFrameParser::default();
    while let Some(chunk) = bytes.next().await {
        for byte in chunk? {
            if let Some(frame) = parser.push(byte) {
                if on_frame(frame).is_break() {
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}
