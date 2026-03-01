use axum::extract::connect_info::ConnectInfo;
use axum::extract::{Json, State};
use axum::http::{HeaderMap, StatusCode};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::lfd::auth::ParsedToken;
use crate::lfd::http::routes::ApiError;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, ApiMessage, ApiResult};

#[derive(Debug, Deserialize)]
pub struct RevokeTokensRequest {
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Serialize)]
pub struct RevokeTokensResponse {
    pub revoked: u32,
}

pub async fn revoke_tokens_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(body): Json<RevokeTokensRequest>,
) -> ApiResult<RevokeTokensResponse> {
    let provided = match crate::lfd::auth::extract_token(&headers) {
        ParsedToken::Present(token) => Some(token),
        ParsedToken::Missing | ParsedToken::Malformed => None,
    };

    if !state.auth.local_admin_authorized(provided, peer.ip()) {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "local static token required",
        ));
    }

    let Some(ledger) = state.auth.connection_ledger() else {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "connection token ledger unavailable",
        ));
    };

    let revoked = if body.all {
        ledger.revoke_all().await.map_err(map_ledger_error)?
    } else {
        let Some(prefix) = body.prefix.as_deref() else {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "prefix is required unless --all is set",
            ));
        };
        ledger.revoke(prefix).await.map_err(map_ledger_error)?
    };

    Ok(Json(RevokeTokensResponse { revoked }))
}

fn map_ledger_error(error: crate::lfd::token_ledger::TokenLedgerError) -> ApiError {
    match error {
        crate::lfd::token_ledger::TokenLedgerError::InvalidPrefix => {
            api_error(StatusCode::BAD_REQUEST, "invalid token hash prefix")
        }
        other => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(other.to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::post;
    use axum::Router;
    use secrecy::SecretString;
    use sha2::Digest;
    use std::sync::Arc;
    use tempfile::tempdir;
    use time::OffsetDateTime;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    use crate::lfd::auth::{auth_middleware, AuthFailureThrottle, AuthProvider};
    use crate::lfd::config::{ExecutorConfig, GitHubConfig, HttpSecurityConfig};
    use crate::lfd::events::EventHub;
    use crate::lfd::executor::WaveExecutor;
    use crate::lfd::output::OutputHub;
    use crate::lfd::provider_auth::ProviderAuthService;
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::sessions::SessionManager;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use crate::lfd::token_ledger::TokenLedger;

    async fn test_http_state(
        local_token: &str,
        ledger: TokenLedger,
        store: SharedStore,
        output_dir: std::path::PathBuf,
    ) -> HttpState {
        let scheduler = Arc::new(Scheduler::new(1));
        let output_hub = OutputHub::new(128, output_dir);
        let event_hub = EventHub::new(128);
        let sessions = SessionManager::new(store.clone());
        let executor = Arc::new(
            WaveExecutor::new(
                store.clone(),
                scheduler.clone(),
                output_hub.clone(),
                event_hub.clone(),
                sessions.clone(),
                ExecutorConfig::default(),
                GitHubConfig::default(),
            )
            .expect("build executor"),
        );

        HttpState {
            store: store.clone(),
            scheduler,
            executor,
            event_hub,
            output_hub,
            provider_auth: ProviderAuthService::new(store.clone()),
            auth: AuthProvider::DualAuth {
                local_token: SecretString::new(local_token.to_string()),
                ledger,
            },
            registration: None,
            started_at: OffsetDateTime::now_utc(),
            github: GitHubConfig::default(),
            http_security: HttpSecurityConfig::default(),
            auth_failure_throttle: AuthFailureThrottle::new(),
            ci_failure_cache: Arc::new(Mutex::new(std::collections::HashSet::new())),
            sessions,
        }
    }

    async fn spawn_server(state: HttpState) -> String {
        let app = Router::new()
            .route("/v0/tokens/revoke", post(revoke_tokens_handler))
            .route_layer(axum::middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let _server = tokio::spawn(async move {
            axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
                .await
                .expect("serve app");
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn revoke_endpoint_requires_loopback_static_token() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path.clone()))
                .await
                .expect("store"),
        );
        let ledger = TokenLedger::new(db_path.clone()).await.expect("ledger");
        let connection_token = ledger.mint(1).await.expect("mint").remove(0);
        let state = test_http_state(
            "local-admin-token",
            ledger.clone(),
            store,
            tmp.path().join("output"),
        )
        .await;
        let base_url = spawn_server(state).await;

        let response = reqwest::Client::new()
            .post(format!("{base_url}/v0/tokens/revoke"))
            .header("authorization", format!("Bearer {connection_token}"))
            .json(&serde_json::json!({ "all": true }))
            .send()
            .await
            .expect("request");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn revoke_endpoint_revokes_by_hash_prefix() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path.clone()))
                .await
                .expect("store"),
        );
        let ledger = TokenLedger::new(db_path.clone()).await.expect("ledger");
        let token = ledger.mint(1).await.expect("mint").remove(0);
        let digest = sha2::Sha256::digest(token.as_bytes());
        let hash = hex::encode(digest);
        let prefix = &hash[..10];

        let state = test_http_state(
            "local-admin-token",
            ledger.clone(),
            store,
            tmp.path().join("output"),
        )
        .await;
        let base_url = spawn_server(state).await;

        let response = reqwest::Client::new()
            .post(format!("{base_url}/v0/tokens/revoke"))
            .header("authorization", "Bearer local-admin-token")
            .json(&serde_json::json!({ "prefix": prefix }))
            .send()
            .await
            .expect("request");

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!ledger.validate(&token).await.expect("validate revoked"));
    }
}
