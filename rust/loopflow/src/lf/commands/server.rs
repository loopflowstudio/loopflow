//! Client access has no execution or installation authority.
use crate::lf::ServerCommand;

pub fn run(command: &ServerCommand) -> anyhow::Result<()> {
    match command {
        ServerCommand::Run => crate::server::run(),
        ServerCommand::Status => {
            println!(
                "{}",
                serde_json::to_string(&crate::server::client::status()?)?
            );
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
    }
}
