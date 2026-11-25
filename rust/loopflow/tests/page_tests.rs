//! Integration tests for review pages, loopback carrier, and `lf page`.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use loopflow::server::wire::{
    ClientIdentity, ClientMessage, Hello, Operation, ServerMessage, ServerOperation, MIN_REVISION,
    REVISION,
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

#[test]
fn page_build_expands_diffs_components_data_lf_and_rejects_broken_references() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir_in("/tmp").unwrap();

    // Seed a tracked file, commit it, then modify it and create an untracked file.
    fs::write(
        repo.path().join("tracked.rs"),
        "fn main() {\n    let old = 1;\n}\n",
    )
    .unwrap();
    let add = Command::new("git")
        .current_dir(repo.path())
        .args(["add", "tracked.rs"])
        .status()
        .unwrap();
    assert!(add.success());
    let commit = Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-m", "seed tracked file"])
        .status()
        .unwrap();
    assert!(commit.success());

    let head_sha = String::from_utf8(
        Command::new("git")
            .current_dir(repo.path())
            .args(["rev-parse", "--short=10", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_owned();

    // Modify tracked.rs and add an untracked file.
    fs::write(
        repo.path().join("tracked.rs"),
        "fn main() {\n    let updated = 2;\n    let extra = 3;\n}\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("new_file.rs"),
        "pub fn hello() -> &'static str {\n    \"hi\"\n}\n",
    )
    .unwrap();

    // Add a repository theme and a custom component override.
    fs::create_dir_all(repo.path().join(".lf/components/callout")).unwrap();
    fs::write(
        repo.path().join(".lf/theme.css"),
        ":root { --lf-accent: #123456; }\n",
    )
    .unwrap();
    fs::write(
        repo.path().join(".lf/components/callout/template.html"),
        "<aside class=\"custom-callout\"><strong>{{label}}</strong><slot></slot></aside>",
    )
    .unwrap();
    fs::write(
        repo.path().join(".lf/components/callout/style.css"),
        ".custom-callout { border-left: 2px solid var(--lf-accent); }",
    )
    .unwrap();

    // Bind a Task to this checkout so the stamp carries `work.kind = "task"`.
    let created = lf_cmd(repo.path(), home.path())
        .args(["task", "create", "Build page test", "--json"])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "task create failed: {}",
        String::from_utf8_lossy(&created.stderr)
    );
    let task_json: serde_json::Value = serde_json::from_slice(&created.stdout).unwrap();
    let issue = task_json["task"]["identifier"].as_str().unwrap().to_owned();

    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).unwrap();
    let source = scratch.join("ship-review.page.html");
    fs::write(
        &source,
        format!(
            r#"<h1>Retry budget</h1>
<p>Built on <code>{head_sha}</code> with a <code data-no-check>deadbeef12</code> literal.</p>

<lf-component name="callout" label="Invariant">
  <p>One write path.</p>
</lf-component>

<lf-diff path="tracked.rs">
  <lf-note old-line="2">Removed legacy constant.</lf-note>
  <lf-note line="3">Added extra counter.</lf-note>
</lf-diff>

<lf-diff path="new_file.rs"></lf-diff>

<lf-file path="tracked.rs" lines="1-2"></lf-file>

<lf-component name="decision" id="f1" title="Prune retry log" recommendation="File Task">
  <p>Rows grow unbounded on long Tasks.</p>
</lf-component>

<button data-lf='{{"kind":"task.create","title":"Prune retry log"}}'>File Task now</button>
"#
        ),
    )
    .unwrap();

    let built = lf_cmd(repo.path(), home.path())
        .args([
            "--task",
            &issue,
            "page",
            "build",
            "scratch/ship-review.page.html",
            "--base",
            "HEAD",
        ])
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "lf page build failed: {}",
        String::from_utf8_lossy(&built.stderr)
    );

    let output_path = scratch.join("ship-review.html");
    assert!(output_path.is_file());
    let html = fs::read_to_string(&output_path).unwrap();

    // Real diff lines (including untracked file `+` lines) and both line notes.
    assert!(
        html.contains("let updated = 2;"),
        "missing updated diff line"
    );
    assert!(html.contains("let old = 1;"), "missing deleted diff line");
    assert!(
        html.contains("Removed legacy constant."),
        "missing old-line note"
    );
    assert!(
        html.contains("Added extra counter."),
        "missing new-line note"
    );
    assert!(html.contains("Hide line notes"), "missing notes toggle");
    assert!(
        html.contains("pub fn hello()"),
        "missing untracked file diff"
    );

    // Theme, component override, and builtin decision component.
    assert!(
        html.contains("--lf-accent: #123456;"),
        "missing repo .lf/theme.css"
    );
    assert!(
        html.contains("custom-callout"),
        "missing repo component override"
    );
    assert!(
        html.contains("data-choice=\"f1\""),
        "missing builtin decision choice"
    );

    // Strict expanded `data-lf` operation JSON.
    assert!(
        html.contains("__LF_REQUEST_ID__"),
        "missing request_id sentinel"
    );
    assert!(
        html.contains("\"kind\":\"task.create\""),
        "missing task.create kind"
    );
    assert!(
        html.contains("\"run\":false"),
        "missing expanded run default"
    );

    // Work stamp (`work.kind = "task"` + `build_id`) and runtime script.
    let stamp_start = html
        .find(r#"<script type="application/json" id="lf-page">"#)
        .expect("missing stamp script");
    let stamp_json_start = stamp_start + r#"<script type="application/json" id="lf-page">"#.len();
    let stamp_json_end = html[stamp_json_start..].find("</script>").unwrap() + stamp_json_start;
    let stamp: serde_json::Value =
        serde_json::from_str(&html[stamp_json_start..stamp_json_end]).unwrap();
    assert_eq!(stamp["work"]["kind"], "task");
    assert_eq!(stamp["work"]["issue"], issue);
    assert_eq!(stamp["path"], "scratch/ship-review.html");
    assert_eq!(stamp["build_id"].as_str().unwrap().len(), 16);
    assert_eq!(stamp["protocol"]["revision"], REVISION);
    assert_eq!(stamp["protocol"]["min_revision"], MIN_REVISION);
    assert!(
        html.contains("window.__lfDesktopApplyState"),
        "missing runtime JS"
    );
    assert!(
        html.contains(":not(:has(> p, > label, > h3, > h4, > legend))::before"),
        "missing guarded data-label pseudo-element selector"
    );
    assert!(
        !html.contains("revision: 10"),
        "runtime must read protocol revision from stamp rather than hardcoding 10"
    );

    // Negative build checks: each fails non-zero naming element and line.
    for (name, body, expected_elem, expected_line) in [
        (
            "missing-path",
            "<h1>T</h1>\n<lf-file path=\"does/not/exist.rs\"></lf-file>\n",
            "lf-file",
            "line 2",
        ),
        (
            "unknown-sha",
            "<h1>T</h1>\n<p>See <code>0badc0ffee12</code></p>\n",
            "<code>",
            "line 2",
        ),
        (
            "invalid-op",
            "<h1>T</h1>\n<button data-lf='{\"kind\":\"task.create\",\"bogus\":true}'>Go</button>\n",
            "<button",
            "line 2",
        ),
        (
            "forbidden-script",
            "<h1>T</h1>\n\n<script>console.log(1)</script>\n",
            "<script>",
            "line 3",
        ),
    ] {
        let bad_path = scratch.join(format!("{name}.page.html"));
        fs::write(&bad_path, body).unwrap();
        let res = lf_cmd(repo.path(), home.path())
            .args([
                "page",
                "build",
                &format!("scratch/{name}.page.html"),
                "--base",
                "HEAD",
            ])
            .output()
            .unwrap();
        assert!(!res.status.success(), "{name} should have failed");
        let err = String::from_utf8_lossy(&res.stderr);
        assert!(
            err.contains(expected_elem) && err.contains(expected_line),
            "{name}: expected {expected_elem} and {expected_line}, got: {err}"
        );
    }
}

fn seed_session(
    home: &std::path::Path,
    cwd: &std::path::Path,
    id: &str,
    interactive: bool,
) -> String {
    let store = loopflow_execution::store::open_sqlite(&home.join("loopflow.db")).unwrap();
    loopflow_execution::store::sessions::create_session(
        &store,
        loopflow_execution::session::LfSession {
            captured: None,
            id: id.into(),
            artifact_key: None,
            caller_artifact_key: None,
            input_published: false,
            cwd: cwd.into(),
            skill: None,
            provider: Some("claude".into()),
            model: None,
            node: None,
            iterations: None,
            task_id: None,
            wave_id: None,
            graph_lf_process_id: None,
            work_source: None,
            bound_at: None,
            interactive,
            repo: None,
            title: id.into(),
            title_source: loopflow_execution::session::TitleSource::Generated,
            request: None,
            completed_at: None,
            created_at: 1,
        },
        None,
    )
    .unwrap();
    serde_json::to_string(&loopflow_storage::process::AgentCaller {
        session_id: id.into(),
        agent_process_id: None,
        origin_lf_process_id: loopflow_model::id::LfProcessId::new(),
        trace_id: None,
    })
    .unwrap()
}

#[test]
fn page_open_routes_interactive_plain_terminal_to_browser_and_headless_worker_quietly() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir_in("/tmp").unwrap();
    let _server = MachineServer::start(env!("CARGO_BIN_EXE_lf"), home.path());

    let scratch = repo.path().join("scratch");
    fs::create_dir_all(&scratch).unwrap();
    fs::write(
        scratch.join("explore.html"),
        "<!doctype html><h1>Explore</h1>",
    )
    .unwrap();

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
    let test_path = format!("{}:/usr/bin:/bin", fake_bin.display());

    // Headless worker session (`interactive == false`) prints URL quietly even if LF_DESKTOP is set.
    let headless_caller = seed_session(home.path(), repo.path(), "session_headless_worker", false);
    let headless = lf_cmd(repo.path(), home.path())
        .env("LF_AGENT_CALLER", headless_caller)
        .env("LF_DESKTOP", "1")
        .env("PATH", &test_path)
        .args(["page", "open", "scratch/explore.html"])
        .output()
        .unwrap();
    assert!(
        headless.status.success(),
        "headless worker lf page open failed: {}",
        String::from_utf8_lossy(&headless.stderr)
    );
    let expected_url = String::from_utf8(headless.stdout)
        .unwrap()
        .trim()
        .to_owned();
    assert!(
        expected_url.starts_with("http://127.0.0.1:"),
        "expected loopback URL, got: {expected_url}"
    );
    assert!(
        !opener_marker.exists(),
        "headless worker must not invoke OS browser opener"
    );

    // Interactive plain-terminal session (`interactive == true`, no multiplexer viewed_placement)
    // opens the loopback URL in the browser and prints it.
    let interactive_caller = seed_session(
        home.path(),
        repo.path(),
        "session_interactive_plain_tty",
        true,
    );
    let interactive = lf_cmd(repo.path(), home.path())
        .env("LF_AGENT_CALLER", interactive_caller)
        .env("PATH", &test_path)
        .args(["page", "open", "scratch/explore.html"])
        .output()
        .unwrap();
    assert!(
        interactive.status.success(),
        "interactive plain-terminal lf page open failed: {}",
        String::from_utf8_lossy(&interactive.stderr)
    );
    let interactive_url = String::from_utf8(interactive.stdout)
        .unwrap()
        .trim()
        .to_owned();
    assert_eq!(interactive_url, expected_url);
    assert_eq!(
        fs::read_to_string(&opener_marker)
            .expect("interactive plain-terminal session should invoke OS browser opener")
            .trim(),
        expected_url
    );
}
