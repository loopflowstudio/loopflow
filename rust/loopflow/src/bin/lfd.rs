use clap::{Parser, Subcommand};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::EnvFilter;

use loopflow::lfd;
use loopflow::store;

const MAX_LOG_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
struct RotatingLog {
    state: Arc<Mutex<RotatingLogState>>,
}

#[derive(Debug)]
struct RotatingLogState {
    path: PathBuf,
    file: Option<File>,
    bytes: u64,
    max_bytes: u64,
}

#[derive(Debug)]
struct RotatingLogWriter {
    state: Arc<Mutex<RotatingLogState>>,
}

impl RotatingLog {
    fn open(path: PathBuf, max_bytes: u64) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let bytes = file.metadata()?.len();
        Ok(Self {
            state: Arc::new(Mutex::new(RotatingLogState {
                path,
                file: Some(file),
                bytes,
                max_bytes,
            })),
        })
    }
}

impl RotatingLogState {
    fn rotate_before(&mut self, incoming_bytes: usize) -> io::Result<()> {
        if self.bytes == 0 || self.bytes.saturating_add(incoming_bytes as u64) <= self.max_bytes {
            return Ok(());
        }

        self.file.take();
        let previous = self.path.with_extension("log.previous");
        if previous.exists() {
            std::fs::remove_file(&previous)?;
        }
        if self.path.exists() {
            std::fs::rename(&self.path, previous)?;
        }
        self.file = Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?,
        );
        self.bytes = 0;
        Ok(())
    }
}

impl Write for RotatingLogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("lfd log lock poisoned"))?;
        state.rotate_before(buf.len())?;
        let written = state
            .file
            .as_mut()
            .expect("rotating log always owns an open file")
            .write(buf)?;
        state.bytes = state.bytes.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.state
            .lock()
            .map_err(|_| io::Error::other("lfd log lock poisoned"))?
            .file
            .as_mut()
            .expect("rotating log always owns an open file")
            .flush()
    }
}

impl<'a> MakeWriter<'a> for RotatingLog {
    type Writer = RotatingLogWriter;

    fn make_writer(&'a self) -> Self::Writer {
        RotatingLogWriter {
            state: Arc::clone(&self.state),
        }
    }
}

fn daemon_log_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("LF_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|path| path.join(".lf")))
        .ok_or_else(|| anyhow::anyhow!("cannot resolve Loopflow home for lfd logs"))?;
    Ok(home.join("logs/lfd.log"))
}

fn init_tracing() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lfd=info,loopflow=info"));
    let writer = RotatingLog::open(daemon_log_path()?, MAX_LOG_BYTES)?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(writer)
        .init();
    Ok(())
}

#[derive(Parser)]
#[command(name = "lfd")]
#[command(about = "Loopflow Home daemon: Wave startup, webhook ingress, and liveness")]
#[command(version = loopflow::build_info::BUILD_VERSION)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon, start assigned Waves, accept signed deliveries, and
    /// serve liveness probes. Always opens the store.
    Serve {
        /// Address to bind (default 127.0.0.1:8080; non-loopback requires
        /// LF_LFD_ALLOW_NON_LOOPBACK=1).
        #[arg(long, default_value = "127.0.0.1:8080")]
        addr: String,

        /// Repository root containing wave/ endpoint files, for /status.
        /// Defaults to the Loopflow repo root of this checkout.
        #[arg(long)]
        repo: Option<String>,

        /// Internal one-shot id used by `lf start` to correlate daemon startup.
        #[arg(long, hide = true, requires_all = ["startup_receipt", "startup_socket"])]
        startup_attempt: Option<String>,

        /// Internal durable startup receipt written before signaling the caller.
        #[arg(long, hide = true, requires_all = ["startup_attempt", "startup_socket"])]
        startup_receipt: Option<PathBuf>,

        /// Internal Unix socket used to signal the waiting `lf start` process.
        #[arg(long, hide = true, requires_all = ["startup_attempt", "startup_receipt"])]
        startup_socket: Option<PathBuf>,

        /// Receipt capability for keeper health startup during install settlement.
        #[arg(long, hide = true)]
        install_switch: Option<String>,
    },
    /// Render and load the launchd (macOS) or systemd user (Linux) service so
    /// the daemon stays up across reboots. Service files never carry secrets —
    /// source Linear credentials from Doppler before starting.
    Install {
        /// Address the installed daemon binds.
        #[arg(long, default_value = "127.0.0.1:8080")]
        addr: String,

        /// Repository whose worktrees the machine daemon owns. Defaults to the
        /// Loopflow repo root of this checkout.
        #[arg(long)]
        repo: Option<String>,
    },
    /// Report whether the daemon service is loaded and running.
    Status,
    /// Unload and remove the daemon service file.
    Uninstall,
}

fn main() -> anyhow::Result<()> {
    loopflow::machine_install::dispatch_entry_gate(
        &loopflow::machine_install::ArtifactRole::Daemon,
    )?;
    let cli = Cli::parse();
    let install_switch = match &cli.command {
        Commands::Serve { install_switch, .. } => install_switch.as_deref(),
        _ => None,
    };
    loopflow::machine_install::authorize_current_for_switch(
        &loopflow::machine_install::ArtifactRole::Daemon,
        install_switch,
    )?;
    init_tracing()?;
    let rt = tokio::runtime::Runtime::new()?;

    match cli.command {
        Commands::Serve {
            addr,
            repo,
            startup_attempt,
            startup_receipt,
            startup_socket,
            install_switch: _,
        } => {
            let startup = match (startup_attempt, startup_receipt, startup_socket) {
                (Some(attempt_id), Some(receipt_path), Some(socket_path)) => {
                    Some(lfd::StartupSignal {
                        attempt_id,
                        receipt_path,
                        socket_path,
                    })
                }
                (None, None, None) => None,
                _ => unreachable!("clap requires the complete startup signal"),
            };
            let result = run_serve(&rt, addr, repo, startup.clone());
            if let (Err(error), Some(startup)) = (&result, startup) {
                if let Err(report_error) = rt.block_on(startup.report(lfd::StartupState::Failed {
                    reason: error.to_string(),
                })) {
                    tracing::warn!(%report_error, "could not publish failed lfd startup receipt");
                }
            }
            result
        }
        Commands::Install { addr, repo } => {
            let lfd_path = std::env::current_exe()
                .map_err(|e| anyhow::anyhow!("cannot locate the lfd binary: {e}"))?;
            let repo_root = match repo {
                Some(path) => std::path::PathBuf::from(path),
                None => loopflow::lf::commands::util::find_repo_root()
                    .map_err(|e| anyhow::anyhow!("cannot find repo root: {e}"))?,
            };
            let spec = lfd::service::ServiceSpec {
                lfd_path,
                addr,
                repo_root,
                lf_home: std::env::var_os("LF_HOME").map(std::path::PathBuf::from),
                db_path: std::env::var_os("LF_DB_PATH").map(std::path::PathBuf::from),
                path_env: std::env::var("PATH").ok(),
                doppler_project: std::env::var("DOPPLER_PROJECT").ok(),
                doppler_config: std::env::var("DOPPLER_CONFIG").ok(),
            };
            let file = lfd::service::install(&spec)?;
            println!(
                "installed lfd service ({}): {}",
                file.platform,
                file.path.display()
            );
            println!("credentials are resolved from environment or Doppler at daemon startup");
            Ok(())
        }
        Commands::Status => {
            println!("{}", lfd::service::status()?);
            Ok(())
        }
        Commands::Uninstall => {
            let path = lfd::service::uninstall()?;
            println!("removed lfd service file: {}", path.display());
            Ok(())
        }
    }
}

fn run_serve(
    rt: &tokio::runtime::Runtime,
    addr: String,
    repo: Option<String>,
    startup: Option<lfd::StartupSignal>,
) -> anyhow::Result<()> {
    let repo_root = match repo {
        Some(path) => std::path::PathBuf::from(path),
        None => loopflow::lf::commands::util::find_repo_root()
            .map_err(|e| anyhow::anyhow!("cannot find repo root: {e}"))?,
    };

    let socket: std::net::SocketAddr = addr
        .parse()
        .map_err(|error| anyhow::anyhow!("invalid --addr {addr:?}: {error}"))?;

    // The store is always open: the delivery inbox lives there. A machine with
    // no registry yet cannot host the daemon.
    let store = rt.block_on(async {
        store::open_existing_store().await.ok_or_else(|| {
            anyhow::anyhow!("no Loopflow registry on this machine; run `lf` once to create it")
        })
    })?;
    let store = std::sync::Arc::new(store);

    // Linear config is optional: the daemon runs without it, but
    // /linear/webhook returns 503 until both variables are configured.
    let linear = read_linear_config();
    rt.block_on(lfd::serve(repo_root, socket, store, linear, startup))
}

/// Read Linear webhook config from env. `None` when either the secret or the
/// viewer id is unset/empty — the daemon runs but `/linear/webhook` returns 503.
fn read_linear_config() -> Option<lfd::LinearConfig> {
    let secret = std::env::var("LF_LINEAR_WEBHOOK_SECRET").ok()?;
    let viewer_id = std::env::var("LF_LINEAR_VIEWER_ID").ok()?;
    if secret.is_empty() || viewer_id.is_empty() {
        return None;
    }
    Some(lfd::LinearConfig {
        secret: std::sync::Arc::new(secret.into_bytes()),
        viewer_id: std::sync::Arc::new(viewer_id),
    })
}

#[cfg(test)]
mod tests {
    use super::{MakeWriter, RotatingLog};
    use std::io::Write;

    #[test]
    fn daemon_log_keeps_only_one_bounded_predecessor() {
        let directory = tempfile::tempdir().expect("create temp directory");
        let path = directory.path().join("lfd.log");
        let log = RotatingLog::open(path.clone(), 8).expect("open log");

        log.make_writer().write_all(b"12345678").expect("write log");
        log.make_writer().write_all(b"next").expect("rotate log");
        log.make_writer()
            .write_all(b"56789")
            .expect("rotate log again");

        assert_eq!(std::fs::read(&path).expect("read current log"), b"56789");
        assert_eq!(
            std::fs::read(path.with_extension("log.previous")).expect("read previous log"),
            b"next"
        );
    }
}
