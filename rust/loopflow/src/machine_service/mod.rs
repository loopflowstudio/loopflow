//! Standalone transport for shared reads and commands; owns no execution.
pub mod admission;
pub mod client;
pub mod installation;
mod remote;
mod service;
pub mod wire;

use std::path::PathBuf;

pub fn socket_path() -> anyhow::Result<PathBuf> {
    // Reuse the private, short per-Machine transport directory and its checks.
    Ok(crate::transport::private_directory()?.join("machine.sock"))
}

pub fn run(remote_port: Option<u16>) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(service::run(remote_port))
}
