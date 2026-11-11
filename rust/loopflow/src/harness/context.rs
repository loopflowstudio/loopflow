//! Stable provider hook declarations and separately saved launch context. No user files
//! or host hook trust decisions are replaced; Claude folder trust is granted only by
//! `claude_trust`, for lf-created worktrees. Saved sources survive driver replacement.
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::context_block::ContextDelivery;
use crate::prompt::write_prompt_log;

/// `turn_hooks` declares the turn and tool boundaries; `inbox` lets the
/// Machine server wake the conversation, which only a managed launch's
/// registered Session needs.
pub(crate) fn claude_args(
    context: &ContextDelivery,
    turn_hooks: bool,
    inbox: bool,
) -> Result<Vec<String>> {
    let mut hooks = context.hook_settings(turn_hooks)?;
    if turn_hooks {
        hooks["PostToolUseFailure"] = hooks["PostToolUse"].clone();
    }
    let mut settings = json!({"hooks": hooks});
    if inbox {
        // The Machine server wakes an idle conversation with steers and Task
        // updates through Claude's cross-session inbox. lf runs in no
        // permission mode it could attest, and a session that bypasses
        // prompts holds such a message for review unless it accepts peers.
        settings["crossSessionInbound"] = json!("accept");
    }
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
    let spec = json!({"executable":crate::os_process::resolve_lf_binary()?, "delivery":context, "instructions":instructions, "managed":managed});
    let delivery = write_prompt_log(
        &context.repo,
        &serde_json::to_string(context)?,
        "context-delivery",
        None,
    )?;
    let source = format!(
        "const spec = {};\nconst delivery = {};\n{}",
        spec,
        serde_json::to_string(&delivery)?,
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

/// A native terminal launch is a conversation an owner holds: it gets every
/// hook, whatever named its Work. Only a managed one accepts wakes.
pub(crate) fn native_args(
    harness: &str,
    context: Option<&ContextDelivery>,
    managed: bool,
) -> Result<Vec<String>> {
    match (harness, context) {
        ("claude", Some(context)) => claude_args(context, true, managed),
        ("pi", context) => pi_args(context, true, true),
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
        "const spec = {};\n{}",
        json!({"native":native, "managed":managed, "executable":crate::os_process::resolve_lf_binary()?,
            "log":home.join("logs").join("pi-extension.log")}),
        include_str!("context_pi.mjs")
    );
    let extension = save_hook(&home, "pi", "mjs", source.as_bytes())?;
    Ok(vec!["--extension".into(), extension.display().to_string()])
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
            super::claude_args(&contexts[0], true, true).unwrap(),
            super::claude_args(&contexts[1], true, true).unwrap()
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

    /// Only a managed launch's conversation is registered for waking, so only
    /// it accepts messages from other processes without review.
    #[test]
    fn only_managed_claude_launches_accept_inbox_messages() {
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
        let settings = |managed: bool| -> serde_json::Value {
            let args = super::native_args("claude", Some(&context), managed).unwrap();
            assert_eq!(args[0], "--settings");
            serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap()
        };
        let bare = settings(false);
        assert!(
            bare["hooks"]["Stop"].is_array(),
            "bare keeps its turn hooks"
        );
        assert!(bare.get("crossSessionInbound").is_none());
        assert_eq!(settings(true)["crossSessionInbound"], "accept");
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
                "/<session-flags>/config.toml:session_start:2:0",
                "/<session-flags>/config.toml:stop:0:0",
                "/<session-flags>/config.toml:user_prompt_submit:0:0",
            ]
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

    fn run_probe(directory: &std::path::Path, source: &str) -> String {
        let probe = directory.join("probe.mjs");
        std::fs::write(&probe, source).unwrap();
        let output = std::process::Command::new("node")
            .arg(&probe)
            .env("LF_CONTEXT_DELIVERY", directory.join("delivery.json"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::read_to_string(directory.join("calls")).unwrap_or_default()
    }

    /// One conversation through the OpenCode plugin: a prompt, its context
    /// request, then the turn's end.
    fn opencode_calls(managed: bool) -> String {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let repo = tempfile::tempdir().unwrap();
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
const request = {{ sessionID: 'native', system: [] }};
await hooks.context(request);
assert.deepEqual(request.system.map((part) => part.text), ['Fixed instructions', 'BLOCK']);
end();
await new Promise((resolve) => setTimeout(resolve, 200));
await close();
"#,
                plugin = serde_json::to_string(&format!("{plugin}/index.js")).unwrap()
            ),
        )
    }

    #[test]
    fn a_bare_opencode_run_serves_context_and_reports_no_turn_end() {
        let managed = opencode_calls(true);
        assert!(managed.contains("--moment start") && managed.contains("--moment stop"));
        let bare = opencode_calls(false);
        assert!(bare.contains("--moment start"), "{bare}");
        assert_eq!(bare.lines().count(), 1, "{bare}");
    }

    /// One headless Pi turn through the extension, with Pi's package stood in.
    fn pi_calls(managed: bool) -> String {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("LF_BIN", recording_lf(home.path()));
        let args = super::pi_args(None, false, managed).unwrap();
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
            "export const createBashToolDefinition = () => ({});
export const createLocalBashOperations = () => ({});
",
        )
        .unwrap();
        run_probe(
            home.path(),
            &format!(
                r#"import assert from 'node:assert/strict';
import extension from {extension};
const handlers = {{}};
const fire = async (name, event) => {{
  let result;
  for (const handler of handlers[name] ?? []) result = await handler(event, ctx);
  return result;
}};
const pi = {{
  on: (name, handler) => {{ (handlers[name] ??= []).push(handler); }},
  getSettings: () => ({{}}), getAllTools: () => [], getActiveTools: () => [],
  sendMessage: () => {{}},
}};
const ctx = {{ cwd: '/', sessionManager: {{ getSessionFile: () => '/pi/session.jsonl' }},
  abort() {{}}, shutdown() {{}} }};
extension(pi);
await fire('session_start', {{ type: 'session_start' }});
await fire('before_agent_start', {{}});
const request = await fire('context_with_system', {{ messages: [{{ role: 'system', sections: {{}} }}] }});
assert.equal(request.messages[0].sections.loopflow, 'BLOCK');
await fire('agent_start', {{ type: 'agent_start' }});
await fire('tool_execution_start', {{ type: 'tool_execution_start', toolCallId: 'tool' }});
await fire('tool_execution_end', {{ type: 'tool_execution_end', toolCallId: 'tool' }});
await new Promise((resolve) => setTimeout(resolve, 1200));
await fire('agent_settled', {{ type: 'agent_settled' }});
await fire('session_shutdown', {{}});
"#,
                extension = serde_json::to_string(&args[1]).unwrap()
            ),
        )
    }

    #[test]
    fn a_bare_pi_run_serves_context_without_steer_polls_or_a_turn_end() {
        let managed = pi_calls(true);
        for moment in ["start", "tool", "stop"] {
            assert!(
                managed.contains(&format!("--moment {moment}\n")),
                "{moment}: {managed}"
            );
        }
        let bare = pi_calls(false);
        assert_eq!(bare, "__context-block --moment start\n");
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
