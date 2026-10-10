//! The native TUI connects through a local HTTP relay, never directly to the
//! surviving server. Mutations hold their frozen attachment fence through dispatch,
//! never through the streamed answer.
use std::sync::Arc;

use anyhow::{anyhow, ensure, Result};
use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{header, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use serde_json::Value;

use super::agent_process::AttachmentOwner;
use crate::id::AgentSessionId;

pub(crate) struct OpenCodeConnection {
    pub owner: AttachmentOwner,
    pub thread: AgentSessionId,
    pub endpoint: String,
    pub directory: String,
    pub password: String,
}

impl std::fmt::Debug for OpenCodeConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenCodeConnection")
            .field("thread", &self.thread)
            .finish_non_exhaustive()
    }
}

impl OpenCodeConnection {
    pub async fn serve(self, listener: tokio::net::TcpListener) -> Result<()> {
        let app = axum::Router::new()
            .fallback(relay)
            .with_state(Arc::new(self));
        axum::serve(listener, app).await?;
        Ok(())
    }

    async fn forward(&self, request: Request) -> Result<Response> {
        let expected = format!(
            "Basic {}",
            base64::engine::general_purpose::STANDARD.encode(format!("opencode:{}", self.password))
        );
        if request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            != Some(&expected)
        {
            return Ok(StatusCode::UNAUTHORIZED.into_response());
        }
        let method = request.method().clone();
        let path = request.uri().path().to_string();
        // Never allow encoded separators or dot segments to change the selected
        // conversation after the authority check and URL normalization.
        ensure!(
            !path.contains('%') && !path.split('/').any(|p| p == "." || p == ".."),
            "Invalid native path"
        );
        let mut url = reqwest::Url::parse(&format!("{}{}", self.endpoint, path))?;
        if let Some(query) = request.uri().query() {
            url.set_query(Some(query));
        }
        let query: Vec<_> = url
            .query_pairs()
            .filter(|(key, _)| key != "directory")
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        url.set_query(None);
        url.query_pairs_mut()
            .extend_pairs(query)
            .append_pair("directory", &self.directory);
        if method == Method::GET {
            let response = reqwest::Client::new()
                .get(url)
                .header("x-opencode-directory", &self.directory)
                .send()
                .await?;
            return stream_response(response);
        }
        // Admission, deletion, global configuration and provider shutdown do not
        // belong to an attached conversation client. Keep these out of the relay.
        ensure!(
            method == Method::POST,
            "Native mutation is not supported by this conversation connection"
        );
        let bytes = to_bytes(request.into_body(), 16 * 1024 * 1024).await?;
        let payload = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)?
        };
        if let Some(id) = path
            .strip_prefix("/permission/")
            .and_then(|path| path.strip_suffix("/reply"))
        {
            ensure!(
                !id.is_empty() && !id.contains('/'),
                "Invalid permission identity"
            );
            let snapshot = super::opencode_history::read_snapshot(
                &reqwest::Client::new(),
                &self.endpoint,
                &self.thread,
            )
            .await?;
            snapshot
                .reply_native_permission(&self.endpoint, &self.thread, &self.owner, id, payload)
                .await?;
            return Ok(axum::Json(true).into_response());
        }
        let prefix = format!("/session/{}/", self.thread);
        let operation = path.strip_prefix(&prefix).unwrap_or_default();
        ensure!(
            matches!(operation, "message" | "prompt_async" | "abort"),
            "Native mutation is not supported by this conversation connection"
        );
        let thread = self.thread.clone();
        let directory = self.directory.clone();
        let prompt = matches!(operation, "message" | "prompt_async");
        let owner = self.owner.clone();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        // The provider streams response headers before generation. Only request
        // dispatch is fenced and timed; draining the answer cannot prevent stop
        // or takeover. Cancellation leaves the blocking writer holding its fence.
        let response = tokio::task::spawn_blocking(move || {
            let (store, session, attachment) = &owner;
            store.with_session_attachment(session, attachment, || {
                Ok((|| -> Result<_> {
                    if prompt {
                        let request = payload["messageID"]
                            .as_str()
                            .ok_or_else(|| anyhow!("Native prompt has no message identity"))?;
                        ensure!(
                            store.session_request(session, &thread, request)?.is_none(),
                            "Native prompt already attempted; not replaying"
                        );
                        let origin = store.session_turn_origin(session, attachment)?;
                        store.record_session_request(&thread, request, &origin)?;
                    }
                    let mut request = client.post(url).header("x-opencode-directory", directory);
                    if !payload.is_null() {
                        request = request.json(&payload);
                    }
                    super::dispatch::within(std::time::Duration::from_secs(10), request.send())
                        .ok_or_else(|| anyhow!("Native dispatch timed out; outcome is unknown"))?
                        .map_err(Into::into)
                })())
            })?
        })
        .await??;
        stream_response(response)
    }
}

// Reads and mutations preserve the same status, content type and streaming body.
fn stream_response(response: reqwest::Response) -> Result<Response> {
    let mut output = Response::builder().status(response.status());
    if let Some(content_type) = response.headers().get(header::CONTENT_TYPE) {
        output = output.header(header::CONTENT_TYPE, content_type);
    }
    Ok(output.body(Body::from_stream(response.bytes_stream()))?)
}

async fn relay(State(connection): State<Arc<OpenCodeConnection>>, request: Request) -> Response {
    match connection.forward(request).await {
        Ok(response) => response,
        Err(error) => (StatusCode::CONFLICT, error.to_string()).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use axum::body::Body;
    use axum::extract::State;
    use axum::response::Response;
    use axum::routing::post;
    use axum::Json;
    use serde_json::{json, Value};
    use tokio::sync::{mpsc, Mutex};

    use super::OpenCodeConnection;
    use crate::id::LfProcessId;
    use crate::store::sqlite::SqliteStore;

    #[derive(Clone)]
    struct Provider {
        prompts: Arc<Mutex<Vec<Value>>>,
        body: Arc<Mutex<Option<Body>>>,
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_prompt_stream_does_not_hold_authority_and_stale_clients_cannot_write() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("conversation", "run_00000000000000000000000000000001");
        let first_process = LfProcessId::new();
        let second_process = LfProcessId::new();
        let sql = rusqlite::Connection::open(&path).unwrap();
        for process in [&first_process, &second_process] {
            sql.execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process],
            )
            .unwrap();
        }
        let first = store
            .claim_session_attachment("conversation", None, &first_process, false)
            .unwrap();
        let prompts = Arc::new(Mutex::new(Vec::<Value>::new()));
        let (finish, receiver) = mpsc::channel::<Result<String, std::io::Error>>(1);
        let body = Arc::new(Mutex::new(Some(Body::from_stream(
            tokio_stream::wrappers::ReceiverStream::new(receiver),
        ))));
        let state = Provider {
            prompts: prompts.clone(),
            body,
        };
        let server = axum::Router::new()
            .route(
                "/session/thread/message",
                post(
                    |State(provider): State<Provider>, Json(value): Json<Value>| async move {
                        provider.prompts.lock().await.push(value);
                        let body = provider.body.lock().await.take().unwrap();
                        Response::builder()
                            .header("content-type", "application/json")
                            .body(body)
                            .unwrap()
                    },
                ),
            )
            .with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, server).await.unwrap() });
        let relay_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let relay_url = format!("http://{}", relay_listener.local_addr().unwrap());
        let relay = tokio::spawn(
            OpenCodeConnection {
                owner: (store.clone(), "conversation".into(), first.clone()),
                thread: "thread".into(),
                endpoint,
                directory: "/fixture".into(),
                password: "fixture".into(),
            }
            .serve(relay_listener),
        );
        let client = reqwest::Client::new();
        let url = format!("{relay_url}/session/thread/message");
        let refused = client
            .post(&url)
            .json(&json!({"messageID":"unauthenticated"}))
            .send()
            .await
            .unwrap();
        assert_eq!(refused.status(), 401);
        let response = client
            .post(&url)
            .basic_auth("opencode", Some("fixture"))
            .json(&json!({"messageID":"request", "parts":[{"type":"text","text":"kept"}]}))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        let repeated = client
            .post(&url)
            .basic_auth("opencode", Some("fixture"))
            .json(&json!({"messageID":"request"}))
            .send()
            .await
            .unwrap();
        assert_eq!(repeated.status(), 409, "uncertain input was replayed");
        // Response headers arrived; the answer is still withheld. A transfer
        // must not wait for model output or the response body to finish.
        let transfer_store = store.clone();
        let original = first.clone();
        let second = tokio::time::timeout(
            Duration::from_secs(2),
            tokio::task::spawn_blocking(move || {
                transfer_store.claim_session_attachment(
                    "conversation",
                    Some(&original),
                    &second_process,
                    false,
                )
            }),
        )
        .await
        .unwrap()
        .unwrap()
        .unwrap();
        let third = store
            .claim_session_attachment("conversation", Some(&second), &first_process, false)
            .unwrap();
        assert_ne!(first.token, third.token);
        let origin = store
            .session_request("conversation", &"thread".into(), "request")
            .unwrap()
            .unwrap()
            .0;
        assert_eq!(origin.agent_process_id, third.agent_process_id);
        assert_eq!(origin.lf_process_id, first_process);
        let stale = client
            .post(&url)
            .basic_auth("opencode", Some("fixture"))
            .json(&json!({"messageID":"stale"}))
            .send()
            .await
            .unwrap();
        assert_eq!(stale.status(), 409);
        assert!(store
            .session_request("conversation", &"thread".into(), "stale")
            .unwrap()
            .is_none());
        assert_eq!(
            prompts.lock().await.as_slice(),
            &[json!({"messageID":"request", "parts":[{"type":"text","text":"kept"}]})]
        );
        finish
            .send(Ok("{\"result\":\"preserved\"}".into()))
            .await
            .unwrap();
        drop(finish);
        assert_eq!(
            response.json::<Value>().await.unwrap(),
            json!({"result":"preserved"})
        );
        relay.abort();
        server.abort();
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_permissions_preserve_choice_and_refuse_foreign_repeated_and_stale_replies() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        let process = LfProcessId::new();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [&process],
            )
            .unwrap();
        let attachment = store
            .claim_session_attachment("conversation", None, &process, false)
            .unwrap();
        let thread = "thread".into();
        let origin = store
            .session_turn_origin("conversation", &attachment)
            .unwrap();
        store
            .record_session_request(&thread, "request", &origin)
            .unwrap();
        let replies = Arc::new(Mutex::new(Vec::<Value>::new()));
        let server =
            axum::Router::new()
                .route(
                    "/permission",
                    axum::routing::get(|| async {
                        Json(json!([
                            {"id":"choice","sessionID":"thread","tool":{"messageID":"assistant"}},
                            {"id":"stale","sessionID":"thread","tool":{"messageID":"assistant"}},
                            {"id":"foreign","sessionID":"other","tool":{"messageID":"assistant"}}
                        ]))
                    }),
                )
                .route(
                    "/session/thread/message",
                    axum::routing::get(|| async {
                        Json(json!([
                            {"info":{"id":"assistant","sessionID":"thread","parentID":"request"}}
                        ]))
                    }),
                )
                .route(
                    "/permission/{id}/reply",
                    post(
                        |State(replies): State<Arc<Mutex<Vec<Value>>>>,
                         Json(value): Json<Value>| async move {
                            replies.lock().await.push(value);
                            Json(true)
                        },
                    ),
                )
                .with_state(replies.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move { axum::serve(listener, server).await.unwrap() });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let relay = tokio::spawn(
            OpenCodeConnection {
                owner: (store.clone(), "conversation".into(), attachment.clone()),
                thread,
                endpoint,
                directory: "/fixture".into(),
                password: "fixture".into(),
            }
            .serve(listener),
        );
        let client = reqwest::Client::new();
        let payload = json!({"reply":"reject","message":"Keep this file unchanged"});
        for (id, status) in [("foreign", 409), ("choice", 200), ("choice", 409)] {
            let response = client
                .post(format!("{url}/permission/{id}/reply"))
                .basic_auth("opencode", Some("fixture"))
                .json(&payload)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), status);
        }
        store
            .claim_session_attachment("conversation", Some(&attachment), &process, false)
            .unwrap();
        let response = client
            .post(format!("{url}/permission/stale/reply"))
            .basic_auth("opencode", Some("fixture"))
            .json(&json!({"reply":"always"}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 409);
        let saved = store.session_history("conversation", 0, 0).unwrap();
        let replies_saved: Vec<_> = saved
            .iter()
            .filter_map(|event| event.payload.get("permission_reply"))
            .collect();
        assert_eq!(
            replies_saved,
            vec![&json!({
                "id":"choice", "request":"request", "response":payload
            })]
        );
        assert_eq!(*replies.lock().await, vec![payload]);
        relay.abort();
        server.abort();
    }
}
