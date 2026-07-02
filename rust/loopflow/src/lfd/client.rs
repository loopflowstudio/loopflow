//! Minimal blocking HTTP client for local `lf` CLI calls into a running `lfd`.
//!
//! Base URL and auth token resolution mirror the Python client
//! (`python/loopflow/client.py`): env override first, then the local default.

use std::time::Duration;

use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{HeaderValue, AUTHORIZATION};

pub fn resolve_base_url() -> String {
    for key in ["LFD_URL", "LFD_HTTP_ADDR"] {
        if let Ok(raw) = std::env::var(key) {
            return normalize_base_url(&raw);
        }
    }
    "http://127.0.0.1:2486".to_string()
}

fn normalize_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

pub fn auth_header() -> Option<String> {
    for key in ["LFD_TOKEN", "LFD_AUTH_TOKEN"] {
        if let Ok(value) = std::env::var(key) {
            let token = value.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }

    let token = std::fs::read_to_string(crate::lfd::session_token::token_path()).ok()?;
    let trimmed = token.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn authorize(request: RequestBuilder) -> RequestBuilder {
    let Some(token) = auth_header() else {
        return request;
    };
    let value = format!("Bearer {token}");
    let Ok(header) = HeaderValue::from_str(&value) else {
        return request;
    };
    request.header(AUTHORIZATION, header)
}

pub fn blocking_client(timeout: Duration) -> reqwest::Result<Client> {
    Client::builder().timeout(timeout).build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }

    #[test]
    fn resolve_base_url_defaults_to_local_port() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::remove_var("LFD_URL");
        std::env::remove_var("LFD_HTTP_ADDR");
        assert_eq!(resolve_base_url(), "http://127.0.0.1:2486");
    }

    #[test]
    fn resolve_base_url_normalizes_bare_host_port() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::remove_var("LFD_URL");
        std::env::set_var("LFD_HTTP_ADDR", "0.0.0.0:2486/");
        let result = resolve_base_url();
        std::env::remove_var("LFD_HTTP_ADDR");
        assert_eq!(result, "http://0.0.0.0:2486");
    }

    #[test]
    fn resolve_base_url_prefers_lfd_url() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("LFD_URL", "https://example.com/");
        std::env::set_var("LFD_HTTP_ADDR", "127.0.0.1:9999");
        let result = resolve_base_url();
        std::env::remove_var("LFD_URL");
        std::env::remove_var("LFD_HTTP_ADDR");
        assert_eq!(result, "https://example.com");
    }

    #[test]
    fn auth_header_reads_lfd_token_env() {
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("LFD_TOKEN", "abc123");
        let result = auth_header();
        std::env::remove_var("LFD_TOKEN");
        assert_eq!(result.as_deref(), Some("abc123"));
    }
}
