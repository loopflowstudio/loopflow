use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::profile::RouteScope;
use crate::provider_account::{account_login, match_account, nearest_provider_route, AccountMatch};
use crate::provider_auth::Provider;
use crate::repository::CanonicalRepo;
use crate::store::{ProviderAccount, ProviderAccountId, SharedStore};

pub(super) fn show_route(repo: Option<&str>, default: bool, json: bool) -> Result<()> {
    let reports = crate::engine::account::route(&std::env::current_dir()?, repo, default)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        for report in reports {
            print!("{}", report.render());
        }
    }
    Ok(())
}

/// How one provider's account is selected for a repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteReport {
    pub provider: Provider,
    pub scope: String,
    pub ambient: bool,
    /// Each conversation runs in its account's own home rather than the
    /// provider's native one.
    pub isolated: bool,
    /// The stored account the native home is signed in as; absent when
    /// isolated or when its login belongs to no stored account.
    pub active_account: Option<ProviderAccountId>,
    pub candidates: Vec<RouteAccount>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteAccount {
    pub account_id: ProviderAccountId,
    pub login: Option<String>,
    pub credential_state: String,
    pub routing: String,
    pub cooldown_until: Option<i64>,
    /// The account's most-used window that has not reset.
    pub tightest: Option<String>,
}

impl RouteReport {
    /// The route as `lf account route` prints it.
    pub fn render(&self) -> String {
        let mut out = format!("{} ({})\n", self.provider, self.scope);
        if self.isolated {
            out.push_str("  isolated: each conversation runs in its account's own home\n");
        } else {
            let active = self.active_account.as_ref().map(|active| {
                self.candidates
                    .iter()
                    .find(|account| account.account_id == *active)
                    .and_then(|account| account.login.clone())
                    .unwrap_or_else(|| active.to_string())
            });
            out.push_str(&format!(
                "  shared: {} is signed in as {}\n",
                crate::provider_account::activation::native_home(self.provider, None).display(),
                active.as_deref().unwrap_or("no stored account")
            ));
        }
        if self.ambient {
            out.push_str("  ambient\n");
        } else if self.candidates.is_empty() {
            out.push_str("  no eligible managed account\n");
        }
        for (position, account) in self.candidates.iter().enumerate() {
            out.push_str(&format!(
                "  {}. {} · {} · {}\n",
                position + 1,
                account.account_id,
                account.login.as_deref().unwrap_or("login unknown"),
                account.credential_state
            ));
            if let Some(tightest) = &account.tightest {
                out.push_str(&format!("     {tightest}\n"));
            }
        }
        out
    }
}

/// Each provider's route for `repo_id` (the default route with none).
pub(crate) async fn routes(
    store: Option<&SharedStore>,
    repo_id: Option<&CanonicalRepo>,
) -> Result<Vec<RouteReport>> {
    let mut reports = Vec::new();
    for provider in [Provider::Claude, Provider::Codex] {
        let route = match store {
            Some(store) => nearest_provider_route(store, repo_id, None, provider).await?,
            None => None,
        };
        let scope = match route {
            Some(route) if route.scope == RouteScope::Default => "default".into(),
            Some(route) => route.scope.id().to_owned(),
            None => "automatic".into(),
        };
        let (accounts, diagnostic) =
            match crate::provider_account::inspect_provider_route(store, repo_id, provider).await {
                Ok(accounts) => (accounts, None),
                Err(crate::provider_account::ProviderAccountError::NoEligibleAccount {
                    ..
                }) => (
                    Some(vec![]),
                    Some("no eligible account in the selected route".into()),
                ),
                Err(error) => return Err(error.into()),
            };
        let ambient = accounts.is_none();
        let limits = match store {
            Some(store) => {
                store
                    .provider_account_limits(Some(provider.as_str()))
                    .await?
            }
            None => vec![],
        };
        let mut candidates = Vec::new();
        for account in accounts.unwrap_or_default() {
            let tightest = crate::provider_account::tightest_window(
                provider.as_str(),
                &account.account_id,
                &limits,
                now_unix(),
            )
            .map(|limit| format!("{} {}% used", limit.window, limit.used_percent));
            candidates.push(RouteAccount {
                account_id: account.account_id,
                login: account.login_email.map(|email| email.to_string()),
                credential_state: account.credential_state.as_str().into(),
                routing: account.routing_state.as_str().into(),
                cooldown_until: account.cooldown_until,
                tightest,
            });
        }
        let isolated = crate::provider_account::activation::launch_isolated();
        let active_account = match store {
            Some(store) if !isolated => {
                let stored = store
                    .list_provider_accounts(Some(provider.as_str()))
                    .await?;
                crate::provider_account::activation::active_account(
                    provider,
                    &crate::provider_account::activation::native_home(provider, None),
                    &stored,
                )
                .map(|account| account.account_id.clone())
            }
            _ => None,
        };
        reports.push(RouteReport {
            provider,
            scope,
            ambient,
            isolated,
            active_account,
            candidates,
            diagnostic,
        });
    }
    Ok(reports)
}

pub(crate) async fn find_provider_account(
    store: &SharedStore,
    provider: Provider,
    raw_email: &str,
) -> Result<ProviderAccount> {
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    let candidates = accounts.iter().collect::<Vec<_>>();
    match match_account(&candidates, raw_email.trim()) {
        AccountMatch::One(account) => Ok(account.clone()),
        AccountMatch::None => Err(anyhow!(
            "managed {} login '{}' does not exist",
            provider,
            raw_email.trim()
        )),
        AccountMatch::Ambiguous(accounts) => Err(anyhow!(
            "{} login prefix '{}' is ambiguous: {}",
            provider,
            raw_email.trim(),
            accounts
                .iter()
                .map(|account| account_login(account))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

fn now_unix() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}

#[cfg(test)]
mod tests {
    use super::RouteReport;

    #[test]
    fn route_json_preserves_account_identity_and_unknowns() {
        let fixture = include_str!("../../../../../tests/fixtures/dto/auth_routes.json");
        let reports: Vec<RouteReport> = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            serde_json::to_value(&reports).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
        assert!(reports[0].isolated && reports[0].active_account.is_none());
        assert_eq!(
            reports[1].active_account.as_ref().map(|id| id.as_str()),
            Some("personal")
        );
        let mut missing: serde_json::Value = serde_json::from_str(fixture).unwrap();
        missing[0].as_object_mut().unwrap().remove("isolated");
        assert!(serde_json::from_value::<Vec<RouteReport>>(missing).is_err());
        assert_eq!(reports[0].candidates[0].account_id.as_str(), "engineering");
        assert_eq!(
            reports[0].candidates[0].login.as_deref(),
            Some("engineering@example.com")
        );
        assert!(reports[1].ambient);
        assert!(reports[1].candidates.is_empty());
    }
}
