//! Credential command framing; acquisition and transfer live with Machine operations.
use crate::lf::MachineCredentialCommand;
use loopflow_execution::machine::credentials;
use std::io::Read;

pub(super) async fn receive(cmd: &MachineCredentialCommand) -> anyhow::Result<()> {
    let MachineCredentialCommand::Receive = cmd;
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    credentials::receive(&bytes).await
}
