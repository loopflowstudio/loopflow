//! Public launch proofs for the Session delivery cutover.
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

use loopflow_test_support::TestRepo;

#[test]
fn claude_startup_preserves_large_combined_seed_and_correlated_answer() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
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
import fcntl, hashlib, json, os, pathlib, select, subprocess, sys
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
    output = subprocess.check_output(hook, shell=True, input=json.dumps({'session_id': 'startup-native', 'hook_event_name': 'SessionStart', 'source': 'startup'}).encode())
    assert json.loads(output)['hookSpecificOutput']['additionalContext']
    (home / 'startup-complete').touch()
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
}
