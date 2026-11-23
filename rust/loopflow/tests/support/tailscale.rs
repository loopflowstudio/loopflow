//! A private LocalAPI daemon. Tests can change its owner, hold a certificate
//! request open, or stop it while the production server has a connected phone.
use std::net::SocketAddr;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, StatusCode, Uri};
use axum::routing::get;
use loopflow_execution::machine::mobile::{Configuration, TailscaleIdentity};
use serde_json::{json, Value};
use tokio::sync::watch;

pub fn status(user: u64) -> Value {
    json!({
        "BackendState": "Running",
        "Self": {
            "ID": "node1",
            "DNSName": "fixture.tail.ts.net.",
            "UserID": user,
            "TailscaleIPs": ["127.0.0.1"]
        },
        "User": {user.to_string(): {"LoginName": "owner@example.com"}}
    })
}

pub fn approval(port: u16) -> Configuration {
    Configuration {
        enabled: true,
        endpoint: format!("https://fixture.tail.ts.net:{port}"),
        owner: TailscaleIdentity {
            node: "node1".into(),
            user: 1,
        },
    }
}

pub struct Daemon {
    _runtime: tokio::runtime::Runtime,
    requests: mpsc::Receiver<String>,
    pub status: watch::Sender<Value>,
    pub peer: watch::Sender<Value>,
    pub hold_certificate: watch::Sender<bool>,
}

impl Daemon {
    pub fn start(home: &Path, status: &Value, pem: String) -> Self {
        let (peer, whois) = watch::channel(json!({
            "Node":{"StableID":"fixture-phone","User":status["Self"]["UserID"]},
            "UserProfile":{"ID":status["Self"]["UserID"]}
        }));
        let (status, host) = watch::channel(status.clone());
        let (hold_certificate, hold) = watch::channel(false);
        let (observed, requests) = mpsc::channel();
        let router = axum::Router::new().fallback(get(move |uri: Uri, headers: HeaderMap| {
            let (host, whois, mut hold) = (host.clone(), whois.clone(), hold.clone());
            let observed = observed.clone();
            let pem = pem.clone();
            async move {
                if headers.get("host").and_then(|v| v.to_str().ok())
                    != Some("local-tailscaled.sock")
                    || headers.contains_key("authorization")
                {
                    return Err(StatusCode::UNAUTHORIZED);
                }
                let _ = observed.send(uri.to_string());
                match (uri.path(), uri.query()) {
                    ("/localapi/v0/status", query @ (None | Some("peers=false"))) => {
                        let mut status = host.borrow().clone();
                        if query.is_some() {
                            status["User"] = Value::Null;
                        }
                        Ok(status.to_string())
                    }
                    (
                        "/localapi/v0/cert/fixture.tail.ts.net",
                        Some("type=pair&min_validity=24h"),
                    ) => {
                        hold.wait_for(|held| !held)
                            .await
                            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
                        Ok(pem)
                    }
                    ("/localapi/v0/whois", Some(_)) => {
                        let url = reqwest::Url::parse(&format!("http://localhost{uri}")).unwrap();
                        let mut query = url.query_pairs();
                        if !query.clone().any(|(k, v)| k == "proto" && v == "tcp")
                            || !query.any(|(k, v)| k == "addr" && v.parse::<SocketAddr>().is_ok())
                        {
                            return Err(StatusCode::BAD_REQUEST);
                        }
                        Ok(whois.borrow().to_string())
                    }
                    _ => Err(StatusCode::NOT_FOUND),
                }
            }
        }));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        {
            let _entered = runtime.enter();
            let listener = tokio::net::UnixListener::bind(home.join("tailscaled.sock")).unwrap();
            runtime.spawn(async move { axum::serve(listener, router).await.unwrap() });
        }
        Self {
            _runtime: runtime,
            requests,
            status,
            peer,
            hold_certificate,
        }
    }

    pub fn wait_for(&self, path: &str) {
        let until = Instant::now() + Duration::from_secs(15);
        while self
            .requests
            .recv_timeout(until.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("missing LocalAPI request {path}"))
            != path
        {}
    }
}
