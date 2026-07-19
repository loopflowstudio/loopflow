use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context};

use crate::durable::{AgentInvocationId, AnswerRoute, AskExchange, AskId, WorkRef};
use crate::store::{open_store, storage_config_from_env, Store};

pub fn run(args: &[String]) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run_async(args))
}

async fn run_async(args: &[String]) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    let lease = crate::ops::required_run_lease(&store)
        .await
        .map_err(|error| anyhow!(error.to_string()))?;
    let invocation_id = ambient_invocation_id()?;
    let (ask, recovering) = parse_request(&store, &lease, &invocation_id, args).await?;
    if ask.answer.is_none() {
        if let Err(error) = wake_parent(&store, &ask.route).await {
            tracing::warn!(ask_id = %ask.id, %error, "Ask parent wake failed; will retry");
        }
    }
    let answer = wait_for_answer(
        &store,
        &lease,
        &invocation_id,
        ask,
        recovering,
        Duration::from_secs(5),
        true,
    )
    .await?;
    println!("{answer}");
    Ok(())
}

async fn wait_for_answer(
    store: &Store,
    lease: &crate::durable::RunLease,
    invocation_id: &AgentInvocationId,
    mut ask: AskExchange,
    recovering: bool,
    retry_interval: Duration,
    retry_parent_wake: bool,
) -> anyhow::Result<String> {
    loop {
        ask = store
            .current_ask(lease, invocation_id, Some(&ask.id))
            .await?;
        if let Some(answer) = ask.answer {
            return Ok(answer.text);
        }
        if recovering {
            tracing::debug!(ask_id = %ask.id, "waiting on existing Ask");
        }
        tokio::time::sleep(retry_interval).await;
        if retry_parent_wake {
            if let Err(error) = wake_parent(store, &ask.route).await {
                tracing::warn!(ask_id = %ask.id, %error, "Ask parent wake failed; will retry");
            }
        }
    }
}

async fn parse_request(
    store: &Store,
    lease: &crate::durable::RunLease,
    invocation_id: &AgentInvocationId,
    args: &[String],
) -> anyhow::Result<(AskExchange, bool)> {
    match args {
        [] => Err(anyhow!("usage: lf ask <question> | lf ask wait [<ask-id>]")),
        [command] if command == "wait" => {
            Ok((store.current_ask(lease, invocation_id, None).await?, true))
        }
        [command, ask_id] if command == "wait" => {
            let ask_id = AskId::parse(ask_id)?;
            Ok((
                store
                    .current_ask(lease, invocation_id, Some(&ask_id))
                    .await?,
                true,
            ))
        }
        args if args.first().is_some_and(|command| command == "wait") => {
            Err(anyhow!("usage: lf ask wait [<ask-id>]"))
        }
        args => {
            let question = args.join(" ");
            Ok((
                store.open_ask(lease, invocation_id, &question).await?,
                false,
            ))
        }
    }
}

fn ambient_invocation_id() -> anyhow::Result<AgentInvocationId> {
    let value = std::env::var(crate::durable::AGENT_INVOCATION_ENV)
        .context("lf ask requires LF_AGENT_INVOCATION_ID from the active agent Turn")?;
    AgentInvocationId::parse(&value).map_err(Into::into)
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use time::OffsetDateTime;

    use super::wait_for_answer;
    use crate::durable::{
        AdvanceReceipt, Containment, ControlCtx, InvocationRoute, RunAdvance, RunTrigger, WorkRef,
    };
    use crate::id::WaveId;
    use crate::planning::{LinearProjectId, ProjectPlan};
    use crate::project::{Project, ProjectId};
    use crate::store::{open_store, StorageConfig};
    use crate::wave::Wave;

    async fn start_invocation(
        store: &crate::store::Store,
        work: &WorkRef,
        containment: &str,
    ) -> (crate::durable::RunLease, crate::durable::AgentInvocation) {
        let (_, lease) = store.reserve_run(work, RunTrigger::User).await.unwrap();
        store
            .advance_run(
                &lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: containment.to_string(),
                    },
                    cwd: "/repo".into(),
                },
            )
            .await
            .unwrap();
        let AdvanceReceipt::Invocation(invocation) = store
            .advance_run(
                &lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute {
                        provider: "codex".to_string(),
                        model: None,
                        account_id: None,
                    },
                    surface: "headless".to_string(),
                    resume_token: None,
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Invocation receipt")
        };
        (lease, invocation)
    }

    #[tokio::test]
    async fn blocking_wait_returns_parent_answer_without_advancing_the_turn() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let parent_work = WorkRef::Wave(wave.id().clone());
        let (parent_lease, _) = start_invocation(&store, &parent_work, "ask-parent").await;
        let now = OffsetDateTime::now_utc();
        let project = Project {
            id: ProjectId::new(),
            plan: ProjectPlan {
                id: LinearProjectId::new("project-ask-proof").unwrap(),
                slug: "ask-proof".to_string(),
                name: "Ask proof".to_string(),
                prompt_context: "Prove the blocking exchange.".to_string(),
                pm_snapshot_synced_at: now.unix_timestamp(),
            },
            wave_id: wave.id().clone(),
            iteration: 0,
            observation_cursor: 0,
            last_state_fingerprint: None,
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: None,
            abandon_intent: None,
            created_at: now,
            updated_at: now,
        };
        store.create_project(&project).await.unwrap();
        let child_work = WorkRef::Project(project.id);
        let (child_lease, child_invocation) =
            start_invocation(&store, &child_work, "ask-child").await;
        let AdvanceReceipt::Turn(turn) = store
            .advance_run(
                &child_lease,
                RunAdvance::TurnStarting {
                    invocation_id: child_invocation.id.clone(),
                },
            )
            .await
            .unwrap()
        else {
            panic!("expected Turn receipt")
        };
        let ask = store
            .open_ask(&child_lease, &child_invocation.id, "Which proof matters?")
            .await
            .unwrap();

        let wait_store = store.clone();
        let wait_lease = child_lease.clone();
        let wait_invocation = child_invocation.id.clone();
        let wait_ask = ask.clone();
        let waiter = tokio::spawn(async move {
            wait_for_answer(
                &wait_store,
                &wait_lease,
                &wait_invocation,
                wait_ask,
                false,
                Duration::from_millis(5),
                false,
            )
            .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(!waiter.is_finished(), "lf ask must block before an Answer");

        store
            .answer_ask(
                &ControlCtx::Run(&parent_lease),
                &ask.id,
                "The durable shell exchange.",
            )
            .await
            .unwrap();
        assert_eq!(
            waiter.await.unwrap().unwrap(),
            "The durable shell exchange."
        );
        let rerun = store
            .open_ask(&child_lease, &child_invocation.id, "Which proof matters?")
            .await
            .unwrap();
        assert_eq!(rerun.id, ask.id);
        assert!(rerun.answer.is_some(), "a shell retry recovers its Answer");
        let current = store
            .current_ask(&child_lease, &child_invocation.id, Some(&ask.id))
            .await
            .unwrap();
        assert_eq!(current.turn_id, turn.id);
    }
}
