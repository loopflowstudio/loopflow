//! Machine endpoint, protocol values and blocking client. No request dispatch.
pub mod client;
pub mod mobile;
pub mod schema;
pub(crate) mod ssh;
pub mod wire;

use std::path::PathBuf;

pub fn socket_path() -> anyhow::Result<PathBuf> {
    // Keep the installed endpoint stable across upgrades.
    Ok(crate::transport::private_directory()?.join("machine.sock"))
}
