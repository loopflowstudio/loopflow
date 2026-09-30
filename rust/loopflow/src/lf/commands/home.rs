//! Inspect durable Home identity and routes.

use crate::lf::HomeCommand;
use anyhow::anyhow;

pub fn run(cmd: &HomeCommand) -> anyhow::Result<()> {
    match cmd {
        HomeCommand::Desktop
        | HomeCommand::Screenshot { .. }
        | HomeCommand::Install { .. }
        | HomeCommand::SyncSkills { .. }
        | HomeCommand::Doctor { .. }
        | HomeCommand::Ssh { .. } => {
            unreachable!("startup and operation commands dispatch separately")
        }
        HomeCommand::User { json } => {
            let name = crate::engine::config::load_user_name()?;
            if *json {
                println!("{}", serde_json::to_string(&name)?);
            } else if let Some(name) = name {
                println!("{name}");
            }
            Ok(())
        }
        HomeCommand::Id { json } => id_cmd(*json),
        HomeCommand::Observe {
            home_id,
            route,
            json,
        } => observe_cmd(home_id, route, *json),
    }
}

fn id_cmd(json: bool) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    let home = runtime.block_on(async {
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("lf home id needs an initialized local store"))?
            .local_home()
            .await
            .map_err(anyhow::Error::from)
    })?;
    if json {
        println!("{}", serde_json::to_string(&home)?);
    } else {
        println!("{}", home.id);
    }
    Ok(())
}

fn observe_cmd(home_id: &crate::durable::HomeId, route: &str, json: bool) -> anyhow::Result<()> {
    let parsed = crate::engine::wave_home::HomeRoute::parse(route)
        .ok_or_else(|| anyhow!("invalid Home route: {route:?}"))?;
    if !parsed.is_remote() {
        return Err(anyhow!(
            "the local Home route is managed by `lf home id`; observe only remote SSH routes"
        ));
    }
    let route = parsed.to_string();
    let runtime = tokio::runtime::Runtime::new()?;
    let home = runtime.block_on(async {
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("lf home observe needs an initialized local store"))?
            .observe_home(home_id, &route)
            .await
            .map_err(anyhow::Error::from)
    })?;
    if json {
        println!("{}", serde_json::to_string(&home)?);
    } else {
        println!("{}  {}", home.id, home.route);
    }
    Ok(())
}

fn validate_expected_home(local: &crate::durable::HomeId) -> anyhow::Result<()> {
    let Ok(raw) = std::env::var(crate::lf::commands::ssh::EXPECTED_HOME_ID_ENV) else {
        return Ok(());
    };
    let expected = crate::durable::HomeId::parse(&raw)
        .map_err(|error| anyhow!("invalid expected Home id: {error}"))?;
    if expected != *local {
        return Err(anyhow!(
            "refusing Home-addressed command: expected Home {expected}, local Home is {local}"
        ));
    }
    Ok(())
}

pub fn validate_expected_home_process() -> anyhow::Result<()> {
    if std::env::var_os(crate::lf::commands::ssh::EXPECTED_HOME_ID_ENV).is_none() {
        return Ok(());
    }
    let runtime = tokio::runtime::Runtime::new()?;
    let local = runtime.block_on(async {
        crate::store::open_existing_store()
            .await
            .ok_or_else(|| anyhow!("Home-addressed command needs an initialized local store"))?
            .local_home()
            .await
            .map_err(anyhow::Error::from)
    })?;
    validate_expected_home(&local.id)
}
