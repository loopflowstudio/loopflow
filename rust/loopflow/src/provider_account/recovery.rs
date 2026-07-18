//! Pure route readiness and recovery policy for sequential Launch replacement.

use std::collections::HashSet;

use thiserror::Error;

use crate::durable::{LaunchId, LaunchRoute};
use crate::engine::config::parse_agent;
use crate::store::{
    AccountLimitRow, CredentialState, ProviderAccount, ProviderAccountId, RoutingState,
};

/// The canonical provider and model selected for one agent Launch.
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
}

/// One exact provider, model, and optional managed-account route.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExactRoute {
    pub agent: AgentRoute,
    pub account_id: Option<ProviderAccountId>,
}

impl TryFrom<&LaunchRoute> for ExactRoute {
    type Error = ExactRouteError;

    fn try_from(route: &LaunchRoute) -> Result<Self, Self::Error> {
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

impl From<&ExactRoute> for LaunchRoute {
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
    #[error("Launch route provider cannot be empty")]
    EmptyProvider,
    #[error("invalid Launch route account id '{account_id}': {reason}")]
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
    Launch(ExactRoute),
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
            return RecoveryChoice::Launch(candidate.route.clone());
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
    pub launch_id: LaunchId,
    pub route: ExactRoute,
    pub recovery_predecessor: Option<LaunchId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RecoveryHistoryError {
    #[error("recovery history is missing Launch {0}")]
    MissingLaunch(LaunchId),
    #[error("recovery history contains a cycle at Launch {0}")]
    Cycle(LaunchId),
}

/// Walk recovery-predecessor links and return oldest-to-newest route exclusions.
pub fn derive_chain_exclusions(
    history: &[RecoveryHistoryEntry],
    failed_launch_id: &LaunchId,
) -> Result<Vec<ExactRoute>, RecoveryHistoryError> {
    let mut cursor = Some(failed_launch_id.clone());
    let mut seen = HashSet::new();
    let mut routes = Vec::new();

    while let Some(launch_id) = cursor {
        if !seen.insert(launch_id.clone()) {
            return Err(RecoveryHistoryError::Cycle(launch_id));
        }
        let entry = history
            .iter()
            .find(|entry| entry.launch_id == launch_id)
            .ok_or_else(|| RecoveryHistoryError::MissingLaunch(launch_id.clone()))?;
        routes.push(entry.route.clone());
        cursor = entry.recovery_predecessor.clone();
    }
    routes.reverse();
    Ok(routes)
}

#[cfg(test)]
mod tests {
    use time::{Date, Month};

    use super::*;

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
            &[limit("work", 80, Some(NOW + 60))],
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
            RecoveryChoice::Launch(personal)
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
            RecoveryChoice::Launch(backup)
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
            RecoveryChoice::Launch(second)
        );
    }

    #[test]
    fn route_recovery_policy_derives_only_the_current_chain() {
        let earlier = route("claude", Some("work"));
        let current = route("claude", Some("personal"));
        let earlier_id = LaunchId::new();
        let current_id = LaunchId::new();
        let history = vec![
            RecoveryHistoryEntry {
                launch_id: earlier_id,
                route: earlier.clone(),
                recovery_predecessor: None,
            },
            RecoveryHistoryEntry {
                launch_id: current_id.clone(),
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
            RecoveryChoice::Launch(earlier)
        );
    }

    #[test]
    fn route_recovery_policy_rebuilds_linked_chain_in_order() {
        let first = route("claude", Some("work"));
        let second = route("claude", Some("personal"));
        let first_id = LaunchId::new();
        let second_id = LaunchId::new();
        let history = vec![
            RecoveryHistoryEntry {
                launch_id: first_id.clone(),
                route: first.clone(),
                recovery_predecessor: None,
            },
            RecoveryHistoryEntry {
                launch_id: second_id.clone(),
                route: second.clone(),
                recovery_predecessor: Some(first_id),
            },
        ];

        assert_eq!(
            derive_chain_exclusions(&history, &second_id).unwrap(),
            vec![first, second]
        );
    }

    #[test]
    fn exact_route_projection_round_trips_launch_route() {
        let launch_route = LaunchRoute {
            provider: "claude".to_string(),
            model: Some("opus".to_string()),
            account_id: Some("work".to_string()),
        };

        let exact = ExactRoute::try_from(&launch_route).unwrap();

        assert_eq!(LaunchRoute::from(&exact), launch_route);
    }
}
