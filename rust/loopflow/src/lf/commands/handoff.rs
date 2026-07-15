use std::collections::BTreeMap;

use anyhow::{anyhow, Context};
use serde::Serialize;

use crate::engine::wave_home::WaveHome;
use crate::interactive_handoff::{
    InteractiveHandoff, InteractiveHandoffAttach, InteractiveHandoffId, InteractiveHandoffOutcome,
    OpenInteractiveHandoff,
};
use crate::lf::HandoffCommand;
use crate::store::{open_store, storage_config_from_env, Store};

#[derive(Debug, Serialize)]
struct OpenResult {
    created: bool,
    handoff: InteractiveHandoff,
}

pub fn run(command: &HandoffCommand) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(command))
}

async fn run_async(command: &HandoffCommand) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    match command {
        HandoffCommand::Open {
            parent,
            home,
            cwd,
            provider,
            provider_session,
            generation,
            reason,
            environment,
            json,
            attach_argv,
        } => {
            let request = OpenInteractiveHandoff {
                parent: parent.parse()?,
                home: home.parse::<WaveHome>().map_err(|error| anyhow!(error))?,
                cwd: cwd.clone(),
                provider: provider.clone(),
                provider_session_id: provider_session.clone(),
                body_generation: *generation,
                reason: reason.clone(),
                environment: parse_environment(environment)?,
                attach_argv: attach_argv.clone(),
            };
            let (handoff, created) = store.open_interactive_handoff(request).await?;
            if *json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&OpenResult { created, handoff })?
                );
            } else {
                let action = if created { "opened" } else { "reused" };
                println!(
                    "{action} handoff {} for {} ({})",
                    handoff.id,
                    handoff.parent,
                    handoff.status.as_str()
                );
            }
        }
        HandoffCommand::Status { session_id, json } => {
            let handoff = load_handoff(&store, session_id).await?;
            print_handoff(&handoff, *json)?;
        }
        HandoffCommand::Attach { session_id, json } => {
            let session_id = parse_session_id(session_id)?;
            let handoff = store.attach_interactive_handoff(&session_id).await?;
            print_attach(&handoff.attach_descriptor(), *json)?;
        }
        HandoffCommand::Complete {
            session_id,
            summary,
            json,
        } => {
            finish(
                &store,
                session_id,
                InteractiveHandoffOutcome::Completed {
                    summary: summary.clone(),
                },
                *json,
            )
            .await?;
        }
        HandoffCommand::Back {
            session_id,
            summary,
            json,
        } => {
            finish(
                &store,
                session_id,
                InteractiveHandoffOutcome::HandedBack {
                    summary: summary.clone(),
                },
                *json,
            )
            .await?;
        }
        HandoffCommand::Fail {
            session_id,
            reason,
            json,
        } => {
            finish(
                &store,
                session_id,
                InteractiveHandoffOutcome::Failed {
                    reason: reason.clone(),
                },
                *json,
            )
            .await?;
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

async fn load_handoff(store: &Store, session_id: &str) -> anyhow::Result<InteractiveHandoff> {
    let session_id = parse_session_id(session_id)?;
    store
        .get_interactive_handoff(&session_id)
        .await?
        .ok_or_else(|| anyhow!("interactive handoff {session_id} not found"))
}

async fn finish(
    store: &Store,
    session_id: &str,
    outcome: InteractiveHandoffOutcome,
    json: bool,
) -> anyhow::Result<()> {
    let session_id = parse_session_id(session_id)?;
    let handoff = store
        .finish_interactive_handoff(&session_id, &outcome)
        .await?;
    print_handoff(&handoff, json)
}

fn parse_session_id(value: &str) -> anyhow::Result<InteractiveHandoffId> {
    InteractiveHandoffId::parse(value).map_err(Into::into)
}

fn parse_environment(values: &[String]) -> anyhow::Result<BTreeMap<String, String>> {
    let mut environment = BTreeMap::new();
    for value in values {
        let (key, entry) = value
            .split_once('=')
            .ok_or_else(|| anyhow!("environment entry {value:?} must be KEY=VALUE"))?;
        if key.trim().is_empty() {
            return Err(anyhow!("environment key cannot be empty"));
        }
        if environment
            .insert(key.to_string(), entry.to_string())
            .is_some()
        {
            return Err(anyhow!("duplicate environment key {key:?}"));
        }
    }
    Ok(environment)
}

fn print_handoff(handoff: &InteractiveHandoff, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(handoff)?);
    } else {
        println!(
            "{}  {}\n  parent: {}\n  provider: {}\n  cwd: {}\n  reason: {}",
            handoff.id,
            handoff.status.as_str(),
            handoff.parent,
            handoff.provider,
            handoff.cwd.display(),
            handoff.reason
        );
    }
    Ok(())
}

fn print_attach(attach: &InteractiveHandoffAttach, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(attach)?);
    } else {
        println!(
            "{}  {}\n  host: {}\n  cwd: {}\n  env: {}\n  argv: {}",
            attach.session_id,
            attach.status.as_str(),
            attach.host,
            attach.cwd.display(),
            serde_json::to_string(&attach.environment)?,
            serde_json::to_string(&attach.argv)?,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_environment;

    #[test]
    fn environment_parser_preserves_values_without_building_shell_text() {
        let parsed =
            parse_environment(&["LF_HOME=/tmp/lf".to_string(), "EMPTY=".to_string()]).unwrap();
        assert_eq!(parsed.get("LF_HOME").map(String::as_str), Some("/tmp/lf"));
        assert_eq!(parsed.get("EMPTY").map(String::as_str), Some(""));
        assert!(parse_environment(&["LF_HOME=/a".to_string(), "LF_HOME=/b".to_string()]).is_err());
    }
}
