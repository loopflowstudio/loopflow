//! Client access has no execution or installation authority.
use crate::lf::ServerCommand;

pub fn run(command: &ServerCommand) -> anyhow::Result<()> {
    match command {
        ServerCommand::Receipt { request_id, .. } => {
            println!("{}", serde_json::to_string_pretty(&crate::engine::creation::lookup(request_id)?)?);
            Ok(())
        }
        ServerCommand::Run => crate::server::run(),
        ServerCommand::Status => {
            println!(
                "{}",
                serde_json::to_string(&crate::server::client::status()?)?
            );
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
