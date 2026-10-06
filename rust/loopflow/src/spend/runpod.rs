//! Read-access evidence only: Runpod billing buckets are not settled invoices.
use std::time::Duration;

use chrono::{Datelike, NaiveDate};
use secrecy::{ExposeSecret, SecretString};

use crate::provider_auth::doppler::{self, DopplerReference};
use crate::spend::AccessOutcome;

const ENDPOINT: &str = "https://api.runpod.io/v2/billing";
const MAX_BODY: usize = 1024 * 1024;

pub(crate) async fn verify(
    reference: &DopplerReference,
    period: &str,
) -> (AccessOutcome, Option<String>) {
    let Some((start, end)) = window(period) else {
        return (
            AccessOutcome::Unavailable,
            Some("invalid billing period; no secret fetched".into()),
        );
    };
    let secret = match doppler::resolve(reference).await {
        Ok(secret) => secret,
        Err(_) => {
            return (
                AccessOutcome::Unavailable,
                Some("Doppler online lookup unavailable or denied".into()),
            )
        }
    };
    let outcome = read(ENDPOINT, &secret, &start, &end).await;
    let gap = match outcome {
        AccessOutcome::Success => "Runpod billing history read succeeded; account identity, read-only enforcement and current credential version unverified; no invoice imported",
        AccessOutcome::Denied => "Runpod billing read denied (401/403); read-only enforcement unverified",
        AccessOutcome::Unavailable => "Runpod billing read unavailable or invalid response; read-only enforcement unverified",
    };
    (outcome, Some(gap.into()))
}

fn window(period: &str) -> Option<(String, String)> {
    if period.len() != 7 {
        return None;
    }
    let start = NaiveDate::parse_from_str(&format!("{period}-01"), "%Y-%m-%d").ok()?;
    let end = start.checked_add_months(chrono::Months::new(1))?;
    if start.year() < 1 || end.year() > 9999 {
        return None;
    }
    Some((format!("{start}T00:00:00Z"), format!("{end}T00:00:00Z")))
}

// The sole production caller supplies ENDPOINT; there is no URL or command override.
async fn read(endpoint: &str, secret: &SecretString, start: &str, end: &str) -> AccessOutcome {
    let Ok(mut url) = reqwest::Url::parse(endpoint) else {
        return AccessOutcome::Unavailable;
    };
    url.query_pairs_mut().extend_pairs([
        ("startTime", start),
        ("endTime", end),
        ("bucketSize", "day"),
    ]);
    tokio::time::timeout(Duration::from_secs(100), async {
        let client = match reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(30))
            .build()
        {
            Ok(client) => client,
            Err(_) => return AccessOutcome::Unavailable,
        };
        for attempt in 0..3 {
            let response = client
                .get(url.clone())
                .bearer_auth(secret.expose_secret())
                .send()
                .await;
            let mut response = match response {
                Ok(response) => response,
                Err(_) if attempt < 2 => continue,
                Err(_) => return AccessOutcome::Unavailable,
            };
            match response.status().as_u16() {
                401 | 403 => return AccessOutcome::Denied,
                429 | 500..=599 if attempt < 2 => {
                    // Do not retry sooner than the server requests. Unrecognized/date-based
                    // Retry-After or a delay beyond this probe's deadline leaves it unavailable.
                    let delay = match response.headers().get(reqwest::header::RETRY_AFTER) {
                        Some(value) => {
                            match value.to_str().ok().and_then(|s| s.parse::<u64>().ok()) {
                                Some(seconds) if seconds <= 30 => seconds,
                                _ => return AccessOutcome::Unavailable,
                            }
                        }
                        None => 1,
                    };
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    continue;
                }
                200 => {}
                _ => return AccessOutcome::Unavailable,
            }
            let mut bytes = Vec::new();
            loop {
                match response.chunk().await {
                    Ok(Some(chunk)) if bytes.len() + chunk.len() <= MAX_BODY => {
                        bytes.extend_from_slice(&chunk)
                    }
                    Ok(None) => break,
                    _ => return AccessOutcome::Unavailable,
                }
            }
            // Parse only to establish a billing response. No response data, error,
            // header or financial amount leaves the probe or becomes invoice evidence.
            let Ok(body) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                return AccessOutcome::Unavailable;
            };
            let records = body["records"].as_array();
            let metadata = &body["metadata"];
            if records.is_some_and(|records| {
                metadata["recordCount"].as_u64() == Some(records.len() as u64)
            }) && metadata["query"]["startTime"] == start
                && metadata["query"]["endTime"] == end
                && metadata["query"]["bucketSize"] == "day"
                && metadata["totals"]["totalAmount"].is_number()
            {
                return AccessOutcome::Success;
            }
            return AccessOutcome::Unavailable;
        }
        AccessOutcome::Unavailable
    })
    .await
    .unwrap_or(AccessOutcome::Unavailable)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use axum::{
        extract::{Query, State},
        http::{HeaderMap, StatusCode},
        response::IntoResponse,
        routing::get,
        Router,
    };
    use secrecy::SecretString;
    use serde_json::json;

    use super::{read, window, MAX_BODY};
    use crate::spend::AccessOutcome;

    async fn serve(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/v2/billing", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (endpoint, server)
    }

    async fn billing(
        State(expected): State<String>,
        headers: HeaderMap,
        Query(query): Query<HashMap<String, String>>,
    ) -> axum::response::Response {
        let authorized = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value == format!("Bearer {expected}"));
        if !authorized {
            return (StatusCode::FORBIDDEN, expected).into_response();
        }
        axum::Json(json!({"records": [], "metadata": {
            "recordCount": 0, "query": query, "totals": {"totalAmount": 0}
        }}))
        .into_response()
    }

    #[tokio::test]
    async fn billing_probe_sanitizes_synthetic_provider_evidence() {
        let value = uuid::Uuid::new_v4().to_string();
        let app = Router::new()
            .route("/v2/billing", get(billing))
            .with_state(value.clone());
        let (endpoint, server) = serve(app).await;
        let (start, end) = window("2026-09").unwrap();
        let outcome = read(&endpoint, &SecretString::new(value.clone()), &start, &end).await;
        assert_eq!(outcome, AccessOutcome::Success);
        assert!(!serde_json::to_string(&outcome).unwrap().contains(&value));
        let denied = read(
            &endpoint,
            &SecretString::new(uuid::Uuid::new_v4().to_string()),
            &start,
            &end,
        )
        .await;
        assert_eq!(denied, AccessOutcome::Denied);
        assert!(!format!("{denied:?}").contains(&value));
        server.abort();
        let _ = server.await;
        assert_eq!(
            read(&endpoint, &SecretString::new(value), &start, &end).await,
            AccessOutcome::Unavailable
        );
    }

    #[tokio::test]
    async fn billing_probe_rejects_redirects_invalid_and_oversized_responses() {
        for (status, oversized) in [
            (StatusCode::UNAUTHORIZED, false),
            (StatusCode::FORBIDDEN, false),
            (StatusCode::FOUND, false),
            (StatusCode::OK, false),
            (StatusCode::OK, true),
            (StatusCode::TOO_MANY_REQUESTS, false),
            (StatusCode::SERVICE_UNAVAILABLE, false),
        ] {
            let value = uuid::Uuid::new_v4().to_string();
            let body = value.clone();
            let redirects = Arc::new(AtomicUsize::new(0));
            let redirected = redirects.clone();
            let attempts = Arc::new(AtomicUsize::new(0));
            let attempted = attempts.clone();
            let app = Router::new()
                .route(
                    "/v2/billing",
                    get(move || {
                        attempted.fetch_add(1, Ordering::SeqCst);
                        let body = if oversized {
                            body.repeat(MAX_BODY / body.len() + 1)
                        } else {
                            body.clone()
                        };
                        async move {
                            (
                                status,
                                [
                                    ("location", "/trap"),
                                    (
                                        "retry-after",
                                        if status == StatusCode::SERVICE_UNAVAILABLE {
                                            "0"
                                        } else {
                                            "999"
                                        },
                                    ),
                                ],
                                body,
                            )
                        }
                    }),
                )
                .route(
                    "/trap",
                    get(move || async move {
                        redirected.fetch_add(1, Ordering::SeqCst);
                        "unexpected redirect"
                    }),
                );
            let (endpoint, server) = serve(app).await;
            let (start, end) = window("2026-09").unwrap();
            let outcome = read(&endpoint, &SecretString::new(value.clone()), &start, &end).await;
            let expected = if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
            {
                AccessOutcome::Denied
            } else {
                AccessOutcome::Unavailable
            };
            assert_eq!(outcome, expected);
            assert_eq!(redirects.load(Ordering::SeqCst), 0);
            assert_eq!(
                attempts.load(Ordering::SeqCst),
                if status == StatusCode::SERVICE_UNAVAILABLE {
                    3
                } else {
                    1
                }
            );
            assert!(!format!("{outcome:?}").contains(&value));
            server.abort();
        }
    }

    #[test]
    fn billing_probe_uses_complete_calendar_months() {
        assert_eq!(
            window("2026-12"),
            Some(("2026-12-01T00:00:00Z".into(), "2027-01-01T00:00:00Z".into()))
        );
        for invalid in ["2026-13", "2026-1", "x", "0000-01", "9999-12"] {
            assert!(window(invalid).is_none());
        }
    }
}
