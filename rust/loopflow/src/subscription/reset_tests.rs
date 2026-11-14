use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{claude_resets, redeem_claude_reset, select_reset};
use crate::profile::EmailAddress;
use crate::provider_account::{new_account, resolve_provider_account_exact};
use crate::provider_auth::Provider;
use crate::store::{AccountLimitWindow, ProviderAccount, ProviderAccountId, SharedStore};

fn usage(fable: u8) -> Value {
    json!({
        "limits":[
            {"group":"weekly","percent":70,"resets_at":"2099-01-01T00:00:00Z"},
            {"group":"weekly","percent":fable,"resets_at":"2099-01-01T00:00:00Z","scope":{"model":{"display_name":"Fable"}}}
        ],
        "cedar_ember":{"eligible":true,"ineligible_reason":null,"grants":[
            {"id":"weekly-reset","label":"Weekly reset","type":"weekly","resets_left":1,"resets_total":1,
             "starts_at":"2026-01-01T00:00:00Z","ends_at":"2099-01-01T00:00:00Z",
             "paused":false,"usable_now":true,"use_requires_limit":false,"gives_back_weekly":false,
             "clears":["seven_day","seven_day_overage_included"],"unavailable_reason":null},
            {"id":"fable-reset","label":"Fable reset","type":"full","resets_left":1,"resets_total":1,
             "ends_at":"2099-01-01T00:00:00Z","paused":false,"usable_now":true,"use_requires_limit":false,
             "gives_back_weekly":true,"clears":["seven_day_overage_included"],"unavailable_reason":null},
            {"id":"spent-reset","label":"Spent","type":"full","resets_left":0,"resets_total":1,
             "ends_at":"2099-01-01T00:00:00Z","paused":false,"usable_now":false,"gives_back_weekly":true,
             "clears":["seven_day_overage_included"],"unavailable_reason":"unavailable"}
        ]}
    })
}

#[test]
fn claude_reset_entitlements_and_selection() {
    let body = usage(18);
    let resets = claude_resets(&body).unwrap().unwrap();
    assert_eq!(resets.usable, 2);
    let details = resets.resets.as_ref().unwrap();
    assert_eq!(details[0].scope, "weekly");
    assert_eq!(details[1].scope, "fable");
    assert_eq!(details[1].clears, ["seven_day_fable"]);
    assert!(!details[2].usable_now);
    assert!(select_reset(Some(&resets), None, None, false, true)
        .unwrap_err()
        .to_string()
        .contains("--scope fable, weekly"));
    assert_eq!(
        select_reset(Some(&resets), Some("fable"), None, false, true)
            .unwrap()
            .unwrap()
            .id,
        "fable-reset"
    );
    assert!(select_reset(Some(&resets), Some("session"), None, false, true).is_err());
    assert!(select_reset(Some(&resets), None, Some("spent-reset"), false, true).is_err());
    assert_eq!(
        select_reset(Some(&resets), None, Some("spent-reset"), true, true)
            .unwrap()
            .unwrap()
            .id,
        "spent-reset"
    );
    assert!(claude_resets(&json!({"cedar_ember":null}))
        .unwrap()
        .is_none());
    let mut unavailable = body.clone();
    unavailable["cedar_ember"]["eligible"] = json!(false);
    unavailable["cedar_ember"]["ineligible_reason"] = json!("tier");
    let unavailable = claude_resets(&unavailable).unwrap().unwrap();
    assert_eq!(unavailable.usable, 0);
    assert_eq!(unavailable.unavailable.as_deref(), Some("tier"));
    assert!(
        select_reset(Some(&unavailable), Some("fable"), None, false, true)
            .unwrap_err()
            .to_string()
            .contains("tier")
    );
    let mut paused = body;
    paused["cedar_ember"]["grants"][1]["paused"] = json!(true);
    assert!(!claude_resets(&paused).unwrap().unwrap().resets.unwrap()[1].usable_now);
}

struct Endpoints;
impl Drop for Endpoints {
    fn drop(&mut self) {
        for key in [
            "LF_TEST_CLAUDE_PROFILE_URL",
            "LF_TEST_CLAUDE_USAGE_URL",
            "LF_TEST_CLAUDE_RESET_URL",
            "LF_TEST_CLAUDE_TOKEN_URL",
        ] {
            std::env::remove_var(key);
        }
    }
}

async fn serve(
    responses: Vec<(u16, Value)>,
    at_request: impl Fn(usize) + Send + 'static,
) -> (Endpoints, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::env::set_var("LF_TEST_CLAUDE_PROFILE_URL", format!("{base}/profile"));
    std::env::set_var("LF_TEST_CLAUDE_USAGE_URL", format!("{base}/usage"));
    std::env::set_var("LF_TEST_CLAUDE_TOKEN_URL", format!("{base}/token"));
    std::env::set_var("LF_TEST_CLAUDE_RESET_URL", &base);
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for (index, (status, body)) in responses.into_iter().enumerate() {
            let (mut stream, _) =
                tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            let mut request = Vec::new();
            let header_end;
            loop {
                let mut buffer = [0; 4096];
                let read = stream.read(&mut buffer).await.unwrap();
                assert_ne!(read, 0);
                request.extend_from_slice(&buffer[..read]);
                if let Some(end) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = end + 4;
                    break;
                }
            }
            let header = String::from_utf8_lossy(&request[..header_end]);
            let length = header
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")
                        .map(|v| v.parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            while request.len() < header_end + length {
                let mut buffer = [0; 4096];
                let read = stream.read(&mut buffer).await.unwrap();
                assert_ne!(read, 0);
                request.extend_from_slice(&buffer[..read]);
            }
            requests.push(String::from_utf8(request).unwrap());
            at_request(index);
            let body = body.to_string();
            stream.write_all(format!("HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
        }
        requests
    });
    (Endpoints, server)
}

fn account(home: &std::path::Path) -> ProviderAccount {
    std::fs::create_dir_all(home).unwrap();
    let raw = json!({"claudeAiOauth":{"accessToken":"fixture","refreshToken":"refresh-fixture","expiresAt":4102444800000i64}}).to_string();
    std::fs::write(home.join(".credentials.json"), &raw).unwrap();
    let mut account = new_account(
        Provider::Claude,
        ProviderAccountId::parse("fixture-reset").unwrap(),
        home.into(),
        Some(EmailAddress::parse("reset@example.com").unwrap()),
    );
    account.observed_email = Some("reset@example.com".into());
    account.observed_subject = Some("reset-user".into());
    account.observed_credential_digest =
        Some(crate::provider_account::identity::credential_digest(&raw));
    account
}

fn profile() -> Value {
    json!({"account":{"email":"reset@example.com","uuid":"reset-user"},"organization":{"uuid":"reset-org"}})
}

fn post(requests: &[String]) -> Value {
    let posts = requests
        .iter()
        .filter(|r| r.starts_with("POST /api/"))
        .collect::<Vec<_>>();
    assert_eq!(posts.len(), 1);
    assert!(posts[0].starts_with("POST /api/organizations/reset-org/reset_rate_limits "));
    serde_json::from_str(posts[0].split("\r\n\r\n").nth(1).unwrap()).unwrap()
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn claude_reset_scoped_outcomes_and_retry_keep_request_identity() {
    let _env = crate::journal::test_env_lock();
    let root = tempfile::tempdir().unwrap();
    let account = account(root.path());
    for outcome in [
        "reset",
        "already_used",
        "not_limited",
        "cooldown",
        "ineligible",
        "unavailable",
    ] {
        let mut before = usage(100);
        // A retry must work even when the grant has been spent.
        let retry = outcome == "already_used";
        if retry {
            before["cedar_ember"]["grants"][1]["usable_now"] = json!(false);
        }
        let (_endpoints, server) = serve(
            vec![
                (200, profile()),
                (200, before),
                (200, json!({"result":outcome,"reason":"fixture-reason"})),
                (200, usage(0)),
            ],
            |_| {},
        )
        .await;
        let result = redeem_claude_reset(
            &account,
            std::slice::from_ref(&account),
            "same-key",
            retry.then_some("fable-reset"),
            Some("fable"),
            retry,
        )
        .await
        .unwrap();
        assert_eq!(result.outcome, outcome);
        assert_eq!(result.reason.as_deref(), Some("fixture-reason"));
        assert_eq!(result.scope.as_deref(), Some("fable"));
        assert_eq!(result.after.unwrap().windows[1].used_percent, 0);
        let requests = server.await.unwrap();
        assert!(requests[1].starts_with("GET /usage?cedar_ember=1&skip_spend=1 "));
        assert_eq!(
            post(&requests),
            json!({"program":"cedar_ember","grant_id":"fable-reset","request_id":"same-key"})
        );
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn claude_reset_errors_do_not_claim_recovery_or_repeat_a_spend() {
    let _env = crate::journal::test_env_lock();
    let root = tempfile::tempdir().unwrap();
    let account = account(root.path());
    for (status, reply) in [
        (500, json!({})),
        (429, json!({})),
        (403, json!({})),
        (200, json!({"result":"surprise"})),
    ] {
        let (_endpoints, server) = serve(
            vec![(200, profile()), (200, usage(100)), (status, reply)],
            |_| {},
        )
        .await;
        let error = redeem_claude_reset(
            &account,
            std::slice::from_ref(&account),
            "lost-key",
            None,
            Some("fable"),
            false,
        )
        .await
        .unwrap_err()
        .to_string();
        if status == 429 {
            assert!(error.contains("rate limited"), "{error}");
        }
        assert!(
            error.contains("--idempotency-key lost-key --reset-id fable-reset"),
            "{error}"
        );
        post(&server.await.unwrap());
    }
    let (_endpoints, server) = serve(
        vec![
            (200, profile()),
            (200, usage(100)),
            (200, json!({"result":"reset"})),
            (500, json!({})),
        ],
        |_| {},
    )
    .await;
    let result = redeem_claude_reset(
        &account,
        std::slice::from_ref(&account),
        "key",
        None,
        Some("fable"),
        false,
    )
    .await
    .unwrap();
    assert_eq!(result.outcome, "reset");
    assert!(result.after.is_err());
    post(&server.await.unwrap());
    for body in [
        json!({"cedar_ember":null}),
        json!({"cedar_ember":{"eligible":true,"grants":[]}}),
        usage(100),
    ] {
        let (_endpoints, server) = serve(vec![(200, profile()), (200, body)], |_| {}).await;
        assert!(redeem_claude_reset(
            &account,
            std::slice::from_ref(&account),
            "key",
            None,
            None,
            false
        )
        .await
        .is_err());
        assert_eq!(server.await.unwrap().len(), 2);
    }
}

async fn seed_limits(store: &SharedStore, account: &ProviderAccount, percent: u8) {
    store
        .upsert_provider_account_limits(
            "claude",
            &account.account_id,
            &[AccountLimitWindow {
                window: "seven_day_fable".into(),
                used_percent: percent,
                resets_at: Some(4070908800),
                plan: None,
                model: Some("fable".into()),
                limited_until: Some(4070908805),
            }],
            "stream",
        )
        .await
        .unwrap();
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn claude_reset_command_reconciles_routing_and_keeps_newer_rejection() {
    let _env = crate::journal::test_env_lock();
    let root = tempfile::tempdir().unwrap();
    let _homes = crate::test_ambient::EnvGuard::clear(&[
        "LF_HOME",
        "CLAUDE_CONFIG_DIR",
        "LF_ACCOUNT_SELECTION",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "ANTHROPIC_API_KEY",
    ]);
    std::env::set_var("LF_HOME", root.path().join("machine"));
    std::env::set_var("CLAUDE_CONFIG_DIR", root.path().join("native"));
    let account = account(&root.path().join("account"));
    let store = crate::provider_account::open_account_store().await.unwrap();
    store.upsert_provider_account(&account).await.unwrap();
    seed_limits(&store, &account, 100).await;
    assert!(resolve_provider_account_exact(
        Provider::Claude,
        None,
        Some(&account.account_id),
        None,
        Some("fable"),
        Some(root.path())
    )
    .await
    .is_err());
    for (outcome, before, after) in [
        ("reset", 0, (500, json!({}))),
        ("not_limited", 100, (200, usage(100))),
    ] {
        let (_endpoints, server) = serve(
            vec![
                (200, profile()),
                (200, usage(before)),
                (200, json!({"result":outcome})),
                after,
            ],
            |_| {},
        )
        .await;
        let result = crate::lf::commands::account::redeem_reset(
            "claude",
            "reset@",
            None,
            None,
            Some("fable"),
        )
        .await
        .unwrap();
        server.await.unwrap();
        assert_eq!(result.outcome, outcome);
        let limits = store.provider_account_limits(Some("claude")).await.unwrap();
        assert_eq!(
            limits
                .iter()
                .find(|w| w.window == "seven_day_fable")
                .unwrap()
                .used_percent,
            100
        );
        assert!(resolve_provider_account_exact(
            Provider::Claude,
            None,
            Some(&account.account_id),
            None,
            Some("fable"),
            Some(root.path())
        )
        .await
        .is_err());
    }
    let (_endpoints, server) = serve(
        vec![
            (200, profile()),
            (200, usage(100)),
            (200, json!({"result":"reset"})),
            (200, usage(0)),
        ],
        |_| {},
    )
    .await;
    let result =
        crate::lf::commands::account::redeem_reset("claude", "reset@", None, None, Some("fable"))
            .await
            .unwrap();
    assert_eq!(result.outcome, "reset");
    assert!(!result.newer_observation_kept);
    let requests = server.await.unwrap();
    assert_eq!(post(&requests)["request_id"], result.idempotency_key);
    let route = resolve_provider_account_exact(
        Provider::Claude,
        None,
        Some(&account.account_id),
        None,
        Some("fable"),
        Some(root.path()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(route.account_id(), &account.account_id);
    let limits = store.provider_account_limits(Some("claude")).await.unwrap();
    assert_eq!(
        limits
            .iter()
            .find(|w| w.window == "seven_day")
            .unwrap()
            .used_percent,
        70
    );
    assert_eq!(
        limits
            .iter()
            .find(|w| w.window == "seven_day_fable")
            .unwrap()
            .used_percent,
        0
    );

    // Write a later rejection while the after-read is in flight.
    let store_copy = store.clone();
    let account_copy = account.clone();
    let (_endpoints, server) = serve(
        vec![
            (200, profile()),
            (200, usage(100)),
            (200, json!({"result":"reset"})),
            (200, usage(0)),
        ],
        move |index| {
            if index == 3 {
                let store = store_copy.clone();
                let account = account_copy.clone();
                let thread = std::thread::spawn(move || {
                    tokio::runtime::Runtime::new().unwrap().block_on(async {
                        seed_limits(&store, &account, 100).await;
                    })
                });
                thread.join().unwrap();
            }
        },
    )
    .await;
    let result =
        crate::lf::commands::account::redeem_reset("claude", "reset@", None, None, Some("fable"))
            .await
            .unwrap();
    server.await.unwrap();
    assert!(result.newer_observation_kept);
    let limits = store.provider_account_limits(Some("claude")).await.unwrap();
    assert_eq!(
        limits
            .iter()
            .find(|w| w.window == "seven_day_fable")
            .unwrap()
            .used_percent,
        100
    );
    assert_eq!(
        limits
            .iter()
            .find(|w| w.window == "seven_day")
            .unwrap()
            .used_percent,
        70
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn claude_reset_authentication_refreshes_once_and_reuses_the_same_attempt() {
    let _env = crate::journal::test_env_lock();
    for rejected_again in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let account = account(root.path());
        let mut responses = vec![
            (200, profile()),
            (200, usage(100)),
            (401, json!({})),
            (
                200,
                json!({"access_token":"refreshed-fixture","expires_in":3600}),
            ),
            (200, profile()),
            if rejected_again {
                (401, json!({}))
            } else {
                (200, json!({"result":"reset"}))
            },
        ];
        if !rejected_again {
            responses.push((200, usage(0)));
        }
        let (_endpoints, server) = serve(responses, |_| {}).await;
        let result = redeem_claude_reset(
            &account,
            std::slice::from_ref(&account),
            "same-key",
            None,
            Some("fable"),
            false,
        )
        .await;
        if rejected_again {
            assert!(matches!(
                result,
                Err(super::SubscriptionError::NeedsLogin(_))
            ));
        } else {
            assert_eq!(result.unwrap().outcome, "reset");
        }
        let requests = server.await.unwrap();
        let posts = requests
            .iter()
            .filter(|r| r.starts_with("POST /api/"))
            .collect::<Vec<_>>();
        assert_eq!(posts.len(), 2);
        let bodies = posts
            .iter()
            .map(|r| r.split("\r\n\r\n").nth(1).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(bodies[0], bodies[1]);
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn claude_reset_identity_and_credential_changes_send_nothing() {
    let _env = crate::journal::test_env_lock();
    let root = tempfile::tempdir().unwrap();
    let account = account(root.path());
    let mut wrong = profile();
    wrong["account"]["email"] = json!("other@example.com");
    let (_endpoints, server) = serve(vec![(200, wrong)], |_| {}).await;
    assert!(redeem_claude_reset(
        &account,
        std::slice::from_ref(&account),
        "key",
        None,
        Some("fable"),
        false
    )
    .await
    .is_err());
    assert_eq!(server.await.unwrap().len(), 1);
    let home = root.path().to_path_buf();
    let (_endpoints, server) = serve(vec![(200, profile()), (200, usage(100))], move |index| {
        if index == 1 {
            std::fs::write(
                home.join(".credentials.json"),
                json!({"claudeAiOauth":{"accessToken":"replacement"}}).to_string(),
            )
            .unwrap();
        }
    })
    .await;
    let error = redeem_claude_reset(
        &account,
        std::slice::from_ref(&account),
        "key",
        None,
        Some("fable"),
        false,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("credential changed"), "{error}");
    assert_eq!(server.await.unwrap().len(), 2);
}

#[test]
fn codex_reset_count_and_provider_selection_survive_missing_details() {
    let resets = super::codex_resets(&json!({"availableCount":3,"credits":null})).unwrap();
    assert_eq!(resets.usable, 3);
    assert!(resets.resets.is_none());
    assert!(select_reset(Some(&resets), None, None, false, false)
        .unwrap()
        .is_none());
    assert!(
        select_reset(Some(&resets), None, Some("opaque"), false, false)
            .unwrap()
            .is_none()
    );
    assert!(select_reset(Some(&resets), Some("weekly"), None, false, false).is_err());
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn claude_reset_usage_rejection_refreshes_before_spending() {
    let _env = crate::journal::test_env_lock();
    let root = tempfile::tempdir().unwrap();
    let account = account(root.path());
    let (_endpoints, server) = serve(
        vec![
            (200, profile()),
            (401, json!({})),
            (
                200,
                json!({"access_token":"refreshed-fixture","expires_in":3600}),
            ),
            (200, profile()),
            (200, usage(100)),
            (200, json!({"result":"reset"})),
            (200, usage(0)),
        ],
        |_| {},
    )
    .await;
    let result = redeem_claude_reset(
        &account,
        std::slice::from_ref(&account),
        "key",
        None,
        Some("weekly"),
        false,
    )
    .await
    .unwrap();
    assert_eq!(result.scope.as_deref(), Some("weekly"));
    assert_eq!(post(&server.await.unwrap())["grant_id"], "weekly-reset");
}
