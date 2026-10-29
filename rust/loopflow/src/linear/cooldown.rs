//! One Linear cooldown and budget per Home, shared by repositories and
//! refreshed logins. The locks cover only these small files, never network I/O
//! or local saves.
//!
//! Every response's `x-ratelimit-*` headers update the budget. As the hourly
//! allowance falls, non-urgent requests are paced and then held, leaving the
//! last tenth for urgent work: Task and Project creation and PR links run
//! inside [`urgent`]. A held request fails as `RateLimited` before it is sent,
//! so callers retain and retry it exactly like one paused by the cooldown.
use std::fs::{File, OpenOptions};
use std::future::Future;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::{header::HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::linear::{LinearError, LinearResult};

/// Below this share of an allowance, only urgent requests are sent.
const RESERVE_PERCENT: i64 = 10;
/// Below this share, non-urgent requests are spaced so what remains above the
/// reserve lasts until the allowance resets.
const PACE_PERCENT: i64 = 25;
/// Seconds a finished scope delta answers reads while the budget is healthy.
pub(crate) const HEALTHY_FRESH_SECS: i64 = 10;
const HEALTHY_FRESH_MS: i64 = HEALTHY_FRESH_SECS * 1000;

tokio::task_local! {
    static URGENT: ();
}

/// Run `work` with access to the reserved budget: requests it makes are never
/// paced or held for the reserve, only paused by an actual rate limit.
pub(crate) async fn urgent<F: Future>(work: F) -> F::Output {
    URGENT.scope((), work).await
}

pub(super) fn is_urgent() -> bool {
    URGENT.try_with(|_| ()).is_ok()
}

/// The Home holding Linear's shared request state for `endpoint`.
pub(super) fn home(endpoint: &str) -> PathBuf {
    let root = crate::store::lf_home_dir();
    // Stand-in endpoints never touch the installed Home, even in unit tests
    // without a fixture ledger. Each server has its own ephemeral Home.
    #[cfg(test)]
    let root = if endpoint != super::LINEAR_BASE_URL {
        std::env::temp_dir()
            .join(format!("lf-linear-tests-{}", std::process::id()))
            .join(endpoint_key(endpoint))
    } else {
        root
    };
    #[cfg(not(test))]
    let _ = endpoint;
    root
}

fn endpoint_key(endpoint: &str) -> String {
    format!("{:x}", Sha256::digest(endpoint.as_bytes()))
}

/// One rate-limit allowance as Linear last reported it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Allowance {
    pub limit: Option<i64>,
    pub remaining: i64,
    /// Epoch milliseconds when the allowance refills.
    pub reset_ms: Option<i64>,
}

/// The request-count and complexity allowances a response reported.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Allowances {
    pub requests: Option<Allowance>,
    pub complexity: Option<Allowance>,
    /// This request's complexity, from `x-complexity`.
    pub cost: Option<i64>,
}

impl Allowances {
    pub(super) fn from_headers(headers: &HeaderMap) -> Self {
        let allowance = |scope: &str| {
            Some(Allowance {
                limit: header_number(headers, &format!("x-ratelimit-{scope}-limit")),
                remaining: header_number(headers, &format!("x-ratelimit-{scope}-remaining"))?,
                reset_ms: header_number(headers, &format!("x-ratelimit-{scope}-reset")),
            })
        };
        Self {
            requests: allowance("requests"),
            complexity: allowance("complexity"),
            cost: header_number(headers, "x-complexity"),
        }
    }
}

/// What this Home last learned of the shared allowance, and when the next
/// paced non-urgent request may go.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Budget {
    requests: Option<Allowance>,
    complexity: Option<Allowance>,
    /// The latest request's complexity, the expected cost of the next one.
    cost: Option<i64>,
    paced_until_ms: i64,
}

impl Budget {
    fn observe(&mut self, allowances: &Allowances) {
        if allowances.requests.is_some() {
            self.requests = allowances.requests;
        }
        if allowances.complexity.is_some() {
            self.complexity = allowances.complexity;
        }
        if allowances.cost.is_some() {
            self.cost = allowances.cost;
        }
    }

    /// How tight the budget is at `now`.
    fn pressure(&self, now: i64) -> Pressure {
        let cost = self.cost.unwrap_or(1).max(1);
        let mut interval = 0;
        for (allowance, cost) in [(self.requests, 1), (self.complexity, cost)] {
            let Some(Allowance {
                limit: Some(limit),
                remaining,
                reset_ms: Some(reset),
            }) = allowance
            else {
                continue;
            };
            if reset <= now || limit <= 0 {
                continue;
            }
            let reserve = limit * RESERVE_PERCENT / 100;
            if remaining <= reserve {
                return Pressure::Reserve { reset_ms: reset };
            }
            if remaining <= limit * PACE_PERCENT / 100 {
                let requests_left = ((remaining - reserve) / cost).max(1);
                interval = interval.max((reset - now) / requests_left);
            }
        }
        if interval > 0 {
            Pressure::Paced {
                interval_ms: interval,
            }
        } else {
            Pressure::Healthy
        }
    }

    /// Admit one non-urgent request, or the time to try again.
    fn admit(&mut self, now: i64) -> Result<(), i64> {
        match self.pressure(now) {
            Pressure::Reserve { reset_ms } => return Err(reset_ms),
            Pressure::Paced { interval_ms } => {
                if self.paced_until_ms > now {
                    return Err(self.paced_until_ms);
                }
                self.paced_until_ms = now + interval_ms;
            }
            Pressure::Healthy => {}
        }
        // Concurrent processes see this send before its response arrives.
        if let Some(requests) = &mut self.requests {
            requests.remaining -= 1;
        }
        Ok(())
    }

    fn fresh_for_ms(&self, now: i64) -> i64 {
        match self.pressure(now) {
            Pressure::Healthy => HEALTHY_FRESH_MS,
            Pressure::Paced { interval_ms } => interval_ms.max(HEALTHY_FRESH_MS),
            Pressure::Reserve { reset_ms } => (reset_ms - now).max(HEALTHY_FRESH_MS),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pressure {
    Healthy,
    Paced { interval_ms: i64 },
    Reserve { reset_ms: i64 },
}

/// Seconds a finished Linear team delta stands in for Linear before reads ask
/// again: 10 while the budget is healthy, one pacing interval while it is
/// paced, and until reset inside the reserve.
pub(crate) fn delta_fresh_secs() -> i64 {
    let budget = File::open(Cooldown::for_endpoint(super::LINEAR_BASE_URL).budget)
        .map(|mut file| read_budget(&mut file))
        .unwrap_or_default();
    budget.fresh_for_ms(now_ms()) / 1000
}

#[derive(Debug)]
pub(super) struct Cooldown {
    path: PathBuf,
    budget: PathBuf,
}

impl Cooldown {
    pub(super) fn for_endpoint(endpoint: &str) -> Self {
        let dir = home(endpoint).join("locks/linear-cooldown");
        let key = endpoint_key(endpoint);
        Self {
            budget: dir.join(format!("{key}.budget")),
            path: dir.join(key),
        }
    }

    pub(super) async fn check(&self) -> LinearResult<()> {
        let mut file = lock(&self.path).await?;
        let until = read_deadline(&mut file)?;
        if until > now_ms() {
            return Err(paused(until));
        }
        Ok(())
    }

    /// Hold a non-urgent request while the shared budget is low.
    pub(super) async fn admit(&self) -> LinearResult<()> {
        let mut file = lock(&self.budget).await?;
        let mut budget = read_budget(&mut file);
        let admitted = budget.admit(now_ms());
        write_budget(&mut file, &budget)?;
        admitted.map_err(paused)
    }

    pub(super) async fn observe(
        &self,
        status: StatusCode,
        headers: &HeaderMap,
        body: &[u8],
    ) -> LinearResult<()> {
        let now = now_ms();
        let allowances = Allowances::from_headers(headers);
        if allowances != Allowances::default() {
            let mut file = lock(&self.budget).await?;
            let mut budget = read_budget(&mut file);
            budget.observe(&allowances);
            write_budget(&mut file, &budget)?;
        }
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
        let mut file = lock(&self.path).await?;
        let previous = read_deadline(&mut file)?;
        if until > previous {
            file.seek(SeekFrom::Start(0)).map_err(io_error)?;
            file.set_len(0).map_err(io_error)?;
            write!(file, "{until}").map_err(io_error)?;
            file.sync_data().map_err(io_error)?;
        }
        if previous <= now {
            tracing::warn!(
                limit = tripped(headers),
                reset_epoch_ms = until,
                "Linear rate limit reached; synchronization paused until reset; pending writes retained"
            );
        }
        // A successful final request still owns its response and must settle.
        if limited {
            return Err(paused(until.max(previous)));
        }
        Ok(())
    }
}

async fn lock(path: &Path) -> LinearResult<File> {
    std::fs::create_dir_all(path.parent().expect("cooldown has a parent")).map_err(io_error)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
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

/// An unreadable budget is an unknown one: admit, and let the next response refill it.
fn read_budget(file: &mut File) -> Budget {
    let mut value = String::new();
    if file.read_to_string(&mut value).is_err() || value.is_empty() {
        return Budget::default();
    }
    serde_json::from_str(&value).unwrap_or_default()
}

fn write_budget(file: &mut File, budget: &Budget) -> LinearResult<()> {
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    file.set_len(0).map_err(io_error)?;
    serde_json::to_writer(&mut *file, budget)
        .map_err(|error| io_error(std::io::Error::other(error)))
}

/// Linear's rate-limit scopes, each with its own `x-ratelimit-<scope>-*` headers.
const SCOPES: [&str; 3] = ["requests", "endpoint-requests", "complexity"];

fn header_number(headers: &HeaderMap, key: &str) -> Option<i64> {
    headers.get(key)?.to_str().ok()?.trim().parse().ok()
}

fn exhausted(headers: &HeaderMap, scope: &str) -> bool {
    header_number(headers, &format!("x-ratelimit-{scope}-remaining")).is_some_and(|n| n <= 0)
}

/// Which allowances a paused response ran out of.
fn tripped(headers: &HeaderMap) -> String {
    let scopes: Vec<_> = SCOPES
        .into_iter()
        .filter(|scope| exhausted(headers, scope))
        .collect();
    if scopes.is_empty() {
        "unreported".into()
    } else {
        scopes.join(",")
    }
}

pub(super) fn now_ms() -> i64 {
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
    let mut reset: Option<i64> = None;
    let mut any_exhausted = limited;
    for scope in SCOPES {
        let empty = exhausted(headers, scope);
        any_exhausted |= empty;
        if empty || limited {
            if let Some(value) = header_number(headers, &format!("x-ratelimit-{scope}-reset"))
                .filter(|value| *value >= 0)
            {
                let value = value.max(now);
                reset = Some(reset.unwrap_or(value).max(value));
            }
        }
    }
    if !any_exhausted {
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
    use super::{home, reset_at, urgent, Allowance, Budget, Cooldown};
    use crate::linear::request_log::{self, RequestOutcome};
    use crate::linear::test_server::{self, json_response, QueuedResponse};
    use crate::linear::LinearClient;
    use reqwest::{header::HeaderMap, StatusCode};
    use serde_json::json;

    fn initiatives(remaining: i64, reset: i64) -> QueuedResponse {
        let mut response = json_response(
            StatusCode::OK,
            json!({"data":{"initiatives":{"nodes":[],"pageInfo":{"hasNextPage":false,"endCursor":null}}}}),
        );
        for (key, value) in [
            ("x-ratelimit-requests-limit", 1000),
            ("x-ratelimit-requests-remaining", remaining),
            ("x-ratelimit-requests-reset", reset),
            ("x-complexity", 3),
        ] {
            response.headers.push((key.into(), value.to_string()));
        }
        response
    }

    fn turn_on_request_log(url: &str) {
        let home = home(url);
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("config.yaml"), "logs:\n  linear_requests: true\n").unwrap();
    }

    #[tokio::test]
    async fn request_log_is_off_by_default() {
        let reset = super::now_ms() + 3_600_000;
        let (url, requests) = test_server::spawn(vec![initiatives(900, reset)]).await;
        let client = LinearClient::with_base_url("token".into(), None, url.clone());
        client.initiatives().await.unwrap();
        assert_eq!(requests.lock().await.len(), 1);
        assert!(!request_log::log_path(&home(&url)).exists());
    }

    #[tokio::test]
    async fn request_log_records_operation_outcome_and_allowance() {
        let reset = super::now_ms() + 3_600_000;
        let (url, _) = test_server::spawn(vec![initiatives(900, reset)]).await;
        turn_on_request_log(&url);
        let client = LinearClient::with_base_url("token".into(), None, url.clone());
        client.initiatives().await.unwrap();
        let records = request_log::read_since(&home(&url), 0).unwrap();
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.operation, "ListInitiatives");
        assert_eq!(record.outcome, RequestOutcome::Sent);
        assert_eq!(record.status, Some(200));
        assert_eq!(record.cost, Some(3));
        assert_eq!(
            record.requests,
            Some(Allowance {
                limit: Some(1000),
                remaining: 900,
                reset_ms: Some(reset),
            })
        );
    }

    #[tokio::test]
    async fn low_budget_holds_non_urgent_requests_and_sends_urgent_ones() {
        let reset = super::now_ms() + 3_600_000;
        // The first response leaves 5% of the hour's requests: inside the reserve.
        let (url, requests) =
            test_server::spawn(vec![initiatives(50, reset), initiatives(49, reset)]).await;
        turn_on_request_log(&url);
        let client = LinearClient::with_base_url("token".into(), None, url.clone());
        client.initiatives().await.unwrap();
        assert_eq!(
            client.initiatives().await.unwrap_err(),
            crate::linear::LinearError::RateLimited { reset_at_ms: reset }
        );
        assert_eq!(requests.lock().await.len(), 1);
        assert!(urgent(client.initiatives()).await.unwrap().is_empty());
        assert_eq!(requests.lock().await.len(), 2);
        let outcomes: Vec<_> = request_log::read_since(&home(&url), 0)
            .unwrap()
            .into_iter()
            .map(|record| (record.outcome, record.urgent))
            .collect();
        assert_eq!(
            outcomes,
            [
                (RequestOutcome::Sent, false),
                (RequestOutcome::Deferred, false),
                (RequestOutcome::Sent, true),
            ]
        );
    }

    #[test]
    fn falling_budget_spaces_non_urgent_requests_until_reset() {
        let allowance = |remaining| Allowance {
            limit: Some(1000),
            remaining,
            reset_ms: Some(1_000_000),
        };
        let mut budget = Budget {
            requests: Some(allowance(500)),
            ..Budget::default()
        };
        assert_eq!(budget.admit(0), Ok(()));
        assert_eq!(budget.admit(0), Ok(()));
        // 20% left: 100 requests above the reserve share the 1000 s until reset.
        budget.requests = Some(allowance(200));
        assert_eq!(budget.admit(0), Ok(()));
        assert_eq!(budget.admit(1), Err(10_000));
        assert_eq!(budget.admit(10_000), Ok(()));
        budget.requests = Some(allowance(100));
        assert_eq!(budget.admit(20_000), Err(1_000_000));
        // A reset allowance no longer holds anything back.
        assert_eq!(budget.admit(1_000_000), Ok(()));
    }

    #[test]
    fn team_delta_freshness_stretches_as_the_budget_falls() {
        let allowance = |remaining| Allowance {
            limit: Some(1000),
            remaining,
            reset_ms: Some(1_000_000),
        };
        let mut budget = Budget::default();
        assert_eq!(budget.fresh_for_ms(0), 10_000);
        budget.requests = Some(allowance(500));
        assert_eq!(budget.fresh_for_ms(0), 10_000);
        budget.requests = Some(allowance(110));
        assert_eq!(budget.fresh_for_ms(0), 100_000);
        budget.requests = Some(allowance(90));
        assert_eq!(budget.fresh_for_ms(0), 1_000_000);
    }

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
