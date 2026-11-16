//! Credential command framing; acquisition and transfer live with Machine operations.
use crate::lf::MachineCredentialCommand;
use loopflow_execution::machine::credentials;
use std::io::Read;

pub(super) async fn receive(cmd: &MachineCredentialCommand) -> anyhow::Result<()> {
    match cmd {
        MachineCredentialCommand::Inspect {
            provider,
            login,
            full,
        } => {
            println!("{}", credentials::inspect(*provider, login, *full).await?);
        }
        MachineCredentialCommand::Receive => {
            let mut bytes = Vec::new();
            std::io::stdin()
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            credentials::receive(&bytes).await?;
        }
        MachineCredentialCommand::Renew => credentials::renew().await?,
    }
    Ok(())
}
