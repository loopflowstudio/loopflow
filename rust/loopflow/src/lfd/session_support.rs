use std::path::Path;
use anyhow::Result;

use crate::engine::process::{resolve_lf_binary, shell_escape, start_tmux_session};
use crate::lfd::id::LfdId;
use crate::lfd::types::{RunStatus, Session};

pub(crate) fn is_active_run_status(status: RunStatus) -> bool {
    matches!(
        status,
        RunStatus::Pending | RunStatus::Running | RunStatus::Waiting
    )
}

pub(crate) fn is_ephemeral_worktree_path(path: &str) -> bool {
    let worktree_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    has_fork_suffix(worktree_name)
        || has_run_suffix(worktree_name)
        || has_run_id_segment(worktree_name)
}

/// `<repo>.<wave>.<short-run-id>` — a Task Session worktree.
///
/// Three or more dot segments ending in exactly 8 hex chars. Two-segment
/// `<repo>.<name>` human/wave worktrees and preserved `<name>.<timestamp>`
/// trees (13-char `YYYYMMDD_HHMM` suffix) never match.
fn has_run_id_segment(path_component: &str) -> bool {
    let Some((prefix, suffix)) = path_component.rsplit_once('.') else {
        return false;
    };
    prefix.contains('.') && suffix.len() == 8 && suffix.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn has_fork_suffix(path_component: &str) -> bool {
    let Some((_, suffix)) = path_component.rsplit_once("-fork-") else {
        return false;
    };
    !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
}

fn has_run_suffix(path_component: &str) -> bool {
    let Some((_, suffix)) = path_component.rsplit_once("-run-") else {
        return false;
    };
    !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(crate) fn build_lf_skill_command(
    skill_name: &str,
    batch: bool,
    directions: &[String],
    docs: &[String],
    wave_name: &str,
) -> Vec<String> {
    let mut cmd = vec![
        resolve_lf_binary().to_string_lossy().to_string(),
        skill_name.to_string(),
    ];
    append_lf_run_options(&mut cmd, batch, directions, docs, wave_name);
    cmd
}

/// Where a tmux-wrapped session records its exit code (read by whoever
/// reconciles the session — the session supervisor today).
pub(crate) fn tmux_exit_file(cwd: &Path, session_id: &LfdId) -> PathBuf {
    cwd.join(".lf/tmp/sessions")
        .join(format!("{session_id}.exit"))
}

/// Wrapper tail for run-to-completion sessions: propagate the exit code.
pub(crate) const TMUX_EXIT_TAIL: &str = r#"exit "$EXIT_CODE""#;

/// The exit-file shell wrapper every tmux-backed session runs — the single
/// authoring site of the exit-file wire contract.
///
/// Two invariants live here:
/// - `unset LFD_SESSION_INHERITED` first: a fresh tmux server inherits the
///   dispatcher's environment (verified empirically), so a dispatcher that is
///   itself a registered session would leak `LFD_SESSION_INHERITED=1` into
///   the worker's login shell. That flips `classify_run_context` from
///   OwnSession to NeedsRegistration and the worker registers a duplicate row
///   instead of adopting the one the dispatcher created. The session's own
///   env contract is re-exported explicitly by the inline prefix.
/// - The exit code lands in the session's exit file so whoever reconciles
///   the row (a running lfd, or the next boot) can close it.
pub(crate) fn tmux_shell_command(session: &Session, tail: &str) -> String {
    let exit_file = tmux_exit_file(Path::new(&session.cwd), &session.id);
    let exit_dir = exit_file
        .parent()
        .expect("tmux exit file always has a parent");
    let env_prefix = session
        .env
        .iter()
        .map(|(key, value)| format!("{key}={} ", shell_escape(value)))
        .collect::<String>();
    let command = session
        .argv
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "unset LFD_SESSION_INHERITED; mkdir -p {exit_dir}; rm -f {exit_file}; {env_prefix}{command}; EXIT_CODE=$?; printf '%s' \"$EXIT_CODE\" > {exit_file}; {tail}",
        exit_dir = shell_escape(&exit_dir.display().to_string()),
        exit_file = shell_escape(&exit_file.display().to_string()),
    )
}

/// Launch `session` detached in tmux running the exit-file wrapper, then
/// enable mouse mode so scroll events reach tmux rather than the inner shell.
/// Callers own the session row's lifecycle (start on `Ok`, fail on `Err`).
pub(crate) async fn launch_session_in_tmux(session: &Session, tail: &str) -> Result<()> {
    let shell_command = tmux_shell_command(session, tail);
    start_tmux_session(&session.tmux_name, &session.cwd, &shell_command).await
}

fn append_lf_run_options(
    cmd: &mut Vec<String>,
    batch: bool,
    directions: &[String],
    docs: &[String],
    wave_name: &str,
) {
    if batch {
        cmd.push("-b".to_string());
    }
    cmd.push("--no-direction".to_string());
    for direction in directions {
        cmd.push("-d".to_string());
        cmd.push(direction.clone());
    }
    for target in docs {
        cmd.push("--docs".to_string());
        cmd.push(target.clone());
    }
    cmd.push("-w".to_string());
    cmd.push(wave_name.to_string());
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::process::Command;

    use tempfile::TempDir;
    use time::OffsetDateTime;

    use super::{
        is_ephemeral_worktree_path, tmux_shell_command, TMUX_EXIT_TAIL,
    };
    use crate::engine::workspace::ensure_wave_worktree;
    use crate::engine::worktrees::worktree_path as wave_worktree_path;
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{Session, SessionStatus, SessionUse, TMUX_TERMINAL_SOURCE};

    fn run_git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git command should run");
        if !output.status.success() {
            panic!(
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent directories");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn setup_repo_with_remote() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
        let temp = TempDir::new().expect("temp dir");
        let origin = temp.path().join("origin.git");
        let main_repo = temp.path().join("main");
        std::fs::create_dir_all(&main_repo).expect("create repo dir");

        run_git(temp.path(), &["init", "--bare", "-b", "main", "origin.git"]);
        run_git(&main_repo, &["init", "-b", "main"]);
        run_git(&main_repo, &["config", "user.email", "test@example.com"]);
        run_git(&main_repo, &["config", "user.name", "Test User"]);
        write_file(&main_repo.join("README.md"), "initial\n");
        run_git(&main_repo, &["add", "."]);
        run_git(&main_repo, &["commit", "-m", "initial"]);
        run_git(
            &main_repo,
            &["remote", "add", "origin", origin.to_str().unwrap_or("")],
        );
        run_git(&main_repo, &["push", "-u", "origin", "main"]);
        run_git(
            &main_repo,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        (temp, main_repo, origin)
    }

    /// The exit-file wire contract, pinned. Tmux sessions run through this
    /// one wrapper: it clears the inherited-session marker the tmux server
    /// leaks from the parent's environment, exports the session env
    /// inline, and records the exit code in the session's exit file.
    #[test]
    fn tmux_shell_command_unsets_inherited_marker_and_wires_exit_file() {
        let session_id = LfdId::new();
        let session = Session {
            id: session_id.clone(),
            wave_id: LfdId::new(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::Worker,
            skill: "implement".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo.wave.a1b2c3d4".to_string(),
            argv: vec![
                "lf".to_string(),
                "implement:".to_string(),
                "Do it".to_string(),
            ],
            env: BTreeMap::from([
                ("LFD_SESSION_ID".to_string(), session_id.to_string()),
                ("LFD_AGENT_ROLE".to_string(), "worker".to_string()),
            ]),
            source: TMUX_TERMINAL_SOURCE.to_string(),
            tmux_name: "lf-test".to_string(),
            status: SessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        };

        let wrapper = tmux_shell_command(&session, TMUX_EXIT_TAIL);

        assert!(
            wrapper.starts_with("unset LFD_SESSION_INHERITED; "),
            "the wrapper must clear the marker a fresh tmux server inherits \
             from the parent, or workers register duplicate rows: {wrapper}"
        );
        assert!(
            wrapper.contains(&format!("LFD_SESSION_ID='{session_id}' ")),
            "the session env is re-exported inline: {wrapper}"
        );
        assert!(
            wrapper.contains(&format!(
                "'/tmp/repo.wave.a1b2c3d4/.lf/tmp/sessions/{session_id}.exit'"
            )),
            "exit code lands in the session's exit file: {wrapper}"
        );
        assert!(wrapper.ends_with(TMUX_EXIT_TAIL));
    }

    #[test]
    fn is_ephemeral_worktree_path_detects_numeric_fork_suffix() {
        assert!(is_ephemeral_worktree_path("/tmp/repo.wave-fork-0"));
        assert!(is_ephemeral_worktree_path("/tmp/repo-fork-123"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave-fork-x"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave.fork-1"));
    }

    #[test]
    fn is_ephemeral_worktree_path_detects_run_suffix() {
        assert!(is_ephemeral_worktree_path("/tmp/repo.wave-run-a1b2c3d4"));
        assert!(is_ephemeral_worktree_path("/tmp/repo-run-deadbeef"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave-run-"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave-run-xyz!"));
    }

    #[test]
    fn is_ephemeral_worktree_path_ignores_non_fork_paths() {
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave.main"));
    }

    #[test]
    fn is_ephemeral_worktree_path_detects_run_id_segment() {
        // <repo>.<wave>.<short-run-id> — a Task Session worktree.
        assert!(is_ephemeral_worktree_path("/tmp/repo.wave.a1b2c3d4"));
        // Two segments = human or wave worktree.
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.feature"));
        // Preserved worktrees carry a timestamp suffix, not a run id.
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave.20260703_1511"));
        // Non-hex or wrong-length final segments.
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave.release"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave.a1b2"));
    }

    #[test]
    fn ensure_wave_worktree_reuses_existing_sibling_without_rebasing() {
        let (_temp, main_repo, origin) = setup_repo_with_remote();
        let wave_name = "agent-embedding";
        let worktree = wave_worktree_path(&main_repo, wave_name);

        run_git(&main_repo, &["checkout", "-b", wave_name]);
        write_file(&main_repo.join("shared.txt"), "wave branch change\n");
        run_git(&main_repo, &["add", "."]);
        run_git(&main_repo, &["commit", "-m", "wave branch change"]);
        run_git(&main_repo, &["checkout", "main"]);

        run_git(
            &main_repo,
            &[
                "worktree",
                "add",
                worktree.to_str().unwrap_or(""),
                wave_name,
            ],
        );

        let collaborator = main_repo
            .parent()
            .expect("main repo parent")
            .join("collaborator-conflict");
        run_git(
            main_repo.parent().expect("main repo parent"),
            &[
                "clone",
                origin.to_str().unwrap_or(""),
                collaborator.to_str().unwrap_or(""),
            ],
        );
        run_git(&collaborator, &["config", "user.email", "test@example.com"]);
        run_git(&collaborator, &["config", "user.name", "Test User"]);
        write_file(&collaborator.join("shared.txt"), "main branch change\n");
        run_git(&collaborator, &["add", "."]);
        run_git(&collaborator, &["commit", "-m", "main branch change"]);
        run_git(&collaborator, &["push"]);

        let (resolved_worktree, resolved_branch) =
            ensure_wave_worktree(&main_repo, wave_name).expect("reuse existing worktree");

        assert_eq!(resolved_worktree, worktree.to_string_lossy());
        assert_eq!(resolved_branch, wave_name);
        let shared =
            std::fs::read_to_string(worktree.join("shared.txt")).expect("read shared file");
        assert_eq!(shared, "wave branch change\n");
    }
}
