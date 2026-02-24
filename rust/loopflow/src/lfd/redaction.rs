use std::path::Path;

/// Redact operator-visible error/status text.
pub fn sanitize_operator_message(message: &str) -> String {
    let mut sanitized = redact_known_paths(message);
    sanitized = redact_bearer_token_segments(&sanitized);
    sanitized = redact_long_secret_segments(&sanitized);
    sanitized = redact_internal_identifiers(&sanitized);
    sanitized
}

fn redact_known_paths(message: &str) -> String {
    let mut sanitized = message.to_string();
    if let Some(home) = dirs::home_dir() {
        let home = home.to_string_lossy().to_string();
        if !home.is_empty() {
            sanitized = sanitized.replace(&home, "[REDACTED_PATH]");
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let cwd = cwd.to_string_lossy().to_string();
        if !cwd.is_empty() {
            sanitized = sanitized.replace(&cwd, "[REDACTED_PATH]");
        }
    }

    for token in extract_absolute_path_tokens(message) {
        sanitized = sanitized.replace(&token, "[REDACTED_PATH]");
    }
    sanitized
}

fn redact_bearer_token_segments(message: &str) -> String {
    let mut result = String::new();
    let mut remaining = message;
    while let Some(idx) = remaining.find("Bearer ") {
        result.push_str(&remaining[..idx]);
        let after = &remaining[idx + "Bearer ".len()..];
        let token = after.split_whitespace().next().unwrap_or_default();
        if token.is_empty() {
            result.push_str("Bearer");
            remaining = after;
        } else {
            result.push_str("Bearer [REDACTED_TOKEN]");
            remaining = &after[token.len()..];
        }
    }
    result.push_str(remaining);
    result
}

fn redact_long_secret_segments(message: &str) -> String {
    message
        .split_whitespace()
        .map(redact_word_if_secret)
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact_word_if_secret(word: &str) -> String {
    let trimmed = word.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == ',');
    let looks_secret = trimmed.len() >= 24
        && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
        && trimmed.chars().any(|ch| ch.is_ascii_digit())
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.');
    if looks_secret {
        word.replace(trimmed, "[REDACTED_TOKEN]")
    } else {
        word.to_string()
    }
}

fn redact_internal_identifiers(message: &str) -> String {
    let mut sanitized = message.to_string();
    for marker in [
        "docker://",
        "/var/lib/docker/volumes/",
        "volume ",
        "container ",
    ] {
        if sanitized.contains(marker) {
            sanitized = sanitized.replace(marker, "[REDACTED_INTERNAL] ");
        }
    }
    sanitized
}

fn extract_absolute_path_tokens(message: &str) -> Vec<String> {
    message
        .split_whitespace()
        .map(|word| word.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == ','))
        .filter(|word| is_absolute_path_token(word))
        .map(str::to_string)
        .collect()
}

fn is_absolute_path_token(word: &str) -> bool {
    !word.is_empty() && Path::new(word).is_absolute()
}

#[cfg(test)]
mod tests {
    use super::sanitize_operator_message;

    #[test]
    fn sanitize_operator_message_redacts_paths_and_tokens() {
        let raw = "failed at /tmp/worktree with Bearer abcdef0123456789abcdef0123456789";
        let sanitized = sanitize_operator_message(raw);
        assert!(!sanitized.contains("/tmp/worktree"));
        assert!(!sanitized.contains("abcdef0123456789abcdef0123456789"));
        assert!(sanitized.contains("[REDACTED_PATH]"));
        assert!(sanitized.contains("[REDACTED_TOKEN]"));
    }

    #[test]
    fn sanitize_operator_message_redacts_home_path() {
        let Some(home) = dirs::home_dir() else {
            return;
        };
        let raw = format!("cannot open {}", home.display());
        let sanitized = sanitize_operator_message(&raw);
        assert!(!sanitized.contains(&home.to_string_lossy().to_string()));
        assert!(sanitized.contains("[REDACTED_PATH]"));
    }
}
