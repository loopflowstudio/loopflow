//! Bulk moves over the Session selector: `lf session move` and `lf account
//! drain`. Interactive Sessions only. A login switch is one Machine-wide act
//! every open Claude follows; an agent change gives each Session a fresh
//! seeded thread, now when idle, after its turn when busy. Nothing here
//! writes preferences.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::{list, new_thread, SessionRecord, SessionState};
use crate::id::LfSessionId;
use crate::provider_account::activation;
use crate::provider_auth::Provider;
use crate::session::SessionFilter;
use crate::store::{ProviderAccount, RoutingState, SharedStore, StorageConfig};

/// Where the selected Sessions go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MoveTarget {
    /// Another login, by email: the provider's shared login switches once.
    Account(String),
    /// Another `harness[:model]`: each Session continues on a seeded thread.
    Agent(String),
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MoveReport {
    pub moved: Vec<Moved>,
    /// Busy Sessions; each moves when its turn ends.
    pub pending: Vec<Moved>,
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Moved {
    pub id: LfSessionId,
    pub title: String,
    pub how: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Skipped {
    pub id: LfSessionId,
    pub title: String,
    pub reason: String,
}

impl MoveReport {
    pub(crate) fn print(&self) {
        for moved in &self.moved {
            println!("{}  {}: {}", moved.id, moved.title, moved.how);
        }
        for pending in &self.pending {
            println!("{}  {}: {}", pending.id, pending.title, pending.how);
        }
        for skipped in &self.skipped {
            println!(
                "{}  {}: skipped, {}",
                skipped.id, skipped.title, skipped.reason
            );
        }
        if self.moved.is_empty() && self.pending.is_empty() && self.skipped.is_empty() {
            println!("No Sessions selected.");
        }
    }
}

/// Move the interactive Sessions `filter` selects to `target`.
pub(crate) async fn move_sessions(
    store: &SharedStore,
    filter: &SessionFilter,
    target: &MoveTarget,
) -> Result<MoveReport> {
    let mut selection = filter.clone();
    selection.interactive = Some(true);
    selection.limit = 0;
    selection.offset = 0;
    selection.after = None;
    if let MoveTarget::Agent(agent) = target {
        let (harness, _) = crate::config::parse_agent(agent);
        if crate::harness::HarnessKind::parse(&harness).is_none() {
            bail!("unknown agent {agent}: name harness[:model], such as claude:opus or codex");
        }
    }
    let mut report = MoveReport::default();
    let mut logins: Vec<(Provider, Result<ProviderAccount>)> = Vec::new();
    for session in list(store, &selection).await? {
        match target {
            MoveTarget::Account(email) => {
                move_login(store, &session, email, &mut logins, &mut report).await;
            }
            MoveTarget::Agent(agent) => move_agent(store, &session, agent, &mut report).await,
        }
    }
    Ok(report)
}

/// Switch the provider's shared login once; every Session on it follows. A
/// Session pinned to its own login is named for `lf resume --account`.
async fn move_login(
    store: &SharedStore,
    session: &SessionRecord,
    email: &str,
    logins: &mut Vec<(Provider, Result<ProviderAccount>)>,
    report: &mut MoveReport,
) {
    let Some(provider) = session
        .provider
        .as_deref()
        .and_then(|provider| provider.parse::<Provider>().ok())
    else {
        report.skip(session, "its provider has no managed logins");
        return;
    };
    if let Some(account) = &session.account {
        report.skip(
            session,
            format!(
                "pinned to {account}; `lf resume --account {email} {}` moves it",
                session.id
            ),
        );
        return;
    }
    if !logins.iter().any(|(switched, _)| *switched == provider) {
        let switched = switch_login(store, provider, email).await;
        logins.push((provider, switched));
    }
    let switched = &logins
        .iter()
        .find(|(switched, _)| *switched == provider)
        .expect("the provider's login was just switched")
        .1;
    match switched {
        Ok(account) => report.moved.push(Moved {
            id: session.id.clone(),
            title: session.title.clone(),
            how: match provider {
                Provider::Claude => format!(
                    "{} is signed in as {}; it follows at its next request",
                    provider.display_name(),
                    account_login(account)
                ),
                _ => format!(
                    "{} is signed in as {}; it follows when resumed",
                    provider.display_name(),
                    account_login(account)
                ),
            },
        }),
        Err(error) => report.skip(session, format!("{error:#}")),
    }
}

async fn switch_login(
    store: &SharedStore,
    provider: Provider,
    email: &str,
) -> Result<ProviderAccount> {
    let account =
        crate::lf::commands::profile::find_provider_account(store, provider, email).await?;
    let native = activation::native_home(provider, None);
    activation::activate(
        store,
        provider,
        &account.account_id,
        &native,
        activation::SwitchCause::Person,
    )
    .await
    .with_context(|| format!("switch {} to {email}", provider.display_name()))?;
    Ok(account)
}

/// An idle Session continues on `agent` at its next open; a busy one after
/// its turn, through its Stop hook.
async fn move_agent(
    store: &SharedStore,
    session: &SessionRecord,
    agent: &str,
    report: &mut MoveReport,
) {
    let busy = session.availability.is_none()
        && matches!(session.state, SessionState::Active | SessionState::Detached);
    if busy {
        match store.sqlite.record_session_move(session.id.as_str(), agent) {
            Ok(()) => report.pending.push(Moved {
                id: session.id.clone(),
                title: session.title.clone(),
                how: format!("continues on {agent} after its turn"),
            }),
            Err(error) => report.skip(session, format!("{error:#}")),
        }
        return;
    }
    match new_thread(store, session.id.as_str(), Some(agent), false, false).await {
        Ok(_) => report.moved.push(Moved {
            id: session.id.clone(),
            title: session.title.clone(),
            how: format!("continues on {agent} at its next open"),
        }),
        Err(error) => report.skip(session, format!("{error:#}")),
    }
}

impl MoveReport {
    fn skip(&mut self, session: &SessionRecord, reason: impl Into<String>) {
        self.skipped.push(Skipped {
            id: session.id.clone(),
            title: session.title.clone(),
            reason: reason.into(),
        });
    }
}

/// Stop routing to `email` and move the open Sessions on it: to `to`, else to
/// the route's next login. Returns the login they went to with the report.
pub(crate) async fn drain_account(
    store: &SharedStore,
    provider: Provider,
    email: &str,
    to: Option<&str>,
) -> Result<(ProviderAccount, MoveReport)> {
    let mut drained =
        crate::lf::commands::profile::find_provider_account(store, provider, email).await?;
    drained.routing_state = RoutingState::ExplicitOnly;
    store.upsert_provider_account(&drained).await?;
    let target = match to {
        Some(to) => {
            crate::lf::commands::profile::find_provider_account(store, provider, to).await?
        }
        None => crate::provider_account::inspect_provider_route(Some(store), None, provider)
            .await?
            .unwrap_or_default()
            .into_iter()
            .find(|account| account.account_id != drained.account_id)
            .ok_or_else(|| {
                anyhow!(
                    "no other {} login can take its Sessions; name one with --to",
                    provider.display_name()
                )
            })?,
    };
    let filter = SessionFilter {
        on: Some(drained.account_id.to_string()),
        repo: None,
        ..SessionFilter::default()
    };
    let report = move_sessions(
        store,
        &filter,
        &MoveTarget::Account(account_login(&target).to_owned()),
    )
    .await?;
    Ok((target, report))
}

/// The move `lf session move` left for a busy Session, applied once its turn
/// ends. `thread` is the provider's conversation id from the Stop hook.
pub(crate) fn apply_pending_move_blocking(home: &Path, thread: &str) -> Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = Arc::new(
            crate::store::open_store(&StorageConfig::sqlite(home.join("loopflow.db"))).await?,
        );
        let Some(session) = super::session_by_id(&store, thread).await? else {
            return Ok(());
        };
        let Some(agent) = store.sqlite.take_session_move(session.id.as_str())? else {
            return Ok(());
        };
        new_thread(&store, session.id.as_str(), Some(&agent), false, false)
            .await
            .map(drop)
    })
}

fn account_login(account: &ProviderAccount) -> &str {
    account
        .login_email
        .as_ref()
        .map_or(account.account_id.as_str(), |email| email.as_str())
}
