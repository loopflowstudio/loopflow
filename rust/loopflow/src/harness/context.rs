//! Stable provider hook declarations and separately saved launch context. No user files
//! or host hook trust decisions are replaced; Claude folder trust is granted only by
//! `claude_trust`, for lf-created worktrees. Saved sources survive driver replacement.
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::context_block::ContextDelivery;
use crate::prompt::write_prompt_log;

pub(crate) fn claude_args(context: &ContextDelivery, managed: bool) -> Result<Vec<String>> {
    let mut hooks = context.hook_settings(managed)?;
    if managed {
        hooks["PostToolUseFailure"] = hooks["PostToolUse"].clone();
    }
    let settings = json!({"hooks": hooks});
    let bytes = serde_json::to_vec(&settings)?;
    let path = save_hook(&context.home, "claude", "json", &bytes)?;
    Ok(vec!["--settings".into(), path.display().to_string()])
}

/// The app-server's session-flags `hooks`: a wrapper's declarations, then lf's,
/// each of lf's trusted so Codex runs it without review. Wrapper declarations keep
/// their indices so their trust IDs survive; host entries and persisted decisions
/// stay as found. Hash protocol: codex-rs/hooks/src/engine/discovery.rs (0.164.0).
pub(crate) fn codex_hooks(
    context: &ContextDelivery,
    session_hooks: Option<&Value>,
    managed: bool,
) -> Result<Value> {
    let mut hooks = session_hooks.cloned().unwrap_or_else(|| json!({}));
    let additions = context.hook_settings(managed)?;
    let mut states = hooks
        .get("state")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (event, event_name) in [
        ("SessionStart", "session_start"),
        ("Stop", "stop"),
        ("PostToolUse", "post_tool_use"),
        ("UserPromptSubmit", "user_prompt_submit"),
    ] {
        let Some(groups) = additions[event].as_array() else {
            continue;
        };
        let mut combined = hooks
            .get(event)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for group in groups {
            // Codex trusts each handler on its own, with its defaults filled in
            // (verified with `hooks/list` on 0.166.0).
            let handlers = group["hooks"].as_array().into_iter().flatten();
            for (index, handler) in handlers.enumerate() {
                let mut handler = handler.clone();
                for (field, default) in [("async", json!(false)), ("timeout", json!(600))] {
                    if handler.get(field).is_none() {
                        handler[field] = default;
                    }
                }
                let mut identity = json!({"event_name":event_name, "hooks":[handler]});
                if let Some(matcher) = group.get("matcher") {
                    identity["matcher"] = matcher.clone();
                }
                identity.sort_all_objects();
                let hash = hex::encode(Sha256::digest(serde_json::to_vec(&identity)?));
                states.insert(
                    format!(
                        "/<session-flags>/config.toml:{event_name}:{}:{index}",
                        combined.len()
                    ),
                    json!({"trusted_hash":format!("sha256:{hash}")}),
                );
            }
            combined.push(group.clone());
        }
        hooks[event] = json!(combined);
    }
    hooks["state"] = Value::Object(states);
    Ok(hooks)
}

pub(crate) fn add_opencode_context(
    config: &mut Value,
    context: &ContextDelivery,
    instructions: &str,
    managed: bool,
) -> Result<()> {
    let spec = json!({"executable":crate::os_process::resolve_lf_binary()?, "delivery":context, "instructions":instructions, "managed":managed,
        "server":server_spec("lf opencode plugin")});
    let delivery = write_prompt_log(
        &context.repo,
        &serde_json::to_string(context)?,
        "context-delivery",
        None,
    )?;
    let source = format!(
        "const spec = {};\nconst delivery = {};\n{}\n{}",
        spec,
        serde_json::to_string(&delivery)?,
        include_str!("server_connection.js"),
        include_str!("context_opencode.js")
    );
    let path = write_prompt_log(&context.repo, &source, "opencode-plugin", None)?;
    let plugin = path.with_extension("plugin");
    std::fs::create_dir(&plugin)?;
    std::fs::rename(path, plugin.join("index.js"))?;
    std::fs::write(
        plugin.join("package.json"),
        r#"{"name":"loopflow-context","type":"module","exports":{"./server":"./index.js"}}"#,
    )?;
    config
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("OpenCode configuration must be an object"))?
        .entry("plugins")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("OpenCode plugin config must be an array"))?
        .push(json!(format!("file://{}", plugin.display())));
    Ok(())
}

/// A native terminal launch an owner holds gets every hook, whatever named its
/// Work: they say when it can take a reply. A bare one in the terminal's
/// foreground gets only the context hooks.
pub(crate) fn native_args(harness: &str, context: Option<&ContextDelivery>) -> Result<Vec<String>> {
    let held = crate::transport::current().is_some();
    match (harness, context) {
        ("claude", Some(context)) => claude_args(context, held),
        ("pi", context) => pi_args(context, true, held),
        _ => Ok(Vec::new()),
    }
}

pub(crate) fn pi_args(
    context: Option<&ContextDelivery>,
    native: bool,
    managed: bool,
) -> Result<Vec<String>> {
    let home = context
        .map(|c| c.home.clone())
        .unwrap_or_else(crate::store::lf_home_dir);
    let source = format!(
        "const spec = {};\n{}\n{}",
        json!({"native":native, "managed":managed, "executable":crate::os_process::resolve_lf_binary()?,
            "log":home.join("logs").join("pi-extension.log"), "server":server_spec("lf pi extension"),
            "bashGuard":pi_bash_guard()}),
        include_str!("context_pi.mjs"),
        include_str!("server_connection.js")
    );
    let extension = save_hook(&home, "pi", "mjs", source.as_bytes())?;
    Ok(vec!["--extension".into(), extension.display().to_string()])
}

/// Pi's bash guard, which the extension runs with `/bin/sh -c` ahead of each
/// bash command, passing the command's FIFO and Pi's PID. When the shell leads
/// a detached group whose leader Pi started, it hands the group to the watchdog
/// that ends it with Pi. Any other group starts nothing, and the command runs
/// either way. No `lf` process starts.
fn pi_bash_guard() -> String {
    let watchdog = crate::os_process::WATCHDOG
        .to_str()
        .expect("the watchdog script is ASCII");
    // No backslashes: fish keeps them special inside single quotes. Opening
    // the FIFO read-write first keeps the read-only open from waiting: a
    // provider that already closed its writer leaves EOF, so the watchdog ends
    // the group at once. The watchdog holds the reader before this script
    // exits, and its output is not the tool's, or Pi would wait on it.
    format!(
        r#"read -r group _ <<EOF
$(ps -o pgid=,ppid= -p $$ 2>/dev/null)
EOF
read -r _ parent <<EOF
$(ps -o pgid=,ppid= -p "${{group:-0}}" 2>/dev/null)
EOF
read -r provider_group _ <<EOF
$(ps -o pgid=,ppid= -p "$2" 2>/dev/null)
EOF
[ "$parent" = "$2" ] && [ "$group" != "$provider_group" ] && [ -p "$1" ] || exit 0
exec 4<>"$1" 3<"$1" 4>&-
set -- "$group"
(
{watchdog}
) <&3 >/dev/null 2>&1 3<&- &"#
    )
}

/// What a plugin needs to open its `machine.sock` connection: the `hello` a
/// hook sends, under the plugin's own name. Null when the socket has no
/// address, so every boundary is a recording gap rather than a failed launch.
fn server_spec(name: &str) -> Value {
    match crate::server::socket_path() {
        Ok(socket) => json!({"socket":socket, "revision":crate::server::wire::REVISION,
            "min_revision":crate::server::wire::MIN_REVISION, "platform":std::env::consts::OS,
            "name":name, "version":crate::build_info::BUILD_VERSION}),
        Err(error) => {
            tracing::warn!(error = %format!("{error:#}"), "plugin starts without a server address");
            Value::Null
        }
    }
}

fn save_hook(home: &Path, provider: &str, extension: &str, bytes: &[u8]) -> Result<PathBuf> {
    let directory = home.join("hook-settings");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!(
        "{provider}-{}.{extension}",
        hex::encode(Sha256::digest(bytes))
    ));
    if !path.exists() {
        let file = tempfile::NamedTempFile::new_in(&directory)?;
        std::fs::write(file.path(), bytes)?;
        file.persist(&path)?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::add_opencode_context;
    use crate::context_block::ContextDelivery;
    use sha2::{Digest, Sha256};

    #[test]
    fn hook_identity_is_shared_across_tasks_and_sessions() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let repos = [tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap()];
        let contexts: Vec<_> = repos
            .iter()
            .enumerate()
            .map(|(index, repo)| ContextDelivery {
                repository: None,
                repo: repo.path().to_owned(),
                home: home.path().to_owned(),
                wave: Some(format!("wave-{index}")),
                skill_file: Some(repo.path().join("saved-skill.md")),
                submitted_docs: None,
                references: vec![repo.path().join("task-reference.md")],
            })
            .collect();
        let first = super::codex_hooks(&contexts[0], None, true).unwrap();
        assert_eq!(first, super::codex_hooks(&contexts[1], None, true).unwrap());
        assert_eq!(
            super::claude_args(&contexts[0], true).unwrap(),
            super::claude_args(&contexts[1], true).unwrap()
        );
        let mut saved = Vec::new();
        for context in &contexts {
            let mut command = std::process::Command::new("provider");
            context.configure_hooks(&mut command).unwrap();
            let path = command
                .get_envs()
                .find(|(key, _)| *key == "LF_CONTEXT_DELIVERY")
                .unwrap()
                .1
                .unwrap();
            let restored: ContextDelivery =
                serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(&restored, context);
            saved.push(path.to_owned());
        }
        assert_ne!(saved[0], saved[1]);
        // Later launches never overwrite a retained native resume's sources.
        let restored: ContextDelivery =
            serde_json::from_slice(&std::fs::read(&saved[0]).unwrap()).unwrap();
        assert_eq!(restored, contexts[0]);
    }

    #[test]
    fn codex_trusts_only_loopflow_declarations_after_host_hooks() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let context = ContextDelivery {
            repository: None,
            repo: home.path().to_owned(),
            home: home.path().to_owned(),
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
        };
        let host = serde_json::json!({
            "SessionStart": [{"hooks": [{"type": "command", "command": "host-hook"}]}],
            "state": {
                "host-trust": {"trusted_hash": "unchanged"},
                "/<session-flags>/config.toml:session_start:0:0": {"trusted_hash": "host"},
            },
        });
        let hooks = &super::codex_hooks(&context, Some(&host), true).unwrap();
        assert_eq!(hooks["SessionStart"][0]["hooks"][0]["command"], "host-hook");
        let state = hooks["state"].as_object().unwrap();
        assert_eq!(state["host-trust"]["trusted_hash"], "unchanged");
        assert_eq!(
            state["/<session-flags>/config.toml:session_start:0:0"]["trusted_hash"],
            "host"
        );
        let mut trusted: Vec<_> = state
            .keys()
            .filter(|key| *key != "host-trust")
            .map(String::as_str)
            .collect();
        trusted.sort_unstable();
        assert_eq!(
            trusted,
            [
                "/<session-flags>/config.toml:post_tool_use:0:0",
                "/<session-flags>/config.toml:session_start:0:0",
                "/<session-flags>/config.toml:session_start:1:0",
                "/<session-flags>/config.toml:session_start:1:1",
                "/<session-flags>/config.toml:session_start:2:0",
                "/<session-flags>/config.toml:stop:0:0",
                "/<session-flags>/config.toml:stop:0:1",
                "/<session-flags>/config.toml:user_prompt_submit:0:0",
            ]
        );
        // Codex hashes a handler with its defaults filled in (codex-cli 0.166.0
        // `hooks/list`), so an async handler without a timeout is trusted too.
        let handler = &hooks["Stop"][0]["hooks"][1];
        let mut identity = serde_json::json!({"event_name":"stop", "hooks":[{
            "type":"command", "command":handler["command"], "async":true, "timeout":600}]});
        identity.sort_all_objects();
        let hash = hex::encode(Sha256::digest(serde_json::to_vec(&identity).unwrap()));
        assert_eq!(
            state["/<session-flags>/config.toml:stop:0:1"]["trusted_hash"],
            format!("sha256:{hash}")
        );
        // Each Loopflow entry trusts the exact declaration at its own index.
        assert_ne!(
            state["/<session-flags>/config.toml:session_start:1:0"],
            state["/<session-flags>/config.toml:session_start:2:0"]
        );
    }

    /// A stand-in `lf` that logs its arguments and answers a context block.
    fn recording_lf(directory: &std::path::Path) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let lf = directory.join("lf");
        std::fs::write(
            &lf,
            format!(
                "#!/bin/sh\necho \"$*\" >> '{}'\ncat > /dev/null\necho '{{\"hookSpecificOutput\":{{\"additionalContext\":\"BLOCK\"}}}}'\n",
                directory.join("calls").display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&lf, std::fs::Permissions::from_mode(0o755)).unwrap();
        lf
    }

    /// A stand-in Machine server on the plugin's socket: it welcomes every
    /// `hello` (a frozen one never does), answers the first `answer_steers`
    /// boundary with one batch, and keeps each hello and request in
    /// `server.json`.
    const STAND_IN_SERVER: &str = r#"import { createServer } from 'node:net';
import { writeFileSync } from 'node:fs';
const seen = { hellos: [], requests: [] };
let answered = false;
const keep = () => writeFileSync(SEEN, JSON.stringify(seen));
const server = createServer((socket) => {
  let buffer = '';
  socket.setEncoding('utf8');
  socket.on('data', (chunk) => {
    buffer += chunk;
    for (let end = buffer.indexOf('\n'); end >= 0; end = buffer.indexOf('\n')) {
      const message = JSON.parse(buffer.slice(0, end));
      buffer = buffer.slice(end + 1);
      if (message.type === 'hello') {
        seen.hellos.push(message);
        keep();
        if (FROZEN) continue;
        socket.write(JSON.stringify({ type: 'welcome' }) + '\n');
        continue;
      }
      const operation = message.operation;
      seen.requests.push(operation);
      keep();
      const steers = !answered && (ANSWER_STEERS)(operation)
        ? { batch: 'batch-1', text: 'STEER ' } : null;
      answered ||= steers !== null;
      socket.write(JSON.stringify({ type: 'response', id: message.id, trace_id: 't',
        result: { steers, waiting: false } }) + '\n');
    }
  });
});
await new Promise((resolve) => server.listen(SOCKET, resolve));
"#;

    struct Probe {
        /// Each `lf` the plugin started, one line of arguments per process.
        calls: String,
        /// What the stand-in server saw: `hellos` and `requests`.
        server: serde_json::Value,
    }

    /// Run `source` under Node beside a stand-in server. `answer_steers` is a
    /// JavaScript predicate over a request's operation.
    fn run_probe(
        directory: &std::path::Path,
        managed: bool,
        frozen: bool,
        answer_steers: &str,
        source: &str,
    ) -> Probe {
        let socket = crate::server::socket_path().unwrap();
        let seen = directory.join("server.json");
        let server = STAND_IN_SERVER
            .replace("SEEN", &serde_json::to_string(&seen).unwrap())
            .replace("SOCKET", &serde_json::to_string(&socket).unwrap())
            .replace("ANSWER_STEERS", answer_steers)
            .replace("FROZEN", &frozen.to_string());
        let probe = directory.join("probe.mjs");
        std::fs::write(&probe, format!("{server}\n{source}\nprocess.exit(0);\n")).unwrap();
        let mut node = std::process::Command::new("node");
        node.arg(&probe)
            .env("LF_CONTEXT_DELIVERY", directory.join("delivery.json"))
            .env_remove(crate::steer::PROVIDER_CONTEXT_ENV);
        if managed {
            node.env(
                crate::steer::PROVIDER_CONTEXT_ENV,
                r#"{"session":"session","agent_process_id":"process","capability":"capability"}"#,
            );
        }
        let output = node.output().unwrap();
        let _ = std::fs::remove_file(&socket);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Probe {
            calls: std::fs::read_to_string(directory.join("calls")).unwrap_or_default(),
            server: std::fs::read(&seen)
                .map(|bytes| serde_json::from_slice(&bytes).unwrap())
                .unwrap_or_else(|_| serde_json::json!({"hellos":[],"requests":[]})),
        }
    }

    fn moments(probe: &Probe) -> Vec<&str> {
        probe.server["requests"]
            .as_array()
            .unwrap()
            .iter()
            .map(|request| request["moment"].as_str().unwrap())
            .collect()
    }

    /// One conversation through the OpenCode plugin: a prompt, three model
    /// requests with a compaction before the third, then the turn's end.
    fn opencode_turn(managed: bool) -> Probe {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir(repo.path().join("machine")).unwrap();
        std::env::set_var("LF_HOME", repo.path().join("machine"));
        std::env::set_var("LF_BIN", recording_lf(repo.path()));
        let context = ContextDelivery {
            repository: None,
            repo: repo.path().to_owned(),
            home: repo.path().join("machine"),
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
        };
        let mut config = serde_json::json!({});
        add_opencode_context(&mut config, &context, "Fixed instructions", managed).unwrap();
        let plugin = config["plugins"][0].as_str().unwrap();
        run_probe(
            repo.path(),
            managed,
            false,
            "(operation) => operation.moment === 'tool'",
            &format!(
                r#"import assert from 'node:assert/strict';
import plugin from {plugin};
const hooks = {{}};
let end;
const ended = new Promise((resolve) => {{ end = resolve; }});
const close = await plugin.setup({{
  session: {{ hook: async (name, callback) => {{ hooks[name] = callback; }} }},
  event: {{ subscribe: async function* () {{
    await ended;
    yield {{ type: 'session.execution.succeeded', data: {{ sessionID: 'native' }} }};
  }} }},
}});
await hooks.prompt({{ sessionID: 'native' }});
const texts = [];
for (const compact of [false, false, true]) {{
  if (compact) await hooks.compaction({{ sessionID: 'native' }});
  const request = {{ sessionID: 'native', system: [] }};
  await hooks.context(request);
  texts.push(request.system.map((part) => part.text));
}}
const steered = {steered};
assert.deepEqual(texts[0], ['Fixed instructions', 'BLOCK']);
assert.deepEqual(texts[1], ['Fixed instructions', steered ? 'STEER BLOCK' : 'BLOCK']);
end();
await new Promise((resolve) => setTimeout(resolve, 200));
await close();
"#,
                plugin = serde_json::to_string(&format!("{plugin}/index.js")).unwrap(),
                steered = managed,
            ),
        )
    }

    #[test]
    fn a_managed_opencode_turn_asks_the_server_and_renders_only_at_start_and_compaction() {
        let managed = opencode_turn(true);
        let renders: Vec<_> = managed
            .calls
            .lines()
            .map(|call| call.rsplit(' ').next().unwrap())
            .collect();
        assert_eq!(renders, ["start", "compact"], "{}", managed.calls);
        assert_eq!(moments(&managed), ["start", "tool", "compact", "stop"]);
        // One connection per boundary, each opened with a hook's hello.
        let hellos = managed.server["hellos"].as_array().unwrap();
        assert_eq!(hellos.len(), 4, "{}", managed.server);
        assert!(hellos
            .iter()
            .all(|hello| hello["client"]["kind"] == "plugin"));
        // The batch the second request carried is named by the next one.
        let requests = &managed.server["requests"];
        assert_eq!(requests[1]["printed"], serde_json::json!([]));
        assert_eq!(requests[2]["printed"], serde_json::json!(["batch-1"]));
        assert_eq!(requests[1]["adapter"]["session"], "session");
    }

    #[test]
    fn a_bare_opencode_run_serves_context_and_opens_no_connection() {
        let bare = opencode_turn(false);
        assert_eq!(bare.calls.lines().count(), 2, "{}", bare.calls);
        assert!(bare.calls.contains("--moment start") && bare.calls.contains("--moment compact"));
        assert!(moments(&bare).is_empty(), "{}", bare.server);
        assert!(bare.server["hellos"].as_array().unwrap().is_empty());
    }

    /// One headless Pi turn with two tool calls through the extension, with
    /// Pi's package stood in.
    fn pi_turn(native: bool, managed: bool, frozen: bool) -> Probe {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_BIN", recording_lf(home.path()));
        let args = super::pi_args(None, native, managed).unwrap();
        let package = home
            .path()
            .join("hook-settings/node_modules/@earendil-works/pi-coding-agent");
        std::fs::create_dir_all(&package).unwrap();
        std::fs::write(
            package.join("package.json"),
            r#"{"name":"@earendil-works/pi-coding-agent","type":"module","exports":"./index.js"}"#,
        )
        .unwrap();
        std::fs::write(
            package.join("index.js"),
            "import { execFileSync } from 'node:child_process';
export const createBashToolDefinition = () => ({});
export const createLocalBashOperations = () => ({
  exec: async (command) => {
    execFileSync('/bin/sh', ['-c', command]);
    return { exitCode: 0 };
  },
});
",
        )
        .unwrap();
        run_probe(
            home.path(),
            managed,
            frozen,
            "(operation) => operation.moment !== 'start' && operation.callback.type === 'tool_execution_start'
  || operation.callback.hook_event_name === 'PiSteer'",
            &format!(
                r#"import assert from 'node:assert/strict';
import {{ readFileSync }} from 'node:fs';
import extension from {extension};
const handlers = {{}};
const steers = [];
const fire = async (name, event) => {{
  let result;
  for (const handler of handlers[name] ?? []) result = await handler(event, ctx);
  return result;
}};
const pi = {{
  on: (name, handler) => {{ (handlers[name] ??= []).push(handler); }},
  getSettings: () => ({{}}), getAllTools: () => [], getActiveTools: () => [],
  sendMessage: (message, options) => {{ steers.push([message.content, options.deliverAs]); }},
}};
const ctx = {{ cwd: '/', sessionManager: {{ getSessionFile: () => '/pi/session.jsonl' }},
  abort() {{}}, shutdown() {{}} }};
extension(pi);
await fire('session_start', {{ type: 'session_start' }});
const {{ operations }} = await fire('user_bash', {{}});
await operations.exec('printf ran > ' + {ran}, '/', {{}});
assert.equal(readFileSync({ran}, 'utf8'), 'ran');
await fire('before_agent_start', {{}});
const request = await fire('context_with_system', {{ messages: [{{ role: 'system', sections: {{}} }}] }});
assert.equal(request.messages[0].sections.loopflow, 'BLOCK');
await fire('agent_start', {{ type: 'agent_start' }});
for (const tool of ['one', 'two']) {{
  await fire('tool_execution_start', {{ type: 'tool_execution_start', toolCallId: tool }});
  await fire('tool_execution_end', {{ type: 'tool_execution_end', toolCallId: tool }});
}}
await fire('agent_settled', {{ type: 'agent_settled' }});
await fire('session_shutdown', {{}});
assert.deepEqual(steers, {steers});
"#,
                extension = serde_json::to_string(&args[1]).unwrap(),
                steers = if managed && !frozen { r#"[["STEER ", "steer"]]"# } else { "[]" },
                ran = serde_json::to_string(&home.path().join("ran").display().to_string()).unwrap(),
            ),
        )
    }

    #[test]
    fn a_managed_native_pi_turn_reports_events_and_takes_steers_without_starting_lf() {
        let native = pi_turn(true, true, false);
        // One render at start, beside the conversation's Task-update watcher;
        // no lf per event, tool, steer check or bash command.
        let calls: Vec<_> = native
            .calls
            .lines()
            .filter(|call| !call.ends_with("task-updates"))
            .collect();
        assert_eq!(
            calls,
            ["__context-block --moment start"],
            "{}",
            native.calls
        );
        assert_eq!(
            moments(&native),
            ["event", "start", "event", "event", "event", "event", "event", "event", "stop"]
        );
        let requests = native.server["requests"].as_array().unwrap();
        let types: Vec<_> = requests
            .iter()
            .filter(|request| request["moment"] == "event")
            .map(|request| request["callback"]["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            [
                "session_start",
                "agent_start",
                "tool_execution_start",
                "tool_execution_end",
                "tool_execution_start",
                "tool_execution_end",
                "agent_settled"
            ]
        );
        // The steer the first tool start answered is confirmed by the next event.
        assert_eq!(requests[4]["printed"], serde_json::json!(["batch-1"]));
        assert_eq!(requests[2]["callback"]["sessionFile"], "/pi/session.jsonl");
    }

    #[test]
    fn a_driven_pi_turn_asks_for_steers_and_a_bare_one_opens_no_connection() {
        let driven = pi_turn(false, true, false);
        assert_eq!(driven.calls, "__context-block --moment start\n");
        // Pi's RPC driver records the events; the extension asks only for steers.
        assert_eq!(
            moments(&driven),
            ["start", "tool", "tool", "tool", "tool", "tool", "tool", "stop"]
        );
        let bare = pi_turn(false, false, false);
        assert_eq!(bare.calls, "__context-block --moment start\n");
        assert!(moments(&bare).is_empty(), "{}", bare.server);
    }

    #[test]
    fn a_native_pi_turn_against_a_frozen_server_waits_only_where_a_steer_can_land() {
        let frozen = pi_turn(true, true, true);
        // Nine boundaries reach a live server. With no answer, only session
        // start, the start render, each tool start, the turn end and `stop`
        // wait their two seconds; the rest fall into the open gap.
        assert_eq!(frozen.server["hellos"].as_array().unwrap().len(), 6);
        assert!(moments(&frozen).is_empty(), "{}", frozen.server);
    }

    /// Run `guard; <rest>` as Pi's local bash does: a shell leading its own
    /// group, started by this process, with `provider` as Pi's PID.
    fn guarded_tool(fifo: &std::path::Path, provider: &str, rest: &str) -> std::process::Child {
        use std::os::unix::process::CommandExt;
        std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "/bin/sh -c \"$1\" lf-tool-lifeline \"$2\" {provider}\n{rest}"
            ))
            .arg("tool")
            .arg(super::pi_bash_guard())
            .arg(fifo)
            .stdout(std::process::Stdio::piped())
            .process_group(0)
            .spawn()
            .unwrap()
    }

    fn tool_fifo(directory: &std::path::Path) -> std::path::PathBuf {
        let fifo = directory.join("lifeline");
        assert!(std::process::Command::new("/usr/bin/mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success());
        fifo
    }

    #[test]
    fn pi_bash_guard_ends_a_detached_tool_group_with_its_provider() {
        use std::io::Read;
        use std::os::unix::process::ExitStatusExt;
        let directory = tempfile::tempdir().unwrap();
        let fifo = tool_fifo(directory.path());
        // Pi holds the writer, read-write so opening it never waits.
        let writer = std::fs::File::options()
            .read(true)
            .write(true)
            .open(&fifo)
            .unwrap();
        let mut tool = guarded_tool(
            &fifo,
            &std::process::id().to_string(),
            "printf ran; exec sleep 30 >&-",
        );
        // The watchdog keeps none of the tool's output, so Pi sees it end.
        let mut output = String::new();
        tool.stdout
            .take()
            .unwrap()
            .read_to_string(&mut output)
            .unwrap();
        assert_eq!(output, "ran");
        drop(writer);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        let status = loop {
            if let Some(status) = tool.try_wait().unwrap() {
                break status;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the tool outlived its provider"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        };
        assert_eq!(status.signal(), Some(libc::SIGTERM));
    }

    #[test]
    fn pi_bash_guard_refuses_the_providers_own_group_and_the_command_runs() {
        let directory = tempfile::tempdir().unwrap();
        let fifo = tool_fifo(directory.path());
        // No writer: a watchdog would end the group at once.
        for provider in ["$$", "1"] {
            let tool = guarded_tool(&fifo, provider, "sleep 0.5; printf ran");
            let output = tool.wait_with_output().unwrap();
            assert!(output.status.success(), "{provider}: {:?}", output.status);
            assert_eq!(output.stdout, b"ran", "{provider}");
        }
    }

    #[test]
    fn pi_extension_names_lf_without_an_inherited_lf_bin() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let executable = std::env::current_exe().unwrap();
        std::env::set_var("LF_BIN", &executable);

        let args = super::pi_args(None, true, true).unwrap();

        // A fresh machine's Pi process inherits no LF_BIN; the source carries it.
        let source = std::fs::read_to_string(&args[1]).unwrap();
        assert!(source.contains(&serde_json::to_string(&executable).unwrap()));
        assert!(!source.contains("process.env.LF_BIN"));
    }

    #[test]
    fn opencode_context_preserves_settings_and_existing_plugins() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let repo = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", repo.path().join("machine"));
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        let context = ContextDelivery {
            repository: None,
            repo: repo.path().to_owned(),
            home: repo.path().join("machine"),
            wave: None,
            skill_file: None,
            submitted_docs: None,
            references: Vec::new(),
        };
        let original = serde_json::json!({
            "plugins": ["existing-plugin"],
            "permissions": [{"action":"edit","resource":"*","effect":"deny"}],
            "model": "fixture/model"
        });
        let mut config = original.clone();
        add_opencode_context(&mut config, &context, "Fixed instructions", true).unwrap();
        assert_eq!(config["permissions"], original["permissions"]);
        assert_eq!(config["model"], original["model"]);
        let plugins = config["plugins"].as_array().unwrap();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0], "existing-plugin");
        let path = plugins[1]
            .as_str()
            .unwrap()
            .strip_prefix("file://")
            .unwrap();
        assert!(
            std::fs::read_to_string(std::path::Path::new(path).join("index.js"))
                .unwrap()
                .contains("Fixed instructions")
        );
    }
}
