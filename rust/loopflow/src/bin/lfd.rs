use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use loopflow::lfd;
use loopflow::store;

#[derive(Parser)]
#[command(name = "lfd")]
#[command(about = "Loopflow Home daemon: durable webhook ingress + liveness")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the daemon: accept signed Linear deliveries into the durable inbox
    /// and serve liveness probes. Always opens the store.
    Serve {
        /// Address to bind (default 127.0.0.1:8080; non-loopback requires
        /// LF_LFD_AUTH_TOKEN).
        #[arg(long, default_value = "127.0.0.1:8080")]
        addr: String,

        /// Repository root containing wave/ endpoint files, for /status.
        /// Defaults to the Loopflow repo root of this checkout.
        #[arg(long)]
        repo: Option<String>,
    },
    /// Render and load the launchd (macOS) or systemd user (Linux) service so
    /// the daemon stays up across reboots. Service files never carry secrets —
    /// source Linear credentials from Doppler before starting.
    Install {
        /// Address the installed daemon binds.
        #[arg(long, default_value = "127.0.0.1:8080")]
        addr: String,
    },
    /// Report whether the daemon service is loaded and running.
    Status,
    /// Unload and remove the daemon service file.
    Uninstall,
}

fn main() -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lfd=info,loopflow=info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .without_time()
        .init();

    let cli = Cli::parse();
    let rt = tokio::runtime::Runtime::new()?;

    match cli.command {
        Commands::Serve { addr, repo } => {
            let repo_root = match repo {
                Some(path) => std::path::PathBuf::from(path),
                None => loopflow::lf::commands::util::find_repo_root()
                    .map_err(|e| anyhow::anyhow!("cannot find repo root: {e}"))?,
            };

            let socket: std::net::SocketAddr = addr
                .parse()
                .map_err(|error| anyhow::anyhow!("invalid --addr {addr:?}: {error}"))?;

            // The store is always open: the delivery inbox lives there. A
            // machine with no registry yet cannot host the daemon.
            let store = rt.block_on(async {
                store::open_existing_store().await.ok_or_else(|| {
                    anyhow::anyhow!(
                        "no Loopflow registry on this machine; run `lf` once to create it"
                    )
                })
            })?;
            let store = std::sync::Arc::new(store);

            // Linear config is optional: the daemon runs without it, but
            // /linear/webhook returns 503 until LF_LINEAR_WEBHOOK_SECRET and
            // LF_LINEAR_VIEWER_ID are both set.
            let linear = read_linear_config();

            rt.block_on(lfd::serve(repo_root, socket, store, linear))
        }
        Commands::Install { addr } => {
            let lfd_path = std::env::current_exe()
                .map_err(|e| anyhow::anyhow!("cannot locate the lfd binary: {e}"))?;
            let spec = lfd::service::ServiceSpec {
                lfd_path,
                addr,
                lf_home: std::env::var_os("LF_HOME").map(std::path::PathBuf::from),
                db_path: std::env::var_os("LF_DB_PATH").map(std::path::PathBuf::from),
            };
            let file = lfd::service::install(&spec)?;
            println!(
                "installed lfd service ({}): {}",
                file.platform,
                file.path.display()
            );
            println!(
                "source Linear credentials before the daemon starts, e.g.:\n  \
                 doppler run -- launchctl setenv LF_LINEAR_WEBHOOK_SECRET $LF_LINEAR_WEBHOOK_SECRET"
            );
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
