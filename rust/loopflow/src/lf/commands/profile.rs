use crate::provider_account::routing::NextOutcome;
use crate::repository::CanonicalRepo;
use anyhow::Result;

pub(crate) async fn show_next(
    repo: Option<&str>,
    default: bool,
    json: bool,
    agent: Option<&str>,
) -> Result<()> {
    let repo_id = if default {
        None
    } else {
        resolve_repo_id(repo)?
    };
    let launches = crate::provider_account::routing::next(repo_id.as_ref(), agent).await?;
    if json {
        println!("{}", serde_json::to_string_pretty(&launches)?);
    } else {
        let width = launches
            .iter()
            .filter_map(|launch| launch.login.as_deref())
            .map(|login| login.chars().count())
            .max()
            .unwrap_or(0);
        for launch in &launches {
            let login = launch.login.as_deref().unwrap_or("—");
            println!(
                "  {:<6}  {login:<width$}  {}",
                launch.provider.as_str(),
                launch.reason
            );
            if let Some(other) = &launch.without_order {
                println!("  {:<6}  {:<width$}  without that order: {other}", "", "");
            }
        }
    }
    // A needed login outranks a wait: the person can fix it now.
    let exit = launches
        .iter()
        .filter_map(|launch| match launch.outcome {
            NextOutcome::LoginNeeded => {
                Some(crate::provider_account::operations::EXIT_LOGIN_NEEDED)
            }
            NextOutcome::Blocked => Some(crate::provider_account::operations::EXIT_BLOCKED),
            NextOutcome::Ready | NextOutcome::Ambient => None,
        })
        .max();
    match exit {
        Some(code) => Err(crate::process::CommandExit(code).into()),
        None => Ok(()),
    }
}

fn resolve_repo_id(raw_repo: Option<&str>) -> Result<Option<CanonicalRepo>> {
    match raw_repo {
        Some(repo) => Ok(Some(crate::repository::discover(std::path::Path::new(
            repo,
        ))?)),
        None => crate::repository::current(),
    }
}
