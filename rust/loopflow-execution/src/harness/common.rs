use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub(super) struct TurnInProgressGuard {
    flag: Arc<AtomicBool>,
    armed: bool,
}

impl TurnInProgressGuard {
    pub fn new(flag: Arc<AtomicBool>) -> Self {
        Self { flag, armed: true }
    }

    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TurnInProgressGuard {
    fn drop(&mut self) {
        if self.armed {
            self.flag.store(false, Ordering::SeqCst);
        }
    }
}

/// Why a headless turn failed when its agent asked a person something.
pub(super) fn unanswered_question(harness: &str, question: &str) -> String {
    let question = question.trim();
    if question.is_empty() {
        format!("{harness} asked a question with nobody there to answer it")
    } else {
        format!("{harness} asked a question with nobody there to answer it: {question}")
    }
}

pub(super) fn spawn_stderr_logger<R>(
    stderr: R,
    target: &'static str,
    diagnostic: Option<Arc<Mutex<String>>>,
) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                tracing::debug!(stderr_target = target, "{line}");
                if let Some(diagnostic) = &diagnostic {
                    *diagnostic.lock().expect("stderr diagnostic lock poisoned") = line;
                }
            }
        }
    })
}

/// A provider's stderr, kept beside the launch's capture as
/// `<harness>.stderr.log`: its own failures and its hooks' recording gaps.
/// `None` for a launch that has no capture.
pub(super) fn diagnostics(
    config: &crate::agent::AgentConfig,
    harness: &str,
) -> anyhow::Result<Option<std::fs::File>> {
    use anyhow::Context;
    let Some(key) = config.env.get(crate::session_record::CAPTURE_KEY_ENV) else {
        return Ok(None);
    };
    let path = crate::session_record::capture_dir(key)?.join(format!("{harness}.stderr.log"));
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true).read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let log = options
        .open(&path)
        .with_context(|| format!("open {harness} diagnostics {}", path.display()))?;
    Ok(Some(log))
}

/// Redact credential material from an error a provider printed before it is
/// recorded: `Authorization` header values, `Bearer` tokens and common token
/// query parameters an upstream error can carry.
pub(super) fn sanitize_error_message(message: &str) -> String {
    use regex::Regex;
    let mut out = message.to_string();
    // Bearer first so `Authorization: Bearer <token>` is caught by both the
    // bearer pattern and the authorization-header pattern. The bearer pattern
    // stops at whitespace or a comma so the trailing comma that delimits the
    // authorization-header value survives as a stop boundary for the next
    // pattern — otherwise `Bearer <tok>, token=abc123` lets the authorization
    // regex eat through `token=abc123` before the param redaction runs.
    let redactions: [(Regex, &str); 3] = [
        (
            Regex::new(r"(?i)bearer\s+[^,\s]+").expect("valid credential pattern"),
            "bearer [redacted]",
        ),
        (
            Regex::new(r"(?i)authorization:\s*[^\n,]+").expect("valid credential pattern"),
            "authorization: [redacted]",
        ),
        (
            Regex::new(r"(?i)(token|key|access_token|api_key)=\S+")
                .expect("valid credential pattern"),
            "$1=[redacted]",
        ),
    ];
    for (re, replacement) in &redactions {
        out = re.replace_all(&out, *replacement).to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::sanitize_error_message;

    #[test]
    fn sanitize_error_message_redacts_credentials() {
        let input = "request to https://api.example.com/v1/chat?api_key=sk-secret123 failed: \
                     Authorization: Bearer super-secret-token, token=abc123";
        let sanitized = sanitize_error_message(input);
        assert!(
            !sanitized.contains("sk-secret123"),
            "api_key leaked: {sanitized}"
        );
        assert!(
            !sanitized.contains("super-secret-token"),
            "bearer token leaked: {sanitized}"
        );
        assert!(
            !sanitized.contains("abc123"),
            "token param leaked: {sanitized}"
        );
        assert!(sanitized.contains("api_key=[redacted]"));
        assert!(sanitized.contains("authorization: [redacted]"));
        assert!(sanitized.contains("token=[redacted]"));
    }

    #[test]
    fn sanitize_error_message_preserves_non_credential_text() {
        let input = "chunk stream ended (reqwest: EOF)";
        let sanitized = sanitize_error_message(input);
        assert_eq!(sanitized, input);
    }
}
