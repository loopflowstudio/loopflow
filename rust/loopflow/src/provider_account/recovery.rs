//! Pure route readiness and recovery policy for sequential AgentInvocation replacement.

use std::collections::HashSet;

use thiserror::Error;

use crate::durable::{
    AdvanceReceipt, AgentInvocationId, BoundaryState, CapabilityRef, ContainmentObservation,
    InvocationRoute, RunAdvance, RunLease, StopCause, StopReceipt, Wait, WaitOn,
};
use crate::engine::config::parse_agent;
use crate::harness::Harness;
use crate::provider_account::lease::{AccountLease, AccountLeaseClient};
use crate::provider_auth::{AuthStatus, Provider, ProviderAuthService};
use crate::store::{
    AccountLimitRow, CredentialState, ProviderAccount, ProviderAccountId, RoutingState,
    SharedStore, StoreError,
};

/// The canonical provider and model selected for one AgentInvocation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentRoute {
    pub provider: String,
    pub model: Option<String>,
}

impl AgentRoute {
    pub fn parse(agent: &str) -> Result<Self, ExactRouteError> {
        let (provider, model) = parse_agent(agent);
        Self::new(provider, model)
    }

    pub fn new(provider: String, model: Option<String>) -> Result<Self, ExactRouteError> {
        if provider.trim().is_empty() {
            return Err(ExactRouteError::EmptyProvider);
        }
        Ok(Self { provider, model })
    }

    pub fn agent(&self) -> String {
        match &self.model {
            Some(model) => format!("{}:{model}", self.provider),
            None => self.provider.clone(),
        }
    }
}

/// One exact provider, model, and optional managed-account route.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExactRoute {
    pub agent: AgentRoute,
    pub account_id: Option<ProviderAccountId>,
}

impl TryFrom<&InvocationRoute> for ExactRoute {
    type Error = ExactRouteError;

    fn try_from(route: &InvocationRoute) -> Result<Self, Self::Error> {
        let agent = AgentRoute::new(route.provider.clone(), route.model.clone())?;
        let account_id = route
            .account_id
            .as_deref()
            .map(ProviderAccountId::parse)
            .transpose()
            .map_err(|reason| ExactRouteError::InvalidAccountId {
                account_id: route.account_id.clone().unwrap_or_default(),
                reason,
            })?;
        Ok(Self { agent, account_id })
    }
}

impl From<&ExactRoute> for InvocationRoute {
    fn from(route: &ExactRoute) -> Self {
        Self {
            provider: route.agent.provider.clone(),
            model: route.agent.model.clone(),
            account_id: route
                .account_id
                .as_ref()
                .map(|account_id| account_id.as_str().to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ExactRouteError {
    #[error("Invocation route provider cannot be empty")]
    EmptyProvider,
    #[error("invalid Invocation route account id '{account_id}': {reason}")]
    InvalidAccountId { account_id: String, reason: String },
}

/// Why one exact route cannot be selected at this recovery boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RouteUnavailable {
    Credential,
    Capacity { resets_at: Option<i64> },
    Policy,
}

/// Fixed invocation evidence that is not stored on [`ProviderAccount`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteEvidence {
    pub within_grant: bool,
    pub explicitly_selected: bool,
    pub credential_resolves: bool,
}

/// One ordered recovery candidate and its current readiness evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteCandidate {
    pub route: ExactRoute,
    pub readiness: Result<(), RouteUnavailable>,
    pub strained: bool,
}

/// Project existing account and grant facts into one typed route decision.
pub fn project_route_candidate(
    route: ExactRoute,
    account: Option<&ProviderAccount>,
    limits: &[AccountLimitRow],
    evidence: RouteEvidence,
    now: i64,
    today: time::Date,
) -> RouteCandidate {
    let account = route.account_id.as_ref().and_then(|account_id| {
        account.filter(|account| {
            account.provider == route.agent.provider && account.account_id == *account_id
        })
    });
    let strained = account.is_some_and(|account| {
        super::active_account_strain(&account.provider, &account.account_id, limits, now).is_some()
    });
    let readiness = if !evidence.within_grant {
        Err(RouteUnavailable::Policy)
    } else if route.account_id.is_none() {
        evidence
            .credential_resolves
            .then_some(())
            .ok_or(RouteUnavailable::Credential)
    } else if let Some(account) = account {
        let routing = account.effective_routing_state(today);
        if routing == RoutingState::Disabled
            || (routing == RoutingState::ExplicitOnly && !evidence.explicitly_selected)
        {
            Err(RouteUnavailable::Policy)
        } else if account.credential_state != CredentialState::Connected
            || !evidence.credential_resolves
        {
            Err(RouteUnavailable::Credential)
        } else if let Some(resets_at) = capacity_reset(account, limits, now) {
            Err(RouteUnavailable::Capacity { resets_at })
        } else {
            Ok(())
        }
    } else {
        Err(RouteUnavailable::Credential)
    };

    RouteCandidate {
        route,
        readiness,
        strained,
    }
}

fn capacity_reset(
    account: &ProviderAccount,
    limits: &[AccountLimitRow],
    now: i64,
) -> Option<Option<i64>> {
    let cooldown = account.cooldown_until.filter(|until| *until > now);
    let exhausted = limits
        .iter()
        .filter(|limit| {
            limit.provider == account.provider
                && limit.account_id == account.account_id
                && limit.used_percent >= 100
                && limit.resets_at.is_none_or(|resets_at| resets_at > now)
        })
        .map(|limit| limit.resets_at)
        .collect::<Vec<_>>();

    if cooldown.is_none() && exhausted.is_empty() {
        return None;
    }
    if exhausted.iter().any(Option::is_none) {
        return Some(None);
    }
    Some(
        cooldown
            .into_iter()
            .chain(exhausted.into_iter().flatten())
            .max(),
    )
}

/// The next bounded action after a retryable failure may be replaced safely.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecoveryChoice {
    Invoke(ExactRoute),
    AwaitCapability {
        reasons: Vec<(ExactRoute, RouteUnavailable)>,
    },
}

/// Choose the next route from primary-provider candidates followed by backup.
///
/// `ordered_candidates` must already carry the existing chooser's explicit
/// preference, declared route order, and strain demotion. This function owns
/// only chain exclusion and same-provider-before-backup exhaustion.
pub fn plan_route_recovery(
    ordered_candidates: &[RouteCandidate],
    chain_excluded: &[ExactRoute],
) -> RecoveryChoice {
    let primary_provider = ordered_candidates
        .first()
        .map(|candidate| candidate.route.agent.provider.as_str());

    for primary in [true, false] {
        if let Some(candidate) = ordered_candidates.iter().find(|candidate| {
            let is_primary =
                primary_provider.is_some_and(|provider| candidate.route.agent.provider == provider);
            is_primary == primary
                && !chain_excluded.contains(&candidate.route)
                && candidate.readiness.is_ok()
        }) {
            return RecoveryChoice::Invoke(candidate.route.clone());
        }
    }

    let reasons = ordered_candidates
        .iter()
        .filter(|candidate| !chain_excluded.contains(&candidate.route))
        .filter_map(|candidate| {
            candidate
                .readiness
                .as_ref()
                .err()
                .cloned()
                .map(|reason| (candidate.route.clone(), reason))
        })
        .collect();
    RecoveryChoice::AwaitCapability { reasons }
}

/// Minimal durable-history projection needed to rebuild the current chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryHistoryEntry {
    pub invocation_id: AgentInvocationId,
    pub route: ExactRoute,
    pub recovery_predecessor: Option<AgentInvocationId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RecoveryHistoryError {
    #[error("recovery history is missing AgentInvocation {0}")]
    MissingInvocation(AgentInvocationId),
    #[error("recovery history contains a cycle at AgentInvocation {0}")]
    Cycle(AgentInvocationId),
}

/// Walk recovery-predecessor links and return oldest-to-newest route exclusions.
pub fn derive_chain_exclusions(
    history: &[RecoveryHistoryEntry],
    failed_invocation_id: &AgentInvocationId,
) -> Result<Vec<ExactRoute>, RecoveryHistoryError> {
    let mut cursor = Some(failed_invocation_id.clone());
    let mut seen = HashSet::new();
    let mut routes = Vec::new();

    while let Some(invocation_id) = cursor {
        if !seen.insert(invocation_id.clone()) {
            return Err(RecoveryHistoryError::Cycle(invocation_id));
        }
        let entry = history
            .iter()
            .find(|entry| entry.invocation_id == invocation_id)
            .ok_or_else(|| RecoveryHistoryError::MissingInvocation(invocation_id.clone()))?;
        routes.push(entry.route.clone());
        cursor = entry.recovery_predecessor.clone();
    }
    routes.reverse();
    Ok(routes)
}

#[derive(Debug, Error)]
pub(crate) enum RunRouteRecoveryError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Account(#[from] crate::provider_account::ProviderAccountError),
    #[error(transparent)]
    ExactRoute(#[from] ExactRouteError),
    #[error(transparent)]
    History(#[from] RecoveryHistoryError),
    #[error("Run {0} has no AgentInvocation history to recover")]
    MissingInvocation(crate::durable::RunId),
}

/// The PRD-38 replacement seam consumed one route choice without taking over
/// containment or effect judgment from its caller.
#[derive(Debug)]
pub(crate) enum RecoverySettlement {
    RecoveryRun { lease: RunLease, route: ExactRoute },
    AwaitCapability { wait: Wait },
}

#[derive(Debug)]
pub(crate) enum RecoveryStopOutcome {
    Stopped(StoppedInvocation),
    Fenced {
        error: String,
        stop: Box<StopReceipt>,
    },
}

/// Proof that the current executor positively stopped its provider containment.
/// Only [`stop_invocation_for_recovery`] can mint it.
#[derive(Debug)]
pub(crate) struct StoppedInvocation {
    invocation_id: AgentInvocationId,
}

/// Read the fixed account grant and this Run's durable AgentInvocation chain, then apply
/// the pure same-provider-before-backup policy.
pub(crate) async fn plan_run_route_recovery(
    store: &SharedStore,
    lease: &RunLease,
    backup_agent: Option<&str>,
) -> Result<RecoveryChoice, RunRouteRecoveryError> {
    let mut invocation_generations = Vec::new();
    let mut run_id = Some(lease.run_id.clone());
    while let Some(current_run_id) = run_id {
        invocation_generations.push(store.invocations_for_run(&current_run_id).await?);
        run_id = store.run_by_id(&current_run_id).await?.retry_of;
    }
    invocation_generations.reverse();
    let invocations = invocation_generations
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let first = invocations
        .first()
        .ok_or_else(|| RunRouteRecoveryError::MissingInvocation(lease.run_id.clone()))?;
    let current = invocations
        .last()
        .expect("non-empty AgentInvocation history has a current AgentInvocation");
    let primary_agent = ExactRoute::try_from(&first.route)?.agent;
    let fixed_client = AccountLeaseClient::from_env()?;
    let fixed_lease = fixed_client
        .as_ref()
        .map(AccountLeaseClient::describe)
        .transpose()?;

    let mut candidates = route_candidates(
        store,
        &primary_agent,
        fixed_client.as_ref(),
        fixed_lease.as_ref(),
    )
    .await?;
    if let Some(backup) = backup_agent
        .map(str::trim)
        .filter(|agent| !agent.is_empty())
    {
        let backup = AgentRoute::parse(backup)?;
        if backup != primary_agent {
            candidates.extend(
                route_candidates(store, &backup, fixed_client.as_ref(), fixed_lease.as_ref())
                    .await?,
            );
        }
    }
    let mut seen = HashSet::new();
    candidates.retain(|candidate| seen.insert(candidate.route.clone()));

    // `retry_of` preserves one consecutive recovery chain even though each
    // replacement containment owns a distinct Run.
    let history = invocations
        .iter()
        .enumerate()
        .map(|(index, invocation)| {
            Ok(RecoveryHistoryEntry {
                invocation_id: invocation.id.clone(),
                route: ExactRoute::try_from(&invocation.route)?,
                recovery_predecessor: index
                    .checked_sub(1)
                    .map(|previous| invocations[previous].id.clone()),
            })
        })
        .collect::<Result<Vec<_>, ExactRouteError>>()?;
    let excluded = derive_chain_exclusions(&history, &current.id)?;
    Ok(plan_route_recovery(&candidates, &excluded))
}

async fn route_candidates(
    store: &SharedStore,
    agent: &AgentRoute,
    fixed_client: Option<&AccountLeaseClient>,
    fixed_lease: Option<&AccountLease>,
) -> Result<Vec<RouteCandidate>, RunRouteRecoveryError> {
    let Some(provider) = managed_provider(agent) else {
        return Ok(vec![
            accountless_candidate(store, agent, fixed_lease.is_none()).await,
        ]);
    };
    let (account_ids, preferred) = match fixed_lease {
        Some(lease) => match lease.grant(provider) {
            Some(grant) => (grant.accounts.clone(), grant.preferred),
            None => {
                return Ok(vec![project_route_candidate(
                    ExactRoute {
                        agent: agent.clone(),
                        account_id: None,
                    },
                    None,
                    &[],
                    RouteEvidence {
                        within_grant: false,
                        explicitly_selected: false,
                        credential_resolves: false,
                    },
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                    time::OffsetDateTime::now_utc().date(),
                )])
            }
        },
        None => {
            let repo_id = super::current_repo_id()?;
            match super::provider_route_account_ids(store, repo_id.as_ref(), provider).await? {
                Some(accounts) if !accounts.is_empty() => (accounts, 0),
                _ => {
                    return Ok(vec![accountless_candidate(store, agent, true).await]);
                }
            }
        }
    };

    let local_facts = match fixed_client {
        Some(_) => None,
        None => Some((
            store
                .list_provider_accounts(Some(provider.as_str()))
                .await?,
            store
                .provider_account_limits(Some(provider.as_str()))
                .await?,
        )),
    };
    let now = time::OffsetDateTime::now_utc();
    let mut candidates = Vec::with_capacity(account_ids.len());
    for (index, account_id) in account_ids.iter().enumerate() {
        let (account, limits, credential_resolves) = match fixed_client {
            Some(client) => {
                let facts = client.account_facts(provider, account_id)?;
                (facts.account, facts.limits, facts.credential_available)
            }
            None => {
                let (accounts, limits) = local_facts
                    .as_ref()
                    .expect("local account facts exist without a forwarded lease");
                let account = accounts
                    .iter()
                    .find(|account| account.account_id == *account_id)
                    .cloned();
                let credential_resolves = account.as_ref().is_some_and(|account| {
                    account.home.as_deref().is_some_and(std::path::Path::exists)
                });
                (account, limits.clone(), credential_resolves)
            }
        };
        candidates.push(project_route_candidate(
            ExactRoute {
                agent: agent.clone(),
                account_id: Some(account_id.clone()),
            },
            account.as_ref(),
            &limits,
            RouteEvidence {
                within_grant: true,
                explicitly_selected: index < preferred,
                credential_resolves,
            },
            now.unix_timestamp(),
            now.date(),
        ));
    }
    let preferred = preferred.min(candidates.len());
    candidates[preferred..].sort_by_key(|candidate| candidate.strained);
    Ok(candidates)
}

async fn accountless_candidate(
    store: &SharedStore,
    agent: &AgentRoute,
    within_grant: bool,
) -> RouteCandidate {
    let credential_resolves = match auth_provider(agent) {
        Some(provider) => ProviderAuthService::new(store.clone())
            .status(provider)
            .await
            .is_ok_and(|snapshot| matches!(snapshot.status, AuthStatus::Active { .. })),
        None => false,
    };
    let now = time::OffsetDateTime::now_utc();
    project_route_candidate(
        ExactRoute {
            agent: agent.clone(),
            account_id: None,
        },
        None,
        &[],
        RouteEvidence {
            within_grant,
            explicitly_selected: false,
            credential_resolves,
        },
        now.unix_timestamp(),
        now.date(),
    )
}

fn managed_provider(agent: &AgentRoute) -> Option<Provider> {
    match agent.provider.as_str() {
        "claude" => Some(Provider::Claude),
        "codex" => Some(Provider::Codex),
        _ => None,
    }
}

fn auth_provider(agent: &AgentRoute) -> Option<Provider> {
    match agent.provider.as_str() {
        "claude" => Some(Provider::Claude),
        "codex" => Some(Provider::Codex),
        "opencode" | "opencodezen" => Some(Provider::OpenCodeZen),
        _ => None,
    }
}

/// End the failed AgentInvocation, then either atomically hand authority to a
/// new Recovery Run or record a typed capability Wait. The caller has already
/// stopped the provider process and committed to exiting the old containment.
pub(crate) async fn settle_route_recovery(
    store: &SharedStore,
    lease: &RunLease,
    stopped: StoppedInvocation,
    choice: RecoveryChoice,
) -> Result<RecoverySettlement, RunRouteRecoveryError> {
    store
        .advance_run(
            lease,
            RunAdvance::InvocationEnded {
                invocation_id: stopped.invocation_id,
                outcome: BoundaryState::Failed,
            },
        )
        .await?;
    match choice {
        RecoveryChoice::Invoke(route) => {
            let (_, recovery_lease) = store.reserve_recovery_run(lease).await?;
            Ok(RecoverySettlement::RecoveryRun {
                lease: recovery_lease,
                route,
            })
        }
        RecoveryChoice::AwaitCapability { reasons } => {
            let receipt = store
                .advance_run(
                    lease,
                    RunAdvance::Wait {
                        on: WaitOn::Capability {
                            capability: CapabilityRef {
                                kind: "provider_route".to_string(),
                                key: capability_key(&reasons),
                            },
                        },
                    },
                )
                .await?;
            let AdvanceReceipt::Wait(wait) = receipt else {
                unreachable!("RunAdvance::Wait returns a Wait receipt")
            };
            Ok(RecoverySettlement::AwaitCapability { wait })
        }
    }
}

/// Stop the provider subtree before consuming a route choice. Failure leaves
/// the old AgentInvocation and Run fenced as unprovable; no stopped token
/// exists, so a caller cannot allocate a Recovery Run through this seam.
pub(crate) async fn stop_invocation_for_recovery(
    store: &SharedStore,
    lease: &RunLease,
    invocation_id: &AgentInvocationId,
    harness: &mut dyn Harness,
) -> Result<RecoveryStopOutcome, RunRouteRecoveryError> {
    match harness.stop().await {
        Ok(()) => Ok(RecoveryStopOutcome::Stopped(StoppedInvocation {
            invocation_id: invocation_id.clone(),
        })),
        Err(error) => {
            let error = format!("provider containment stop failed: {error}");
            let stop = store
                .stop_run(
                    lease,
                    StopCause::Failed {
                        reason: error.clone(),
                    },
                    ContainmentObservation::Unprovable,
                )
                .await?;
            Ok(RecoveryStopOutcome::Fenced {
                error,
                stop: Box::new(stop),
            })
        }
    }
}

pub(crate) fn capability_key(reasons: &[(ExactRoute, RouteUnavailable)]) -> String {
    if reasons.is_empty() {
        return "recovery_chain_exhausted".to_string();
    }
    reasons
        .iter()
        .map(|(route, reason)| {
            let account = route
                .account_id
                .as_ref()
                .map(ProviderAccountId::as_str)
                .unwrap_or("ambient");
            let reason = match reason {
                RouteUnavailable::Credential => "credential".to_string(),
                RouteUnavailable::Capacity { resets_at } => resets_at
                    .map(|reset| format!("capacity_until_{reset}"))
                    .unwrap_or_else(|| "capacity".to_string()),
                RouteUnavailable::Policy => "policy".to_string(),
            };
            format!("{}/{account}:{reason}", route.agent.agent())
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use async_trait::async_trait;
    use time::{Date, Month};

    use super::*;
    use crate::durable::{
        AdvanceReceipt, AgentInvocation, Containment, RunState, RunTrigger, WorkRef, WorkStatus,
    };
    use crate::engine::agent::AgentConfig;
    use crate::harness::Harness;
    use crate::id::WaveId;
    use crate::store::{open_store, StorageConfig};
    use crate::wave::Wave;

    const NOW: i64 = 1_000;

    fn today() -> Date {
        Date::from_calendar_date(2026, Month::July, 18).unwrap()
    }

    fn route(provider: &str, account_id: Option<&str>) -> ExactRoute {
        ExactRoute {
            agent: AgentRoute::new(provider.to_string(), None).unwrap(),
            account_id: account_id.map(|value| ProviderAccountId::parse(value).unwrap()),
        }
    }

    fn account(account_id: &str) -> ProviderAccount {
        ProviderAccount {
            provider: "claude".to_string(),
            account_id: ProviderAccountId::parse(account_id).unwrap(),
            home: None,
            login_email: None,
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: NOW,
            updated_at: NOW,
        }
    }

    fn evidence() -> RouteEvidence {
        RouteEvidence {
            within_grant: true,
            explicitly_selected: false,
            credential_resolves: true,
        }
    }

    fn limit(account_id: &str, used_percent: u8, resets_at: Option<i64>) -> AccountLimitRow {
        AccountLimitRow {
            provider: "claude".to_string(),
            account_id: ProviderAccountId::parse(account_id).unwrap(),
            window: "weekly".to_string(),
            used_percent,
            resets_at,
            plan: None,
            observed_at: NOW,
            source: "test".to_string(),
        }
    }

    fn candidate(route: ExactRoute, readiness: Result<(), RouteUnavailable>) -> RouteCandidate {
        RouteCandidate {
            route,
            readiness,
            strained: false,
        }
    }

    #[derive(Debug)]
    struct StopHarness {
        fails: bool,
    }

    #[async_trait]
    impl Harness for StopHarness {
        async fn start(&mut self, _config: &AgentConfig) -> anyhow::Result<()> {
            Ok(())
        }

        async fn send_input(&mut self, _content: &str) -> anyhow::Result<()> {
            Ok(())
        }

        async fn interrupt(&mut self) -> anyhow::Result<()> {
            Ok(())
        }

        async fn stop(&mut self) -> anyhow::Result<()> {
            if self.fails {
                anyhow::bail!("native descendants remain live");
            }
            Ok(())
        }

        fn provider_session_id(&self) -> Option<String> {
            None
        }
    }

    async fn wave_work() -> (SharedStore, WorkRef) {
        let directory = tempfile::tempdir().unwrap().keep();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(directory.join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = Wave::new(
            WaveId::new(),
            "route-recovery".to_string(),
            directory.display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let work = WorkRef::Wave(wave.id().clone());
        (store, work)
    }

    async fn start_invocation(
        store: &SharedStore,
        work: &WorkRef,
        route: &ExactRoute,
    ) -> (RunLease, AgentInvocation) {
        let (_run, lease) = store.reserve_run(work, RunTrigger::User).await.unwrap();
        start_run(store, &lease, route).await;
        let invocation = append_invocation(store, &lease, route).await;
        (lease, invocation)
    }

    async fn start_run(store: &SharedStore, lease: &RunLease, route: &ExactRoute) {
        store
            .advance_run(
                lease,
                RunAdvance::RunStarting {
                    containment: Containment::Tmux {
                        name: format!("lf-route-recovery-{}", route.agent.provider),
                    },
                    cwd: PathBuf::from("/tmp/route-recovery"),
                },
            )
            .await
            .unwrap();
    }

    async fn append_invocation(
        store: &SharedStore,
        lease: &RunLease,
        route: &ExactRoute,
    ) -> AgentInvocation {
        let receipt = store
            .advance_run(
                lease,
                RunAdvance::InvocationStarting {
                    route: InvocationRoute::from(route),
                    surface: "headless".to_string(),
                    resume_token: None,
                },
            )
            .await
            .unwrap();
        let AdvanceReceipt::Invocation(invocation) = receipt else {
            panic!("expected Invocation receipt")
        };
        invocation
    }

    #[test]
    fn exact_route_projection_keeps_unknown_capacity_runnable() {
        let account = account("work");
        let candidate = project_route_candidate(
            route("claude", Some("work")),
            Some(&account),
            &[],
            evidence(),
            NOW,
            today(),
        );

        assert_eq!(candidate.readiness, Ok(()));
    }

    #[test]
    fn exact_route_projection_distinguishes_missing_credentials() {
        let mut account = account("work");
        account.credential_state = CredentialState::Missing;
        let candidate = project_route_candidate(
            route("claude", Some("work")),
            Some(&account),
            &[],
            evidence(),
            NOW,
            today(),
        );

        assert_eq!(candidate.readiness, Err(RouteUnavailable::Credential));
    }

    #[test]
    fn exact_route_projection_distinguishes_active_cooldown() {
        let mut account = account("work");
        account.cooldown_until = Some(NOW + 60);
        let candidate = project_route_candidate(
            route("claude", Some("work")),
            Some(&account),
            &[],
            evidence(),
            NOW,
            today(),
        );

        assert_eq!(
            candidate.readiness,
            Err(RouteUnavailable::Capacity {
                resets_at: Some(NOW + 60)
            })
        );
    }

    #[test]
    fn exact_route_projection_distinguishes_exhausted_window() {
        let account = account("work");
        let candidate = project_route_candidate(
            route("claude", Some("work")),
            Some(&account),
            &[limit("work", 100, None)],
            evidence(),
            NOW,
            today(),
        );

        assert_eq!(
            candidate.readiness,
            Err(RouteUnavailable::Capacity { resets_at: None })
        );
    }

    #[test]
    fn exact_route_projection_distinguishes_policy_exclusion() {
        let account = account("work");
        let mut route_evidence = evidence();
        route_evidence.within_grant = false;
        let candidate = project_route_candidate(
            route("claude", Some("work")),
            Some(&account),
            &[],
            route_evidence,
            NOW,
            today(),
        );

        assert_eq!(candidate.readiness, Err(RouteUnavailable::Policy));
    }

    #[test]
    fn exact_route_projection_requires_explicit_only_selection() {
        let mut account = account("work");
        account.routing_state = RoutingState::ExplicitOnly;
        let exact_route = route("claude", Some("work"));
        let automatic = project_route_candidate(
            exact_route.clone(),
            Some(&account),
            &[],
            evidence(),
            NOW,
            today(),
        );
        let mut explicit_evidence = evidence();
        explicit_evidence.explicitly_selected = true;
        let explicit = project_route_candidate(
            exact_route,
            Some(&account),
            &[],
            explicit_evidence,
            NOW,
            today(),
        );

        assert_eq!(automatic.readiness, Err(RouteUnavailable::Policy));
        assert_eq!(explicit.readiness, Ok(()));
    }

    #[test]
    fn exact_route_projection_marks_strain_without_excluding() {
        let account = account("work");
        let candidate = project_route_candidate(
            route("claude", Some("work")),
            Some(&account),
            &[limit("work", 95, Some(NOW + 60))],
            evidence(),
            NOW,
            today(),
        );

        assert_eq!(candidate.readiness, Ok(()));
        assert!(candidate.strained);
    }

    #[test]
    fn exact_route_projection_requires_accountless_credential_evidence() {
        let exact_route = route("opencode", None);
        let available =
            project_route_candidate(exact_route.clone(), None, &[], evidence(), NOW, today());
        let mut missing_evidence = evidence();
        missing_evidence.credential_resolves = false;
        let missing =
            project_route_candidate(exact_route, None, &[], missing_evidence, NOW, today());

        assert_eq!(available.readiness, Ok(()));
        assert_eq!(missing.readiness, Err(RouteUnavailable::Credential));
    }

    #[test]
    fn route_recovery_policy_prefers_remaining_same_provider_route() {
        let work = route("claude", Some("work"));
        let personal = route("claude", Some("personal"));
        let backup = route("codex", Some("backup"));
        let candidates = vec![
            candidate(work, Err(RouteUnavailable::Capacity { resets_at: None })),
            candidate(personal.clone(), Ok(())),
            candidate(backup, Ok(())),
        ];

        assert_eq!(
            plan_route_recovery(&candidates, &[]),
            RecoveryChoice::Invoke(personal)
        );
    }

    #[test]
    fn route_recovery_policy_uses_backup_after_primary_exhaustion() {
        let backup = route("codex", Some("backup"));
        let candidates = vec![
            candidate(
                route("claude", Some("work")),
                Err(RouteUnavailable::Capacity { resets_at: None }),
            ),
            candidate(backup.clone(), Ok(())),
        ];

        assert_eq!(
            plan_route_recovery(&candidates, &[]),
            RecoveryChoice::Invoke(backup)
        );
    }

    #[test]
    fn route_recovery_policy_reports_typed_exhaustion() {
        let primary = route("claude", Some("work"));
        let backup = route("codex", Some("backup"));
        let candidates = vec![
            candidate(primary.clone(), Err(RouteUnavailable::Credential)),
            candidate(backup.clone(), Err(RouteUnavailable::Policy)),
        ];

        assert_eq!(
            plan_route_recovery(&candidates, &[]),
            RecoveryChoice::AwaitCapability {
                reasons: vec![
                    (primary, RouteUnavailable::Credential),
                    (backup, RouteUnavailable::Policy),
                ]
            }
        );
    }

    #[test]
    fn route_recovery_policy_excludes_each_current_chain_route() {
        let first = route("claude", Some("work"));
        let second = route("claude", Some("personal"));
        let candidates = vec![
            candidate(first.clone(), Ok(())),
            candidate(second.clone(), Ok(())),
        ];

        assert_eq!(
            plan_route_recovery(&candidates, &[first]),
            RecoveryChoice::Invoke(second)
        );
    }

    #[test]
    fn route_recovery_policy_derives_only_the_current_chain() {
        let earlier = route("claude", Some("work"));
        let current = route("claude", Some("personal"));
        let earlier_id = AgentInvocationId::new();
        let current_id = AgentInvocationId::new();
        let history = vec![
            RecoveryHistoryEntry {
                invocation_id: earlier_id,
                route: earlier.clone(),
                recovery_predecessor: None,
            },
            RecoveryHistoryEntry {
                invocation_id: current_id.clone(),
                route: current.clone(),
                recovery_predecessor: None,
            },
        ];

        assert_eq!(
            derive_chain_exclusions(&history, &current_id).unwrap(),
            vec![current]
        );
        assert_eq!(
            plan_route_recovery(
                &[candidate(earlier.clone(), Ok(()))],
                std::slice::from_ref(&earlier),
            ),
            RecoveryChoice::AwaitCapability { reasons: vec![] }
        );
        assert_eq!(
            plan_route_recovery(&[candidate(earlier.clone(), Ok(()))], &[]),
            RecoveryChoice::Invoke(earlier)
        );
    }

    #[test]
    fn route_recovery_policy_rebuilds_linked_chain_in_order() {
        let first = route("claude", Some("work"));
        let second = route("claude", Some("personal"));
        let first_id = AgentInvocationId::new();
        let second_id = AgentInvocationId::new();
        let history = vec![
            RecoveryHistoryEntry {
                invocation_id: first_id.clone(),
                route: first.clone(),
                recovery_predecessor: None,
            },
            RecoveryHistoryEntry {
                invocation_id: second_id.clone(),
                route: second.clone(),
                recovery_predecessor: Some(first_id),
            },
        ];

        assert_eq!(
            derive_chain_exclusions(&history, &second_id).unwrap(),
            vec![first, second]
        );
    }

    #[tokio::test]
    async fn failed_containment_stop_cannot_create_a_successor_invocation() {
        let (store, work) = wave_work().await;
        let first_route = route("claude", Some("work"));
        let (lease, first) = start_invocation(&store, &work, &first_route).await;
        let mut harness = StopHarness { fails: true };

        let stopped = stop_invocation_for_recovery(&store, &lease, &first.id, &mut harness)
            .await
            .unwrap();

        let RecoveryStopOutcome::Fenced { stop, .. } = stopped else {
            panic!("failed stop must fence the Run")
        };
        assert_eq!(stop.containment, ContainmentObservation::Unprovable);
        assert_eq!(stop.run.state, RunState::Stopping);
        assert_eq!(
            store
                .invocations_for_run(&lease.run_id)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(store
            .open_invocation_for_run(&lease.run_id)
            .await
            .unwrap()
            .unwrap()
            .ended_at
            .is_none());
        assert_eq!(
            store.current_run(&work).await.unwrap().unwrap().state,
            RunState::Stopping
        );
        assert!(store.validate_run_lease(&lease).await.is_err());
        assert!(store
            .advance_run(
                &lease,
                RunAdvance::InvocationEnded {
                    invocation_id: first.id,
                    outcome: BoundaryState::Failed,
                },
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn recovery_settlement_creates_recovery_runs_for_new_containment() {
        let (store, work) = wave_work().await;
        let work_route = route("claude", Some("work"));
        let personal_route = route("claude", Some("personal"));
        let backup_route = route("codex", Some("backup"));
        let (first_lease, first) = start_invocation(&store, &work, &work_route).await;
        let run_id = first_lease.run_id.clone();
        store
            .advance_run(
                &first_lease,
                RunAdvance::InvocationEnded {
                    invocation_id: first.id.clone(),
                    outcome: BoundaryState::Failed,
                },
            )
            .await
            .unwrap();
        let mut harness = StopHarness { fails: false };
        let RecoveryStopOutcome::Stopped(stopped) =
            stop_invocation_for_recovery(&store, &first_lease, &first.id, &mut harness)
                .await
                .unwrap()
        else {
            panic!("successful stop must mint settlement proof")
        };
        let RecoverySettlement::RecoveryRun {
            lease: second_lease,
            route: second_route,
        } = settle_route_recovery(
            &store,
            &first_lease,
            stopped,
            RecoveryChoice::Invoke(personal_route.clone()),
        )
        .await
        .unwrap()
        else {
            panic!("expected same-provider successor")
        };
        assert_eq!(second_route, personal_route);
        assert_ne!(second_lease.run_id, run_id);
        assert_eq!(
            store.run_by_id(&run_id).await.unwrap().state,
            RunState::Ended
        );
        assert_eq!(
            store
                .run_by_id(&second_lease.run_id)
                .await
                .unwrap()
                .retry_of,
            Some(run_id.clone())
        );
        assert!(store.validate_run_lease(&first_lease).await.is_err());
        start_run(&store, &second_lease, &second_route).await;
        let second = append_invocation(&store, &second_lease, &second_route).await;

        let RecoveryStopOutcome::Stopped(stopped) =
            stop_invocation_for_recovery(&store, &second_lease, &second.id, &mut harness)
                .await
                .unwrap()
        else {
            panic!("successful second stop must mint settlement proof")
        };
        let RecoverySettlement::RecoveryRun {
            lease: third_lease,
            route: third_route,
        } = settle_route_recovery(
            &store,
            &second_lease,
            stopped,
            RecoveryChoice::Invoke(backup_route.clone()),
        )
        .await
        .unwrap()
        else {
            panic!("expected backup-provider successor")
        };
        assert_eq!(third_route, backup_route);
        assert_ne!(third_lease.run_id, second_lease.run_id);
        assert_eq!(
            store.run_by_id(&third_lease.run_id).await.unwrap().retry_of,
            Some(second_lease.run_id.clone())
        );
        assert!(store.validate_run_lease(&second_lease).await.is_err());
        start_run(&store, &third_lease, &third_route).await;
        let third = append_invocation(&store, &third_lease, &third_route).await;

        let first_invocations = store.invocations_for_run(&run_id).await.unwrap();
        assert_eq!(first_invocations.len(), 1);
        assert_eq!(first_invocations[0].id, first.id);
        assert_eq!(first_invocations[0].route, first.route);
        assert!(first_invocations[0].ended_at.is_some());
        let second_invocations = store
            .invocations_for_run(&second_lease.run_id)
            .await
            .unwrap();
        assert_eq!(second_invocations.len(), 1);
        assert_eq!(second_invocations[0].id, second.id);
        assert_eq!(second_invocations[0].route, second.route);
        assert!(second_invocations[0].ended_at.is_some());
        let third_invocations = store
            .invocations_for_run(&third_lease.run_id)
            .await
            .unwrap();
        assert_eq!(third_invocations.len(), 1);
        assert_eq!(third_invocations[0].id, third.id);
        assert_eq!(third_invocations[0].route, third.route);
        assert!(third_invocations[0].ended_at.is_none());
    }

    #[tokio::test]
    async fn exhausted_routes_record_a_typed_capability_wait() {
        let (store, work) = wave_work().await;
        let first_route = route("claude", Some("work"));
        let backup_route = route("codex", Some("backup"));
        let (lease, first) = start_invocation(&store, &work, &first_route).await;
        let mut harness = StopHarness { fails: false };
        let RecoveryStopOutcome::Stopped(stopped) =
            stop_invocation_for_recovery(&store, &lease, &first.id, &mut harness)
                .await
                .unwrap()
        else {
            panic!("successful stop must mint settlement proof")
        };

        let RecoverySettlement::AwaitCapability { wait } = settle_route_recovery(
            &store,
            &lease,
            stopped,
            RecoveryChoice::AwaitCapability {
                reasons: vec![
                    (first_route, RouteUnavailable::Credential),
                    (backup_route, RouteUnavailable::Policy),
                ],
            },
        )
        .await
        .unwrap() else {
            panic!("expected typed capability Wait")
        };

        let WaitOn::Capability { capability } = &wait.on else {
            panic!("expected capability Wait")
        };
        assert_eq!(capability.kind, "provider_route");
        assert_eq!(capability.key, "claude/work:credential,codex/backup:policy");
        let WorkStatus::Waiting { wait: stored_wait } = store.work_status(&work).await.unwrap()
        else {
            panic!("expected Work to expose the capability Wait")
        };
        assert_eq!(stored_wait.id, wait.id);
        assert_eq!(stored_wait.on, wait.on);
        assert_eq!(
            store
                .invocations_for_run(&lease.run_id)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn exact_route_projection_round_trips_invocation_route() {
        let invocation_route = InvocationRoute {
            provider: "claude".to_string(),
            model: Some("opus".to_string()),
            account_id: Some("work".to_string()),
        };

        let exact = ExactRoute::try_from(&invocation_route).unwrap();

        assert_eq!(InvocationRoute::from(&exact), invocation_route);
    }
}
