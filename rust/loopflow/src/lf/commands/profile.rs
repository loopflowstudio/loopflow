use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::profile::RouteScope;
use crate::provider_account::{account_login, match_account, nearest_provider_route, AccountMatch};
use crate::provider_auth::Provider;
use crate::repository::CanonicalRepo;
use crate::store::{CredentialState, ProviderAccount, ProviderAccountId, SharedStore};

/// What `lf account next` found for one provider.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum NextOutcome {
    Ready,
    /// Every login in the route is out or paused for now.
    Blocked,
    /// No login in the route has a working credential.
    LoginNeeded,
    /// No stored login: launches use the provider's own.
    Ambient,
}

#[derive(Debug, Serialize, Deserialize)]
struct NextLaunch {
    provider: Provider,
    /// `default`, `repository`, `wave:<id>`, or `automatic` with no order.
    scope: String,
    outcome: NextOutcome,
    account_id: Option<ProviderAccountId>,
    login: Option<String>,
    reason: String,
    /// The login with no order in the way, when the order changes the pick.
    without_order: Option<String>,
}

pub(super) async fn show_next(repo: Option<&str>, default: bool, json: bool) -> Result<()> {
    let repo_id = if default {
        None
    } else {
        resolve_repo_id(repo)?
    };
    let store = crate::provider_account::read_account_store()?;
    let mut launches = Vec::new();
    for provider in [Provider::Claude, Provider::Codex] {
        launches.push(next_launch(store.as_ref(), repo_id.as_ref(), provider).await?);
    }
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
            NextOutcome::LoginNeeded => Some(super::account::EXIT_LOGIN_NEEDED),
            NextOutcome::Blocked => Some(super::account::EXIT_BLOCKED),
            NextOutcome::Ready | NextOutcome::Ambient => None,
        })
        .max();
    match exit {
        Some(code) => Err(crate::process::CommandExit(code).into()),
        None => Ok(()),
    }
}

async fn next_launch(
    store: Option<&SharedStore>,
    repo_id: Option<&CanonicalRepo>,
    provider: Provider,
) -> Result<NextLaunch> {
    let launch =
        |scope: String, outcome, account: Option<&ProviderAccount>, reason: String| NextLaunch {
            provider,
            scope,
            outcome,
            account_id: account.map(|account| account.account_id.clone()),
            login: account.map(|account| account_login(account).to_owned()),
            reason,
            without_order: None,
        };
    let accounts = match store {
        Some(store) => {
            store
                .list_provider_accounts(Some(provider.as_str()))
                .await?
        }
        None => vec![],
    };
    let Some(store) = store.filter(|_| !accounts.is_empty()) else {
        return Ok(launch(
            "automatic".into(),
            NextOutcome::Ambient,
            None,
            format!("no stored login; launches use {provider}'s own"),
        ));
    };
    let route = nearest_provider_route(store, repo_id, None, provider).await?;
    let (scope, order) = match &route {
        Some(route) => match &route.scope {
            RouteScope::Default => ("default".to_string(), "your default order".to_string()),
            RouteScope::Repo(_) => ("repository".into(), "this repository's order".into()),
            RouteScope::Wave(wave) => (format!("wave:{wave}"), format!("Wave {wave}'s order")),
        },
        None => ("automatic".into(), String::new()),
    };
    let limits = store
        .provider_account_limits(Some(provider.as_str()))
        .await?;
    let now = now_unix();
    let ready = crate::provider_account::inspect_provider_route(Some(store), repo_id, provider)
        .await?
        .unwrap_or_default();
    let held = crate::provider_account::hold::held(provider);
    let Some(chosen) = held
        .as_ref()
        .and_then(|held| ready.iter().find(|account| account.account_id == *held))
        .or_else(|| ready.first())
    else {
        let candidates =
            crate::provider_account::provider_route_account_ids(store, repo_id, None, provider)
                .await?
                .unwrap_or_default();
        let named: Vec<_> = candidates
            .iter()
            .filter_map(|id| accounts.iter().find(|account| account.account_id == *id))
            .collect();
        let outcome = if named
            .iter()
            .all(|account| account.credential_state != CredentialState::Connected)
        {
            NextOutcome::LoginNeeded
        } else {
            NextOutcome::Blocked
        };
        let reasons = if named.is_empty() {
            "no login is in rotation; lf account unpause <login>".to_string()
        } else {
            named
                .iter()
                .map(|account| {
                    crate::provider_account::account_unavailable_reason(account, &limits, None, now)
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        return Ok(launch(scope, outcome, None, reasons));
    };
    let restricted = route.as_ref().is_some_and(|route| {
        let today = OffsetDateTime::now_utc().date();
        route.accounts.len()
            < accounts
                .iter()
                .filter(|account| account.eligible_for_automatic_routing(today))
                .count()
    });
    let reason = if held.as_ref() == Some(&chosen.account_id) {
        "held by lf account use until it fills up".to_string()
    } else if restricted
        && route
            .as_ref()
            .is_some_and(|route| route.accounts.len() == 1)
    {
        format!("{order} names only it")
    } else if restricted && ready.len() == 1 {
        format!("the only login ready in {order}")
    } else if ready.len() == 1 {
        "the only login ready for this launch".to_string()
    } else {
        match crate::provider_account::weekly_room(chosen, &limits, now) {
            Some((left, reset)) => format!(
                "{left}% left before {}: the most to spend in the time left",
                super::account::account_status::local_time(reset)
            ),
            None => "no weekly reading yet, so a full week to spend".to_string(),
        }
    };
    let mut next = launch(scope, NextOutcome::Ready, Some(chosen), reason);
    if restricted {
        // Only a pick with no order at all is "without that order".
        let unordered = if nearest_provider_route(store, None, None, provider)
            .await?
            .is_none()
        {
            crate::provider_account::inspect_provider_route(Some(store), None, provider)
                .await?
                .unwrap_or_default()
        } else {
            vec![]
        };
        if let Some(other) = unordered
            .first()
            .filter(|other| other.account_id != chosen.account_id)
        {
            next.without_order = Some(
                match crate::provider_account::weekly_room(other, &limits, now) {
                    Some((left, _)) => format!("{}, {left}% left", account_login(other)),
                    None => account_login(other).to_owned(),
                },
            );
        }
    }
    Ok(next)
}

/// The stored login an argument names: `claude=work@`, or `work@` when only
/// one provider has it. `verb` spells the retry for an ambiguous name.
pub(crate) async fn resolve_login(
    store: &SharedStore,
    provider: Option<Provider>,
    login: &str,
    verb: &str,
) -> Result<ProviderAccount> {
    let login = login.trim();
    let providers = match provider {
        Some(provider) => vec![provider],
        None => vec![Provider::Claude, Provider::Codex],
    };
    let mut found = Vec::new();
    for provider in providers {
        let accounts = store
            .list_provider_accounts(Some(provider.as_str()))
            .await?;
        let candidates = accounts.iter().collect::<Vec<_>>();
        match match_account(&candidates, login) {
            AccountMatch::One(account) => found.push(account.clone()),
            AccountMatch::None => {}
            AccountMatch::Ambiguous(accounts) => {
                return Err(anyhow!(
                    "{provider} login '{login}' could be {}",
                    accounts
                        .iter()
                        .map(|account| account_login(account))
                        .collect::<Vec<_>>()
                        .join(" or ")
                ))
            }
        }
    }
    let verb = if verb.is_empty() {
        String::new()
    } else {
        format!(" {verb}")
    };
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(anyhow!(
            "no stored login matches '{login}'; lf account lists them"
        )),
        _ => Err(anyhow!(
            "{login} is both claude and codex: lf account{verb} claude={login}"
        )),
    }
}

fn resolve_repo_id(raw_repo: Option<&str>) -> Result<Option<CanonicalRepo>> {
    match raw_repo {
        Some(repo) => Ok(Some(CanonicalRepo::discover(std::path::Path::new(repo))?)),
        None => CanonicalRepo::current(),
    }
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

#[cfg(test)]
mod tests {
    use super::{NextLaunch, NextOutcome};

    #[test]
    fn next_json_keeps_identity_reasons_and_unknowns() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/account_next.json");
        let launches: Vec<NextLaunch> = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            serde_json::to_value(&launches).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
        assert_eq!(
            launches[1].without_order.as_deref(),
            Some("jack@example.com, 60% left")
        );
        assert_eq!(launches[2].outcome, NextOutcome::LoginNeeded);
        assert!(launches[2].login.is_none());
        let mut missing: serde_json::Value = serde_json::from_str(fixture).unwrap();
        missing[0].as_object_mut().unwrap().remove("reason");
        assert!(serde_json::from_value::<Vec<NextLaunch>>(missing).is_err());
    }
}
