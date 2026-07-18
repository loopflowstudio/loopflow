use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{anyhow, Context};

use crate::durable::{BoundaryState, LaunchId, LaunchSurface};
use crate::engine::wave_home::HomeRoute;
use crate::lf::LaunchCommand;
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(command: &LaunchCommand) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(command))
}

async fn run_async(command: &LaunchCommand) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    match command {
        LaunchCommand::List { active, json } => {
            let surfaces = store.launch_surfaces(*active).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&surfaces)?);
            } else if surfaces.is_empty() {
                println!("No Launches.");
            } else {
                for surface in surfaces {
                    print_surface(&surface, false)?;
                }
            }
        }
        LaunchCommand::Status { launch_id, json } | LaunchCommand::Attach { launch_id, json } => {
            let surface = load_surface(&store, launch_id).await?;
            print_surface(&surface, *json)?;
        }
        LaunchCommand::Handback {
            launch_id,
            outcome,
            json,
        } => {
            let launch_id = parse_launch_id(launch_id)?;
            let outcome = parse_outcome(outcome)?;
            let surface = store.handback_launch(&launch_id, outcome).await?;
            print_surface(&surface, *json)?;
        }
        LaunchCommand::Present { launch_id } => {
            let surface = load_surface(&store, launch_id).await?;
            present(&surface)?;
        }
    }
    Ok(())
}

async fn open_shared_store() -> anyhow::Result<Store> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    open_store(&config)
        .await
        .context("open the shared Loopflow store")
}

async fn load_surface(store: &Store, launch_id: &str) -> anyhow::Result<LaunchSurface> {
    let launch_id = parse_launch_id(launch_id)?;
    store
        .launch_surface(&launch_id)
        .await?
        .ok_or_else(|| anyhow!("Launch {launch_id} not found"))
}

fn parse_launch_id(value: &str) -> anyhow::Result<LaunchId> {
    LaunchId::parse(value).map_err(Into::into)
}

fn parse_outcome(value: &str) -> anyhow::Result<BoundaryState> {
    match value {
        "succeeded" => Ok(BoundaryState::Succeeded),
        "failed" => Ok(BoundaryState::Failed),
        "interrupted" => Ok(BoundaryState::Interrupted),
        "unknown" => Ok(BoundaryState::Unknown),
        value => Err(anyhow!("invalid Launch handback outcome {value:?}")),
    }
}

fn print_surface(surface: &LaunchSurface, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(surface)?);
    } else {
        println!(
            "{}  {:?}\n  work: {}:{}\n  provider: {}\n  home: {}\n  cwd: {}",
            surface.launch.id,
            surface.launch.state,
            surface.work.kind(),
            surface.work.id(),
            surface.launch.route.provider,
            surface.home_route,
            surface.launch.cwd.display(),
        );
    }
    Ok(())
}

fn present(surface: &LaunchSurface) -> anyhow::Result<()> {
    let argv = surface
        .attach_argv
        .as_ref()
        .ok_or_else(|| anyhow!("Launch {} has no attach route", surface.launch.id))?;
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| anyhow!("Launch {} has an empty attach route", surface.launch.id))?;
    let home = HomeRoute::parse(&surface.home_route)
        .ok_or_else(|| anyhow!("invalid Home route {:?}", surface.home_route))?;
    let mut command = if let Some(destination) = home.ssh_destination() {
        let mut command = Command::new("ssh");
        if let Some(port) = home.ssh_port() {
            command.args(["-p", &port.to_string()]);
        }
        command.arg(destination).arg("--").arg(program).args(args);
        command
    } else {
        let mut command = Command::new(program);
        command.args(args).current_dir(&surface.launch.cwd);
        command
    };
    let error = command.exec();
    Err(anyhow!("failed to exec Launch attach route: {error}"))
}
