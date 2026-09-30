use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};

pub(crate) const DISCORD_TOKEN_ENV: &str = "LF_DISCORD_TOKEN";
/// The SSH destination by which the current foreground `lf` was reached.
/// It is invocation context, not durable Home identity.
pub(crate) const SSH_TARGET_ENV: &str = "LF_SSH_TARGET";

/// Owns a child process group until its work is known to be complete.
///
/// The child must be spawned into a fresh process group whose id is its pid.
/// Interrupt cleanup is registered because the CLI signal handler exits the
/// process without running Rust destructors.
#[derive(Debug)]
pub(crate) struct ProcessGroupGuard {
    pid: Arc<AtomicU32>,
}

impl ProcessGroupGuard {
    pub(crate) fn new(pid: u32) -> Self {
        assert!(pid > 1, "owned process group must have a child pid");
        let pid = Arc::new(AtomicU32::new(pid));
        let interrupt_pid = Arc::clone(&pid);
        crate::engine::agent::register_interrupt_cleanup(move || {
            terminate_process_group(interrupt_pid.swap(0, Ordering::AcqRel));
        });
        Self { pid }
    }

    pub(crate) fn terminate(&self) {
        terminate_process_group(self.pid.swap(0, Ordering::AcqRel));
    }

    pub(crate) fn disarm(&self) {
        self.pid.store(0, Ordering::Release);
    }
}

impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn terminate_process_group(pid: u32) {
    if pid == 0 {
        return;
    }

    #[cfg(unix)]
    if let Ok(pid) = i32::try_from(pid) {
        // SAFETY: callers spawn the child into a fresh process group whose id
        // is its pid; a negative pid targets only that owned group.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    crate::engine::platform::kill_process(pid);
}

pub(crate) fn current_process_group_id() -> Option<u32> {
    // SAFETY: getpgrp has no preconditions and does not dereference memory.
    let process_group = unsafe { libc::getpgrp() };
    u32::try_from(process_group).ok().filter(|id| *id > 1)
}

pub(crate) fn resolve_lf_binary() -> PathBuf {
    if let Some(path) = select_binary_override(
        crate::build_info::provenance(),
        std::env::var_os(crate::store::CONTROL_BIN_ENV),
        std::env::var_os("LF_BIN"),
    ) {
        return path;
    }

    if let Ok(path) = std::env::var("CARGO_BIN_EXE_lf") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    current_or_sibling_lf_binary().unwrap_or_else(|| PathBuf::from("lf"))
}

fn current_or_sibling_lf_binary() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    if current.file_name().is_some_and(|name| name == "lf") {
        return Some(current);
    }
    let sibling = current.parent()?.join("lf");
    sibling.exists().then_some(sibling)
}

/// Select the official runtime at each step boundary. Ambient PATH and control
/// pins are not explicit locks; PATH remains a fallback on uninstalled machines.
pub(crate) fn resolve_step_lf_binary(cwd: &Path) -> Result<PathBuf> {
    if let Some(cli) = official_lf_binary()? {
        return Ok(cli);
    }
    let search_path = std::env::var_os("PATH").unwrap_or_default();
    if let Some(path) = std::env::split_paths(&search_path)
        .map(|directory| cwd.join(directory).join("lf"))
        .find(|candidate| candidate.is_file())
    {
        return Ok(path);
    }
    std::env::current_exe().context("resolve Flow driver executable as final fallback")
}

fn select_binary_override(
    provenance: crate::build_info::BuildProvenance,
    control: Option<std::ffi::OsString>,
    ordinary: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let selected = if provenance.is_release() {
        control.or(ordinary)
    } else {
        ordinary
    }?;
    if selected.is_empty() {
        None
    } else {
        Some(PathBuf::from(selected))
    }
}

/// The launch-boundary counterpart to [`select_binary_override`]: it takes only
/// the ordinary `LF_BIN` value and has no control input at all. The current
/// Home is never resolved through `LF_CONTROL_BIN`, in any provenance — that
/// pin is the historical binary a legacy body must stop relaunching through.
fn select_current_home_binary(ordinary: Option<std::ffi::OsString>) -> Option<PathBuf> {
    ordinary
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Resolve the `lf` a Work launch will use: an absolute path that exists.
///
/// `resolve_lf_binary` may hand back the bare name `lf`, which a child resolves
/// against the *login* shell's PATH inside tmux — a third binary, chosen by
/// neither the Work nor its launcher. Work that cannot name its own
/// executable is not created.
pub(crate) fn resolve_pinned_lf_binary() -> Result<PathBuf> {
    if let Some(selection) = crate::machine_install::selection_for_current_executable()? {
        return Ok(selection.verified_cli()?.to_path_buf());
    }
    let candidate = resolve_lf_binary();
    if candidate.is_absolute() {
        return if candidate.exists() {
            Ok(candidate)
        } else {
            Err(anyhow!(
                "lf binary {} does not exist; set LF_BIN to the lf this Work should run",
                candidate.display()
            ))
        };
    }
    which_on_path(&candidate).ok_or_else(|| {
        anyhow!(
            "cannot resolve an absolute path for `{}`; set LF_BIN to the lf this Work should run",
            candidate.display()
        )
    })
}

/// Pin one process generation to immutable executable bytes.
///
/// The installed `lf` is normally a mutable symlink. Exact-frontier promotion
/// may repoint it while a resident body is running, so the body carries the
/// canonical target in `LF_CONTROL_BIN`. A later body launch deliberately
/// resolves the current Home again and picks up the promoted binary.
pub(crate) fn pin_control_binary(lf_bin: &Path) -> PathBuf {
    std::fs::canonicalize(lf_bin).unwrap_or_else(|_| lf_bin.to_path_buf())
}

/// Capture the current process's control context — this process's `lf`, store,
/// and `LF_HOME` — for propagating down to a vendored subprocess. In a release
/// build this honors `LF_CONTROL_*`, so a running body hands its own Run context
/// (not the machine's Home) to the provider CLI it spawns.
///
/// This is NOT the launch resolver. Use [`current_home_execution_context`] to
/// launch or relaunch Work: launching through the control context would
/// perpetuate the historical binary a legacy body was created with.
pub(crate) fn pinned_execution_context() -> Result<crate::child::ChildExecutionContext> {
    let db_path = crate::store::database_path_from_env()
        .map_err(|error| anyhow!("cannot resolve the Run database path: {error}"))?;
    Ok(crate::child::ChildExecutionContext {
        lf_bin: resolve_pinned_lf_binary()?,
        db_path,
        lf_home: crate::store::lf_home_dir(),
    })
}

fn uninstalled_lf_binary() -> PathBuf {
    if let Some(bin) = select_current_home_binary(std::env::var_os("LF_BIN")) {
        return bin;
    }
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_lf") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if !crate::build_info::provenance().is_release()
        && crate::machine_install::selection_for_current_executable()
            .is_ok_and(|selection| selection.is_none())
    {
        let development = resolve_lf_binary();
        if development.is_absolute() {
            return development;
        }
    }
    if let Some(installed) = which_on_path(Path::new("lf")) {
        return installed;
    }
    current_or_sibling_lf_binary().unwrap_or_else(|| PathBuf::from("lf"))
}

/// Resolve the selected installation at this child boundary, independently of
/// execution placement. Uninstalled machines retain source execution.
pub(crate) fn resolve_current_home_lf_binary_checked() -> Result<PathBuf> {
    if let Some(cli) = official_lf_binary()? {
        return Ok(cli);
    }
    let candidate = uninstalled_lf_binary();
    if candidate.is_absolute() {
        return if candidate.exists() {
            Ok(candidate)
        } else {
            Err(anyhow!(
                "lf binary {} does not exist; set LF_BIN to the current Home lf",
                candidate.display()
            ))
        };
    }
    which_on_path(&candidate).ok_or_else(|| {
        anyhow!(
            "cannot resolve an absolute path for `{}`; set LF_BIN to the current Home lf",
            candidate.display()
        )
    })
}

fn official_lf_binary() -> Result<Option<PathBuf>> {
    #[cfg(test)]
    let root = match std::env::var_os("LF_TEST_TASK_INSTALL_ROOT") {
        Some(root) => PathBuf::from(root),
        None => return Ok(None),
    };
    #[cfg(not(test))]
    let root = crate::machine_install::root()?;
    let Some(selection) = crate::machine_install::current_selection(&root)? else {
        return Ok(None);
    };
    Ok(Some(selection.verified_cli()?.to_path_buf()))
}

/// Resolve the current Home execution context for launching Work: the
/// current Home `lf`, store, and `LF_HOME`, ignoring every `LF_CONTROL_*` pin.
///
/// This is the launch/relaunch boundary resolver. Work created under one
/// binary and resumed under another launches through the current Home — its
/// worktree, provider history, and directives are unaffected by which binary
/// first created it.
pub(crate) fn current_home_execution_context() -> Result<crate::child::ChildExecutionContext> {
    let db_path = crate::store::current_home_database_path()
        .map_err(|error| anyhow!("cannot resolve the current Home database path: {error}"))?;
    Ok(crate::child::ChildExecutionContext {
        lf_bin: resolve_current_home_lf_binary_checked()?,
        db_path,
        lf_home: crate::store::current_home_lf_home_dir(),
    })
}

pub(crate) fn which_on_path(name: &Path) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

pub(crate) fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

pub(crate) async fn start_lf_session_with_env(
    session: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<()> {
    let context = pinned_execution_context()?;
    start_session_with_context(session, cwd, argv, env, context).await
}

async fn start_session_with_context(
    session: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
    context: crate::child::ChildExecutionContext,
) -> Result<()> {
    let inherited_context = [
        "LF_TRACE_ID",
        "LF_PROCESS_ID",
        crate::lf::WORK_DECLARATION_ENV,
    ]
    .into_iter()
    .filter(|key| !env.iter().any(|(explicit, _)| explicit == key))
    .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
    .collect::<Vec<_>>();
    let mut child_env = env
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect::<Vec<_>>();
    child_env.extend(
        inherited_context
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone())),
    );
    extend_session_control_context(&mut child_env, &context, crate::build_info::provenance());
    let environment = child_env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let shell_command = lf_session_shell_command(argv, &environment);
    start_tmux_session(session, &cwd.display().to_string(), &shell_command).await
}

fn extend_session_control_context(
    child_env: &mut Vec<(String, String)>,
    context: &crate::child::ChildExecutionContext,
    provenance: crate::build_info::BuildProvenance,
) {
    let pinned = [
        (
            crate::store::CONTROL_BIN_ENV,
            context.lf_bin.to_string_lossy().to_string(),
        ),
        (
            crate::store::CONTROL_HOME_ENV,
            context.lf_home.to_string_lossy().to_string(),
        ),
        (
            crate::store::CONTROL_DB_PATH_ENV,
            context.db_path.to_string_lossy().to_string(),
        ),
    ];
    for (key, value) in pinned {
        if !child_env.iter().any(|(existing, _)| existing == key) {
            child_env.push((key.to_string(), value));
        }
    }
    if !provenance.is_release() {
        for (ordinary, control) in [
            ("LF_HOME", crate::store::CONTROL_HOME_ENV),
            ("LF_DB_PATH", crate::store::CONTROL_DB_PATH_ENV),
        ] {
            if child_env.iter().any(|(existing, _)| existing == ordinary) {
                continue;
            }
            let value = child_env
                .iter()
                .find(|(key, _)| key == control)
                .map(|(_, value)| value.clone());
            if let Some(value) = value {
                child_env.push((ordinary.to_string(), value));
            }
        }
    }
}

pub(crate) fn lf_session_shell_command(argv: &[String], env: &[(&str, &str)]) -> String {
    let command = argv
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let env = env
        .iter()
        .map(|(key, value)| format!("{}={}", shell_escape(key), shell_escape(value)))
        .collect::<Vec<_>>()
        .join(" ");
    let clear_context = "if [ -n \"${LF_FORWARDED_SECRET_NAMES:-}\" ]; then unset $LF_FORWARDED_SECRET_NAMES; fi; unset LF_AS LF_FLOW_STEP LF_HUMAN_SESSION LF_HUMAN_SESSION_RUN_BIND LF_RUN_DIR LF_RUN_CONTEXT LF_TRACE_ID LF_PROCESS_ID LF_WAVE_ID LF_RUN_ID LF_INSTALL_SWITCH LF_BIN LF_HOME LF_DB_PATH LF_CONTROL_BIN LF_CONTROL_HOME LF_CONTROL_DB_PATH LF_ACCOUNT_LEASE LF_ACCOUNT_SELECTION LF_FORWARDED_PM_TOKEN LF_FORWARDED_PM_PROVIDER LF_FORWARDED_SECRET_NAMES LF_SSH_TARGET LF_LINEAR_WEBHOOK_SECRET LF_LINEAR_VIEWER_ID LF_GITHUB_WEBHOOK_SECRET LF_GITHUB_WEBHOOK_URL LF_LFD_ALLOW_NON_LOOPBACK LF_TASK_ORIGIN LF_DISCORD_TOKEN GH_TOKEN OPENCODE_API_KEY CLAUDE_CODE_OAUTH_TOKEN ANTHROPIC_API_KEY CODEX_ACCESS_TOKEN OPENAI_API_KEY; export LF_USER_NAME=\"\"";
    if env.is_empty() {
        format!("{clear_context}; exec {command}")
    } else {
        format!("{clear_context}; exec env {env} {command}")
    }
}

pub(crate) async fn start_tmux_session(
    session: &str,
    cwd: &str,
    shell_command: &str,
) -> Result<()> {
    let mut command = tokio::process::Command::new("tmux");
    for name in forwarded_authority_env_names() {
        command.env_remove(name);
    }
    let status = command
        .args([
            "new-session",
            "-d",
            "-s",
            session,
            "-c",
            cwd,
            "/bin/sh",
            "-lc",
            shell_command,
        ])
        .status()
        .await
        .map_err(|err| anyhow!("tmux failed to spawn: {err}"))?;
    if !status.success() {
        return Err(anyhow!("tmux failed to launch session '{session}'"));
    }
    let _ = tokio::process::Command::new("tmux")
        .args(["set-option", "-t", session, "mouse", "on"])
        .status()
        .await;
    Ok(())
}

fn forwarded_authority_env_names() -> Vec<String> {
    let mut names = vec![
        crate::provider_account::lease::ACCOUNT_LEASE_ENV.to_string(),
        crate::provider_account::lease::ACCOUNT_SELECTION_ENV.to_string(),
        "LF_FORWARDED_PM_TOKEN".to_string(),
        "LF_FORWARDED_PM_PROVIDER".to_string(),
        "LF_FORWARDED_SECRET_NAMES".to_string(),
        crate::engine::process::SSH_TARGET_ENV.to_string(),
        "LF_LINEAR_WEBHOOK_SECRET".to_string(),
        "LF_LINEAR_VIEWER_ID".to_string(),
        "LF_GITHUB_WEBHOOK_SECRET".to_string(),
        "LF_GITHUB_WEBHOOK_URL".to_string(),
        "LF_LFD_ALLOW_NON_LOOPBACK".to_string(),
        DISCORD_TOKEN_ENV.to_string(),
        "GH_TOKEN".to_string(),
        "OPENCODE_API_KEY".to_string(),
        "CLAUDE_CODE_OAUTH_TOKEN".to_string(),
        "ANTHROPIC_API_KEY".to_string(),
        "CODEX_ACCESS_TOKEN".to_string(),
        "OPENAI_API_KEY".to_string(),
    ];
    if let Ok(forwarded) = std::env::var("LF_FORWARDED_SECRET_NAMES") {
        names.extend(forwarded.split_whitespace().map(str::to_string));
    }
    names
}

pub(crate) fn tmux_session_slug(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        extend_session_control_context, forwarded_authority_env_names, lf_session_shell_command,
        pin_control_binary, select_binary_override, select_current_home_binary, DISCORD_TOKEN_ENV,
    };
    use crate::build_info::BuildProvenance;
    use crate::child::ChildExecutionContext;

    #[test]
    fn a_body_generation_keeps_one_binary_across_a_global_repoint() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("lf-old");
        let new = dir.path().join("lf-new");
        let installed = dir.path().join("lf");
        std::fs::write(&old, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        std::os::unix::fs::symlink(&old, &installed).unwrap();

        let pinned = pin_control_binary(&installed);
        assert_eq!(pinned, std::fs::canonicalize(&old).unwrap());
        std::fs::remove_file(&installed).unwrap();
        std::os::unix::fs::symlink(&new, &installed).unwrap();

        assert_eq!(std::fs::read(&pinned).unwrap(), b"old");
        assert_eq!(
            std::fs::read(std::fs::canonicalize(&installed).unwrap()).unwrap(),
            b"new"
        );
    }

    #[test]
    fn development_ignores_stale_control_binary_override() {
        assert_eq!(
            select_binary_override(
                BuildProvenance::Development,
                Some("/production/lf".into()),
                Some("/development/lf".into()),
            ),
            Some(PathBuf::from("/development/lf"))
        );
        assert_eq!(
            select_binary_override(
                BuildProvenance::Release,
                Some("/production/lf".into()),
                Some("/ambient/lf".into()),
            ),
            Some(PathBuf::from("/production/lf"))
        );
    }

    /// The launch boundary must resolve the current Home lf (B), never the
    /// historical `LF_CONTROL_BIN` pin (A) — the regression behind stranded
    /// legacy Work bodies. Contrast the two selectors under release provenance:
    /// the old override picks the control pin A, the current-Home selector
    /// picks B and has no way to reach A at all.
    #[test]
    fn current_home_binary_never_resolves_through_the_control_pin() {
        // Old behavior (the bug): a release build prefers LF_CONTROL_BIN (A),
        // even when the current Home LF_BIN (B) is present.
        assert_eq!(
            select_binary_override(
                BuildProvenance::Release,
                Some("/old/A/lf".into()),
                Some("/current/B/lf".into()),
            ),
            Some(PathBuf::from("/old/A/lf")),
        );
        // Fixed: the current-Home selector has no control input, so with
        // LF_BIN=B it resolves B — A is unreachable, in any provenance.
        assert_eq!(
            select_current_home_binary(Some("/current/B/lf".into())),
            Some(PathBuf::from("/current/B/lf")),
        );
        // Empty or absent LF_BIN falls through to PATH/installed lf, never to A.
        assert_eq!(select_current_home_binary(None), None);
        assert_eq!(select_current_home_binary(Some("".into())), None);
    }

    #[test]
    fn persisted_control_binary_wins_over_relaunching_callers_binary() {
        let mut environment = vec![(
            crate::store::CONTROL_BIN_ENV.to_string(),
            "/persisted/lf".to_string(),
        )];
        let caller = ChildExecutionContext {
            lf_bin: PathBuf::from("/caller/lf"),
            lf_home: PathBuf::from("/caller/home"),
            db_path: PathBuf::from("/caller/loopflow.db"),
        };

        extend_session_control_context(&mut environment, &caller, BuildProvenance::Release);

        assert!(environment.iter().any(|(key, value)| {
            key == crate::store::CONTROL_BIN_ENV && value == "/persisted/lf"
        }));
        assert!(!environment
            .iter()
            .any(|(key, value)| { key == crate::store::CONTROL_BIN_ENV && value == "/caller/lf" }));
    }

    #[test]
    fn lf_session_clears_parent_identity_and_exports_its_own() {
        let argv = vec![
            "lf".to_string(),
            "work".to_string(),
            "execute".to_string(),
            "task".to_string(),
            "tsk_123".to_string(),
        ];
        let command = lf_session_shell_command(&argv, &[("LF_WAVE_ID", "infra")]);

        assert!(command.starts_with(
            "if [ -n \"${LF_FORWARDED_SECRET_NAMES:-}\" ]; then unset $LF_FORWARDED_SECRET_NAMES; fi; unset "
        ));
        assert!(command.contains("LF_WAVE_ID LF_RUN_ID LF_INSTALL_SWITCH"));
        assert!(command.contains("LF_INSTALL_SWITCH LF_BIN"));
        assert!(command.contains("LF_ACCOUNT_LEASE LF_ACCOUNT_SELECTION"));
        assert!(command.contains("LF_DISCORD_TOKEN"));
        assert!(command.contains("GH_TOKEN OPENCODE_API_KEY"));
        assert!(command
            .ends_with("exec env 'LF_WAVE_ID'='infra' 'lf' 'work' 'execute' 'task' 'tsk_123'"));
    }

    #[test]
    fn lf_session_without_explicit_identity_does_not_inherit_its_parent() {
        let argv = vec!["lf".to_string(), "wave".to_string(), "child".to_string()];

        let command = lf_session_shell_command(&argv, &[]);

        assert!(command.contains("LF_WAVE_ID LF_RUN_ID"));
        assert!(command.contains("LF_ACCOUNT_LEASE LF_ACCOUNT_SELECTION"));
        assert!(command.ends_with("exec 'lf' 'wave' 'child'"));
    }

    #[test]
    fn preferred_name_in_durable_sessions_requires_explicit_context() {
        let argv = vec![
            "sh".into(),
            "-c".into(),
            "printf '%s' \"${LF_USER_NAME-unset}\"".into(),
        ];
        for name in [None, Some("Maya")] {
            let env = name
                .map(|name| vec![(crate::engine::config::USER_NAME_ENV, name)])
                .unwrap_or_default();
            let command = lf_session_shell_command(&argv, &env);
            let output = std::process::Command::new("sh")
                .args(["-c", &command])
                .env(crate::engine::config::USER_NAME_ENV, "Jack")
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                name.unwrap_or_default()
            );
        }
    }

    #[test]
    fn durable_session_scrubs_every_named_forwarded_secret() {
        let _lock = crate::journal::test_env_lock();
        let previous = std::env::var_os("LF_FORWARDED_SECRET_NAMES");
        std::env::set_var("LF_FORWARDED_SECRET_NAMES", "SENTRY_TOKEN STRIPE_KEY");

        let names = forwarded_authority_env_names();

        match previous {
            Some(value) => std::env::set_var("LF_FORWARDED_SECRET_NAMES", value),
            None => std::env::remove_var("LF_FORWARDED_SECRET_NAMES"),
        }
        assert!(names.iter().any(|name| name == "LF_ACCOUNT_LEASE"));
        assert!(names.iter().any(|name| name == "GH_TOKEN"));
        assert!(names.iter().any(|name| name == "LF_LINEAR_WEBHOOK_SECRET"));
        assert!(names.iter().any(|name| name == "LF_LFD_ALLOW_NON_LOOPBACK"));
        assert!(names.iter().any(|name| name == "LF_DISCORD_TOKEN"));
        assert!(names.iter().any(|name| name == "SENTRY_TOKEN"));
        assert!(names.iter().any(|name| name == "STRIPE_KEY"));
    }

    #[test]
    fn discord_chat_token_is_scrubbed_from_durable_provider_children() {
        assert!(forwarded_authority_env_names()
            .iter()
            .any(|name| name == DISCORD_TOKEN_ENV));
        let command = lf_session_shell_command(&["lf".into(), "wave".into()], &[]);
        assert!(command.contains("unset "));
        assert!(command.contains(DISCORD_TOKEN_ENV));
    }

    #[test]
    fn lf_session_replaces_tmux_invocation_context() {
        let argv = vec![
            "lf".to_string(),
            "work".to_string(),
            "execute".to_string(),
            "task".to_string(),
            "tsk_123".to_string(),
        ];
        let command = lf_session_shell_command(
            &argv,
            &[
                ("LF_TRACE_ID", "run-1"),
                ("LF_PROCESS_ID", "process-1"),
                ("LF_DB_PATH", "/tmp/current.db"),
                ("LF_HOME", "/tmp/lf"),
            ],
        );

        assert!(command.contains("LF_WAVE_ID LF_RUN_ID"));
        assert!(command.contains("LF_ACCOUNT_LEASE LF_ACCOUNT_SELECTION"));
        assert!(command.ends_with(
            "exec env 'LF_TRACE_ID'='run-1' 'LF_PROCESS_ID'='process-1' 'LF_DB_PATH'='/tmp/current.db' 'LF_HOME'='/tmp/lf' 'lf' 'work' 'execute' 'task' 'tsk_123'"
        ));
    }
}

#[cfg(not(test))]
pub(crate) async fn start_home_session(session: &str, cwd: &Path, argv: &[String]) -> Result<()> {
    start_home_session_with_env(session, cwd, argv, &[]).await
}

#[cfg(not(test))]
pub(crate) async fn start_home_session_with_env(
    session: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<()> {
    let context = current_home_execution_context()?;
    let lf_bin = context.lf_bin.to_string_lossy().to_string();
    let mut environment = vec![("LF_BIN", lf_bin.as_str())];
    environment.extend_from_slice(env);
    start_session_with_context(session, cwd, argv, &environment, context).await
}
