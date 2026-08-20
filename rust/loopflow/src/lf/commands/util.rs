use anyhow::{anyhow, bail, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

use crate::engine::{check_cli_available, codex_permission_args, workspace_add_dirs, LaunchTarget};
use crate::provider_auth::Provider;

pub fn find_repo_root() -> Result<PathBuf> {
    crate::engine::repo::find_repo_root()
}

pub(crate) fn parse_since(value: &str, now: OffsetDateTime) -> Result<OffsetDateTime> {
    if let Ok(timestamp) = OffsetDateTime::parse(value, &Rfc3339) {
        return Ok(timestamp);
    }
    let (amount, unit) = value.split_at(value.len().saturating_sub(1));
    let amount: i64 = amount
        .parse()
        .map_err(|_| anyhow!("invalid --since '{value}'; use 7d, 24h, 30m, or RFC3339"))?;
    if amount < 0 {
        return Err(anyhow!("--since duration must be non-negative"));
    }
    let seconds_per_unit = match unit {
        "d" => 86_400,
        "h" => 3_600,
        "m" => 60,
        _ => {
            return Err(anyhow!(
                "invalid --since '{value}'; use 7d, 24h, 30m, or RFC3339"
            ));
        }
    };
    let seconds = amount
        .checked_mul(seconds_per_unit)
        .ok_or_else(|| anyhow!("--since duration is too large"))?;
    now.checked_sub(Duration::seconds(seconds))
        .ok_or_else(|| anyhow!("--since duration is too large"))
}

/// Message text from the args (joined) or stdin (heredoc-friendly). The
/// Message commands take text arguments or read stdin when omitted.
pub(crate) fn message_text(args: &[String], mut stdin: impl Read) -> Result<String> {
    let joined = args.join(" ").trim().to_string();
    if !joined.is_empty() {
        return Ok(joined);
    }
    let mut buffer = String::new();
    stdin.read_to_string(&mut buffer)?;
    let text = buffer.trim().to_string();
    if text.is_empty() {
        bail!("no message text: pass TEXT or pipe it on stdin");
    }
    Ok(text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionCommand {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionLaunch {
    command: SessionCommand,
    ide_url: Option<String>,
}

pub fn launch_session(
    target: LaunchTarget,
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
) -> Result<()> {
    let launch = build_session_launch(target, harness, model, worktree, prompt)?;

    if target == LaunchTarget::Ide {
        if let Some(url) = launch.ide_url.as_deref() {
            match crate::engine::platform::open_url_checked(url) {
                Ok(()) => return Ok(()),
                Err(err) => eprintln!("Could not open vendor app ({err}); falling back to TUI."),
            }
        } else if harness == "opencode" {
            eprintln!("OpenCode has no standalone app; opening the TUI.");
        }
    }

    spawn_session_command(&launch.command)
}

fn build_session_launch(
    target: LaunchTarget,
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
) -> Result<SessionLaunch> {
    let worktree = absolute_path(worktree);
    let command = build_session_command(harness, model, &worktree, prompt)?;
    let ide_url = if target == LaunchTarget::Ide {
        build_ide_url(harness, &worktree, prompt)
    } else {
        None
    };

    Ok(SessionLaunch { command, ide_url })
}

fn build_ide_url(harness: &str, worktree: &Path, prompt: &str) -> Option<String> {
    let worktree = percent_encode(&worktree.to_string_lossy());
    let prompt = percent_encode(prompt);
    match harness {
        "codex" => Some(format!(
            "codex://threads/new?path={worktree}&prompt={prompt}"
        )),
        "claude" => Some(format!("claude://code/new?folder={worktree}&q={prompt}")),
        "opencode" => None,
        _ => None,
    }
}

pub(crate) fn build_session_command(
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
) -> Result<SessionCommand> {
    let cwd = worktree.to_path_buf();
    let worktree_arg = worktree.to_string_lossy().to_string();

    match harness {
        "codex" => {
            let mut args = vec!["-C".to_string(), worktree_arg];
            if let Some(model) = model {
                args.push("-c".to_string());
                args.push(format!("model=\"{model}\""));
            }
            for dir in workspace_add_dirs(worktree) {
                args.push("--add-dir".to_string());
                args.push(dir.to_string_lossy().to_string());
            }
            args.extend(codex_permission_args(Some(worktree), false, false));
            args.push(prompt.to_string());
            Ok(SessionCommand {
                program: "codex".to_string(),
                args,
                cwd,
            })
        }
        "claude" => {
            let mut args = Vec::new();
            if let Some(model) = model {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            for dir in workspace_add_dirs(worktree) {
                args.push("--add-dir".to_string());
                args.push(dir.to_string_lossy().to_string());
            }
            // Claude's variadic --add-dir otherwise consumes the positional prompt.
            args.push("--".to_string());
            args.push(prompt.to_string());
            Ok(SessionCommand {
                program: "claude".to_string(),
                args,
                cwd,
            })
        }
        "opencode" => {
            let mut args = vec![worktree_arg, "--prompt".to_string(), prompt.to_string()];
            if let Some(model) = model {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            Ok(SessionCommand {
                program: "opencode".to_string(),
                args,
                cwd,
            })
        }
        _ => Err(anyhow!(
            "unsupported session launcher harness '{}'. Use claude, codex, or opencode.",
            harness
        )),
    }
}

pub(crate) fn spawn_session_command(command: &SessionCommand) -> Result<()> {
    let status = session_command_status(command)?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("session launcher exited with status {status}"))
    }
}

pub(crate) fn session_command_status(command: &SessionCommand) -> Result<std::process::ExitStatus> {
    if !check_cli_available(&command.program) {
        return Err(anyhow!(
            "'{}' CLI not found. Install it and rerun `lf init`.",
            command.program
        ));
    }

    let provider = match command.program.as_str() {
        "claude" => Some(Provider::Claude),
        "codex" => Some(Provider::Codex),
        _ => None,
    };
    let account_route = provider
        .map(|provider| crate::provider_account::resolve_provider_account_blocking(provider, None))
        .transpose()
        .map_err(|error| anyhow!("failed to select provider account: {error}"))?
        .flatten();

    let mut process = Command::new(&command.program);
    if provider == Some(Provider::Codex)
        && account_route
            .as_ref()
            .is_some_and(crate::provider_account::ProviderAccountRoute::uses_native_home)
    {
        process.args(["-c", "cli_auth_credentials_store=\"file\""]);
    }
    process
        .args(&command.args)
        .current_dir(&command.cwd)
        .env_remove("LOOPFLOW_DIRECTIVE_FILE");
    crate::provider_auth::apply_provider_env_to_command(&command.program, &mut process);
    if let Some(route) = &account_route {
        tracing::info!(provider = %command.program, "selected managed provider account");
        route.apply(&mut process);
    }
    process.status().map_err(Into::into)
}

fn absolute_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::EmailAddress;
    use crate::provider_account::new_account;
    use crate::store::{
        open_store, CredentialType, ProviderAccountId, ProviderToken, StorageConfig,
        CONTROL_DB_PATH_ENV, CONTROL_HOME_ENV,
    };
    use std::ffi::OsString;

    struct EnvRestore(Vec<(&'static str, Option<OsString>)>);

    impl EnvRestore {
        fn capture(names: &[&'static str]) -> Self {
            Self(
                names
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (name, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    fn path() -> PathBuf {
        PathBuf::from("/tmp/loop flow")
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn git_worktree_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let main = tmp.path().join("repo");
        let worktree = tmp.path().join("repo.feature");
        std::fs::create_dir(&main).expect("create repo dir");
        std::fs::write(main.join("README.md"), "hello\n").expect("write file");

        git(&main, &["init", "-b", "main"]);
        git(&main, &["config", "user.email", "test@example.com"]);
        git(&main, &["config", "user.name", "Test User"]);
        git(&main, &["add", "."]);
        git(&main, &["commit", "-m", "init"]);
        git(
            &main,
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                worktree.to_str().expect("utf8 worktree"),
            ],
        );

        (tmp, main, worktree)
    }

    fn git(repo: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git -C {} {} failed:\n{}",
            repo.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn message_text_prefers_args_then_stdin_then_errors() {
        let text = message_text(&["hello".into(), "world".into()], std::io::empty()).unwrap();
        assert_eq!(text, "hello world");

        let text = message_text(&[], std::io::Cursor::new("from stdin\n")).unwrap();
        assert_eq!(text, "from stdin");

        let err = message_text(&[], std::io::empty()).unwrap_err();
        assert!(err.to_string().contains("no message text"));
    }

    #[test]
    fn session_launch_tui_codex_sets_worktree_model_and_prompt() {
        let launch =
            build_session_launch(LaunchTarget::Tui, "codex", Some("o3"), &path(), "fix it")
                .expect("build launch");

        assert_eq!(launch.command.program, "codex");
        assert_eq!(launch.command.cwd, path());
        assert!(launch.command.args.starts_with(&args(&[
            "-C",
            "/tmp/loop flow",
            "-c",
            "model=\"o3\""
        ])));
        assert_eq!(
            launch.command.args.last().map(String::as_str),
            Some("fix it")
        );
        assert_eq!(
            launch.command.args.contains(&"--sandbox".to_string()),
            crate::engine::codex_permission_args(Some(&path()), false, false)
                .contains(&"--sandbox".to_string())
        );
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn session_launch_tui_codex_adds_main_repo_for_worktree_metadata() {
        let (_tmp, main, worktree) = git_worktree_fixture();

        let launch = build_session_launch(LaunchTarget::Tui, "codex", None, &worktree, "fix it")
            .expect("build launch");

        let idx = launch
            .command
            .args
            .iter()
            .position(|arg| arg == "--add-dir")
            .expect("add-dir flag");
        assert_eq!(
            PathBuf::from(&launch.command.args[idx + 1])
                .canonicalize()
                .unwrap(),
            main.canonicalize().unwrap()
        );
    }

    #[test]
    fn session_launch_tui_claude_runs_in_worktree_with_model_and_prompt() {
        let launch = build_session_launch(
            LaunchTarget::Tui,
            "claude",
            Some("sonnet"),
            &path(),
            "fix it",
        )
        .expect("build launch");

        assert_eq!(
            launch.command,
            SessionCommand {
                program: "claude".to_string(),
                args: args(&["--model", "sonnet", "--", "fix it"]),
                cwd: path(),
            }
        );
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn session_launch_tui_claude_adds_main_repo_for_worktree_metadata() {
        let (_tmp, main, worktree) = git_worktree_fixture();

        let launch = build_session_launch(
            LaunchTarget::Tui,
            "claude",
            Some("sonnet"),
            &worktree,
            "fix it",
        )
        .expect("build launch");

        let idx = launch
            .command
            .args
            .iter()
            .position(|arg| arg == "--add-dir")
            .expect("add-dir flag");
        assert_eq!(
            PathBuf::from(&launch.command.args[idx + 1])
                .canonicalize()
                .unwrap(),
            main.canonicalize().unwrap()
        );
        assert!(launch.command.args.ends_with(&args(&["--", "fix it"])));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn session_launch_tui_claude_uses_a_healthy_managed_login() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_DB_PATH",
            CONTROL_HOME_ENV,
            CONTROL_DB_PATH_ENV,
            "LF_ACCOUNT_LEASE",
            "LF_TEST_SESSION_ENV",
            "CLAUDE_CONFIG_DIR",
            "PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::remove_var("LF_DB_PATH");
        std::env::remove_var(CONTROL_HOME_ENV);
        std::env::remove_var(CONTROL_DB_PATH_ENV);
        std::env::remove_var("LF_ACCOUNT_LEASE");
        std::env::set_var("CLAUDE_CONFIG_DIR", "ambient");

        let bin = temp.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let claude = bin.join("claude");
        std::fs::write(
            &claude,
            "#!/bin/sh\nprintf '%s' \"$CLAUDE_CONFIG_DIR\" > \"$LF_TEST_SESSION_ENV\"\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&claude).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&claude, permissions).unwrap();
        }
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        let capture = temp.path().join("session-env");
        std::env::set_var("LF_TEST_SESSION_ENV", &capture);

        let store = open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
            .await
            .unwrap();
        let account_home = temp.path().join("accounts/claude/jackstah");
        let account = new_account(
            Provider::Claude,
            ProviderAccountId::parse("jackstah").unwrap(),
            account_home.clone(),
            Some(EmailAddress::parse("jackstah@gmail.com").unwrap()),
        );
        store.upsert_provider_account(&account).await.unwrap();

        launch_session(LaunchTarget::Tui, "claude", None, temp.path(), "review it").unwrap();

        assert_eq!(
            std::fs::read_to_string(capture).unwrap(),
            account_home.to_string_lossy()
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn session_launch_tui_opencode_uses_the_stored_zen_credential() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_DB_PATH",
            CONTROL_HOME_ENV,
            CONTROL_DB_PATH_ENV,
            "LF_ACCOUNT_LEASE",
            "LF_TEST_SESSION_ENV",
            "OPENCODE_API_KEY",
            "PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::remove_var("LF_DB_PATH");
        std::env::remove_var(CONTROL_HOME_ENV);
        std::env::remove_var(CONTROL_DB_PATH_ENV);
        std::env::remove_var("LF_ACCOUNT_LEASE");
        std::env::set_var("OPENCODE_API_KEY", "ambient-key");

        let bin = temp.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let opencode = bin.join("opencode");
        std::fs::write(
            &opencode,
            "#!/bin/sh\nprintf '%s' \"$OPENCODE_API_KEY\" > \"$LF_TEST_SESSION_ENV\"\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&opencode).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&opencode, permissions).unwrap();
        }
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        let capture = temp.path().join("session-env");
        std::env::set_var("LF_TEST_SESSION_ENV", &capture);

        let store = open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
            .await
            .unwrap();
        store
            .upsert_provider_token(&ProviderToken {
                provider: Provider::OpenCodeZen.as_str().to_string(),
                access_token: "stored-key".to_string(),
                refresh_token: None,
                oauth_client_id: None,
                expires_at: None,
                login: Some("zen@example.com".to_string()),
                updated_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                credential_type: CredentialType::ApiKey,
            })
            .await
            .unwrap();

        launch_session(
            LaunchTarget::Tui,
            "opencode",
            None,
            temp.path(),
            "review it",
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(capture).unwrap(), "stored-key");
    }

    #[test]
    fn session_launch_tui_opencode_sets_worktree_prompt_and_model() {
        let launch = build_session_launch(
            LaunchTarget::Tui,
            "opencode",
            Some("moonshotai/kimi-k2"),
            &path(),
            "fix it",
        )
        .expect("build launch");

        assert_eq!(
            launch.command,
            SessionCommand {
                program: "opencode".to_string(),
                args: args(&[
                    "/tmp/loop flow",
                    "--prompt",
                    "fix it",
                    "--model",
                    "moonshotai/kimi-k2",
                ]),
                cwd: path(),
            }
        );
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn session_launch_ide_codex_builds_scheme_with_encoded_path_and_prompt() {
        let launch =
            build_session_launch(LaunchTarget::Ide, "codex", None, &path(), "fix & test\nnow")
                .expect("build launch");

        assert_eq!(
            launch.ide_url.as_deref(),
            Some("codex://threads/new?path=%2Ftmp%2Floop%20flow&prompt=fix%20%26%20test%0Anow")
        );
        assert_eq!(launch.command.program, "codex");
    }

    #[test]
    fn session_launch_ide_claude_builds_code_scheme_with_encoded_folder_and_prompt() {
        let launch = build_session_launch(
            LaunchTarget::Ide,
            "claude",
            None,
            &path(),
            "fix & test\nnow",
        )
        .expect("build launch");

        assert_eq!(
            launch.ide_url.as_deref(),
            Some("claude://code/new?folder=%2Ftmp%2Floop%20flow&q=fix%20%26%20test%0Anow")
        );
        assert_eq!(launch.command.program, "claude");
    }

    #[test]
    fn session_launch_ide_opencode_falls_back_to_cli_shape() {
        let launch = build_session_launch(LaunchTarget::Ide, "opencode", None, &path(), "fix it")
            .expect("build launch");

        assert_eq!(launch.command.program, "opencode");
        assert_eq!(launch.ide_url, None);
    }
}
