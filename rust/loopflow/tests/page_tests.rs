//! Integration tests for review pages, loopback carrier, and `lf page`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use loopflow::server::wire::{
    ClientIdentity, ClientMessage, Hello, Operation, ServerMessage, ServerOperation, REVISION,
};
use loopflow_test_support::{MachineServer, TestRepo};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

fn lf_cmd(repo: &std::path::Path, home: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_lf"));
    cmd.current_dir(repo)
        .env("LF_HOME", home)
        .env("HOME", home)
        .env_remove("LF_DESKTOP")
        .env_remove("LF_AGENT_CALLER")
        .env_remove("LF_TERMINAL_ID")
        .env_remove("LF_CAPTURE_KEY")
        .stdin(Stdio::null());
    cmd
}

#[test]
fn loopback_serves_page_enforces_admission_and_steers_task() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir_in("/tmp").unwrap();
    let _server = MachineServer::start(env!("CARGO_BIN_EXE_lf"), home.path());

    // Create a Task in the test repo so `task.steer` has a real target.
    let created = lf_cmd(repo.path(), home.path())
        .args(["task", "create", "Review page steer target", "--json"])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "task create failed: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let task_json: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let issue = task_json["task"]["identifier"]
        .as_str()
        .expect("task create JSON includes task.identifier")
        .to_owned();

    // Write a compiled page and a raw `.page.html` source.
    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).unwrap();
    let page = scratch.join("ship-review.html");
    fs::write(&page, "<!doctype html><h1>Ship review</h1>").unwrap();
    let source = scratch.join("ship-review.page.html");
    fs::write(&source, "<h1>Raw source</h1>").unwrap();

    // Passing `.page.html` to `lf page open` exits non-zero naming `lf page build`.
    let rejected_source = lf_cmd(repo.path(), home.path())
        .args(["page", "open", "scratch/ship-review.page.html", "--print"])
        .output()
        .unwrap();
    assert!(!rejected_source.status.success());
    let stderr = String::from_utf8_lossy(&rejected_source.stderr);
    assert!(
        stderr.contains("lf page build"),
        "expected build hint in stderr, got: {stderr}"
    );

    // `lf page open --print` returns the loopback capability URL.
    let printed = lf_cmd(repo.path(), home.path())
        .args(["page", "open", "scratch/ship-review.html", "--print"])
        .output()
        .unwrap();
    assert!(
        printed.status.success(),
        "lf page open --print failed: {}",
        String::from_utf8_lossy(&printed.stderr)
    );
    let url = String::from_utf8(printed.stdout).unwrap().trim().to_owned();
    assert!(
        url.starts_with("http://127.0.0.1:"),
        "expected loopback URL, got: {url}"
    );

    // Headless `lf page open` (with LF_AGENT_CALLER and no viewer) prints the URL
    // without invoking the OS browser opener.
    let fake_bin = home.path().join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let opener_marker = home.path().join("browser-opened");
    for opener in ["open", "xdg-open"] {
        let script = fake_bin.join(opener);
        fs::write(
            &script,
            format!("#!/bin/sh\necho \"$@\" > {}\n", opener_marker.display()),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let caller = serde_json::to_string(&loopflow_storage::process::AgentCaller {
        session_id: "session_headless_test".into(),
        agent_process_id: None,
        origin_lf_process_id: loopflow_model::id::LfProcessId::new(),
        trace_id: None,
    })
    .unwrap();
    let headless = lf_cmd(repo.path(), home.path())
        .env("LF_AGENT_CALLER", caller)
        .env("PATH", format!("{}:/usr/bin:/bin", fake_bin.display()))
        .args(["page", "open", "scratch/ship-review.html"])
        .output()
        .unwrap();
    assert!(
        headless.status.success(),
        "headless lf page open failed: {}",
        String::from_utf8_lossy(&headless.stderr)
    );
    let headless_url = String::from_utf8(headless.stdout)
        .unwrap()
        .trim()
        .to_owned();
    assert_eq!(headless_url, url);
    assert!(
        !opener_marker.exists(),
        "headless lf page open must never invoke OS browser opener"
    );

    // Parse port and token from `http://127.0.0.1:<port>/<token>/f/...`.
    let parsed_url = reqwest::Url::parse(&url).unwrap();
    let port = parsed_url.port().unwrap();
    let token = parsed_url
        .path_segments()
        .unwrap()
        .next()
        .unwrap()
        .to_owned();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let client = reqwest::Client::new();
        let response = client.get(&url).send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert_eq!(
            response.headers().get("content-security-policy").unwrap(),
            "default-src 'self' 'unsafe-inline' data: blob:; connect-src 'self'"
        );
        assert_eq!(
            response.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            response.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert_eq!(
            response.text().await.unwrap(),
            "<!doctype html><h1>Ship review</h1>"
        );

        // Admission checks: missing Origin, foreign Origin, wrong Host, wrong token → 404.
        let ws_url = format!("ws://127.0.0.1:{port}/{token}/machine");
        let good_origin = format!("http://127.0.0.1:{port}");
        for (target, origin, host_override) in [
            (ws_url.clone(), None, None),
            (ws_url.clone(), Some("https://evil.example"), None),
            (
                ws_url.clone(),
                Some(good_origin.as_str()),
                Some(format!("evil.test:{port}")),
            ),
            (
                format!("ws://127.0.0.1:{port}/bad-token/machine"),
                Some(good_origin.as_str()),
                None,
            ),
        ] {
            let mut req = target.into_client_request().unwrap();
            if let Some(origin) = origin {
                req.headers_mut().insert("origin", origin.parse().unwrap());
            }
            if let Some(host) = host_override {
                req.headers_mut().insert("host", host.parse().unwrap());
            }
            let err = tokio_tungstenite::connect_async(req).await.unwrap_err();
            let tokio_tungstenite::tungstenite::Error::Http(resp) = err else {
                panic!("expected HTTP rejection, got {err:?}");
            };
            assert_eq!(resp.status(), 404);
        }

        // Valid upgrade speaks Machine protocol (`client.kind = "page"`),
        // refuses `page.url` over loopback, and delivers `task.steer` with `delivery.sentence`.
        let mut req = ws_url.into_client_request().unwrap();
        req.headers_mut()
            .insert("origin", good_origin.parse().unwrap());
        let (mut ws, _) = tokio_tungstenite::connect_async(req).await.unwrap();

        let hello = ClientMessage::Hello(Hello {
            revision: REVISION,
            min_revision: REVISION,
            client: ClientIdentity {
                kind: "page".into(),
                platform: "browser".into(),
                name: "review-page".into(),
                version: "1".into(),
            },
        });
        ws.send(Message::Text(serde_json::to_string(&hello).unwrap().into()))
            .await
            .unwrap();
        let welcome = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let Message::Text(text) = welcome else {
            panic!("expected welcome text frame");
        };
        assert!(matches!(
            serde_json::from_str::<ServerMessage>(&text).unwrap(),
            ServerMessage::Welcome(_)
        ));

        // `page.url` is local-Unix-socket-only and refused over loopback.
        let mint_req = ClientMessage::Request {
            id: "mint-loopback".into(),
            operation: Operation::Server(ServerOperation::PageUrl { path: page.clone() }),
            select: None,
            sender: None,
            request_id: None,
        };
        ws.send(Message::Text(
            serde_json::to_string(&mint_req).unwrap().into(),
        ))
        .await
        .unwrap();
        let mint_reply = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let Message::Text(text) = mint_reply else {
            panic!("expected error text frame");
        };
        assert!(matches!(
            serde_json::from_str::<ServerMessage>(&text).unwrap(),
            ServerMessage::Error {
                id: Some(ref id),
                ..
            } if id == "mint-loopback"
        ));

        // `task.steer` saves the steer on the Task and returns `delivery.sentence`.
        let steer_msg = serde_json::json!({
            "type": "request",
            "id": "steer-1",
            "operation": {
                "kind": "task.steer",
                "directory": repo.path(),
                "issue": issue,
                "message": "(1) `scratch/ship-review.html` § Overview › \"Ship review\" Looks solid"
            },
            "select": null,
            "sender": null,
            "request_id": "req-page-steer-1"
        });
        ws.send(Message::Text(steer_msg.to_string().into()))
            .await
            .unwrap();
        let steer_reply = tokio::time::timeout(Duration::from_secs(10), ws.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let Message::Text(text) = steer_reply else {
            panic!("expected response text frame");
        };
        let parsed: ServerMessage = serde_json::from_str(&text).unwrap();
        let ServerMessage::Response { id, result, .. } = parsed else {
            panic!("expected task.steer response, got: {parsed:?}");
        };
        assert_eq!(id, "steer-1");
        let sentence = result["delivery"]["sentence"]
            .as_str()
            .expect("task.steer returns delivery.sentence");
        assert!(!sentence.is_empty(), "delivery.sentence must be non-empty");
        let _ = ws.close(None).await;
    });
}
