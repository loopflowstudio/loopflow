pub mod planning;

use loopflow_test_support::env_lock;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use loopflow::id::WaveId;
use loopflow::linear::{LinearIssueId, LinearProjectId};
use loopflow::store::{StorageConfig, Store};
use loopflow::work::project::{Project, ProjectId};
use loopflow::work::task::{LinearWritebackState, Task, TaskId, TaskPr, TaskPrId};
use loopflow::work::wave::Wave;
use loopflow_test_support::TestRepo;
use tempfile::TempDir;
use time::OffsetDateTime;

pub mod ambient;
#[cfg(unix)]
pub mod rex;

// Shared with the library's cfg(test) store setup; production never embeds it.
pub use loopflow_storage::test_database;

#[allow(dead_code)] // Shared helper compiled into each integration test binary.
pub fn seed_store(home: &Path) {
    test_database::copy(&home.join("loopflow.db")).expect("copy fixture store");
}

/// Agent shells such as Codex export the developer's real provider home
/// (`CODEX_HOME=~/.codex`). An `lf` fixture child that inherits it would swap
/// fixture logins into that home, so every test binary drops both variables
/// before `main`, while no other thread can read the environment. Fixtures
/// that need a provider home set one explicitly.
///
/// `~/.codex` and `~/.claude` are reachable through `HOME` the same way, so
/// the binary also moves `HOME` into a temp directory, keeping Cargo's and
/// rustup's own homes for fixtures that build `lf`. A fixture that clears the
/// environment sets its own `HOME`: with none, `lf` asks the OS for the
/// account's real one.
///
/// A fixture `lf` that signs a Claude home in on macOS runs `security`, so
/// the Keychain stand-in also goes first on PATH for the whole binary: a
/// fixture that forgets its own would otherwise leave a real Keychain item
/// per run. Fixtures that replace PATH outright find no `security` at all.
pub fn isolate_from_the_developers_machine() {
    env::remove_var("CODEX_HOME");
    env::remove_var("CLAUDE_CONFIG_DIR");
    let root = env::temp_dir().join(format!("lf-test-{}", std::process::id()));
    let home = root.join("home");
    fs::create_dir_all(&home).expect("create the test binary's home");
    // Store fixtures also name repositories that are absent on this Machine.
    // Give those claims an identity without borrowing the developer's Git config.
    fs::write(
        home.join(".gitconfig"),
        "[user]\n\tname = Loopflow Test\n\temail = test@loopflow.invalid\n",
    )
    .expect("write the test binary's Git identity");
    if let Some(developer) = env::var_os("HOME").map(std::path::PathBuf::from) {
        for (name, directory) in [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")] {
            if env::var_os(name).is_none() {
                env::set_var(name, developer.join(directory));
            }
        }
    }
    env::set_var("HOME", &home);
    env::set_var("LF_HOME", root.join("lf"));
    let bin = root.join("bin");
    fs::create_dir_all(&bin).expect("create the Keychain stand-in directory");
    write_executable(&bin, "security", FAKE_SECURITY);
    let path = env::var_os("PATH").unwrap_or_default();
    let joined = env::join_paths(std::iter::once(bin).chain(env::split_paths(&path)))
        .expect("temp dir is a valid PATH entry");
    env::set_var("PATH", joined);
}

/// A macOS `security` stand-in that keeps generic passwords in a `keychain`
/// directory beside itself. Signing a fixture's shared Claude home in goes
/// through the Keychain on macOS; with this first on PATH it never reaches
/// the real one. Each item's password is the file named by its service in hex.
#[allow(dead_code)] // Shared Keychain fixture compiled into multiple test crates.
pub const FAKE_SECURITY: &str = r#"#!/usr/bin/python3
import os, shlex, sys
store = os.path.join(os.path.dirname(os.path.realpath(__file__)), 'keychain')
os.makedirs(store, exist_ok=True)

def item(service):
    return os.path.join(store, service.encode().hex())

def options(rest, valued):
    found, flags, args = {}, set(), iter(rest)
    for arg in args:
        if arg in valued:
            found[arg] = next(args)
        else:
            flags.add(arg)
    return found, flags

def missing():
    sys.stderr.write('security: SecKeychainSearchCopyNext: The specified item could not be found in the keychain.\n')
    sys.exit(44)

def run(command, rest):
    if command == 'add-generic-password':
        found, _ = options(rest, {'-a', '-s', '-X', '-w', '-l', '-T', '-D', '-j'})
        password = bytes.fromhex(found['-X']).decode() if '-X' in found else found.get('-w', '')
        open(item(found['-s']), 'w').write(password)
        open(item(found['-s']) + '.acct', 'w').write(found.get('-a', ''))
        return
    found, flags = options(rest, {'-a', '-s'})
    path = item(found.get('-s', ''))
    if not os.path.exists(path):
        missing()
    if command == 'find-generic-password':
        if '-w' in flags:
            print(open(path).read())
        else:
            print('attributes:\n    "acct"<blob>="%s"' % open(path + '.acct').read())
    elif command == 'delete-generic-password':
        os.remove(path)
        os.remove(path + '.acct')
    else:
        sys.exit(1)

if sys.argv[1:2] == ['-i']:
    for line in sys.stdin:
        if line.strip():
            words = shlex.split(line)
            run(words[0], words[1:])
else:
    run(sys.argv[1], sys.argv[2:])
"#;

/// A Python Claude stand-in that can call
/// `keep_claude_turn(request, answer, structured=None)` to keep each turn in
/// Claude's transcript, where lf reads final answers.
#[allow(dead_code)] // Shared native provider fixture compiled into multiple test crates.
pub fn claude_stand_in(script: &str) -> String {
    let (shebang, body) = script
        .split_once('\n')
        .expect("a stand-in script starts with its shebang line");
    format!(
        "{shebang}\nimport sys\nif sys.argv[1:] == ['auth', 'status']: sys.exit(0)\n{}\n{body}",
        include_str!("claude_transcript.py")
    )
}

#[cfg(unix)]
#[allow(dead_code)] // Shared native provider fixture compiled into multiple test crates.
pub fn install_native_codex(bin: &Path) {
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(bin).unwrap();
    let provider = bin.join("codex");
    fs::write(
        &provider,
        format!(
            "#!/bin/sh\nexec /usr/bin/python3 '{}' \"$@\"\n",
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/native_codex.py")
        ),
    )
    .unwrap();
    fs::set_permissions(provider, fs::Permissions::from_mode(0o755)).unwrap();
}

#[allow(dead_code)] // Shared GitHub fixture compiled into multiple test crates.
pub fn github_checks_page(head: &str, checks: &[(&str, &str, bool)]) -> String {
    let nodes: Vec<_> = checks
        .iter()
        .map(|(name, conclusion, required)| {
            serde_json::json!({
                "__typename":"CheckRun", "name":name, "status":"COMPLETED",
                "conclusion":conclusion, "isRequired":required,
                "startedAt":"2026-09-29T00:00:00Z", "detailsUrl":format!("https://ci/{name}"),
                "checkSuite":{"workflowRun":null}
            })
        })
        .collect();
    serde_json::json!({"head":head,"commit":head,"contexts":{
        "nodes":nodes,"pageInfo":{"hasNextPage":false,"endCursor":null}
    }})
    .to_string()
}

#[allow(dead_code)] // Shared GitHub fixture compiled into multiple test crates.
pub fn github_merge_response(
    number: u64,
    head: &str,
    state: &str,
    merge_state: &str,
    request: Option<&str>,
) -> String {
    let queued = request.and_then(|request| request.strip_prefix("queued:"));
    serde_json::json!({"data":{"repository":{"mergeCommitAllowed":true,"squashMergeAllowed":true,"rebaseMergeAllowed":true,"pullRequest":{
        "id":queued.unwrap_or("PR_fixture"), "number":number,
        "url":format!("https://example.com/pr/{number}"),
        "state":state, "isDraft":false, "headRefName":"fixture", "headRefOid":head,
        "mergedAt":if state == "MERGED" { Some("2026-09-29T00:00:00Z") } else { None },
        "mergeCommit":if state == "MERGED" { Some(serde_json::json!({"oid":head})) } else { None },
        "mergeStateStatus":merge_state, "isMergeQueueEnabled":request == Some("awaiting_queue") || queued.is_some(), "isInMergeQueue":queued.is_some(),
        "autoMergeRequest":request.map(|_| serde_json::json!({"enabledAt":"2026-09-29T00:00:00Z"})),
        "mergeQueueEntry":queued.map(|_| serde_json::json!({"id":"queue-entry"})),
        "timelineItems":{"nodes":[]}
    }}}}).to_string()
}

struct HomeOverride {
    _ambient: ambient::EnvGuard,
    previous_lf_home: Option<OsString>,
    _temp: TempDir,
}

impl HomeOverride {
    fn new_temp() -> Self {
        let ambient = ambient::EnvGuard::new();
        let temp = TempDir::new().expect("temp home dir");
        let previous_lf_home = env::var_os("LF_HOME");
        env::remove_var("LF_HOME");
        env::set_var("LF_HOME", temp.path());
        Self {
            _ambient: ambient,
            previous_lf_home,
            _temp: temp,
        }
    }
}

impl Drop for HomeOverride {
    fn drop(&mut self) {
        match &self.previous_lf_home {
            Some(prev) => env::set_var("LF_HOME", prev),
            None => env::remove_var("LF_HOME"),
        }
    }
}

#[allow(dead_code)] // Shared helper compiled into multiple test crates.
pub fn with_clean_home<T>(f: impl FnOnce() -> T) -> T {
    let _lock = env_lock().lock().unwrap_or_else(|err| err.into_inner());
    let _home = HomeOverride::new_temp();
    f()
}

#[allow(dead_code)] // Shared provider mock compiled into multiple test crates.
pub fn codex_app_server_script(output: &str, setup: &str) -> String {
    let output = serde_json::to_string(output)
        .expect("encode mock Codex output")
        .replace('\'', r#"'"'"'"#);
    let rollout = codex_rollout_script();
    // Codex names threads and turns with UUIDs; a resumed thread keeps its id.
    codex_socket_script(&r#"#!/bin/sh
# The launch's login probe.
if [ "$1 $2" = "login status" ]; then echo 'Logged in using ChatGPT'; exit 0; fi
__SETUP__
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r thread_start
thread=$(printf '%s' "$thread_start" | python3 -c 'import json, sys, uuid; print(json.load(sys.stdin).get("params", {}).get("threadId") or uuid.uuid4())')
turn=$(python3 -c 'import uuid; print(uuid.uuid4())')
echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"'"$thread"'"}}}'
read -r turn_start
echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"'"$turn"'"}}}'
echo '{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"'"$thread"'","turn":{"id":"'"$turn"'","status":"inProgress"}}}'
# A fixture that left `tool_turn.py` gets the hooks lf declared, run around a tool.
if [ -f "$LF_HOME/tool_turn.py" ]; then
    hooks=$(printf '%s' "$thread_start" | python3 -c 'import json, sys; print(json.dumps(json.load(sys.stdin)["params"]["config"]["hooks"]))')
    python3 "$LF_HOME/tool_turn.py" "$thread" "$hooks" || exit 1
fi
# A fixture that left `hold-turn` holds the turn open until it is removed.
if [ -f "$LF_HOME/hold-turn" ]; then
    printf '%s' "$thread" > "$LF_HOME/holding"
    while [ -f "$LF_HOME/hold-turn" ]; do sleep 0.05; done
fi
printf '%s\n' '{"jsonrpc":"2.0","method":"item/agentMessage/delta","params":{"threadId":"'"$thread"'","turnId":"'"$turn"'","itemId":"message-test","delta":__OUTPUT__}}'
printf '%s\n' '{"jsonrpc":"2.0","method":"item/completed","params":{"threadId":"'"$thread"'","turnId":"'"$turn"'","item":{"type":"agentMessage","id":"message-test","text":__OUTPUT__,"phase":"final_answer"}}}'
python3 '__ROLLOUT__' "$thread" "$turn" '__OUTPUT__' || exit 1
echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"'"$thread"'","turn":{"id":"'"$turn"'","status":"completed"}}}'
if [ -n "$LF_TEST_CODEX_STDIO" ]; then exit 0; fi
while read -r line; do :; done
"#
    .replace("__SETUP__", setup)
    .replace("__ROLLOUT__", &rollout)
    .replace("__OUTPUT__", &output))
}

/// The script that appends a completed turn to the rollout Codex keeps,
/// where lf reads final answers, quoted for a single-quoted shell word:
/// `python3 '<script>' THREAD TURN ANSWER_JSON`.
#[allow(dead_code)] // Shared provider mock compiled into multiple test crates.
pub fn codex_rollout_script() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/codex_rollout.py"
    )
    .replace('\'', r#"'"'"'"#)
}

#[allow(dead_code)] // Shared provider transport compiled into multiple test crates.
pub fn codex_socket_script(script: &str) -> String {
    let bridge = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/codex_socket.py")
        .replace('\'', r#"'"'"'"#);
    format!(
        r#"#!/bin/sh
if [ "$*" = "login status" ]; then exit 0; fi
if [ -z "$LF_TEST_CODEX_STDIO" ]; then
    case "$*" in
        *--listen*) exec python3 '{bridge}' "$0" "$@" ;;
    esac
fi
{script}
"#,
        script = script.strip_prefix("#!/bin/sh\n").unwrap_or(script),
    )
}

/// A Codex whose first turn the provider refuses, in the words Codex sends:
/// the rollout holds the thread and nothing the agent said.
#[allow(dead_code)] // Shared provider mock compiled into multiple test crates.
pub fn refused_codex_script() -> String {
    codex_socket_script(
        r#"#!/bin/sh
read -r initialize
echo '{"jsonrpc":"2.0","id":1,"result":{}}'
read -r initialized
read -r thread_start
thread=$(python3 -c 'import uuid; print(uuid.uuid4())')
turn=$(python3 -c 'import uuid; print(uuid.uuid4())')
echo '{"jsonrpc":"2.0","id":2,"result":{"thread":{"id":"'"$thread"'"}}}'
read -r turn_start
echo '{"jsonrpc":"2.0","id":3,"result":{"turn":{"id":"'"$turn"'"}}}'
echo '{"jsonrpc":"2.0","method":"turn/started","params":{"threadId":"'"$thread"'","turn":{"id":"'"$turn"'","status":"inProgress"}}}'
day="$CODEX_HOME/sessions/$(date -u +%Y/%m/%d)"
mkdir -p "$day"
printf '%s\n' \
  '{"type":"session_meta","payload":{"id":"'"$thread"'","cwd":"'"$PWD"'"}}' \
  '{"type":"response_item","payload":{"type":"reasoning","id":"thought","summary":[{"type":"summary_text","text":"Checking the task."}]}}' \
  > "$day/rollout-$(date -u +%Y-%m-%dT%H-%M-%S)-$thread.jsonl"
echo '{"jsonrpc":"2.0","method":"error","params":{"threadId":"'"$thread"'","turnId":"'"$turn"'","willRetry":false,"error":{"message":"You’ve hit your usage limit. Upgrade to Pro, or try again at 9:41 PM.","codexErrorInfo":null,"additionalDetails":null}}}'
echo '{"jsonrpc":"2.0","method":"turn/completed","params":{"threadId":"'"$thread"'","turn":{"id":"'"$turn"'","status":"failed","error":{"message":"You’ve hit your usage limit. Upgrade to Pro, or try again at 9:41 PM.","codexErrorInfo":null,"additionalDetails":null}}}}'
if [ -n "$LF_TEST_CODEX_STDIO" ]; then exit 0; fi
while read -r line; do :; done
"#,
    )
}

/// A Claude that answers one turn with `answer` and keeps the prompt it was
/// sent in `received`.
#[allow(dead_code)] // Shared provider mock compiled into multiple test crates.
pub fn answering_claude_script(answer: &str, received: &Path) -> String {
    r#"#!/usr/bin/python3
import json, sys
if sys.argv[1:] == ['auth', 'status']: sys.exit(0)
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
message = json.loads(sys.stdin.readline())
open(__RECEIVED__, 'w').write(message['message']['content'][0]['text'])
message['session_id'] = 'fixture-thread'
print(json.dumps(message), flush=True)
print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'fixture-thread'}), flush=True)
answer = __ANSWER__
print(json.dumps({'type': 'assistant', 'message': {'content': [{'type': 'text', 'text': answer}]}}), flush=True)
print(json.dumps({'type': 'result', 'uuid': 'fixture-result', 'session_id': 'fixture-thread', 'subtype': 'success', 'is_error': False, 'result': answer}), flush=True)
for line in sys.stdin:
    pass
"#
    .replace(
        "__RECEIVED__",
        &serde_json::to_string(&received.display().to_string()).expect("encode path"),
    )
    .replace(
        "__ANSWER__",
        &serde_json::to_string(answer).expect("encode answer"),
    )
}

/// A launch routes by the process directory: the checkout it runs in, never
/// the developer's, whose private route belongs to their Machine.
#[allow(dead_code)] // Shared helper compiled into multiple test crates.
pub struct InDirectory(PathBuf);

#[allow(dead_code)] // Shared helper compiled into multiple test crates.
impl InDirectory {
    pub fn enter(path: &Path) -> Self {
        let previous = env::current_dir().unwrap();
        env::set_current_dir(path).unwrap();
        Self(previous)
    }
}

impl Drop for InDirectory {
    fn drop(&mut self) {
        env::set_current_dir(&self.0).unwrap();
    }
}

pub struct EnvGuard {
    previous_path: Option<String>,
    previous_home: Option<String>,
    previous_lf_home: Option<OsString>,
    previous_lf_bin: Option<OsString>,
    _ambient: ambient::EnvGuard,
    _bin: TempDir,
    _lf_home: TempDir,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    #[allow(dead_code)] // Shared helper compiled into multiple test crates.
    pub fn new(entries: &[(&str, &str)]) -> Self {
        Self::_with_home_and_path(entries, None, true)
    }

    #[allow(dead_code)] // Shared helper used by tests that require PATH isolation.
    pub fn new_isolated(entries: &[(&str, &str)]) -> Self {
        Self::_with_home_and_path(entries, None, false)
    }

    #[allow(dead_code)] // Shared helper used only by tests that need HOME isolation.
    pub fn with_home(entries: &[(&str, &str)], home: Option<&Path>) -> Self {
        Self::_with_home_and_path(entries, home, true)
    }

    fn _with_home_and_path(
        entries: &[(&str, &str)],
        home: Option<&Path>,
        include_existing_path: bool,
    ) -> Self {
        let lock = env_lock().lock().unwrap_or_else(|err| err.into_inner());
        let cli = loopflow_test_support::lf_binary();
        let bin = TempDir::new().expect("temp bin dir");
        write_executable(bin.path(), "security", FAKE_SECURITY);
        for (name, content) in entries {
            write_executable(bin.path(), name, content);
        }
        let previous_path = env::var("PATH").ok();
        let new_path = match (&previous_path, include_existing_path) {
            (Some(prev), true) => format!(
                "{}:{}",
                bin.path().display(),
                loopflow_test_support::path_without_installed_harnesses(prev)
            ),
            _ => bin.path().display().to_string(),
        };
        env::set_var("PATH", new_path);
        let previous_home = env::var("HOME").ok();
        if let Some(home) = home {
            env::set_var("HOME", home);
        }
        let previous_lf_home = env::var_os("LF_HOME");
        let ambient = ambient::EnvGuard::new();
        let previous_lf_bin = env::var_os("LF_BIN");
        env::set_var("LF_BIN", cli);
        let lf_home = TempDir::new().expect("temp lf machine dir");
        env::remove_var("LF_HOME");
        if let Some(home) = home {
            // Keep HOME-based config discovery intact while isolating its store.
            env::set_var("LF_HOME", home.join(".lf"));
        } else {
            env::set_var("LF_HOME", lf_home.path());
        }
        // Native provider homes must never reach the developer's own credentials.
        env::set_var("CODEX_HOME", lf_home.path().join("native-codex"));
        env::set_var("CLAUDE_CONFIG_DIR", lf_home.path().join("native-claude"));
        Self {
            _lock: lock,
            previous_path,
            previous_home,
            previous_lf_home,
            previous_lf_bin,
            _ambient: ambient,
            _bin: bin,
            _lf_home: lf_home,
        }
    }

    #[allow(dead_code)] // Shared helper used by tests that exercise the local registry.
    pub fn with_lf_home(entries: &[(&str, &str)], home: &Path) -> Self {
        let guard = Self::with_home(entries, None);
        env::set_var("LF_HOME", home);
        guard
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if let Some(prev) = &self.previous_path {
            env::set_var("PATH", prev);
        } else {
            env::remove_var("PATH");
        }
        if let Some(prev) = &self.previous_home {
            env::set_var("HOME", prev);
        } else {
            env::remove_var("HOME");
        }
        match &self.previous_lf_bin {
            Some(previous) => env::set_var("LF_BIN", previous),
            None => env::remove_var("LF_BIN"),
        }
        match &self.previous_lf_home {
            Some(prev) => env::set_var("LF_HOME", prev),
            None => env::remove_var("LF_HOME"),
        }
        // The pre-main scrub left no provider home to restore.
        env::remove_var("CODEX_HOME");
        env::remove_var("CLAUDE_CONFIG_DIR");
    }
}

#[allow(dead_code)] // Shared helper compiled into integration tests that do not need Task state.
pub fn bind_task_planning(repo: &TestRepo) {
    repo.create_file(
        ".lf/config.yaml",
        "planning:\n  provider: linear\n  team: team-task-pr-tests\n",
    );
    repo.create_file(".lf/wave/task-pr-tests/GOAL.md", "Keep work.\n");
    repo.create_file(
        ".lf/wave/task-pr-tests/config.yaml",
        "linear:\n  initiative: initiative-task-pr-tests\n",
    );
    repo.stage_all();
    repo.commit("Bind fixture planning before creating Task checkouts");
}

/// Select a Wave's default graph in its committed `config.yaml` in `repo`.
#[allow(dead_code)] // Shared helper compiled into integration tests without Wave graphs.
pub fn select_wave_graph(
    repo: &Path,
    store: &loopflow::store::sqlite::SqliteStore,
    wave: &WaveId,
    name: &str,
) {
    set_wave_setting(repo, store, wave, "graph", name);
}

/// Set `key` in a Wave's committed `config.yaml` in `repo`.
#[allow(dead_code)] // Shared helper compiled into integration tests without Wave settings.
pub fn set_wave_setting(
    repo: &Path,
    store: &loopflow::store::sqlite::SqliteStore,
    wave: &WaveId,
    key: &str,
    value: &str,
) {
    let wave = store.get_wave(wave).unwrap().unwrap();
    let path = repo.join(".lf/wave").join(wave.slug()).join("config.yaml");
    let mut config: serde_yaml_ng::Mapping = fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_yaml_ng::from_str(&content).ok())
        .unwrap_or_default();
    config.insert(key.into(), value.into());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, serde_yaml_ng::to_string(&config).unwrap()).unwrap();
    for args in [
        vec!["add", "--", path.to_str().unwrap()],
        vec!["commit", "--quiet", "-m", "Set the Wave's configuration"],
    ] {
        let status = std::process::Command::new("git")
            .current_dir(repo)
            .args(args)
            .status()
            .unwrap();
        assert!(
            status.success(),
            "git could not commit the Wave's configuration"
        );
    }
}

#[allow(dead_code)] // Shared helper compiled into integration tests that do not need Task state.
pub struct RegisteredTask {
    pub database: std::path::PathBuf,
    pub store: Store,
    pub task: Task,
    pub pr: TaskPr,
}

/// Seed a retained PR row explicitly, independently of checkout placement.
#[allow(dead_code)] // Shared helper compiled into tests without delivery fixtures.
pub fn register_task_with_pr(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    let registered = register_task_fixture(home, worktree, branch, base_commit);
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(registered.store.insert_task_pr(&registered.pr))
        .unwrap();
    registered
}

/// The checkout path as `lf` records it: Git resolves symlinks, so a Task
/// checkout under macOS's `/var` temp directory is stored as `/private/var`.
/// A path that does not exist yet stays as spelled.
fn recorded_checkout(worktree: &Path) -> PathBuf {
    worktree
        .canonicalize()
        .unwrap_or_else(|_| worktree.to_path_buf())
}

#[allow(dead_code)] // Shared helper compiled into suites with published Tasks only.
pub fn register_task_without_pr(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    register_task_fixture(home, worktree, branch, base_commit)
}

fn register_task_fixture(
    home: &Path,
    worktree: &Path,
    branch: &str,
    base_commit: &str,
) -> RegisteredTask {
    let worktree = &recorded_checkout(worktree);
    // Register fixture Tasks before starting the Machine server so its startup
    // and this open cannot compete for the migration write lock.
    seed_store(home);
    let runtime = tokio::runtime::Runtime::new().expect("task test runtime");
    let store = runtime
        .block_on(loopflow::store::open_ephemeral_store(
            &StorageConfig::sqlite(home.join("loopflow.db")),
        ))
        .expect("open task test store");
    let now = OffsetDateTime::now_utc();
    let wave = Wave::new(
        WaveId::new(),
        "task-pr-tests".to_string(),
        worktree.display().to_string(),
    );
    let project = Project {
        id: ProjectId::new(),

        summary: String::new(),
        status: loopflow::work::project::ProjectStatus::Started,
        tracker_id: Some(
            LinearProjectId::new(format!("project-{}", WaveId::new())).expect("project id"),
        ),
        slug: "task-pr-tests".to_string(),
        name: "Task PR tests".to_string(),
        description: "Keep Task PR transitions durable.".to_string(),
        krs: Vec::new(),
        metric_targets: Vec::new(),
        tracker_synced_at: Some(now.unix_timestamp()),

        wave_id: wave.id().clone(),
        created_at: now,
        updated_at: now,
    };
    let task = Task {
        parent_task_id: None,
        repository_id: loopflow::repository::location::ensure_repository(
            &loopflow::store::open_sqlite(&home.join("loopflow.db")).unwrap(),
            wave.repo(),
        )
        .unwrap(),
        id: TaskId::new(),

        revision: 0,
        tracker_id: Some(LinearIssueId::new(format!("issue-{}", WaveId::new())).expect("issue id")),
        identifier: "INF-123".to_string(),
        title: "Prove Task PR transitions".to_string(),
        description: "Exercise the persisted lifecycle.".to_string(),
        tracker_synced_at: Some(now.unix_timestamp()),

        tracker_writeback: LinearWritebackState::Current,
        wave_id: Some(wave.id().clone()),
        project_id: Some(project.id.clone()),
        worktree: Some(worktree.to_path_buf()),
        workspace_slug: "task-pr-proof".to_string(),
        branch: branch.to_string(),
        base_commit: base_commit.to_string(),
        parent_pr_id: None,
        created_at: now,
        updated_at: now,
        rank: 0,
        assignee: None,
        due_date: None,
        state: None,
        completed_at: None,
        observation: loopflow::work::task::Observation::NotRequired,
    };
    let mut pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence: 1,
        slug: task.workspace_slug.clone(),
        branch: branch.to_string(),
        base_commit: base_commit.to_string(),
        parent_pr_id: None,
        publication: None,
        merge_commit: None,
        abandoned_at: None,
        created_at: now,
        updated_at: now,
        ci_observation: None,
        github_observation: None,
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
    };
    runtime.block_on(async {
        store.create_wave(&wave).await.expect("create test wave");
        store
            .create_project(&project)
            .await
            .expect("create test project");
        rusqlite::Connection::open(home.join("loopflow.db"))
            .expect("open fixture database")
            .execute(
                "UPDATE waves SET current_project_id=?2 WHERE id=?1",
                rusqlite::params![wave.id(), project.id.as_str()],
            )
            .expect("select fixture Project");
        planning::seed_unplaced_task(&home.join("loopflow.db"), &task);
        planning::seed_linear_admission(&home.join("loopflow.db"), &wave, &project, &task);
        pr.task_id = task.id.clone();
        store
            .place_task(
                &task.id,
                task.worktree.as_ref().unwrap(),
                &task.workspace_slug,
                &pr,
                None,
            )
            .await
            .expect("create test Task");
        store
            .append_task_event(
                &task.id,
                &loopflow::work::task::TaskEventKind::CheckoutReady {
                    branch: branch.to_string(),
                    base_commit: base_commit.to_string(),
                },
            )
            .await
            .unwrap();
    });
    RegisteredTask {
        store,
        task,
        pr,
        database: home.join("loopflow.db"),
    }
}

/// A second Task in the fixture's Wave and Project, tracking `branch` from its
/// own `worktree`, so a test can tell "this checkout's Task" from "another
/// Task". Registered worktrees are unique, so the sibling needs a path of its
/// own; `TestRepo::create_named_worktree` supplies a real one with the branch.
#[allow(dead_code)] // Shared helper compiled into integration tests with one Task.
pub fn register_sibling_task(
    registered: &RegisteredTask,
    identifier: &str,
    branch: &str,
    worktree: &Path,
) -> Task {
    let worktree = &recorded_checkout(worktree);
    let runtime = tokio::runtime::Runtime::new().expect("task test runtime");
    let now = OffsetDateTime::now_utc();
    let mut task = registered.task.clone();
    task.id = TaskId::new();
    task.tracker_id =
        Some(LinearIssueId::new(format!("issue-{}", WaveId::new())).expect("issue id"));
    task.identifier = identifier.to_string();
    task.title = format!("Sibling {identifier}");
    task.workspace_slug = branch.to_string();
    task.branch = branch.to_string();
    task.worktree = Some(worktree.to_path_buf());
    task.created_at = now;
    task.updated_at = now;
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        slug: branch.to_string(),
        branch: branch.to_string(),
        created_at: now,
        updated_at: now,
        ..registered.pr.clone()
    };
    planning::seed_unplaced_task(&registered.database, &task);
    runtime
        .block_on(registered.store.place_task(
            &task.id,
            task.worktree.as_ref().unwrap(),
            &task.workspace_slug,
            &pr,
            None,
        ))
        .expect("create sibling Task");
    if worktree.is_dir() {
        runtime
            .block_on(registered.store.append_task_event(
                &task.id,
                &loopflow::work::task::TaskEventKind::CheckoutReady {
                    branch: branch.to_string(),
                    base_commit: task.base_commit.clone(),
                },
            ))
            .unwrap();
    }
    task
}

/// A fake `open` / `xdg-open` that records each invocation to `marker`, so a
/// test can count presentation attempts through the recorded boundary. Register
/// it under both `open` and `xdg-open` so the platform opener records on either
/// OS.
#[allow(dead_code)] // Shared helper compiled into multiple test crates.
pub fn counting_open_script(marker: &Path) -> String {
    format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", marker.display())
}

/// Count recorded presentation attempts written by `counting_open_script`.
#[allow(dead_code)] // Shared helper compiled into multiple test crates.
pub fn presentation_attempts(marker: &Path) -> usize {
    std::fs::read_to_string(marker)
        .map(|log| log.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or(0)
}

fn write_executable(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    std::fs::write(&path, content).expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod");
    }
}

/// Record a Graph the way `lf graph` leaves one: a Graph process, its GraphExecution row
/// and one step Process in `cwd`, both exited. Returns the Graph process's id, which
/// names the Graph.
#[allow(dead_code)] // Shared helper compiled into integration tests that record no Graph.
pub fn record_graph(home: &Path, cwd: &Path, graph: &str, label: &str, outcome: &str) -> String {
    use loopflow::graph::{ConcreteSkill, ConcreteStep, Skill};
    let db = rusqlite::Connection::open(home.join("loopflow.db")).expect("open test registry");
    let graph_process = loopflow::id::LfProcessId::new();
    let step = loopflow::id::LfProcessId::new();
    for (id, parent, argv) in [
        (graph_process.clone(), None, vec!["lf", "graph", graph]),
        (
            step.clone(),
            Some(graph_process.clone()),
            vec!["lf", "--batch", "skill", label],
        ),
    ] {
        db.execute(
            "INSERT INTO processes(id,trace_id,parent_lf_process_id,command,repo,cwd,started_at,completed_at,outcome)
             VALUES(?1,?2,?3,?4,?5,?5,1,2,?6)",
            rusqlite::params![
                id.as_str(),
                loopflow::id::TraceId::new().as_str(),
                parent.as_ref().map(|parent| parent.as_str().to_owned()),
                serde_json::to_string(&argv).expect("argv serializes"),
                cwd.to_string_lossy(),
                outcome
            ],
        )
        .expect("record Graph Process");
    }
    let compiled = loopflow::graph::compiled::compile(
        graph,
        &[ConcreteStep::Skill(ConcreteSkill {
            skill: Skill::named(label),
            id: None,
            interactive: false,
            interactive_name: None,
            returns: None,
            sources: Vec::new(),
        })],
    );
    // A Graph launched in a Task's checkout records that Task; fixture
    // checkouts nest no Git worktrees, so the deepest one holding `cwd` is it.
    db.execute(
        "INSERT INTO graph_processes(lf_process_id,graph,compiled,task_id) VALUES(?1,?2,?3,
            (SELECT task_id FROM task_checkouts
             WHERE ?4=rtrim(worktree,'/') OR instr(?4,rtrim(worktree,'/')||'/')=1
             ORDER BY length(worktree) DESC LIMIT 1))",
        rusqlite::params![
            graph_process.as_str(),
            graph,
            serde_json::to_string(&compiled).expect("compiled serializes"),
            cwd.to_string_lossy()
        ],
    )
    .expect("record GraphExecution");
    db.execute(
        "INSERT INTO graph_process_steps(graph_lf_process_id,lf_process_id,node,iterations) VALUES(?1,?2,0,'[[]]')",
        rusqlite::params![graph_process.as_str(), step.as_str()],
    )
    .expect("record Graph step");
    graph_process.to_string()
}

/// Every Graph in launch order: how it ended (`succeeded`, `interrupted`,
/// `failed`, or `None` while it runs or when nobody saw it end) and each
/// recorded step as `{graph, label, key, iterations}`.
#[allow(dead_code)] // Shared helper compiled into integration tests that read no Graph.
pub fn recorded_graphs(home: &Path) -> Vec<(Option<String>, Vec<serde_json::Value>)> {
    let db = rusqlite::Connection::open(home.join("loopflow.db")).expect("open test registry");
    let mut graph_processes = db
        .prepare(
            "SELECT lf_process_id,CASE WHEN ended_at IS NULL OR exit_code IS NULL THEN NULL
                 WHEN exit_code=0 THEN 'succeeded'
                 WHEN exit_code=130 AND error IS NULL THEN 'interrupted' ELSE 'failed' END,
                 graph,compiled FROM graph_processes ORDER BY rowid",
        )
        .expect("select Graph processes");
    let rows = graph_processes
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .expect("read Graph processes")
        .collect::<rusqlite::Result<Vec<(String, Option<String>, String, String)>>>()
        .expect("decode Graph processes");
    rows.into_iter()
        .map(|(graph_process, outcome, graph, compiled)| {
            let compiled: loopflow::graph::compiled::CompiledGraph =
                serde_json::from_str(&compiled).expect("a Graph compiled");
            fn label(nodes: &[loopflow::graph::compiled::GraphNode], key: u32) -> Option<String> {
                nodes.iter().find_map(|node| {
                    if node.key == key {
                        return Some(node.label.clone());
                    }
                    node.paths.iter().find_map(|path| label(&path.steps, key))
                })
            }
            let mut steps = db
                .prepare(
                    "SELECT node,iterations FROM graph_process_steps
                     WHERE graph_lf_process_id=?1 ORDER BY seq",
                )
                .expect("select Graph steps");
            let steps = steps
                .query_map([graph_process], |row| {
                    Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
                })
                .expect("read Graph steps")
                .map(|row| {
                    let (key, iterations) = row.expect("a Graph step");
                    serde_json::json!({
                        "graph": graph,
                        "label": label(&compiled.steps, key).expect("a compiled node"),
                        "key": key,
                        "iterations": serde_json::from_str::<serde_json::Value>(&iterations)
                            .expect("iteration counts"),
                    })
                })
                .collect();
            (outcome, steps)
        })
        .collect()
}

pub mod human_session;

pub mod planning_tasks;

pub mod task_graph;

/// Prepare a Task-bound conversation with its first input for a Managed resume.
#[allow(dead_code)] // Shared support is compiled by every application test binary.
pub fn prepare_native_conversation(
    database: &std::path::Path,
    cwd: &std::path::Path,
    agent: &str,
    skill: Option<&str>,
    request: &str,
) -> String {
    let store = crate::store::open_sqlite(database).unwrap();
    let agent = crate::config::Agent::parse(agent).unwrap();
    let mut session = loopflow_storage::test_fixture::unpublished_conversation(
        None,
        None,
        loopflow_storage::rows::now_unix(),
    );
    session.artifact_key = None;
    session.cwd = cwd.into();
    session.provider = Some(agent.harness.as_str().into());
    session.model = agent.model;
    session.skill = skill.map(str::to_owned);
    session.request = Some(request.into());
    session.interactive = true;
    let session = crate::store::sessions::create_session(&store, session, None).unwrap();
    let task = store.task_checkout_owner(cwd).unwrap().unwrap_or_else(|| {
        store
            .create_task(&loopflow_storage::work::task::NewTask {
                parent_task_id: None,
                id: loopflow_model::durable::TaskId::new(),
                repository_id: crate::repository::location::ensure_repository(
                    &store,
                    &crate::repository::discover(cwd).unwrap().to_string(),
                )
                .unwrap(),
                project_id: None,
                title: "Native conversation".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap()
            .id
    });
    store
        .bind_session_prepared(
            &session.id,
            session.captured,
            &task,
            &session.cwd,
            &loopflow_storage::sqlite::SessionCaller {
                agent: None,
                capture_key: None,
            },
        )
        .unwrap()
        .id
        .to_string()
}
