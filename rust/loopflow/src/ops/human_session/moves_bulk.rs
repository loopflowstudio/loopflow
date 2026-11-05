//! Bulk moves over the Session selector: `lf session move` and `lf account
//! drain`. Interactive Sessions only. A login switch is one Machine-wide act
//! every open Claude follows; an agent change gives each Session a fresh
//! seeded thread, now when idle, after its turn when busy. Nothing here
//! writes preferences.

use std::os::unix::process::CommandExt;
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::{list, new_thread, SessionRecord, SessionState};
use crate::id::LfSessionId;
use crate::provider_account::activation;
use crate::provider_auth::Provider;
use crate::session::SessionFilter;
use crate::store::sqlite::SqliteStore;
use crate::store::{ProviderAccount, RoutingState, SharedStore, StorageConfig};

/// Where the selected Sessions go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MoveTarget {
    /// Another login, by email: the provider's shared login switches once.
    Account(String),
    /// Another `harness[:model]`: each Session continues on a seeded thread.
    Agent(String),
}

impl MoveTarget {
    /// The one target a move names: `--account <email>` or `-a harness[:model]`.
    pub(crate) fn parse(account: Option<&str>, agent: Option<&str>) -> Result<Self> {
        match (account, agent) {
            (Some(email), None) => Ok(Self::Account(email.to_owned())),
            (None, Some(agent)) => {
                let (harness, _) = crate::config::parse_agent(agent);
                if crate::harness::HarnessKind::parse(&harness).is_none() {
                    bail!(
                        "unknown agent {agent}: name harness[:model], such as claude:opus or codex"
                    );
                }
                Ok(Self::Agent(agent.to_owned()))
            }
            (Some(_), Some(_)) => bail!("name one target: --account <email> or -a harness[:model]"),
            (None, None) => {
                bail!("a Session move needs a target: --account <email> or -a harness[:model]")
            }
        }
    }
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
        for moved in self.moved.iter().chain(&self.pending) {
            println!("{}  {}: {}", moved.id, moved.title, moved.how);
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

    fn moved(&mut self, session: &SessionRecord, how: String) {
        self.moved.push(Moved::new(session, how));
    }

    fn pending(&mut self, session: &SessionRecord, how: String) {
        self.pending.push(Moved::new(session, how));
    }

    fn skip(&mut self, session: &SessionRecord, reason: impl Into<String>) {
        self.skipped.push(Skipped {
            id: session.id.clone(),
            title: session.title.clone(),
            reason: reason.into(),
        });
    }
}

impl Moved {
    fn new(session: &SessionRecord, how: String) -> Self {
        Self {
            id: session.id.clone(),
            title: session.title.clone(),
            how,
        }
    }
}

/// Move the interactive Sessions `filter` selects to `target`. The filter
/// must select by `blocked` or `on`. An agent move is per Session; a login
/// switch is per provider and reports every open Session that follows it.
pub(crate) async fn move_sessions(
    store: &SharedStore,
    filter: &SessionFilter,
    target: &MoveTarget,
) -> Result<MoveReport> {
    if !filter.blocked && filter.on.is_none() {
        bail!("a Session move needs a selector: --blocked or --on <account|harness[:model]>");
    }
    let mut selection = filter.clone();
    selection.interactive = Some(true);
    selection.limit = 0;
    selection.offset = 0;
    selection.after = None;
    let mut report = MoveReport::default();
    let selected = list(store, &selection).await?;
    match target {
        MoveTarget::Account(email) => {
            for provider in selected_providers(&selected, &mut report) {
                match switch_login(store, provider, email).await {
                    Ok(account) => {
                        // Read after the switch: a Session is blocked by the
                        // login it now runs on.
                        let open = list(store, &open_interactive()).await?;
                        follow_login(&mut report, provider, &account, &open);
                    }
                    Err(error) => {
                        for session in &selected {
                            if session_provider(session) == Some(provider) {
                                report.skip(session, format!("{error:#}"));
                            }
                        }
                    }
                }
            }
        }
        MoveTarget::Agent(agent) => {
            for session in &selected {
                move_agent(store, session, agent, &mut report).await;
            }
        }
    }
    Ok(report)
}

fn session_provider(session: &SessionRecord) -> Option<Provider> {
    session.provider.as_deref()?.parse().ok()
}

/// Every open interactive Session on this Machine, whatever its repository:
/// a login switch moves them all.
fn open_interactive() -> SessionFilter {
    SessionFilter {
        repo: None,
        limit: 0,
        ..SessionFilter::default()
    }
}

/// The providers whose shared login the selection switches, once each in
/// order; a Session on an unmanaged provider is skipped.
fn selected_providers(selected: &[SessionRecord], report: &mut MoveReport) -> Vec<Provider> {
    let mut providers = Vec::new();
    for session in selected {
        match session_provider(session) {
            Some(provider) if !providers.contains(&provider) => providers.push(provider),
            Some(_) => {}
            None => report.skip(session, "its provider has no managed logins"),
        }
    }
    providers
}

/// The switch moves every open Session of `provider`, selected or not; each
/// is reported with whether `account` can take its model now.
fn follow_login(
    report: &mut MoveReport,
    provider: Provider,
    account: &ProviderAccount,
    open: &[SessionRecord],
) {
    let login = account_login(account);
    let follows = match provider {
        Provider::Claude => "at its next request",
        _ => "when resumed",
    };
    for session in open
        .iter()
        .filter(|session| session_provider(session) == Some(provider))
    {
        let mut how = format!(
            "{} is signed in as {login}; it follows {follows}",
            provider.display_name()
        );
        if session.blocked {
            let model = session.model.as_deref().unwrap_or("its model");
            how.push_str(&format!(", but {login} cannot take {model} now"));
        }
        report.moved(session, how);
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
            Ok(()) => report.pending(session, format!("continues on {agent} after its turn")),
            Err(error) => report.skip(session, format!("{error:#}")),
        }
        return;
    }
    match new_thread(store, session.id.as_str(), Some(agent), false, false).await {
        Ok(_) => report.moved(session, format!("continues on {agent} at its next open")),
        Err(error) => report.skip(session, format!("{error:#}")),
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

/// At a Stop hook: when a move waits for `session`, start its applier beside
/// the provider and return at once. The hook only peeks, through the store it
/// opened for the turn; the applier stops the provider, so it must outlive the
/// hook rather than run inside it.
pub(crate) fn start_pending_move(store: &SqliteStore, session: &str) -> Result<bool> {
    if store.pending_session_move(session)?.is_none() {
        return Ok(false);
    }
    let mut command = std::process::Command::new(crate::os_process::resolve_lf_binary()?);
    command
        .args(["__context-block", "--moment", "move"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // SAFETY: only an async-signal-safe syscall runs between fork and exec.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn().context("start the pending Session move")?;
    Ok(true)
}

/// The applier a Stop hook started. Once the hook has exited, so the turn has
/// ended, the Session continues on the requested agent the way an idle
/// Session's move does.
pub(crate) fn apply_pending_move() -> Result<()> {
    let adapter: crate::steer::SteerAdapter =
        serde_json::from_str(&std::env::var(crate::steer::PROVIDER_CONTEXT_ENV)?)?;
    wait_for_hook_exit();
    let home = crate::store::lf_home_dir();
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = Arc::new(
            crate::store::open_store(&StorageConfig::sqlite(home.join("loopflow.db"))).await?,
        );
        apply_move(&store, &adapter.session).await
    })
}

/// The hook that started this applier is its parent until it exits; the
/// provider ends its turn right after. The wait outlasts the hook's timeout.
fn wait_for_hook_exit() {
    let hook = std::os::unix::process::parent_id();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::os::unix::process::parent_id() == hook && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    std::thread::sleep(std::time::Duration::from_secs(1));
}

/// Continue `session` on its pending move's agent. The request is cleared only
/// after the new input is appended, so a failed or killed apply is retried at
/// the Session's next Stop instead of lost.
async fn apply_move(store: &SharedStore, session: &str) -> Result<()> {
    let Some(agent) = store.sqlite.pending_session_move(session)? else {
        return Ok(());
    };
    new_thread(store, session, Some(&agent), false, false).await?;
    store.sqlite.clear_session_move(session, &agent)?;
    Ok(())
}

fn account_login(account: &ProviderAccount) -> &str {
    account
        .login_email
        .as_ref()
        .map_or(account.account_id.as_str(), |email| email.as_str())
}

#[cfg(test)]
mod tests {
    use super::{
        apply_move, follow_login, selected_providers, start_pending_move, MoveReport, SessionRecord,
    };
    use crate::provider_auth::Provider;

    fn session(title: &str, provider: &str, blocked: bool) -> SessionRecord {
        let mut session: SessionRecord = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/session.json"
        ))
        .unwrap();
        session.id = format!("session_{title}").as_str().into();
        session.title = title.into();
        session.provider = Some(provider.into());
        session.model = Some("opus".into());
        session.blocked = blocked;
        session
    }

    #[test]
    fn a_blocked_selector_reports_every_open_session_that_follows_the_login() {
        let blocked = session("blocked", "claude", true);
        let unselected = session("unselected", "claude", false);
        let codex = session("codex", "codex", false);
        let mut report = MoveReport::default();
        let providers = selected_providers(std::slice::from_ref(&blocked), &mut report);
        assert_eq!(providers, [Provider::Claude]);
        let account = crate::provider_account::new_account(
            Provider::Claude,
            crate::store::ProviderAccountId::parse("work").unwrap(),
            "/accounts/work".into(),
            Some(crate::profile::EmailAddress::parse("work@example.com").unwrap()),
        );
        let still_blocked = session("still blocked", "claude", true);
        follow_login(
            &mut report,
            Provider::Claude,
            &account,
            &[unselected.clone(), codex, still_blocked],
        );
        let hows: Vec<_> = report
            .moved
            .iter()
            .map(|moved| (moved.title.as_str(), moved.how.as_str()))
            .collect();
        assert_eq!(
            hows,
            [
                (
                    "unselected",
                    "Claude is signed in as work@example.com; it follows at its next request"
                ),
                (
                    "still blocked",
                    "Claude is signed in as work@example.com; it follows at its next request, but work@example.com cannot take opus now"
                ),
            ]
        );
        assert!(report.skipped.is_empty());
    }

    #[test]
    fn a_stop_with_no_pending_move_starts_nothing() {
        let home = tempfile::tempdir().unwrap();
        let store =
            crate::store::sqlite::SqliteStore::open_ephemeral(&home.path().join("loopflow.db"))
                .unwrap();
        assert!(!start_pending_move(&store, "session_idle").unwrap());
    }

    #[test]
    fn a_failed_apply_leaves_the_move_for_the_next_stop() {
        let home = super::super::tests::SessionHome::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let store = home.store().await;
            store
                .sqlite
                .record_session_move("session_missing", "codex")
                .unwrap();
            assert!(apply_move(&store, "session_missing").await.is_err());
            assert_eq!(
                store
                    .sqlite
                    .pending_session_move("session_missing")
                    .unwrap(),
                Some("codex".into())
            );
            store
                .sqlite
                .clear_session_move("session_missing", "claude:opus")
                .unwrap();
            assert!(store
                .sqlite
                .pending_session_move("session_missing")
                .unwrap()
                .is_some());
        });
    }
}
