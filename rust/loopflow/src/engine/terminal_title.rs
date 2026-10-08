//! Session names projected into their attached terminal. Only the pre-spawn
//! write uses OSC: writing alongside a native provider can split escape frames.
//! cmux has a separate control channel for its workspace/tab names and renames.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};
use serde_json::{json, Value};

use crate::engine::process::wait_for_exit;
use crate::process::SessionDriver;
use crate::session::AgentSession;
use crate::store::{sqlite::SqliteStore, StoreResult};

fn display_title(name: &str, task: Option<&str>) -> String {
    let clean: String = name.chars().filter(|ch| !ch.is_control()).collect();
    let Some(task) = task else { return clean };
    let purpose = clean
        .strip_prefix(task)
        .filter(|suffix| suffix.is_empty() || suffix.starts_with(char::is_whitespace))
        .unwrap_or(&clean)
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    let purpose = if purpose.is_empty() {
        "session"
    } else {
        &purpose
    };
    format!("{task} {purpose}")
}

#[derive(Debug)]
pub(crate) struct TerminalTitle {
    store: SqliteStore,
    session: String,
    driver: Option<SessionDriver>,
    name: String,
    cmux: Option<(String, String)>,
    checked: Instant,
}

impl TerminalTitle {
    pub(crate) fn prepare(
        environment: &BTreeMap<String, String>,
        harness: &str,
        command: &mut Command,
    ) -> Option<Self> {
        if !std::io::stderr().is_terminal() {
            return None;
        }
        let input = environment.get(crate::session_record::CAPTURE_KEY_ENV)?;
        let mut title = match Self::load(input) {
            Ok(Some(title)) => title,
            Ok(None) => return None,
            Err(error) => {
                tracing::warn!(%error, "cannot read terminal Session name");
                return None;
            }
        };
        // Called before adding any subcommand, positional input or `--` boundary.
        match harness {
            "claude" => {
                command.args(["--name", &title.name]);
                command.env("CLAUDE_CODE_DISABLE_TERMINAL_TITLE", "1");
            }
            "codex" => {
                command.args(["-c", "tui.terminal_title=[]"]);
            }
            _ => {}
        }
        if let Err(error) = write!(std::io::stderr().lock(), "\x1b]0;{}\x07", title.name) {
            tracing::warn!(%error, "cannot set terminal Session name");
        }
        title.publish();
        Some(title)
    }

    fn load(input: &str) -> anyhow::Result<Option<Self>> {
        let store =
            SqliteStore::open_processes_read_only(&crate::store::database_path_from_env()?)?;
        let Some(session) = store.session_for_artifact(input)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            driver: store.session_driver(&session.id)?,
            name: session_title(&store, &session)?,
            store,
            session: session.id,
            cmux: std::env::var("CMUX_WORKSPACE_ID")
                .ok()
                .zip(std::env::var("CMUX_SURFACE_ID").ok())
                .filter(|(workspace, surface)| !workspace.is_empty() && !surface.is_empty()),
            checked: Instant::now(),
        }))
    }

    pub(crate) fn refresh(&mut self) {
        if self.cmux.is_none() || self.checked.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.checked = Instant::now();
        match self.read_name() {
            Ok(Some(name)) if name != self.name => {
                self.name = name;
                self.publish();
            }
            Ok(Some(_)) => {}
            // A transferred driver must stop changing the old terminal's names.
            Ok(None) => self.cmux = None,
            Err(error) => {
                tracing::warn!(%error, "cannot refresh terminal Session name");
                self.cmux = None;
            }
        }
    }

    fn read_name(&self) -> StoreResult<Option<String>> {
        if self.store.session_driver(&self.session)? != self.driver {
            return Ok(None);
        }
        self.store
            .session(&self.session)?
            .map(|session| session_title(&self.store, &session))
            .transpose()
    }

    fn publish(&mut self) {
        let Some((workspace, surface)) = &self.cmux else {
            return;
        };
        let result = rename_cmux(&[
            "rename-workspace",
            "--workspace",
            workspace,
            "--",
            &self.name,
        ])
        .and_then(|()| {
            rename_cmux(&[
                "rename-tab",
                "--workspace",
                workspace,
                "--surface",
                surface,
                "--",
                &self.name,
            ])
        });
        if let Err(error) = result {
            tracing::warn!(%error, "cmux title update failed; Session continues");
            self.cmux = None;
        }
    }
}

fn session_title(store: &SqliteStore, session: &AgentSession) -> StoreResult<String> {
    let task = session
        .task_id
        .as_ref()
        .map(|id| store.task(id))
        .transpose()?
        .flatten();
    Ok(display_title(
        &session.title,
        task.as_ref().map(|task| task.plan.identifier.as_str()),
    ))
}

fn rename_cmux(args: &[&str]) -> std::io::Result<()> {
    let mut child = Command::new("cmux")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let (status, timed_out) = wait_for_exit(&mut child, Some(Duration::from_secs(1)), || {})?;
    if timed_out {
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "cmux title update timed out",
        ))
    } else if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("cmux exited {status}")))
    }
}

/// Native providers own their names and terminal writes; hooks only supply the name.
pub(crate) fn name_native_session(provider: &str, input: &Value) -> anyhow::Result<Option<Value>> {
    let cwd = input["cwd"]
        .as_str()
        .ok_or_else(|| anyhow!("hook has no cwd"))?;
    if !Path::new(cwd)
        .ancestors()
        .any(|path| path.join(".lf").is_dir())
    {
        return Ok(None);
    }
    let event = input["hook_event_name"].as_str().unwrap_or_default();
    if event != "UserPromptSubmit" && !(event == "SessionStart" && input["source"] == "resume") {
        return Ok(None);
    }
    // lf's assembled context has its own attributed name and title transport.
    if input["prompt"].as_str() == Some(crate::engine::prompt::INITIAL_TURN_PROMPT) {
        return Ok(None);
    }
    if let Some(thread) = input["session_id"].as_str() {
        let database = crate::store::database_path_from_env()?;
        if database.exists()
            && !SqliteStore::open_processes_read_only(&database)?
                .sessions_for_provider_thread(thread)?
                .is_empty()
        {
            return Ok(None);
        }
    }
    match provider {
        "claude" => {
            if input["session_title"]
                .as_str()
                .is_some_and(|name| !name.is_empty())
            {
                return Ok(None);
            }
            let request = if event == "UserPromptSubmit" {
                input["prompt"].as_str().map(str::to_owned)
            } else {
                claude_first_request(input)?
            };
            Ok(request.as_deref().and_then(crate::session_record::request_title).map(|title| {
                json!({"hookSpecificOutput": {"hookEventName": event, "sessionTitle": title}})
            }))
        }
        "codex" => {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(3), name_codex_session(input))
                        .await
                        .context("native naming timed out")?
                })?;
            Ok(None)
        }
        _ => Err(anyhow!("unsupported title provider {provider}")),
    }
}

fn claude_first_request(input: &Value) -> anyhow::Result<Option<String>> {
    let Some(path) = input["transcript_path"].as_str() else {
        return Ok(None);
    };
    let file = fs::File::open(path)?;
    for line in std::io::BufReader::new(file).lines() {
        let entry: Value = serde_json::from_str(&line?)?;
        if entry["type"] != "user" || entry["isMeta"] == true {
            continue;
        }
        if let Some(request) = message_text(&entry["message"]["content"]) {
            return Ok(Some(request));
        }
    }
    Ok(None)
}

fn message_text(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_owned());
    }
    let text = content
        .as_array()?
        .iter()
        .filter(|item| item["type"] == "text")
        .filter_map(|item| item["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ");
    (!text.is_empty()).then_some(text)
}

async fn name_codex_session(input: &Value) -> anyhow::Result<()> {
    use crate::harness::codex_connection::read_rpc;

    let id = input["session_id"]
        .as_str()
        .ok_or_else(|| anyhow!("hook has no session_id"))?;
    let home = std::env::var_os("CODEX_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|home| home.join(".codex")))
        .ok_or_else(|| anyhow!("Codex home unavailable"))?;
    let socket =
        tokio::net::UnixStream::connect(home.join("app-server-control/app-server-control.sock"))
            .await
            .context("Codex shared app-server unavailable")?;
    let (mut connection, _) = tokio_tungstenite::client_async("ws://localhost", socket).await?;
    read_rpc(
        &mut connection,
        "initialize",
        json!({"clientInfo": {
            "name": "loopflow_titles", "version": env!("CARGO_PKG_VERSION")
        }}),
    )
    .await?;
    let detail = read_rpc(
        &mut connection,
        "thread/read",
        json!({
            "threadId": id, "includeTurns": input["hook_event_name"] == "SessionStart"
        }),
    )
    .await?;
    let thread = &detail["thread"];
    if thread["name"].as_str().is_some_and(|name| !name.is_empty()) {
        return Ok(());
    }
    let request = input["prompt"].as_str().map(str::to_owned).or_else(|| {
        thread["turns"]
            .as_array()?
            .iter()
            .filter_map(|turn| turn["items"].as_array())
            .flatten()
            .filter(|item| item["type"] == "userMessage")
            .find_map(|item| message_text(&item["content"]))
    });
    if let Some(title) = request
        .as_deref()
        .and_then(crate::session_record::request_title)
    {
        read_rpc(
            &mut connection,
            "thread/name/set",
            json!({"threadId": id, "name": title}),
        )
        .await?;
    }
    Ok(())
}

/// Publish naming hooks alongside installed native integrations, preserving other hooks.
pub(crate) fn install_native_hooks(home: &Path) -> anyhow::Result<()> {
    for (provider, relative) in [
        ("claude", ".claude/settings.json"),
        ("codex", ".codex/hooks.json"),
    ] {
        let path = home.join(relative);
        let path = if path.exists() {
            path.canonicalize()?
        } else {
            path
        };
        let mut document: Value = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => json!({}),
            Err(error) => return Err(error.into()),
        };
        let original = document.clone();
        let root = document
            .as_object_mut()
            .ok_or_else(|| anyhow!("{} must be an object", path.display()))?;
        let hooks = root
            .entry("hooks")
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow!("{} hooks must be an object", path.display()))?;
        let command = format!("lf __session-title {provider}");
        for event in ["SessionStart", "UserPromptSubmit"] {
            let entries = hooks
                .entry(event)
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .ok_or_else(|| anyhow!("{event} hooks must be an array"))?;
            if entries.iter().any(|entry| {
                (event != "SessionStart" || entry["matcher"] == "resume")
                    && entry["hooks"].as_array().is_some_and(|handlers| {
                        handlers.iter().any(|handler| handler["command"] == command)
                    })
            }) {
                continue;
            }
            let mut entry = json!({"hooks": [{"type":"command", "command":command, "timeout":5}]});
            if event == "SessionStart" {
                entry["matcher"] = json!("resume");
            }
            entries.push(entry);
        }
        if document == original {
            continue;
        }
        let parent = path.parent().expect("native hook file has a parent");
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        if path.exists() {
            temporary
                .as_file()
                .set_permissions(fs::metadata(&path)?.permissions())?;
        }
        serde_json::to_writer_pretty(&mut temporary, &document)?;
        writeln!(temporary)?;
        temporary.persist(&path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{display_title, install_native_hooks, name_native_session};
    use serde_json::json;

    #[test]
    fn native_claude_names_requests_and_unnamed_resumes_preserving_custom_names() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join(".lf")).unwrap();
        let mut input = json!({"cwd":root.path(), "hook_event_name":"UserPromptSubmit",
            "prompt":"Plan store migration for archived tasks"});
        let title = name_native_session("claude", &input).unwrap().unwrap();
        assert_eq!(
            title["hookSpecificOutput"]["sessionTitle"],
            "Plan store migration"
        );
        input["session_title"] = json!("Jack's hand name");
        assert!(name_native_session("claude", &input).unwrap().is_none());
        input.as_object_mut().unwrap().remove("session_title");
        input["prompt"] = json!(crate::engine::prompt::INITIAL_TURN_PROMPT);
        assert!(name_native_session("claude", &input).unwrap().is_none());
        let transcript = root.path().join("transcript.jsonl");
        std::fs::write(&transcript, concat!(
            "{\"type\":\"user\",\"isMeta\":true,\"message\":{\"content\":\"Loopflow operating guide\"}}\n",
            "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"Repair native titles today\"}]}}\n"
        )).unwrap();
        input.as_object_mut().unwrap().remove("prompt");
        input["hook_event_name"] = json!("SessionStart");
        input["source"] = json!("resume");
        input["transcript_path"] = json!(transcript);
        assert_eq!(
            name_native_session("claude", &input).unwrap().unwrap()["hookSpecificOutput"]
                ["sessionTitle"],
            "Repair native titles"
        );
        std::fs::remove_dir(root.path().join(".lf")).unwrap();
        assert!(name_native_session("claude", &input).unwrap().is_none());
    }

    #[test]
    fn native_hook_installation_keeps_other_hooks_and_is_idempotent() {
        let home = tempfile::tempdir().unwrap();
        std::fs::create_dir(home.path().join(".codex")).unwrap();
        let shared = home.path().join("shared-hooks.json");
        std::fs::write(&shared, "{}").unwrap();
        std::os::unix::fs::symlink(&shared, home.path().join(".codex/hooks.json")).unwrap();
        let path = home.path().join(".claude/settings.json");
        std::fs::create_dir(path.parent().unwrap()).unwrap();
        let existing = json!({"permissions":{"allow":["Read"]},"hooks":{
            "UserPromptSubmit":[{"hooks":[{"type":"command","command":"other-message-hook"}]}]
        }});
        std::fs::write(&path, existing.to_string()).unwrap();
        install_native_hooks(home.path()).unwrap();
        let first = std::fs::read(&path).unwrap();
        install_native_hooks(home.path()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
        let installed: serde_json::Value = serde_json::from_slice(&first).unwrap();
        assert_eq!(installed["permissions"], existing["permissions"]);
        assert_eq!(
            installed["hooks"]["UserPromptSubmit"][0],
            existing["hooks"]["UserPromptSubmit"][0]
        );
        assert_eq!(
            installed["hooks"]["UserPromptSubmit"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let codex: serde_json::Value =
            serde_json::from_slice(&std::fs::read(home.path().join(".codex/hooks.json")).unwrap())
                .unwrap();
        assert_eq!(codex["hooks"]["SessionStart"][0]["matcher"], "resume");
        assert!(home.path().join(".codex/hooks.json").is_symlink());
        assert_eq!(
            std::fs::read(shared).unwrap(),
            std::fs::read(home.path().join(".codex/hooks.json")).unwrap()
        );
    }

    #[test]
    fn task_first_and_short_purpose_without_duplicate_identifier() {
        assert_eq!(display_title("demo", Some("LOO-412")), "LOO-412 demo");
        assert_eq!(
            display_title("LOO-412 plan store migration details", Some("LOO-412")),
            "LOO-412 plan store migration"
        );
        assert_eq!(
            display_title("LOO-4120 demo", Some("LOO-412")),
            "LOO-412 LOO-4120 demo"
        );
    }

    #[test]
    fn taskless_name_is_preserved_without_terminal_controls() {
        assert_eq!(
            display_title("Hérdr and machine work", None),
            "Hérdr and machine work"
        );
        assert_eq!(display_title("demo\x1b\x07\u{009c}", None), "demo");
    }
}
