use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context};

use crate::durable::{AnswerRoute, AskExchange, AskId, WorkRef};
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(args: &[String]) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run_async(args))
}

async fn run_async(args: &[String]) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    let lease = crate::ops::required_run_lease(&store)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let (mut ask, recovering) = parse_request(&store, &lease, args).await?;
    wake_parent(&store, &ask.route).await?;
    let mut wake = tokio::time::interval(Duration::from_secs(5));
    wake.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ask = store.current_ask(&lease, Some(&ask.id)).await?;
        if let Some(answer) = ask.answer {
            println!("{}", answer.text);
            return Ok(());
        }
        if recovering {
            tracing::debug!(ask_id = %ask.id, "waiting on existing Ask");
        }
        wake.tick().await;
        if let Err(error) = wake_parent(&store, &ask.route).await {
            tracing::warn!(ask_id = %ask.id, %error, "Ask parent wake failed; will retry");
        }
    }
}

async fn parse_request(
    store: &Store,
    lease: &crate::durable::RunLease,
    args: &[String],
) -> anyhow::Result<(AskExchange, bool)> {
    match args {
        [] => Err(anyhow!("usage: lf ask <question> | lf ask wait [<ask-id>]")),
        [command] if command == "wait" => Ok((store.current_ask(lease, None).await?, true)),
        [command, ask_id] if command == "wait" => {
            let ask_id = AskId::parse(ask_id)?;
            Ok((store.current_ask(lease, Some(&ask_id)).await?, true))
        }
        args if args.first().is_some_and(|command| command == "wait") => {
            Err(anyhow!("usage: lf ask wait [<ask-id>]"))
        }
        args => {
            let question = args.join(" ");
            Ok((store.open_ask(lease, &question).await?, false))
        }
    }
}

async fn wake_parent(store: &Store, route: &AnswerRoute) -> anyhow::Result<()> {
    let AnswerRoute::Parent(parent) = route else {
        return Ok(());
    };
    match parent {
        WorkRef::Project(project_id) => crate::ops::project::wake_project(project_id)
            .await
            .map_err(|error| anyhow!(error.to_string())),
        WorkRef::Wave(wave_id) => {
            let wave = store
                .get_wave(wave_id)
                .await?
                .ok_or_else(|| anyhow!("parent Wave {wave_id} is not registered"))?;
            let placement = store.placement(parent).await?;
            crate::home_resident::ensure(&placement.home_id, Path::new(wave.repo())).await?;
            crate::home_resident::start_waves(&placement.home_id, vec![wave_id.clone()]).await
        }
        WorkRef::Task(task_id) => Err(anyhow!(
            "Task {task_id} cannot own child Work and is not an Ask parent"
        )),
    }
}

async fn open_shared_store() -> anyhow::Result<Store> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    open_store(&config)
        .await
        .context("open the shared Loopflow store")
}
