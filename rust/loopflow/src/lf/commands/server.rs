//! `lf server`: the resident server, its readiness and protocol, and the
//! entry another Machine reaches it through.
use crate::lf::ServerCommand;

pub fn run(command: &ServerCommand) -> anyhow::Result<()> {
    match command {
        ServerCommand::Receipt {
            request_id, task, ..
        } => {
            let read = super::read::Read::CreationGet {
                request_id: request_id.clone(),
                task: task.clone(),
            };
            println!("{}", serde_json::to_string_pretty(&read.read()?)?);
            Ok(())
        }
        ServerCommand::Run => crate::server::run(),
        ServerCommand::Connect => crate::server::connect::run(),
        ServerCommand::Status => {
            println!(
                "{}",
                serde_json::to_string(&loopflow_execution::machine::client::status()?)?
            );
            Ok(())
        }
        ServerCommand::Diagnostics { json } => {
            let mut connection = loopflow_execution::machine::client::Connection::open()?;
            let answer = connection.request(crate::server::wire::Operation::Server(
                crate::server::wire::ServerOperation::ServerDiagnostics,
            ))?;
            let diagnostics: crate::server::wire::ServerDiagnostics =
                serde_json::from_value(answer.result)?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&diagnostics)?);
            } else {
                println!(
                    "threads: {}\nstore_opens: {}\nspawns: {}\nheld_facts: {}",
                    diagnostics.threads,
                    diagnostics.store_opens,
                    diagnostics.spawns,
                    diagnostics.held_facts,
                );
            }
            Ok(())
        }
        ServerCommand::Mobile { off, status } => {
            if *status {
                println!(
                    "{}",
                    serde_json::to_string(&crate::server::mobile::status()?)?
                );
            } else if *off {
                crate::server::mobile::disable()?;
                println!("Mobile access is off");
            } else {
                println!(
                    "{}",
                    serde_json::to_string(&crate::server::mobile::enable()?)?
                );
            }
            Ok(())
        }
        ServerCommand::Describe { json: true } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&crate::server::schema::describe())?
            );
            Ok(())
        }
        ServerCommand::Describe { json: false } => {
            println!("{}", crate::server::schema::summary());
            Ok(())
        }
    }
}
