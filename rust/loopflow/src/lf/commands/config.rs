use crate::lf::ConfigCommand;

pub fn run(cmd: &ConfigCommand) -> anyhow::Result<()> {
    match cmd {
        ConfigCommand::User { json } => {
            let name = crate::engine::config::load_user_name()?;
            if *json {
                println!("{}", serde_json::to_string(&name)?);
            } else if let Some(name) = name {
                println!("{name}");
            }
            Ok(())
        }
    }
}
