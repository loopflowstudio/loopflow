//! Public launch proofs for the Session delivery cutover.
mod support;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use loopflow_test_support::TestRepo;

#[test]
fn claude_startup_preserves_large_combined_seed_and_correlated_answer() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let _task =
        support::register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let account_home = home.path().join("accounts/claude/fixture");
    fs::create_dir_all(&account_home).unwrap();
    let credential = serde_json::json!({"claudeAiOauth":{"accessToken":"fixture-only","expiresAt":4102444800000i64}}).to_string();
    fs::write(account_home.join(".credentials.json"), &credential).unwrap();
    let store =
        loopflow::store::sqlite::SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let publication = store
        .publish_repository_steer(
            &loopflow::repository::CanonicalRepo::discover(repo.path()).unwrap(),
            "Fixture author",
            &format!("STARTUP_DIRECTION {}", "界<&>".repeat(20_000)),
        )
        .unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    store
        .upsert_provider_account(&loopflow::store::ProviderAccount {
            provider: "claude".into(),
            account_id: loopflow::store::ProviderAccountId::parse("fixture").unwrap(),
            home: Some(account_home),
            login_email: Some(
                loopflow::profile::EmailAddress::parse("fixture@example.com").unwrap(),
            ),
            observed_email: Some("fixture@example.com".into()),
            observed_subject: Some("fixture".into()),
            observed_credential_digest: Some({
                use sha2::Digest;
                format!("{:x}", sha2::Sha256::digest(credential.as_bytes()))
            }),
            observed_plan: None,
            credential_state: loopflow::store::CredentialState::Connected,
            routing_state: loopflow::store::RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: now,
            updated_at: now,
        })
        .unwrap();
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let skill = repo.path().join(".claude/skills/startup-proof");
    fs::create_dir_all(&skill).unwrap();
    fs::write(
        skill.join("SKILL.md"),
        "---\nname: startup-proof\ndescription: Startup fixture\n---\nReturn the fixture answer.\n",
    )
    .unwrap();
    let clipboard = format!("CLIPBOARD_START{}CLIPBOARD_END", "界".repeat(50_000));
    fs::write(home.path().join("clipboard"), &clipboard).unwrap();
    for name in ["pbpaste", "xclip", "xsel"] {
        let path = bin.join(name);
        fs::write(&path, "#!/bin/sh\ncat \"$LF_HOME/clipboard\"\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let provider = bin.join("claude");
    fs::write(&provider, r#"#!/usr/bin/python3
import fcntl, hashlib, json, os, pathlib, select, shlex, subprocess, sys
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
home = pathlib.Path(os.environ['LF_HOME'])
sys.stderr = open(home / 'provider-errors', 'w', buffering=1)
caller = json.loads(os.environ['LF_AGENT_CALLER'])
lock = home / 'loopflow.session-locks' / hashlib.sha256(caller['session_id'].encode()).hexdigest()
# Force a hook/input overlap without draining a byte of the combined seed.
assert select.select([sys.stdin], [], [], 5)[0], 'no initial input'
with open(lock, 'r+') as fence:
    fcntl.flock(fence, fcntl.LOCK_SH)
    settings = json.load(open(sys.argv[sys.argv.index('--settings') + 1]))
    hook = settings['hooks']['SessionStart'][0]['hooks'][0]['command']
    arguments = shlex.split(hook)
    adapter = json.loads(os.environ['LF_PROVIDER_CONTEXT'])
    assert adapter['agent_process_id'] == caller['agent_process_id']
    previous = home / 'previous-provider'
    if '--resume' in sys.argv:
        assert sys.argv[sys.argv.index('--resume') + 1] == 'startup-native'
        assert previous.read_text() != adapter['agent_process_id'], 'replacement reused old capability'
    previous.write_text(adapter['agent_process_id'])
    output = subprocess.check_output(hook, shell=True, input=json.dumps({'session_id': 'startup-native', 'hook_event_name': 'SessionStart', 'source': 'startup'}).encode())
    context_text = json.loads(output)['hookSpecificOutput']['additionalContext']
    assert len(context_text.encode()) <= 10000
    if '--resume' in sys.argv:
        assert 'STARTUP_DIRECTION' not in context_text, 'resume replayed consumed direction'
    else:
        assert 'STARTUP_DIRECTION' in context_text
        assert '[excerpt; full source above]' in context_text
    (home / 'startup-complete').touch()
if '--resume' in sys.argv:
    query = json.loads(sys.stdin.readline())
    assert 'RESUME_END' in query['message']['content'][0]['text']
else:
    context = json.loads(sys.stdin.readline())
    query = json.loads(sys.stdin.readline())
    assert context['shouldQuery'] is False
    expected = (home / 'clipboard').read_text()
    assert expected in context['message']['content'], 'incomplete non-query context'
    assert query['message']['content'][0]['text'].endswith('QUERY_END'), 'incomplete query'
    assert len(json.dumps(context).encode()) > 131133
assert query['uuid']
query['session_id'] = 'startup-native'
print(json.dumps(query), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'startup-native'}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'answer-id', 'session_id': 'startup-native', 'subtype': 'success', 'is_error': False, 'result': 'CORRELATED_STARTUP_ANSWER'}), flush=True)
sys.stdin.read()
"#).unwrap();
    fs::set_permissions(provider, fs::Permissions::from_mode(0o755)).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path())
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("GIT_ALLOW_PROTOCOL", "file")
        .current_dir(repo.path())
        .args([
            "-b",
            "--isolate",
            "-a",
            "claude",
            "-c",
            "skill",
            "startup-proof",
            "QUERY_END",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
        fs::read_to_string(home.path().join("provider-errors")).unwrap_or_default()
    );
    assert!(home.path().join("startup-complete").exists());
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let (native, answer, turn): (String, String, String) = db
        .query_row(
            "SELECT s.provider_thread,json_extract(e.payload,'$.text'),e.provider_turn
         FROM agent_sessions s JOIN session_events e ON e.session_id=s.id WHERE e.kind='output'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(native, "startup-native");
    assert_eq!(answer, "CORRELATED_STARTUP_ANSWER");
    assert!(!turn.is_empty());
    let session: String = db
        .query_row(
            "SELECT id FROM agent_sessions WHERE provider_thread='startup-native'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let receipts = store.session_steers(&session, 0, 32).unwrap();
    assert_eq!(receipts.items[0].publication.id, publication.id);
    assert_eq!(receipts.items[0].evidence, "emitted");
    assert_eq!(receipts.items[0].excerpted, Some(true));
    assert_eq!(
        store
            .session_steer_source(&session, &publication.id)
            .unwrap()
            .text,
        publication.text
    );
    let resumed = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path())
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .current_dir(repo.path())
        .args([
            "-b",
            "--isolate",
            "session",
            "resume",
            &session,
            "RESUME_END",
        ])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        resumed.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&resumed.stderr),
        fs::read_to_string(home.path().join("provider-errors")).unwrap_or_default()
    );
    let answers: i64 = db.query_row("SELECT count(DISTINCT provider_turn) FROM session_events WHERE session_id=?1 AND kind='output' AND json_extract(payload,'$.text')='CORRELATED_STARTUP_ANSWER'", [&session], |r| r.get(0)).unwrap();
    assert_eq!(
        answers, 2,
        "replacement preserves separate native answer correlation"
    );
}

#[test]
fn public_context_capability_survives_claim_changes_and_rejects_children_and_replacement() {
    use loopflow::id::LfProcessId;
    use loopflow::store::sqlite::SqliteStore;
    use std::io::Write;
    use std::time::{Duration, Instant};

    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let provider = home.path().join("claude");
    fs::write(&provider, r#"#!/usr/bin/python3
import json, os, pathlib, shlex, sys, time
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
home = pathlib.Path(os.environ['LF_HOME'])
settings = json.load(open(sys.argv[sys.argv.index('--settings') + 1]))
args = shlex.split(settings['hooks']['SessionStart'][0]['hooks'][0]['command'])
message = json.loads(sys.stdin.readline())
message['session_id'] = 'handoff-native'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'handoff-native'}), flush=True)
(home / 'handoff.json').write_text(json.dumps({'delivery': args[args.index('--delivery') + 1], 'caller': os.environ['LF_AGENT_CALLER'], 'capture': os.environ['LF_CAPTURE_KEY'], 'capability': os.environ['LF_PROVIDER_CONTEXT']}))
deadline = time.monotonic() + 15
while not (home / 'finish').exists() and time.monotonic() < deadline:
    time.sleep(.01)
"#).unwrap();
    fs::set_permissions(provider, fs::Permissions::from_mode(0o755)).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home.path())
        .env("LF_HOME", home.path())
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env("CLAUDE_CONFIG_DIR", home.path().join(".claude"))
        .env("PATH", format!("{}:/usr/bin:/bin", home.path().display()))
        .current_dir(repo.path())
        .args(["-b", "-a", "claude", ":", "Hold this conversation"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !home.path().join("handoff.json").exists() {
        assert!(Instant::now() < deadline, "provider did not start");
        std::thread::sleep(Duration::from_millis(10));
    }
    let declaration: serde_json::Value =
        serde_json::from_slice(&fs::read(home.path().join("handoff.json")).unwrap()).unwrap();
    let caller: serde_json::Value =
        serde_json::from_str(declaration["caller"].as_str().unwrap()).unwrap();
    let session = caller["session_id"].as_str().unwrap();
    let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    loop {
        let native: Option<String> = db
            .query_row(
                "SELECT provider_thread FROM agent_sessions WHERE id=?1",
                [session],
                |r| r.get(0),
            )
            .unwrap();
        if native.as_deref() == Some("handoff-native") {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "native identity was not recorded"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let first = store.session_attachment(session).unwrap().unwrap();
    let other = LfProcessId::new();
    db.execute(
        "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
        [&other],
    )
    .unwrap();
    let second = store
        .claim_session_attachment(session, Some(&first), &other, false)
        .unwrap();
    let third = store
        .claim_session_attachment(
            session,
            Some(&second),
            first.lf_process_id.as_ref().unwrap(),
            false,
        )
        .unwrap();
    assert_eq!(first.agent_process_id, third.agent_process_id);
    assert_ne!(first.token, third.token);
    let hook = |payload: serde_json::Value| {
        let mut hook = Command::new(env!("CARGO_BIN_EXE_lf"))
            .env_clear()
            .env("HOME", home.path())
            .env("LF_HOME", home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("LF_AGENT_CALLER", declaration["caller"].as_str().unwrap())
            .env("LF_CAPTURE_KEY", declaration["capture"].as_str().unwrap())
            .env(
                "LF_PROVIDER_CONTEXT",
                declaration["capability"].as_str().unwrap(),
            )
            .env("PATH", "/usr/bin:/bin")
            .current_dir(repo.path())
            .args([
                "__context-block",
                "--delivery",
                declaration["delivery"].as_str().unwrap(),
                "--moment",
                "start",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        hook.stdin
            .take()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        hook.wait_with_output().unwrap()
    };
    let direction = store
        .publish_session_steer(session, "Fixture author", "HANDOFF_DIRECTION")
        .unwrap();
    let payload = serde_json::json!({"session_id":"handoff-native", "hook_event_name":"PostToolUse", "tool_use_id":"handoff-tool", "turn_id":"native-turn"});
    let received = hook(payload.clone());
    assert!(
        received.status.success(),
        "{}",
        String::from_utf8_lossy(&received.stderr)
    );
    assert!(String::from_utf8_lossy(&received.stdout).contains("HANDOFF_DIRECTION"));
    assert_eq!(
        store.session_steers(session, 0, 32).unwrap().items[0].evidence,
        "emitted"
    );
    let pending = store
        .publish_session_steer(session, "Fixture author", "NEXT_DIRECTION")
        .unwrap();
    let mut nested = payload.clone();
    nested["agent_id"] = serde_json::json!("child");
    let rejected = hook(nested);
    assert!(rejected.stdout.is_empty());
    let mut foreign = payload.clone();
    foreign["session_id"] = serde_json::json!("child-native");
    let rejected = hook(foreign);
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    assert_eq!(store.session_steers(session, 0, 32).unwrap().pending, 1);
    // Exercise the shipped OpenCode transform against the real receipt command.
    // Internal stream retries reuse one transformed value; a later callback has
    // another adapter invocation and must replace that ephemeral context copy.
    let plugin = home.path().join("transform.mjs");
    let delivery_path = declaration["delivery"].as_str().unwrap();
    let delivery: serde_json::Value =
        serde_json::from_slice(&fs::read(delivery_path).unwrap()).unwrap();
    fs::write(&plugin, format!("const spec = {};\nconst delivery = {};\n{}",
        serde_json::json!({"executable":env!("CARGO_BIN_EXE_lf"),"delivery":delivery,"instructions":""}),
        serde_json::to_string(delivery_path).unwrap(),
        include_str!("../src/harness/context_opencode.js"))).unwrap();
    let probe = home.path().join("transform-proof.mjs");
    fs::write(
        &probe,
        r#"
import assert from 'node:assert/strict';
import plugin from './transform.mjs';
const hooks = await plugin();
await hooks['chat.message']({sessionID:'handoff-native'});
const output = {messages:[{info:{role:'user',id:'user-one',sessionID:'handoff-native'},parts:[]}]};
const transform = hooks['experimental.chat.messages.transform'];
await transform({},output);
const first = JSON.stringify(output);
assert(first.includes('NEXT_DIRECTION'));
assert.equal(JSON.stringify(structuredClone(output)), first); // processor stream retry input
const afterLostTransform = {messages:[{info:{role:'user',id:'user-one',sessionID:'handoff-native'},parts:[]}]};
await transform({},afterLostTransform);
assert(!JSON.stringify(afterLostTransform).includes('NEXT_DIRECTION')); // loss is emitted/unconfirmed
await transform({},output);
assert(!JSON.stringify(output).includes('NEXT_DIRECTION'));
assert.equal(output.messages[0].parts.filter(p=>p.id==='lf-context-user-one').length,1);
const child = {messages:[{info:{role:'user',id:'child',sessionID:'another-native'},parts:[]}]};
await transform({},child);
assert.equal(child.messages[0].parts.length,0);
"#,
    )
    .unwrap();
    let protocol = Command::new("node")
        .arg(&probe)
        .env("LF_HOME", home.path())
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env(
            "LF_PROVIDER_CONTEXT",
            declaration["capability"].as_str().unwrap(),
        )
        .output()
        .unwrap();
    assert!(
        protocol.status.success(),
        "{}",
        String::from_utf8_lossy(&protocol.stderr)
    );
    let page = store.session_steers(session, 0, 32).unwrap();
    assert_eq!(page.pending, 0);
    assert_eq!(
        page.items
            .iter()
            .find(|row| row.publication.id == pending.id)
            .unwrap()
            .evidence,
        "emitted"
    );
    assert_eq!(
        page.items
            .iter()
            .find(|row| row.publication.id == direction.id)
            .unwrap()
            .attempt,
        0
    );
    store
        .publish_session_steer(session, "Fixture author", "FAILED_TOOL_DIRECTION")
        .unwrap();
    let failed = hook(serde_json::json!({"session_id":"handoff-native",
        "hook_event_name":"PostToolUseFailure","tool_use_id":"failed-tool"}));
    assert!(
        failed.status.success(),
        "{}",
        String::from_utf8_lossy(&failed.stderr)
    );
    assert!(String::from_utf8_lossy(&failed.stdout).contains("FAILED_TOOL_DIRECTION"));
    store
        .claim_session_attachment(session, Some(&third), &other, true)
        .unwrap();
    let stale = hook(payload);
    assert!(!stale.status.success());
    assert!(stale.stdout.is_empty());
    fs::write(home.path().join("finish"), "").unwrap();
    let mut child = child;
    child.wait().unwrap();
}

#[test]
fn public_publication_commands_save_direction_without_launching_providers() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    let task =
        support::register_task_without_pr(home.path(), repo.path(), "main", &repo.head_sha());
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .env_clear()
            .env("HOME", home.path())
            .env("LF_HOME", home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_ALLOW_PROTOCOL", "file")
            .current_dir(repo.path())
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    run(&["repo", "comment", "REPOSITORY_DIRECTION", "--steer"]);
    run(&[
        "wave",
        "comment",
        task.task.wave_id.as_str(),
        "WAVE_DIRECTION",
        "--steer",
    ]);
    run(&[
        "comment",
        task.task.id.as_str(),
        "TASK_DIRECTION",
        "--steer",
    ]);
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    conn.execute(
        "INSERT INTO agent_sessions(id,title,title_source,created_at,cwd,input_published,repo)
        VALUES('idle','Idle fixture','human',1,?1,1,?2)",
        rusqlite::params![
            repo.path().to_str().unwrap(),
            loopflow::repository::CanonicalRepo::discover(repo.path())
                .unwrap()
                .to_string()
        ],
    )
    .unwrap();
    let artifact = format!("run_{}", uuid::Uuid::new_v4().simple());
    conn.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
        VALUES('idle','captured',?1,1,json_object('artifact_key',?1))",
        [artifact.as_str()],
    )
    .unwrap();
    conn.execute(
        "UPDATE agent_sessions SET current_capture=?1 WHERE id='idle'",
        [conn.last_insert_rowid()],
    )
    .unwrap();
    run(&["session", "steer", "idle", "DIRECT_DIRECTION"]);
    let page: serde_json::Value = serde_json::from_str(&run(&[
        "session", "steers", "idle", "--json", "--limit", "2",
    ]))
    .unwrap();
    assert_eq!(page["pending"], 4, "{page}");
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert!(page["capability"].as_str().unwrap().contains("idle"));
    let cursor = page["next_after"].as_i64().unwrap().to_string();
    let rest: serde_json::Value = serde_json::from_str(&run(&[
        "session", "steers", "idle", "--json", "--after", &cursor,
    ]))
    .unwrap();
    assert_eq!(rest["items"].as_array().unwrap().len(), 2);
    let publication = rest["items"][1]["publication"]["id"].as_str().unwrap();
    let source: serde_json::Value = serde_json::from_str(&run(&[
        "session",
        "steers",
        "idle",
        "--source",
        publication,
    ]))
    .unwrap();
    assert_eq!(source["text"], "DIRECT_DIRECTION");
    let provider_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM processes WHERE kind='agent'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(provider_count, 0);
}
