//! Standalone transport for shared reads and commands; owns no execution.
pub mod client;
pub mod mobile;
mod remote;
mod service;
pub mod wire;

use std::path::PathBuf;

pub fn socket_path() -> anyhow::Result<PathBuf> {
    // Keep the installed endpoint stable: relays from before the namespace
    // cutover must reconnect to this same listener after an update.
    Ok(crate::transport::private_directory()?.join("machine.sock"))
}

pub fn run() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(service::run())
}
