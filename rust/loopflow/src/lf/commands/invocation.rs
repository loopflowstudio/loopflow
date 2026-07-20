use std::os::unix::process::CommandExt;
use std::process::Command;

use anyhow::{anyhow, Context};

use crate::durable::{AgentInvocationId, BoundaryState, InvocationSurface};
use crate::engine::wave_home::HomeRoute;
use crate::lf::InvocationCommand;
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(command: &InvocationCommand) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(command))
}

async fn run_async(command: &InvocationCommand) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    match command {
        InvocationCommand::List { active, json } => {
            let surfaces = store.invocation_surfaces(*active).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&surfaces)?);
            } else if surfaces.is_empty() {
                println!("No AgentInvocations.");
            } else {
                for surface in surfaces {
                    print_surface(&surface, false)?;
                }
            }
        }
        InvocationCommand::Status {
            invocation_id,
            json,
        }
        | InvocationCommand::Attach {
            invocation_id,
            json,
        } => {
            let surface = load_surface(&store, invocation_id).await?;
            print_surface(&surface, *json)?;
        }
        InvocationCommand::Handback {
            invocation_id,
            outcome,
            json,
        } => {
            let invocation_id = parse_invocation_id(invocation_id)?;
            let outcome = parse_outcome(outcome)?;
            let surface = store.handback_invocation(&invocation_id, outcome).await?;
            print_surface(&surface, *json)?;
        }
        InvocationCommand::Present { invocation_id } => {
            let surface = load_surface(&store, invocation_id).await?;
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

async fn load_surface(store: &Store, invocation_id: &str) -> anyhow::Result<InvocationSurface> {
    let invocation_id = parse_invocation_id(invocation_id)?;
    store
        .invocation_surface(&invocation_id)
        .await?
        .ok_or_else(|| anyhow!("AgentInvocation {invocation_id} not found"))
}

fn parse_invocation_id(value: &str) -> anyhow::Result<AgentInvocationId> {
    AgentInvocationId::parse(value).map_err(Into::into)
}

fn parse_outcome(value: &str) -> anyhow::Result<BoundaryState> {
    match value {
        "succeeded" => Ok(BoundaryState::Succeeded),
        "failed" => Ok(BoundaryState::Failed),
        "interrupted" => Ok(BoundaryState::Interrupted),
        "unknown" => Ok(BoundaryState::Unknown),
        value => Err(anyhow!(
            "invalid AgentInvocation handback outcome {value:?}"
        )),
    }
}

fn print_surface(surface: &InvocationSurface, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(surface)?);
    } else {
        println!(
            "{}  {:?}\n  work: {}:{}\n  provider: {}\n  home: {}\n  cwd: {}",
            surface.invocation.id,
            surface.run.state,
            surface.work.kind(),
            surface.work.id(),
            surface.invocation.route.provider,
            surface.home_route,
            surface
                .run
                .cwd
                .as_ref()
                .map_or_else(|| "-".to_string(), |cwd| cwd.display().to_string()),
        );
    }
    Ok(())
}

fn present(surface: &InvocationSurface) -> anyhow::Result<()> {
    let argv = surface
        .attach_argv
        .as_ref()
        .ok_or_else(|| anyhow!("Invocation {} has no attach route", surface.invocation.id))?;
    let (program, args) = argv.split_first().ok_or_else(|| {
        anyhow!(
            "Invocation {} has an empty attach route",
            surface.invocation.id
        )
    })?;
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
        command.args(args);
        if let Some(cwd) = &surface.run.cwd {
            command.current_dir(cwd);
        }
        command
    };
    let error = command.exec();
    Err(anyhow!(
        "failed to exec AgentInvocation attach route: {error}"
    ))
}
