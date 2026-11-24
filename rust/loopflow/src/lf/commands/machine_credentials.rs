//! Credential command framing; acquisition and transfer live with Machine operations.
use crate::lf::MachineCredentialCommand;
use loopflow_execution::machine::credentials;
use loopflow_execution::ops::human_session;
use std::io::Read;

pub(super) async fn receive(cmd: &MachineCredentialCommand) -> anyhow::Result<()> {
    match cmd {
        MachineCredentialCommand::Restart { id, task } => {
            let store = super::session::open_shared_store().await?;
            let session = if *task {
                let task = store
                    .sqlite
                    .resolve_task_id(id, None)?
                    .ok_or_else(|| anyhow::anyhow!("Task {id} was not found"))?;
                store
                    .task_primary(&task)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Task {id} has no primary Session"))?
            } else {
                human_session::session_by_id(&store, id)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("Session {id} was not found"))?
            };
            human_session::restart_provider(&store.sqlite, &session)?;
            println!("{}", session.id);
        }
        MachineCredentialCommand::Receive => {
            let mut bytes = Vec::new();
            std::io::stdin()
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            credentials::receive(&bytes).await?;
        }
    }
    Ok(())
}
