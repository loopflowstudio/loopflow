//! Standalone transport for shared reads and commands; owns no execution.
mod answers;
pub(crate) mod boundary;
pub mod connect;
mod maintenance;
pub mod mobile;
mod outbox;
mod peers;
mod recovery;
mod remote;
pub mod schema;
mod service;
mod subscriptions;
mod ticket;
mod wake;
pub mod wire;

pub use loopflow_execution::machine::socket_path;

/// Each terminal bridge holds two descriptors, and a supervisor's default soft
/// limit is low. macOS refuses a soft limit above `OPEN_MAX` even when the hard
/// limit is unlimited.
fn raise_descriptor_limit() {
    const WANTED: libc::rlim_t = 10240;
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: both calls read or write only the rlimit passed by reference.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) == 0 && limit.rlim_cur < WANTED {
            limit.rlim_cur = WANTED.min(limit.rlim_max);
            if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
                tracing::warn!("Server could not raise its descriptor limit");
            }
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    raise_descriptor_limit();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(service::run())
}
