//! Credentials scrubbed from what a provider printed.

/// Redact credential material from an error a provider printed before it is
/// recorded: `Authorization` header values, `Bearer` tokens and common token
/// query parameters an upstream error can carry.
pub(crate) fn sanitize_error_message(message: &str) -> String {
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
