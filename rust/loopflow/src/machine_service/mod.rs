//! Standalone transport for shared reads and commands; owns no execution.
pub mod client;
pub mod installation;
mod service;
pub mod wire;

use std::path::PathBuf;

pub fn socket_path() -> anyhow::Result<PathBuf> {
    // Reuse the private, short per-Machine transport directory and its checks.
    Ok(crate::transport::private_directory()?.join("machine.sock"))
}

pub fn run() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(service::run())
}
