//! Offline development copies. The inherited OS sandbox, rather than copied
//! database identity or an environment flag, owns file/process/network authority.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

const MARKER: &str = "clone.json";
const ROOT_ENV: &str = "LF_CLONE_ROOT";
static SELECTED: OnceLock<CloneManifest> = OnceLock::new();
const HISTORY_LIMIT: &str = "Copied Session history is read-only; Codex/Claude continuation and retry are unavailable. Start a new offline conversation inside the clone.";

#[derive(Debug, Subcommand)]
pub enum CloneCommand {
    /// Snapshot a source Machine without copying credentials or live authority
    Create {
        directory: PathBuf,
        #[arg(long)]
        source: PathBuf,
        /// Explicit Codex history home; credentials and configuration are excluded
        #[arg(long)]
        codex_home: Option<PathBuf>,
        /// Explicit Claude history home; credentials and configuration are excluded
        #[arg(long)]
        claude_home: Option<PathBuf>,
    },
    /// Run lf (or a Desktop executable after --) inside the offline clone
    Run {
        directory: PathBuf,
        #[arg(last = true)]
        command: Vec<String>,
    },
    /// Show the snapshot inventory and continuation limitations
    Show { directory: PathBuf },
    /// Remove only this owned disposable directory (close its clients first)
    Dispose { directory: PathBuf },
}

impl CloneCommand {
    /// Whether this command changes state, and so records a Process.
    pub fn changes_state(&self) -> bool {
        match self {
            Self::Show { .. } => false,
            Self::Create { .. } | Self::Run { .. } | Self::Dispose { .. } => true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CloneManifest {
    version: u32,
    directory: PathBuf,
    source: PathBuf,
    provider_sources: BTreeMap<String, PathBuf>,
    build_identity: String,
    ready: bool,
    sessions: BTreeSet<String>,
    artifacts: BTreeMap<String, usize>,
    unavailable: Vec<String>,
}

pub fn run(command: &CloneCommand) -> Result<()> {
    match command {
        CloneCommand::Create {
            directory,
            source,
            codex_home,
            claude_home,
        } => {
            create(
                directory,
                source,
                codex_home.as_deref(),
                claude_home.as_deref(),
            )?;
            show(directory)
        }
        CloneCommand::Show { directory } => show(directory),
        CloneCommand::Run { directory, command } => launch(directory, command),
        CloneCommand::Dispose { directory } => {
            let manifest = read_manifest(directory)?;
            fs::remove_dir_all(&manifest.directory).context("remove owned clone")
        }
    }
}

fn read_manifest(directory: &Path) -> Result<CloneManifest> {
    let directory = directory
        .canonicalize()
        .context("resolve clone directory")?;
    let manifest: CloneManifest = serde_json::from_slice(&fs::read(directory.join(MARKER))?)?;
    if manifest.version != 1 || manifest.directory != directory {
        bail!("Clone was moved or its marker is incompatible; recreate it with lf clone create");
    }
    let production = crate::installation::account_home()?.join(".lf");
    if directory == production || manifest.source.starts_with(&directory) {
        bail!("Clone directory overlaps its source or production Machine");
    }
    for name in ["loopflow.db", "home", "bin", "tmp", "archive"] {
        let path = directory.join(name);
        if path.exists() && !path.canonicalize()?.starts_with(&directory) {
            bail!("Clone path escapes its directory: {name}");
        }
    }
    if crate::store::same_database_file(
        &directory.join("loopflow.db"),
        &manifest.source.join("loopflow.db"),
    )? || crate::store::same_database_file(
        &directory.join("loopflow.db"),
        &production.join("loopflow.db"),
    )? {
        bail!("Clone database aliases its source or production database");
    }
    Ok(manifest)
}

fn save_manifest(manifest: &CloneManifest) -> Result<()> {
    fs::write(
        manifest.directory.join(MARKER),
        serde_json::to_vec_pretty(manifest)?,
    )?;
    Ok(())
}

fn show(directory: &Path) -> Result<()> {
    println!(
        "{}",
        serde_json::to_string_pretty(&read_manifest(directory)?)?
    );
    Ok(())
}

fn create(
    directory: &Path,
    source: &Path,
    codex: Option<&Path>,
    claude: Option<&Path>,
) -> Result<()> {
    require_platform()?;
    let source = source.canonicalize().context("resolve source Machine")?;
    let directory = crate::store::canonicalize_with_missing_tail(directory)?;
    if directory.starts_with(&source) || source.starts_with(&directory) {
        bail!("Clone must be outside the source Machine");
    }
    let provider_sources = [("codex", codex), ("claude", claude)]
        .into_iter()
        .filter_map(|(name, path)| path.map(|path| (name, path)))
        .map(|(name, path)| {
            Ok((
                name.to_string(),
                crate::store::canonicalize_with_missing_tail(path)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    for source in provider_sources.values() {
        if directory.starts_with(source) || source.starts_with(&directory) {
            bail!("Clone must be outside provider history homes");
        }
    }
    // create_dir is exclusive: never adopt, repair, or overwrite an existing home.
    fs::create_dir(&directory).context("create new clone directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    }
    let mut manifest = CloneManifest {
        version: 1, directory, source, provider_sources,
        build_identity: crate::build_info::source_identity().to_string(),
        ready: false, sessions: BTreeSet::new(), artifacts: BTreeMap::new(),
        unavailable: vec![HISTORY_LIMIT.into(),
            "Offline only: external planning, Git publication, scheduling and real accounts are unavailable; source repositories are read-only.".into(),
            "Provider files are best-effort retained history, not an atomic provider snapshot. Missing, changing and symlinked files are reported. Native continuation is not proved.".into()],
    };
    save_manifest(&manifest)?;
    require_nested_sandbox(&manifest)?;
    // Even SQLite's auxiliary files and source WAL are outside write authority.
    sandbox(&manifest, false)?;
    for name in [
        "home",
        "bin",
        "tmp",
        "archive",
        "home/.codex",
        "home/.claude",
    ] {
        fs::create_dir_all(manifest.directory.join(name))?;
    }
    // Migrating the snapshot spills SQLite statement journals and temp tables
    // into TMPDIR; inside the sandbox only the clone's own tmp is writable.
    std::env::set_var("TMPDIR", manifest.directory.join("tmp"));
    fs::copy(std::env::current_exe()?, manifest.directory.join("bin/lf"))?;
    crate::store::copy::snapshot(
        &manifest.source.join("loopflow.db"),
        &manifest.directory.join("loopflow.db"),
    )?;
    let destination = Connection::open(manifest.directory.join("loopflow.db"))?;
    crate::store::migrations::prepare_clone_sqlite(&destination)?;
    manifest.sessions = destination
        .prepare("SELECT id FROM agent_sessions ORDER BY id")?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    sanitize(&destination)?;
    drop(destination);
    for (label, root, children, jsonl_only) in [
        (
            "captures",
            Some(manifest.source.clone()),
            vec!["runs"],
            false,
        ),
        (
            "codex",
            codex.map(Path::to_path_buf),
            vec!["sessions", "archived_sessions"],
            true,
        ),
        (
            "claude",
            claude.map(Path::to_path_buf),
            vec!["projects"],
            true,
        ),
    ] {
        let mut count = 0;
        if let Some(root) = root {
            for child in children {
                let from = root.join(child);
                let to = if label == "captures" {
                    manifest.directory.join(child)
                } else {
                    manifest.directory.join("archive").join(label).join(child)
                };
                copy_history(
                    &from,
                    &to,
                    jsonl_only,
                    &mut count,
                    &mut manifest.unavailable,
                )?;
            }
        } else {
            manifest
                .unavailable
                .push(format!("{label}: no history home supplied"));
        }
        manifest.artifacts.insert(label.to_string(), count);
    }
    manifest.ready = true;
    save_manifest(&manifest)
}

fn sanitize(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA secure_delete=ON;
        BEGIN;
        UPDATE agent_sessions SET agent_process_id=NULL;
        UPDATE processes SET pid=NULL, os_started_at=NULL, endpoint=NULL,
            attached_lf_process_id=NULL, attachment_token=NULL;
        UPDATE machines SET route='copied',label=NULL;
        INSERT INTO machines(id,route,created_at,observed_at)
            VALUES('home_' || lower(hex(randomblob(16))), 'local', unixepoch(), unixepoch());
        DELETE FROM session_activity;
        DELETE FROM provider_routes;
        DELETE FROM provider_tokens;
        UPDATE provider_accounts SET home=NULL, credential_state='missing', routing_state='disabled';
        COMMIT;
        VACUUM;")?;
    Ok(())
}

fn copy_history(
    source: &Path,
    target: &Path,
    jsonl_only: bool,
    count: &mut usize,
    unavailable: &mut Vec<String>,
) -> Result<()> {
    let metadata = match fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(_) => {
            unavailable.push(format!("History unavailable: {}", source.display()));
            return Ok(());
        }
    };
    if metadata.file_type().is_symlink() {
        unavailable.push(format!("Symlink history omitted: {}", source.display()));
    } else if metadata.is_dir() {
        let entries = match fs::read_dir(source) {
            Ok(entries) => entries,
            Err(_) => {
                unavailable.push(format!(
                    "History directory unavailable: {}",
                    source.display()
                ));
                return Ok(());
            }
        };
        for entry in entries {
            let entry = entry?;
            copy_history(
                &entry.path(),
                &target.join(entry.file_name()),
                jsonl_only,
                count,
                unavailable,
            )?;
        }
    } else if metadata.is_file()
        && (!jsonl_only || source.extension().is_some_and(|ext| ext == "jsonl"))
    {
        fs::create_dir_all(target.parent().context("history parent")?)?;
        match fs::copy(source, target) {
            Ok(_) => {
                *count += 1;
                if !fs::metadata(source).is_ok_and(|after| {
                    after.len() == metadata.len()
                        && after.modified().ok() == metadata.modified().ok()
                }) {
                    unavailable.push(format!(
                        "History changed during capture: {}",
                        source.display()
                    ));
                }
            }
            Err(_) => unavailable.push(format!("History copy unavailable: {}", source.display())),
        }
    }
    Ok(())
}

fn launch(directory: &Path, arguments: &[String]) -> Result<()> {
    let manifest = read_manifest(directory)?;
    if !manifest.ready {
        bail!("Clone creation did not finish; dispose and recreate it");
    }
    require_platform()?;
    let executable = manifest.directory.join("bin/lf");
    let mut command = Command::new("/usr/bin/sandbox-exec");
    command.args(["-p", &policy(&manifest, true)?]);
    if arguments.is_empty() {
        command.arg(&executable);
    } else {
        command.args(arguments);
    }
    command.env_clear();
    for (key, value) in environment(&manifest) {
        command.env(key, value);
    }
    command.current_dir(&manifest.directory);
    let status = command.status().context("run isolated clone")?;
    if !status.success() {
        bail!("Clone command exited with {status}");
    }
    Ok(())
}

fn require_nested_sandbox(manifest: &CloneManifest) -> Result<()> {
    // Older macOS releases reject a second sandbox application. Never replace
    // kernel enforcement with an inherited environment flag on those hosts.
    let profile = policy(manifest, true)?;
    let result = Command::new("/usr/bin/sandbox-exec")
        .args([
            "-p",
            &profile,
            "/usr/bin/sandbox-exec",
            "-p",
            &profile,
            "/usr/bin/true",
        ])
        .output()
        .context("probe inherited clone sandbox")?;
    if !result.status.success() {
        bail!("This host cannot re-enter the inherited clone sandbox (macOS 15 is unsupported; verified on macOS 26). No source data was copied. Dispose this incomplete clone and use a capable host; isolation is never bypassed.");
    }
    Ok(())
}

fn environment(manifest: &CloneManifest) -> BTreeMap<&'static str, String> {
    let root = &manifest.directory;
    [
        (ROOT_ENV, root.display().to_string()),
        ("LF_HOME", root.display().to_string()),
        ("LF_BIN", root.join("bin/lf").display().to_string()),
        ("HOME", root.join("home").display().to_string()),
        ("TMPDIR", root.join("tmp").display().to_string()),
        ("CODEX_HOME", root.join("home/.codex").display().to_string()),
        (
            "CLAUDE_CONFIG_DIR",
            root.join("home/.claude").display().to_string(),
        ),
        (
            "PATH",
            format!(
                "{}:/usr/bin:/bin:/usr/sbin:/sbin",
                root.join("bin").display()
            ),
        ),
        ("GIT_CONFIG_NOSYSTEM", "1".into()),
        ("GIT_CONFIG_GLOBAL", "/dev/null".into()),
        ("GIT_OPTIONAL_LOCKS", "0".into()),
        ("SHELL", "/bin/zsh".into()),
        ("TERM", "xterm-256color".into()),
        ("LANG", "en_US.UTF-8".into()),
    ]
    .into_iter()
    .collect()
}

/// Pin marked Machines before any installation, journal or repository dispatch.
/// This also covers direct `LF_HOME=<clone> lf` and Desktop's nested commands.
pub fn enter_selected() -> Result<()> {
    let nested = std::env::var_os(ROOT_ENV).is_some();
    let root = std::env::var_os(ROOT_ENV)
        .map(PathBuf::from)
        .or_else(|| {
            // Selecting the copied executable is selecting its clone, even from a
            // production shell that has not set LF_HOME for it.
            let executable = std::env::current_exe().ok()?;
            let directory = executable.parent()?.parent()?;
            directory
                .join(MARKER)
                .exists()
                .then(|| directory.to_path_buf())
        })
        .or_else(|| {
            let home = crate::store::lf_home_dir();
            home.join(MARKER).exists().then_some(home)
        });
    let Some(root) = root else {
        return Ok(());
    };
    let manifest = read_manifest(&root)?;
    if !manifest.ready {
        bail!("Clone creation did not finish; dispose and recreate it");
    }
    sandbox(&manifest, true)?;
    // A direct LF_HOME selection may originate in an authenticated production
    // shell. Only a clone child's own lf ancestry is useful across this entry.
    for (key, _) in std::env::vars_os() {
        if !(nested && key.to_string_lossy().starts_with("LF_"))
            && key != "RUST_BACKTRACE"
            && key != "RUST_LOG"
        {
            std::env::remove_var(key);
        }
    }
    for (key, value) in environment(&manifest) {
        std::env::set_var(key, value);
    }
    let _ = SELECTED.set(manifest);
    Ok(())
}

pub(crate) fn require_writable_session(id: &str) -> Result<()> {
    if let Some(reason) = session_unavailable(id)? {
        bail!(reason);
    }
    Ok(())
}

pub(crate) fn is_selected() -> bool {
    SELECTED
        .get()
        .is_some_and(|manifest| manifest.directory == crate::store::lf_home_dir())
}

pub(crate) fn session_unavailable(id: &str) -> Result<Option<String>> {
    let home = crate::store::lf_home_dir();
    if let Some(manifest) = SELECTED.get().filter(|manifest| manifest.directory == home) {
        return Ok(manifest
            .sessions
            .contains(id)
            .then(|| HISTORY_LIMIT.to_string()));
    }
    if !home.join(MARKER).exists() {
        return Ok(None);
    }
    let manifest = read_manifest(&home)?;
    Ok(manifest
        .sessions
        .contains(id)
        .then(|| HISTORY_LIMIT.to_string()))
}

fn require_platform() -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!(
            "Isolated clones currently require macOS's inherited sandbox; no unsandboxed fallback"
        );
    }
    Ok(())
}

fn policy(manifest: &CloneManifest, runtime: bool) -> Result<String> {
    let quote = |path: &Path| serde_json::to_string(&path.display().to_string());
    let root = quote(&manifest.directory)?;
    // Unix socket paths cannot fit arbitrary clone paths. Permit only this
    // clone's private transport directory, never the source Machine's sockets.
    let transport = quote(&crate::transport::directory(&manifest.directory)?)?;
    let mut policy = format!(
        r#"(version 1)
(allow default)
(deny file-write*)
(allow file-write* (subpath {root}) (literal "/dev/null") (literal "/dev/ptmx") (regex #"^/dev/(ttys|pty)"))
(allow file-write* (subpath {transport}))
(deny network*)
(allow network-bind network-inbound (local unix-socket (subpath {root})))
(allow network-outbound (remote unix-socket (subpath {root})))
(allow network-bind network-inbound (local unix-socket (subpath {transport})))
(allow network-outbound (remote unix-socket (subpath {transport})))
(deny signal)
(allow signal (target same-sandbox))
(deny process-info-setcontrol)
(allow process-info-setcontrol (target same-sandbox))
(deny appleevent-send)
(deny mach-lookup)
(allow mach-lookup (global-name "com.apple.system.opendirectoryd.libinfo")
    (global-name "com.apple.windowserver.active") (global-name "com.apple.FontObjectsServer"))
"#
    );
    if runtime {
        for path in manifest.provider_sources.values() {
            policy.push_str(&format!(
                "(deny file-read-data (subpath {}))\n",
                quote(path)?
            ));
        }
        // File restrictions also stop provider fallback to OS-account credentials.
        for path in [
            manifest.source.clone(),
            crate::installation::account_home()?.join(".lf"),
            crate::installation::account_home()?.join(".codex"),
            crate::installation::account_home()?.join(".claude"),
            crate::installation::account_home()?.join(".ssh"),
            crate::installation::account_home()?.join("Library/Keychains"),
        ] {
            if !manifest.directory.starts_with(&path) {
                policy.push_str(&format!(
                    "(deny file-read-data (subpath {}))\n",
                    quote(&path)?
                ));
            }
        }
        policy.push_str(&format!(
            "(deny file-write* (subpath {}) (literal {}))\n",
            quote(&manifest.directory.join("archive"))?,
            quote(&manifest.directory.join(MARKER))?
        ));
    }
    Ok(policy)
}

#[cfg(target_os = "macos")]
fn sandbox(manifest: &CloneManifest, runtime: bool) -> Result<()> {
    use std::ffi::{CStr, CString};
    unsafe extern "C" {
        fn sandbox_init(
            profile: *const libc::c_char,
            flags: u64,
            error: *mut *mut libc::c_char,
        ) -> libc::c_int;
        fn sandbox_free_error(error: *mut libc::c_char);
    }
    let policy = CString::new(policy(manifest, runtime)?)?;
    let mut error = std::ptr::null_mut();
    // SAFETY: the NUL-terminated profile and error output remain live for the call.
    if unsafe { sandbox_init(policy.as_ptr(), 0, &mut error) } != 0 {
        let message = if error.is_null() {
            "unknown sandbox error".into()
        } else {
            // SAFETY: sandbox_init owns this NUL-terminated error until freed below.
            let message = unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned();
            // SAFETY: error is the allocation returned by sandbox_init.
            unsafe { sandbox_free_error(error) };
            message
        };
        bail!("Cannot establish clone isolation: {message}");
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn sandbox(_manifest: &CloneManifest, _runtime: bool) -> Result<()> {
    require_platform()
}
