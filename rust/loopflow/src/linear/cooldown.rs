//! One Linear cooldown per Home, shared by repositories and refreshed logins.
//! The lock covers only the deadline file, never network I/O or local saves.
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::time::Duration;

use reqwest::{header::HeaderMap, StatusCode};
use sha2::{Digest, Sha256};

use crate::linear::{LinearError, LinearResult};

#[derive(Debug)]
pub(super) struct Cooldown {
    path: PathBuf,
}

impl Cooldown {
    pub(super) fn for_endpoint(endpoint: &str) -> Self {
        let root = crate::store::lf_home_dir();
        // Stand-in endpoints never touch the installed Home, even in unit tests
        // without a fixture ledger. Each server has its own ephemeral identity.
        #[cfg(test)]
        let root = if endpoint != super::LINEAR_BASE_URL {
            std::env::temp_dir().join(format!("lf-linear-tests-{}", std::process::id()))
        } else {
            root
        };
        Self {
            path: root
                .join("locks/linear-cooldown")
                .join(format!("{:x}", Sha256::digest(endpoint.as_bytes()))),
        }
    }

    async fn lock(&self) -> LinearResult<File> {
        std::fs::create_dir_all(self.path.parent().expect("cooldown has a parent"))
            .map_err(io_error)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .map_err(io_error)?;
        loop {
            match fs2::FileExt::try_lock_exclusive(&file) {
                Ok(()) => return Ok(file),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => return Err(io_error(error)),
            }
        }
    }

    pub(super) async fn check(&self) -> LinearResult<()> {
        let mut file = self.lock().await?;
        let until = read_deadline(&mut file)?;
        if until > now_ms() {
            return Err(paused(until));
        }
        Ok(())
    }

    pub(super) async fn observe(
        &self,
        status: StatusCode,
        headers: &HeaderMap,
        body: &[u8],
    ) -> LinearResult<()> {
        let now = now_ms();
        let parsed = serde_json::from_slice::<serde_json::Value>(body).ok();
        let limited = status == StatusCode::TOO_MANY_REQUESTS
            || parsed
                .as_ref()
                .and_then(|body| body.get("errors"))
                .and_then(|errors| errors.as_array())
                .is_some_and(|errors| {
                    errors.iter().any(|error| {
                        error["extensions"]["code"] == "RATELIMITED"
                            || error["message"].as_str().is_some_and(|message| {
                                message.to_ascii_lowercase().contains("rate limit exceeded")
                            })
                    })
                })
            || (status == StatusCode::BAD_REQUEST
                && String::from_utf8_lossy(body)
                    .to_ascii_lowercase()
                    .contains("rate limit exceeded"));
        let Some(until) = reset_at(headers, limited, now) else {
            return Ok(());
        };
        if until <= now {
            return if limited { Err(paused(until)) } else { Ok(()) };
        }
        let mut file = self.lock().await?;
        let previous = read_deadline(&mut file)?;
        if until > previous {
            file.seek(SeekFrom::Start(0)).map_err(io_error)?;
            file.set_len(0).map_err(io_error)?;
            write!(file, "{until}").map_err(io_error)?;
            file.sync_data().map_err(io_error)?;
        }
        if previous <= now {
            tracing::warn!(reset_epoch_ms = until, "Linear rate limit reached; synchronization paused until reset; pending writes retained");
        }
        // A successful final request still owns its response and must settle.
        if limited {
            return Err(paused(until.max(previous)));
        }
        Ok(())
    }
}

fn now_ms() -> i64 {
    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
}

fn io_error(error: std::io::Error) -> LinearError {
    LinearError::Message(format!("Linear cooldown could not be saved/read: {error}"))
}

fn paused(until: i64) -> LinearError {
    LinearError::RateLimited { reset_at_ms: until }
}

fn read_deadline(file: &mut File) -> LinearResult<i64> {
    let mut value = String::new();
    file.read_to_string(&mut value).map_err(io_error)?;
    if value.is_empty() {
        return Ok(0);
    }
    value
        .parse()
        .map_err(|_| LinearError::Message("Invalid saved Linear cooldown deadline".into()))
}

fn reset_at(headers: &HeaderMap, limited: bool, now: i64) -> Option<i64> {
    let number = |key: &str| headers.get(key)?.to_str().ok()?.parse::<i64>().ok();
    let mut reset: Option<i64> = None;
    let mut exhausted = limited;
    for scope in ["requests", "endpoint-requests", "complexity"] {
        let empty = number(&format!("x-ratelimit-{scope}-remaining")).is_some_and(|n| n <= 0);
        exhausted |= empty;
        if empty || limited {
            if let Some(value) =
                number(&format!("x-ratelimit-{scope}-reset")).filter(|value| *value >= 0)
            {
                let value = value.max(now);
                reset = Some(reset.unwrap_or(value).max(value));
            }
        }
    }
    if !exhausted {
        return None;
    }
    let retry_after = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok());
    if let Some(value) = retry_after.and_then(|value| {
        value
            .parse::<i64>()
            .ok()
            .filter(|delay| *delay >= 0)
            .map(|delay| now.saturating_add(delay.saturating_mul(1000)))
            .or_else(|| {
                chrono::DateTime::parse_from_rfc2822(value)
                    .ok()
                    .map(|date| date.timestamp_millis().max(now))
            })
    }) {
        reset = Some(reset.unwrap_or(value).max(value));
    }
    // An exhausted response with a stale reset or Retry-After: 0 still needs
    // a cooldown. Otherwise each caller immediately sends another request.
    Some(
        reset
            .filter(|until| *until > now)
            .unwrap_or(now + 3_600_000),
    )
}

#[cfg(test)]
mod tests {
    use super::{reset_at, Cooldown};
    use crate::linear::test_server::{self, json_response};
    use crate::linear::LinearClient;
    use reqwest::{header::HeaderMap, StatusCode};
    use serde_json::json;

    #[tokio::test]
    async fn rate_limit_pauses_independent_clients_and_resumes_after_reset() {
        let reset =
            (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) + 3_600_000;
        let mut limited = json_response(
            StatusCode::BAD_REQUEST,
            json!({"errors":[{"message":"Rate limit exceeded", "extensions":{"code":"RATELIMITED"}}]}),
        );
        limited
            .headers
            .push(("x-ratelimit-requests-reset".into(), reset.to_string()));
        let (url, requests) = test_server::spawn(vec![limited,
            json_response(StatusCode::OK, json!({"data":{"initiatives":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}))]).await;
        let first = LinearClient::with_base_url("first-token".into(), None, url.clone());
        assert!(matches!(
            first.initiatives().await,
            Err(crate::linear::LinearError::RateLimited { .. })
        ));
        let second = LinearClient::with_base_url("refreshed-token".into(), None, url.clone());
        assert!(matches!(
            second.initiatives().await,
            Err(crate::linear::LinearError::RateLimited { .. })
        ));
        assert_eq!(requests.lock().await.len(), 1);
        let cooldown = Cooldown::for_endpoint(&url);
        assert_eq!(
            std::fs::read_to_string(&cooldown.path).unwrap(),
            reset.to_string()
        );
        // Exercise expiration without requiring both requests to finish in 250 ms.
        let expired = (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) - 60_000;
        std::fs::write(&cooldown.path, expired.to_string()).unwrap();
        assert!(second.initiatives().await.unwrap().is_empty());
        assert_eq!(requests.lock().await.len(), 2);
    }

    #[test]
    fn cooldown_uses_reset_milliseconds_retry_after_and_safe_missing_reset() {
        let mut headers = HeaderMap::new();
        assert_eq!(reset_at(&headers, false, 1000), None);
        assert_eq!(reset_at(&headers, true, 1000), Some(3_601_000));
        headers.insert("retry-after", "2".parse().unwrap());
        assert_eq!(reset_at(&headers, true, 1000), Some(3000));
        headers.insert(
            "x-ratelimit-endpoint-requests-remaining",
            "0".parse().unwrap(),
        );
        headers.insert(
            "x-ratelimit-endpoint-requests-reset",
            "5000".parse().unwrap(),
        );
        assert_eq!(reset_at(&headers, false, 1000), Some(5000));
        headers.remove("retry-after");
        assert_eq!(reset_at(&headers, true, 6000), Some(3_606_000));
        headers.clear();
        headers.insert("retry-after", "0".parse().unwrap());
        assert_eq!(reset_at(&headers, true, 6000), Some(3_606_000));
        headers.clear();
        headers.insert(
            "retry-after",
            "Thu, 01 Jan 1970 00:00:10 GMT".parse().unwrap(),
        );
        assert_eq!(reset_at(&headers, true, 1000), Some(10000));
    }
}
