//! Account authority forwarded by `lf ssh` through a foreground broker.
//!
//! The origin offers an ordered account catalog without refreshing credentials.
//! The target merges that catalog with its local accounts, then requests only
//! the selected credential. Descendants inherit the same bounded grant and
//! resumed provider sessions stay on the authority that owns their account.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::provider_account::{
    account_login, match_account, AccountMatch, ProviderAccountError, RateLimitSignal,
};
use crate::provider_auth::Provider;
use crate::store::{AccountLimitRow, ProviderAccount, ProviderAccountId, SharedStore};

/// The encoded [`AccountLeaseHandle`] carried across process boundaries.
pub const ACCOUNT_LEASE_ENV: &str = "LF_ACCOUNT_LEASE";
/// Target-side account preferences applied to the merged local/forwarded view.
pub const ACCOUNT_SELECTION_ENV: &str = "LF_ACCOUNT_SELECTION";

/// One provider's ordered grant. Preferred (`--account`-selected) ids form the
/// leading `preferred` entries in `accounts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProviderGrant {
    pub(crate) provider: Provider,
    pub(crate) accounts: Vec<ProviderAccountId>,
    pub(crate) preferred: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AccountLease {
    pub(crate) grants: Vec<ProviderGrant>,
    pub(crate) restricted: bool,
}

impl AccountLease {
    pub(crate) fn grant(&self, provider: Provider) -> Option<&ProviderGrant> {
        self.grants.iter().find(|grant| grant.provider == provider)
    }
}

/// One `--account claude=work` / `--only-account codex=reserve` token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProviderAccountSelector {
    provider: Option<Provider>,
    account: String,
}

impl ProviderAccountSelector {
    fn parse(value: &str) -> Result<Self, ProviderAccountError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(ProviderAccountError::Runtime(
                "account selector cannot be empty".to_string(),
            ));
        }
        let Some((provider, account)) = value.split_once('=') else {
            return Ok(Self {
                provider: None,
                account: value.to_string(),
            });
        };
        let provider = provider.parse::<Provider>().map_err(|_| {
            ProviderAccountError::Runtime(format!(
                "unknown account selector provider '{}'",
                provider.trim()
            ))
        })?;
        if !matches!(provider, Provider::Claude | Provider::Codex) {
            return Err(ProviderAccountError::UnsupportedProvider);
        }
        let account = account.trim();
        if account.is_empty() {
            return Err(ProviderAccountError::Runtime(format!(
                "{provider}= requires a login email or prefix"
            )));
        }
        Ok(Self {
            provider: Some(provider),
            account: account.to_string(),
        })
    }
}

/// Account selection at one CLI boundary. Origin-side SSH flags become grant
/// preferences; target-side flags cross the process boundary through
/// [`ACCOUNT_SELECTION_ENV`] and apply to the merged catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum SelectionMode {
    /// No flags: expose the normal catalog and routes.
    Default,
    /// `--account`: prefer these accounts, keep each provider's route as
    /// fallback.
    Prefer(Vec<ProviderAccountSelector>),
    /// `--only-account`: expose exactly these accounts, no fallback.
    Restrict(Vec<ProviderAccountSelector>),
}

/// Account flags before they become a grant or merged-catalog preference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSelection(SelectionMode);

impl Default for AccountSelection {
    fn default() -> Self {
        Self(SelectionMode::Default)
    }
}

impl AccountSelection {
    pub fn from_flags(
        preferred: &[String],
        restricted: &[String],
    ) -> Result<Self, ProviderAccountError> {
        if !preferred.is_empty() && !restricted.is_empty() {
            return Err(ProviderAccountError::Runtime(
                "--account and --only-account are mutually exclusive".to_string(),
            ));
        }
        let parse = |values: &[String]| {
            values
                .iter()
                .map(|value| ProviderAccountSelector::parse(value))
                .collect::<Result<Vec<_>, _>>()
        };
        if !preferred.is_empty() {
            Ok(Self(SelectionMode::Prefer(parse(preferred)?)))
        } else if !restricted.is_empty() {
            Ok(Self(SelectionMode::Restrict(parse(restricted)?)))
        } else {
            Ok(Self::default())
        }
    }

    pub fn is_default(&self) -> bool {
        matches!(self.0, SelectionMode::Default)
    }

    pub(crate) fn is_restricted(&self) -> bool {
        matches!(self.0, SelectionMode::Restrict(_))
    }

    fn selectors(&self) -> &[ProviderAccountSelector] {
        match &self.0 {
            SelectionMode::Prefer(selectors) | SelectionMode::Restrict(selectors) => selectors,
            SelectionMode::Default => &[],
        }
    }

    pub fn env_value(&self) -> Result<String, ProviderAccountError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| ProviderAccountError::AccountLease(error.to_string()))?;
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    }

    pub(crate) fn from_env() -> Result<Self, ProviderAccountError> {
        let Some(value) = std::env::var_os(ACCOUNT_SELECTION_ENV) else {
            return Ok(Self::default());
        };
        let value = value.into_string().map_err(|_| {
            ProviderAccountError::AccountLease(
                "LF_ACCOUNT_SELECTION is not valid UTF-8".to_string(),
            )
        })?;
        let bytes = URL_SAFE_NO_PAD
            .decode(value.trim())
            .map_err(|error| ProviderAccountError::AccountLease(error.to_string()))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| ProviderAccountError::AccountLease(error.to_string()))
    }

    pub(crate) fn resolved_accounts(
        &self,
        catalog: &[ProviderAccount],
    ) -> Result<Vec<(Provider, ProviderAccountId)>, ProviderAccountError> {
        resolve_selectors(catalog, self.selectors()).map(|resolved| {
            resolved
                .into_iter()
                .map(|resolved| (resolved.provider, resolved.account_id))
                .collect()
        })
    }
}

/// Broker location and request secret, without credentials or account ids.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AccountLeaseHandle {
    pub(crate) socket: PathBuf,
    pub(crate) secret: String,
}

impl std::fmt::Debug for AccountLeaseHandle {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountLeaseHandle")
            .field("socket", &self.socket)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

impl AccountLeaseHandle {
    pub(crate) fn encode(&self) -> Result<String, ProviderAccountError> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| ProviderAccountError::AccountLease(error.to_string()))?;
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    }

    fn decode(value: &str) -> Result<Self, ProviderAccountError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(value.trim())
            .map_err(|error| ProviderAccountError::AccountLease(error.to_string()))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| ProviderAccountError::AccountLease(error.to_string()))
    }
}

/// A resolved root lease plus the broker-private credentials that serve it.
pub(crate) struct PreparedAccountLease {
    pub(crate) lease: AccountLease,
    /// Tokens are populated only after the target selects an account. Tests may
    /// seed this cache directly; production preparation always leaves it empty.
    credentials: HashMap<(Provider, ProviderAccountId), String>,
    unavailable_credentials: HashSet<(Provider, ProviderAccountId)>,
    store: SharedStore,
    restricted: bool,
}

impl std::fmt::Debug for PreparedAccountLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedAccountLease")
            .field("lease", &self.lease)
            .field(
                "cached_credentials",
                &format_args!("{} redacted", self.credentials.len()),
            )
            .field("restricted", &self.restricted)
            .finish()
    }
}

fn supported_providers() -> [Provider; 2] {
    [Provider::Claude, Provider::Codex]
}

#[derive(Debug, Clone)]
struct ResolvedSelector {
    provider: Provider,
    account_id: ProviderAccountId,
}

fn resolve_selectors(
    catalog: &[ProviderAccount],
    selectors: &[ProviderAccountSelector],
) -> Result<Vec<ResolvedSelector>, ProviderAccountError> {
    let mut resolved = Vec::new();
    let mut seen = HashSet::new();
    for selector in selectors {
        let mut selector_matches = Vec::new();
        for provider in supported_providers() {
            if selector
                .provider
                .is_some_and(|selected| selected != provider)
            {
                continue;
            }
            let accounts = catalog
                .iter()
                .filter(|account| account.provider == provider.as_str())
                .collect::<Vec<_>>();
            match match_account(&accounts, &selector.account) {
                AccountMatch::One(account) => selector_matches.push(ResolvedSelector {
                    provider,
                    account_id: account.account_id.clone(),
                }),
                AccountMatch::Ambiguous(matches) => {
                    return Err(ProviderAccountError::Runtime(format!(
                        "'{}' matches several accounts: {}",
                        selector.account,
                        matches
                            .iter()
                            .map(|account| format!(
                                "{}/{}",
                                account.provider,
                                account_login(account)
                            ))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }
                AccountMatch::None => {}
            }
        }
        if selector_matches.is_empty() {
            let provider = selector
                .provider
                .map(|provider| format!("{provider} "))
                .unwrap_or_default();
            return Err(ProviderAccountError::Runtime(format!(
                "no managed {provider}account matches '{}'; see `lf auth accounts`",
                selector.account
            )));
        }
        for matched in selector_matches {
            if !seen.insert((matched.provider, matched.account_id.clone())) {
                return Err(ProviderAccountError::Runtime(format!(
                    "account selector duplicates {}/{}",
                    matched.provider, selector.account
                )));
            }
            resolved.push(matched);
        }
    }
    Ok(resolved)
}

fn grants_for_selection(
    routes: HashMap<Provider, Vec<ProviderAccountId>>,
    resolved: &[ResolvedSelector],
    selection: &AccountSelection,
) -> Vec<ProviderGrant> {
    supported_providers()
        .into_iter()
        .filter_map(|provider| {
            let selected = resolved
                .iter()
                .filter(|selected| selected.provider == provider)
                .map(|selected| selected.account_id.clone())
                .collect::<Vec<_>>();
            let route = routes.get(&provider).cloned().unwrap_or_default();
            let (accounts, preferred) = match &selection.0 {
                SelectionMode::Default => (route, 0),
                SelectionMode::Prefer(_) => {
                    let preferred = selected.len();
                    let mut accounts = selected.clone();
                    for account_id in route {
                        if !accounts.contains(&account_id) {
                            accounts.push(account_id);
                        }
                    }
                    (accounts, preferred)
                }
                SelectionMode::Restrict(_) => (selected.clone(), selected.len()),
            };
            (!accounts.is_empty()).then_some(ProviderGrant {
                provider,
                accounts,
                preferred,
            })
        })
        .collect()
}

/// Open the origin account store and prepare its forwarded catalog. Returns
/// `None` only when the default selection has no store or accounts to offer.
pub(crate) async fn prepare_root_lease(
    selection: &AccountSelection,
) -> Result<Option<PreparedAccountLease>, ProviderAccountError> {
    let store = match crate::store::open_existing_store().await {
        Some(store) => Arc::new(store),
        None if selection.is_default() => return Ok(None),
        None => {
            return Err(ProviderAccountError::Runtime(
                "account store unavailable for --account/--only-account".to_string(),
            ));
        }
    };
    let repo_id = super::current_repo_id()?;
    let catalog = store.list_provider_accounts(None).await?;
    let resolved = resolve_selectors(&catalog, selection.selectors())?;
    let mut routes = HashMap::new();
    for provider in supported_providers() {
        let mut route = super::provider_route_account_ids(&store, repo_id.as_ref(), provider)
            .await?
            .unwrap_or_default();
        for account in catalog
            .iter()
            .filter(|account| account.provider == provider.as_str())
        {
            if !route.contains(&account.account_id) {
                route.push(account.account_id.clone());
            }
        }
        routes.insert(provider, route);
    }
    let grants = grants_for_selection(routes, &resolved, selection);
    if grants.is_empty() {
        if selection.is_default() {
            return Ok(None);
        }
        return Err(ProviderAccountError::Runtime(
            "account selection produced no provider grant".to_string(),
        ));
    }
    let restricted = selection.is_restricted();
    let lease = AccountLease { grants, restricted };
    Ok(Some(PreparedAccountLease {
        lease,
        credentials: HashMap::new(),
        unavailable_credentials: HashSet::new(),
        store,
        restricted,
    }))
}

#[derive(Serialize, Deserialize)]
struct BrokerRequest {
    secret: String,
    operation: BrokerOperation,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BrokerOperation {
    Describe,
    Resolve {
        provider: Provider,
        provider_session_id: Option<String>,
    },
    ResolveExact {
        provider: Provider,
        account_id: ProviderAccountId,
        provider_session_id: Option<String>,
    },
    AccountFacts {
        provider: Provider,
        account_id: ProviderAccountId,
    },
    PinSession {
        provider: Provider,
        provider_session_id: String,
        account_id: ProviderAccountId,
    },
    PinnedAccount {
        provider: Provider,
        provider_session_id: String,
    },
    RecordHealth {
        provider: Provider,
        account_id: ProviderAccountId,
        signal: RateLimitSignal,
    },
    RecordCredentialInvalidated {
        provider: Provider,
        account_id: ProviderAccountId,
        reason: String,
    },
}

#[derive(Serialize, Deserialize)]
pub(crate) struct LeaseResolution {
    pub(crate) account_id: ProviderAccountId,
    access_token: String,
    pub(crate) resume_requested_session: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct LeaseAccountFacts {
    pub(crate) account: Option<ProviderAccount>,
    pub(crate) limits: Vec<AccountLimitRow>,
    pub(crate) credential_available: bool,
}

impl std::fmt::Debug for LeaseResolution {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LeaseResolution")
            .field("account_id", &self.account_id)
            .field("access_token", &"[REDACTED]")
            .finish()
    }
}

impl LeaseResolution {
    pub(crate) fn access_token(&self) -> &str {
        &self.access_token
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum BrokerResponse {
    Lease(AccountLease),
    Resolution(LeaseResolution),
    AccountFacts(Box<LeaseAccountFacts>),
    PinnedAccount(Option<ProviderAccountId>),
    Ok,
    Error(String),
}

struct BrokerState {
    prepared: PreparedAccountLease,
    secret: String,
    /// Preferred accounts already served once this run. A preferred account
    /// gets one attempt bypassing stored demotion; after a rate-limit signal it
    /// is spent and resolution falls through to the fallback route.
    spent_preferences: HashSet<(Provider, ProviderAccountId)>,
}

impl BrokerState {
    async fn access_token(
        &mut self,
        provider: Provider,
        account_id: &ProviderAccountId,
    ) -> Result<String, ProviderAccountError> {
        let key = (provider, account_id.clone());
        if let Some(access_token) = self.prepared.credentials.get(&key) {
            return Ok(access_token.clone());
        }
        let account = self
            .prepared
            .store
            .list_provider_accounts(Some(provider.as_str()))
            .await?
            .into_iter()
            .find(|account| account.account_id == *account_id)
            .ok_or_else(|| ProviderAccountError::NoAuthenticatedAccount {
                provider,
                accounts: format!("'{account_id}'"),
            })?;
        match crate::provider_account::prepare_account_access_token(provider, &account).await {
            Ok(access_token) => {
                self.prepared.credentials.insert(key, access_token.clone());
                Ok(access_token)
            }
            Err(error) => {
                self.prepared.unavailable_credentials.insert(key);
                Err(error)
            }
        }
    }

    async fn resolve(
        &mut self,
        provider: Provider,
        provider_session_id: Option<&str>,
    ) -> Result<LeaseResolution, ProviderAccountError> {
        let grant = self
            .prepared
            .lease
            .grant(provider)
            .cloned()
            .ok_or_else(|| {
                ProviderAccountError::Runtime(format!(
                    "{provider} is unavailable in this forwarded account lease"
                ))
            })?;
        let restricted = self.prepared.restricted;
        let mut requested_session = provider_session_id;
        let mut last_credential_error = None;
        loop {
            let viable = grant
                .accounts
                .iter()
                .filter(|account_id| {
                    !self
                        .prepared
                        .unavailable_credentials
                        .contains(&(provider, (*account_id).clone()))
                })
                .cloned()
                .collect::<Vec<_>>();
            if viable.is_empty() {
                return Err(last_credential_error.unwrap_or_else(|| {
                    ProviderAccountError::NoAuthenticatedAccount {
                        provider,
                        accounts: grant
                            .accounts
                            .iter()
                            .map(|account| format!("'{account}'"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    }
                }));
            }
            let resumed = match requested_session {
                Some(session_id) => self
                    .prepared
                    .store
                    .provider_session_account(provider, session_id)
                    .await?
                    .filter(|account_id| viable.contains(account_id)),
                None => None,
            };
            let preferred = if resumed.is_none() {
                let preferred = grant
                    .accounts
                    .iter()
                    .take(grant.preferred)
                    .find(|preferred| {
                        viable.contains(preferred)
                            && !self
                                .spent_preferences
                                .contains(&(provider, (*preferred).clone()))
                    })
                    .cloned();
                if let Some(account_id) = &preferred {
                    self.spent_preferences
                        .insert((provider, account_id.clone()));
                }
                preferred
            } else {
                None
            };
            let fallback = if restricted {
                viable.clone()
            } else {
                viable
                    .iter()
                    .filter(|account_id| {
                        !grant
                            .accounts
                            .iter()
                            .take(grant.preferred)
                            .any(|id| id == *account_id)
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            };
            let (selected, resume_requested_session) = if let Some(account_id) = resumed {
                (account_id, true)
            } else if let Some(preferred) = preferred {
                (preferred, false)
            } else {
                (
                    self.prepared
                        .store
                        .select_provider_account(provider, &fallback, None)
                        .await?
                        .ok_or_else(|| ProviderAccountError::NoEligibleAccount {
                            provider,
                            accounts: fallback
                                .iter()
                                .map(|account| format!("'{account}'"))
                                .collect::<Vec<_>>()
                                .join(", "),
                        })?
                        .account
                        .account_id,
                    false,
                )
            };
            match self.access_token(provider, &selected).await {
                Ok(access_token) => {
                    return Ok(LeaseResolution {
                        account_id: selected,
                        access_token,
                        resume_requested_session,
                    });
                }
                Err(error) => {
                    tracing::warn!(
                        provider = %provider,
                        account = %selected,
                        "forwarded account credential is unavailable; trying the next candidate: {error}"
                    );
                    last_credential_error = Some(error);
                    requested_session = None;
                }
            }
        }
    }

    async fn resolve_exact(
        &mut self,
        provider: Provider,
        account_id: &ProviderAccountId,
        provider_session_id: Option<&str>,
    ) -> Result<LeaseResolution, ProviderAccountError> {
        self.grant_contains(provider, account_id)?;
        let access_token = self.access_token(provider, account_id).await?;
        let resume_requested_session = match provider_session_id {
            Some(session_id) => self
                .prepared
                .store
                .provider_session_account(provider, session_id)
                .await?
                .is_some_and(|pinned| pinned == *account_id),
            None => false,
        };
        self.spent_preferences
            .insert((provider, account_id.clone()));
        Ok(LeaseResolution {
            account_id: account_id.clone(),
            access_token,
            resume_requested_session,
        })
    }

    fn grant_contains(
        &self,
        provider: Provider,
        account_id: &ProviderAccountId,
    ) -> Result<(), ProviderAccountError> {
        let grant = self.prepared.lease.grant(provider).ok_or_else(|| {
            ProviderAccountError::Runtime(format!(
                "{provider} is unavailable in this forwarded account lease"
            ))
        })?;
        if grant.accounts.contains(account_id) {
            Ok(())
        } else {
            Err(ProviderAccountError::Runtime(format!(
                "{provider}/{account_id} is outside the forwarded account lease"
            )))
        }
    }

    async fn handle(
        &mut self,
        request: BrokerRequest,
    ) -> Result<BrokerResponse, ProviderAccountError> {
        if request.secret != self.secret {
            return Err(ProviderAccountError::Runtime(
                "account lease is missing or expired".to_string(),
            ));
        }
        match request.operation {
            BrokerOperation::Describe => Ok(BrokerResponse::Lease(self.prepared.lease.clone())),
            BrokerOperation::Resolve {
                provider,
                provider_session_id,
            } => Ok(BrokerResponse::Resolution(
                self.resolve(provider, provider_session_id.as_deref())
                    .await?,
            )),
            BrokerOperation::ResolveExact {
                provider,
                account_id,
                provider_session_id,
            } => Ok(BrokerResponse::Resolution(
                self.resolve_exact(provider, &account_id, provider_session_id.as_deref())
                    .await?,
            )),
            BrokerOperation::AccountFacts {
                provider,
                account_id,
            } => {
                self.grant_contains(provider, &account_id)?;
                let mut account = self
                    .prepared
                    .store
                    .list_provider_accounts(Some(provider.as_str()))
                    .await?
                    .into_iter()
                    .find(|account| account.account_id == account_id);
                let credential_available = account.as_ref().is_some_and(|account| {
                    account.home.is_some()
                        && account.credential_state == crate::store::CredentialState::Connected
                        && !self
                            .prepared
                            .unavailable_credentials
                            .contains(&(provider, account_id.clone()))
                });
                if let Some(account) = &mut account {
                    account.home = None;
                }
                let limits = self
                    .prepared
                    .store
                    .provider_account_limits(Some(provider.as_str()))
                    .await?
                    .into_iter()
                    .filter(|limit| limit.account_id == account_id)
                    .collect();
                Ok(BrokerResponse::AccountFacts(Box::new(LeaseAccountFacts {
                    credential_available,
                    account,
                    limits,
                })))
            }
            BrokerOperation::PinnedAccount {
                provider,
                provider_session_id,
            } => {
                let account_id = self
                    .prepared
                    .store
                    .provider_session_account(provider, &provider_session_id)
                    .await?
                    .filter(|account_id| self.grant_contains(provider, account_id).is_ok());
                Ok(BrokerResponse::PinnedAccount(account_id))
            }
            BrokerOperation::PinSession {
                provider,
                provider_session_id,
                account_id,
            } => {
                self.grant_contains(provider, &account_id)?;
                self.spent_preferences
                    .remove(&(provider, account_id.clone()));
                self.prepared
                    .store
                    .pin_provider_session_route(provider, &provider_session_id, &account_id)
                    .await?;
                Ok(BrokerResponse::Ok)
            }
            BrokerOperation::RecordHealth {
                provider,
                account_id,
                signal,
            } => {
                self.grant_contains(provider, &account_id)?;
                if signal.limited {
                    self.spent_preferences
                        .insert((provider, account_id.clone()));
                }
                crate::provider_account::record_rate_limit_signal(
                    &self.prepared.store,
                    provider,
                    &account_id,
                    &signal,
                    "forwarded_stream",
                )
                .await?;
                Ok(BrokerResponse::Ok)
            }
            BrokerOperation::RecordCredentialInvalidated {
                provider,
                account_id,
                reason,
            } => {
                self.grant_contains(provider, &account_id)?;
                self.prepared
                    .store
                    .record_provider_account_credential_invalidated(
                        provider.as_str(),
                        &account_id,
                        &reason,
                    )
                    .await?;
                self.prepared
                    .credentials
                    .remove(&(provider, account_id.clone()));
                self.prepared
                    .unavailable_credentials
                    .insert((provider, account_id.clone()));
                self.spent_preferences.insert((provider, account_id));
                Ok(BrokerResponse::Ok)
            }
        }
    }
}

pub struct AccountLeaseBroker {
    secret: String,
    local_socket: PathBuf,
    remote_socket: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    _directory: tempfile::TempDir,
}

impl std::fmt::Debug for AccountLeaseBroker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountLeaseBroker")
            .field("local_socket", &self.local_socket)
            .field("remote_socket", &self.remote_socket)
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

impl AccountLeaseBroker {
    pub async fn start_root(
        selection: &AccountSelection,
    ) -> Result<Option<Self>, ProviderAccountError> {
        prepare_root_lease(selection)
            .await?
            .map(Self::start)
            .transpose()
    }

    pub fn local_env_value(&self) -> Result<String, ProviderAccountError> {
        self.local_handle().encode()
    }

    pub(crate) fn start(prepared: PreparedAccountLease) -> Result<Self, ProviderAccountError> {
        let directory = tempfile::Builder::new()
            .prefix("lf-account-lease-")
            .tempdir()
            .map_err(|error| ProviderAccountError::Filesystem(error.to_string()))?;
        let local_socket = directory.path().join("broker.sock");
        let cleanup_directory = directory.path().to_path_buf();
        crate::engine::agent::register_interrupt_cleanup(move || {
            let _ = fs::remove_dir_all(&cleanup_directory);
        });
        let listener = UnixListener::bind(&local_socket)
            .map_err(|error| ProviderAccountError::Filesystem(error.to_string()))?;
        listener
            .set_nonblocking(true)
            .map_err(|error| ProviderAccountError::Filesystem(error.to_string()))?;
        let remote_socket = PathBuf::from(format!("/tmp/lf-account-{}.sock", Uuid::new_v4()));
        let secret = Uuid::new_v4().to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_secret = secret.clone();
        let thread = thread::Builder::new()
            .name("lf-account-lease-broker".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("account lease broker runtime");
                let mut state = BrokerState {
                    prepared,
                    secret: thread_secret,
                    spent_preferences: HashSet::new(),
                };
                while !thread_stop.load(Ordering::Acquire) {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            if stream.set_nonblocking(false).is_err() {
                                continue;
                            }
                            let timeout = Some(Duration::from_secs(5));
                            if stream.set_read_timeout(timeout).is_err()
                                || stream.set_write_timeout(timeout).is_err()
                            {
                                continue;
                            }
                            let mut line = String::new();
                            let response = match BufReader::new(&mut stream).read_line(&mut line) {
                                Ok(_) => match serde_json::from_str::<BrokerRequest>(&line) {
                                    Ok(request) => match runtime.block_on(state.handle(request)) {
                                        Ok(response) => response,
                                        Err(error) => BrokerResponse::Error(error.to_string()),
                                    },
                                    Err(error) => BrokerResponse::Error(format!(
                                        "invalid account lease request: {error}"
                                    )),
                                },
                                Err(error) => BrokerResponse::Error(format!(
                                    "read account lease request: {error}"
                                )),
                            };
                            if let Ok(bytes) = serde_json::to_vec(&response) {
                                let _ = stream.write_all(&bytes);
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10));
                        }
                        Err(error) => {
                            tracing::warn!("account lease broker stopped accepting: {error}");
                            break;
                        }
                    }
                }
            })
            .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
        Ok(Self {
            secret,
            local_socket,
            remote_socket,
            stop,
            thread: Some(thread),
            _directory: directory,
        })
    }

    /// Handle a local child inherits: points at the broker's own socket.
    pub(crate) fn local_handle(&self) -> AccountLeaseHandle {
        AccountLeaseHandle {
            socket: self.local_socket.clone(),
            secret: self.secret.clone(),
        }
    }

    /// Handle forwarded over SSH: points at the `-R`-forwarded remote socket.
    pub(crate) fn remote_handle(&self) -> AccountLeaseHandle {
        AccountLeaseHandle {
            socket: self.remote_socket.clone(),
            secret: self.secret.clone(),
        }
    }

    pub(crate) fn local_socket(&self) -> &Path {
        &self.local_socket
    }

    pub(crate) fn remote_socket(&self) -> &Path {
        &self.remote_socket
    }
}

impl Drop for AccountLeaseBroker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = UnixStream::connect(&self.local_socket);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AccountLeaseClient {
    handle: AccountLeaseHandle,
}

impl AccountLeaseClient {
    pub(crate) fn from_env() -> Result<Option<Self>, ProviderAccountError> {
        match std::env::var_os(ACCOUNT_LEASE_ENV) {
            Some(value) if !value.is_empty() => {
                let value = value.into_string().map_err(|_| {
                    ProviderAccountError::AccountLease(
                        "LF_ACCOUNT_LEASE is not valid UTF-8".to_string(),
                    )
                })?;
                Ok(Some(Self {
                    handle: AccountLeaseHandle::decode(&value)?,
                }))
            }
            _ => Ok(None),
        }
    }

    fn request(&self, operation: BrokerOperation) -> Result<BrokerResponse, ProviderAccountError> {
        let mut stream = UnixStream::connect(&self.handle.socket).map_err(|error| {
            ProviderAccountError::Runtime(format!(
                "forwarded account lease is unavailable or expired: {error}"
            ))
        })?;
        let timeout = Some(Duration::from_secs(5));
        stream
            .set_read_timeout(timeout)
            .and_then(|()| stream.set_write_timeout(timeout))
            .map_err(|error| {
                ProviderAccountError::Runtime(format!(
                    "configure forwarded account lease timeout: {error}"
                ))
            })?;
        let bytes = serde_json::to_vec(&BrokerRequest {
            secret: self.handle.secret.clone(),
            operation,
        })
        .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
        stream
            .write_all(&bytes)
            .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
        stream
            .write_all(b"\n")
            .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
        let response = serde_json::from_slice::<BrokerResponse>(&response)
            .map_err(|error| ProviderAccountError::Runtime(error.to_string()))?;
        match response {
            BrokerResponse::Error(error) => Err(ProviderAccountError::Runtime(error)),
            response => Ok(response),
        }
    }

    pub(crate) fn describe(&self) -> Result<AccountLease, ProviderAccountError> {
        match self.request(BrokerOperation::Describe)? {
            BrokerResponse::Lease(lease) => Ok(lease),
            _ => Err(ProviderAccountError::Runtime(
                "account lease broker returned the wrong response".to_string(),
            )),
        }
    }

    #[cfg(test)]
    pub(crate) fn resolve(
        &self,
        provider: Provider,
        provider_session_id: Option<String>,
    ) -> Result<LeaseResolution, ProviderAccountError> {
        match self.request(BrokerOperation::Resolve {
            provider,
            provider_session_id,
        })? {
            BrokerResponse::Resolution(resolution) => Ok(resolution),
            _ => Err(ProviderAccountError::Runtime(
                "account lease broker returned the wrong response".to_string(),
            )),
        }
    }

    pub(crate) fn resolve_exact(
        &self,
        provider: Provider,
        account_id: &ProviderAccountId,
        provider_session_id: Option<String>,
    ) -> Result<LeaseResolution, ProviderAccountError> {
        match self.request(BrokerOperation::ResolveExact {
            provider,
            account_id: account_id.clone(),
            provider_session_id,
        })? {
            BrokerResponse::Resolution(resolution) => Ok(resolution),
            _ => Err(ProviderAccountError::Runtime(
                "account lease broker returned the wrong response".to_string(),
            )),
        }
    }

    pub(crate) fn account_facts(
        &self,
        provider: Provider,
        account_id: &ProviderAccountId,
    ) -> Result<LeaseAccountFacts, ProviderAccountError> {
        match self.request(BrokerOperation::AccountFacts {
            provider,
            account_id: account_id.clone(),
        })? {
            BrokerResponse::AccountFacts(facts) => Ok(*facts),
            _ => Err(ProviderAccountError::Runtime(
                "account lease broker returned the wrong response".to_string(),
            )),
        }
    }

    pub(crate) fn login_email(
        &self,
        provider: Provider,
        account_id: &ProviderAccountId,
    ) -> Result<String, ProviderAccountError> {
        let facts = self.account_facts(provider, account_id)?;
        let account = facts.account.ok_or_else(|| {
            ProviderAccountError::Runtime(format!(
                "forwarded {provider} account has no catalog entry"
            ))
        })?;
        account
            .login_email
            .map(|login| login.to_string())
            .ok_or_else(|| {
                ProviderAccountError::Runtime(format!(
                    "forwarded {provider} account has no login email"
                ))
            })
    }

    pub(crate) fn pin_session(
        &self,
        provider: Provider,
        provider_session_id: &str,
        account_id: &ProviderAccountId,
    ) -> Result<(), ProviderAccountError> {
        match self.request(BrokerOperation::PinSession {
            provider,
            provider_session_id: provider_session_id.to_string(),
            account_id: account_id.clone(),
        })? {
            BrokerResponse::Ok => Ok(()),
            _ => Err(ProviderAccountError::Runtime(
                "account lease broker returned the wrong response".to_string(),
            )),
        }
    }

    pub(crate) fn pinned_account(
        &self,
        provider: Provider,
        provider_session_id: &str,
    ) -> Result<Option<ProviderAccountId>, ProviderAccountError> {
        match self.request(BrokerOperation::PinnedAccount {
            provider,
            provider_session_id: provider_session_id.to_string(),
        })? {
            BrokerResponse::PinnedAccount(account_id) => Ok(account_id),
            _ => Err(ProviderAccountError::Runtime(
                "account lease broker returned the wrong response".to_string(),
            )),
        }
    }

    pub(crate) fn record_health(
        &self,
        provider: Provider,
        account_id: &ProviderAccountId,
        signal: &RateLimitSignal,
    ) -> Result<(), ProviderAccountError> {
        match self.request(BrokerOperation::RecordHealth {
            provider,
            account_id: account_id.clone(),
            signal: signal.clone(),
        })? {
            BrokerResponse::Ok => Ok(()),
            _ => Err(ProviderAccountError::Runtime(
                "account lease broker returned the wrong response".to_string(),
            )),
        }
    }

    pub(crate) fn record_credential_invalidated(
        &self,
        provider: Provider,
        account_id: &ProviderAccountId,
        reason: &str,
    ) -> Result<(), ProviderAccountError> {
        match self.request(BrokerOperation::RecordCredentialInvalidated {
            provider,
            account_id: account_id.clone(),
            reason: reason.to_string(),
        })? {
            BrokerResponse::Ok => Ok(()),
            _ => Err(ProviderAccountError::Runtime(
                "account lease broker returned the wrong response".to_string(),
            )),
        }
    }
}

pub fn account_lease_active() -> bool {
    std::env::var_os(ACCOUNT_LEASE_ENV).is_some_and(|value| !value.is_empty())
}

/// Confirm that this process inherited a valid lease and can reach its broker.
/// `lf ssh` runs this before the target command so an incompatible remote `lf`
/// or a failed socket forward cannot fall through to ambient remote accounts.
pub fn probe_forwarded_authority() -> Result<(), ProviderAccountError> {
    let client = AccountLeaseClient::from_env()?.ok_or_else(|| {
        ProviderAccountError::Runtime("forwarded account lease is unavailable".to_string())
    })?;
    client.describe()?;
    Ok(())
}

pub fn validate_account_selection(
    _selection: &AccountSelection,
) -> Result<(), ProviderAccountError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::os::unix::ffi::OsStringExt;
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::{
        account_lease_active, grants_for_selection, prepare_root_lease, probe_forwarded_authority,
        resolve_selectors, validate_account_selection, AccountLease, AccountLeaseBroker,
        AccountLeaseClient, AccountLeaseHandle, AccountSelection, PreparedAccountLease,
        ProviderAccountSelector, ProviderGrant, ResolvedSelector, ACCOUNT_LEASE_ENV,
        ACCOUNT_SELECTION_ENV,
    };
    use crate::profile::{ProviderRoute, RouteScope};
    use crate::provider_account::{new_account, parse_account_id, RateLimitSignal};
    use crate::provider_auth::Provider;
    use crate::store::{open_store, ProviderAccount, ProviderAccountId, StorageConfig};
    use tempfile::tempdir;
    fn id(value: &str) -> ProviderAccountId {
        parse_account_id(value).unwrap()
    }
    fn account(provider: Provider, account_id: &str, login: &str) -> ProviderAccount {
        let mut account = new_account(
            provider,
            id(account_id),
            PathBuf::from(format!("/accounts/{provider}/{account_id}")),
            Some(crate::profile::EmailAddress::parse(login).unwrap()),
        );
        account.updated_at = 1;
        account
    }
    fn selection_from(flag: &str) -> AccountSelection {
        AccountSelection::from_flags(&[flag.to_string()], &[]).unwrap()
    }
    fn resolved(
        catalog: &[ProviderAccount],
        selection: &AccountSelection,
    ) -> Vec<ResolvedSelector> {
        resolve_selectors(catalog, selection.selectors()).unwrap()
    }
    fn grant(grants: &[ProviderGrant], provider: Provider) -> &ProviderGrant {
        grants
            .iter()
            .find(|grant| grant.provider == provider)
            .unwrap()
    }
    struct RestoreEnv(&'static str, Option<std::ffi::OsString>);
    impl RestoreEnv {
        fn capture(name: &'static str) -> Self {
            Self(name, std::env::var_os(name))
        }
    }
    impl Drop for RestoreEnv {
        fn drop(&mut self) {
            match &self.1 {
                Some(value) => std::env::set_var(self.0, value),
                None => std::env::remove_var(self.0),
            }
        }
    }
    #[test]
    fn preference_orders_selected_first_and_keeps_provider_fallbacks() {
        let catalog = vec![
            account(Provider::Claude, "personal", "personal@example.com"),
            account(Provider::Claude, "work", "work@example.com"),
            account(Provider::Codex, "primary", "primary@example.com"),
            account(Provider::Codex, "work", "work@example.com"),
        ];
        let selection = selection_from("work");
        let grants = grants_for_selection(
            HashMap::from([
                (Provider::Claude, vec![id("personal")]),
                (Provider::Codex, vec![id("primary")]),
            ]),
            &resolved(&catalog, &selection),
            &selection,
        );
        assert_eq!(
            grant(&grants, Provider::Claude).accounts,
            vec![id("work"), id("personal")]
        );
        assert_eq!(
            grant(&grants, Provider::Codex).accounts,
            vec![id("work"), id("primary")]
        );
    }
    #[test]
    fn provider_qualified_selectors_target_one_provider() {
        let catalog = vec![
            account(Provider::Claude, "shared", "claude@example.com"),
            account(Provider::Codex, "shared", "codex@example.com"),
        ];
        let selection = AccountSelection::from_flags(&["codex=codex@".to_string()], &[]).unwrap();
        let grants = grants_for_selection(
            HashMap::from([(Provider::Claude, vec![id("shared")])]),
            &resolved(&catalog, &selection),
            &selection,
        );
        assert_eq!(grants.len(), 2);
        assert_eq!(grant(&grants, Provider::Codex).preferred, 1);
        assert_eq!(grant(&grants, Provider::Claude).preferred, 0);
    }
    #[test]
    fn restriction_exposes_only_selected_providers() {
        let catalog = vec![
            account(Provider::Claude, "personal", "personal@example.com"),
            account(Provider::Codex, "reserve", "reserve@example.com"),
        ];
        let selection = AccountSelection::from_flags(&[], &["codex=reserve".to_string()]).unwrap();
        let grants =
            grants_for_selection(HashMap::new(), &resolved(&catalog, &selection), &selection);
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].provider, Provider::Codex);
        assert_eq!(grants[0].accounts, vec![id("reserve")]);
    }
    #[test]
    fn ambiguous_and_duplicate_selectors_fail_before_launch() {
        let catalog = vec![
            account(
                Provider::Codex,
                "engineering-one",
                "engineering-one@example.com",
            ),
            account(
                Provider::Codex,
                "engineering-two",
                "engineering-two@example.com",
            ),
        ];
        let ambiguous = resolve_selectors(
            &catalog,
            &[ProviderAccountSelector::parse("codex=engineering").unwrap()],
        )
        .unwrap_err();
        assert!(ambiguous.to_string().contains("matches several accounts"));
        let duplicate = resolve_selectors(
            &catalog,
            &[
                ProviderAccountSelector::parse("codex=engineering-one@").unwrap(),
                ProviderAccountSelector::parse("codex=engineering-one@").unwrap(),
            ],
        )
        .unwrap_err();
        assert!(duplicate
            .to_string()
            .contains("duplicates codex/engineering-one@"));
    }
    #[test]
    fn forwarded_account_flags_round_trip_as_target_selection() {
        let _lock = crate::journal::test_env_lock();
        let _restore = RestoreEnv::capture(ACCOUNT_LEASE_ENV);
        let _selection_restore = RestoreEnv::capture(ACCOUNT_SELECTION_ENV);
        let selection = selection_from("codex=reserve");
        std::env::set_var(ACCOUNT_LEASE_ENV, "forwarded");
        validate_account_selection(&selection).unwrap();
        std::env::set_var(ACCOUNT_SELECTION_ENV, selection.env_value().unwrap());
        assert_eq!(AccountSelection::from_env().unwrap(), selection);
        validate_account_selection(&AccountSelection::default()).unwrap();
        std::env::remove_var(ACCOUNT_LEASE_ENV);
        validate_account_selection(&selection).unwrap();
        std::env::set_var(ACCOUNT_LEASE_ENV, std::ffi::OsString::from_vec(vec![0xff]));
        assert!(account_lease_active());
        assert!(AccountLeaseClient::from_env().is_err());
    }
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn root_preparation_is_lazy_and_falls_back_after_the_selected_credential_fails() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _home = RestoreEnv::capture("LF_HOME");
        let _database = RestoreEnv::capture("LF_DB_PATH");
        let _lease = RestoreEnv::capture(ACCOUNT_LEASE_ENV);
        std::env::set_var("LF_HOME", temp.path());
        std::env::remove_var("LF_DB_PATH");
        std::env::remove_var(ACCOUNT_LEASE_ENV);

        let store = Arc::new(
            open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
                .await
                .unwrap(),
        );
        let mut preferred = account(Provider::Claude, "preferred", "preferred@example.com");
        preferred.home = None;
        let mut fallback = account(Provider::Claude, "fallback", "fallback@example.com");
        fallback.home = Some(temp.path().join("claude-fallback"));
        fs::create_dir_all(fallback.home.as_ref().unwrap()).unwrap();
        fs::write(
            fallback.home.as_ref().unwrap().join(".credentials.json"),
            r#"{"claudeAiOauth":{"accessToken":"fallback-secret","expiresAt":4102444800000}}"#,
        )
        .unwrap();
        store.upsert_provider_account(&preferred).await.unwrap();
        store.upsert_provider_account(&fallback).await.unwrap();
        store
            .set_provider_route(&ProviderRoute {
                scope: RouteScope::Default,
                provider: Provider::Claude,
                accounts: vec![fallback.account_id.clone()],
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();

        let selection =
            AccountSelection::from_flags(&["claude=preferred".to_string()], &[]).unwrap();
        let prepared = prepare_root_lease(&selection)
            .await
            .unwrap()
            .expect("the fallback route should produce a lease");
        let grant = prepared.lease.grant(Provider::Claude).unwrap();
        assert_eq!(
            grant.accounts,
            vec![preferred.account_id.clone(), fallback.account_id.clone()]
        );
        assert_eq!(grant.preferred, 1);
        assert!(prepared.credentials.is_empty());

        let broker = AccountLeaseBroker::start(prepared).unwrap();
        let client = AccountLeaseClient {
            handle: broker.local_handle(),
        };
        assert_eq!(
            client.resolve(Provider::Claude, None).unwrap().account_id,
            fallback.account_id
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn target_selection_uses_one_merged_local_and_forwarded_catalog() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let _home = RestoreEnv::capture("LF_HOME");
        let _database = RestoreEnv::capture("LF_DB_PATH");
        let _lease = RestoreEnv::capture(ACCOUNT_LEASE_ENV);
        let _selection = RestoreEnv::capture(ACCOUNT_SELECTION_ENV);

        let target_home = temp.path().join("target");
        let target_database = target_home.join("loopflow.db");
        std::env::set_var("LF_HOME", &target_home);
        std::env::set_var("LF_DB_PATH", &target_database);
        std::env::remove_var(ACCOUNT_LEASE_ENV);
        std::env::remove_var(ACCOUNT_SELECTION_ENV);
        let target_store = Arc::new(
            open_store(&StorageConfig::sqlite(target_database))
                .await
                .unwrap(),
        );
        let mut local = account(Provider::Claude, "local", "local@example.com");
        local.home = Some(target_home.join("accounts/claude/local"));
        target_store.upsert_provider_account(&local).await.unwrap();
        target_store
            .set_provider_route(&ProviderRoute {
                scope: RouteScope::Default,
                provider: Provider::Claude,
                accounts: vec![local.account_id.clone()],
                created_at: 1,
                updated_at: 1,
            })
            .await
            .unwrap();

        let origin_store = Arc::new(
            open_store(&StorageConfig::sqlite(temp.path().join("origin.db")))
                .await
                .unwrap(),
        );
        let mut forwarded = account(Provider::Claude, "forwarded", "forwarded@example.com");
        forwarded.home = Some(temp.path().join("origin/accounts/claude/forwarded"));
        origin_store
            .upsert_provider_account(&forwarded)
            .await
            .unwrap();
        let broker = AccountLeaseBroker::start(PreparedAccountLease {
            lease: AccountLease {
                grants: vec![ProviderGrant {
                    provider: Provider::Claude,
                    accounts: vec![forwarded.account_id.clone()],
                    preferred: 0,
                }],
                restricted: false,
            },
            credentials: HashMap::from([(
                (Provider::Claude, forwarded.account_id.clone()),
                "forwarded-access-token".to_string(),
            )]),
            unavailable_credentials: HashSet::new(),
            store: origin_store,
            restricted: false,
        })
        .unwrap();
        std::env::set_var(ACCOUNT_LEASE_ENV, broker.local_env_value().unwrap());

        let route = crate::provider_account::resolve_provider_account(Provider::Claude, None)
            .await
            .unwrap()
            .expect("the target-local route should be available");
        assert_eq!(route.account_id(), &local.account_id);
        assert!(route.uses_native_home());

        let target_preference =
            AccountSelection::from_flags(&["claude=forwarded@".to_string()], &[]).unwrap();
        std::env::set_var(
            ACCOUNT_SELECTION_ENV,
            target_preference.env_value().unwrap(),
        );
        let route = crate::provider_account::resolve_provider_account(Provider::Claude, None)
            .await
            .unwrap()
            .expect("the forwarded account should be selectable on the target");
        assert_eq!(route.account_id(), &forwarded.account_id);
        assert!(!route.uses_native_home());

        let client = AccountLeaseClient::from_env().unwrap().unwrap();
        let facts = client
            .account_facts(Provider::Claude, &forwarded.account_id)
            .unwrap();
        assert!(facts.account.is_some_and(|account| account.home.is_none()));
    }

    /// Exercise the bounded grant across inheritance, fallback, resume, and cleanup.
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn broker_serves_a_fixed_grant_and_fails_closed_on_drop() {
        let temp = tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(temp.path().join("lease.db")))
                .await
                .unwrap(),
        );
        let grants = vec![
            ProviderGrant {
                provider: Provider::Claude,
                accounts: vec![id("personal"), id("work")],
                preferred: 0,
            },
            ProviderGrant {
                provider: Provider::Codex,
                accounts: vec![id("reserve"), id("primary")],
                preferred: 1,
            },
        ];
        let mut credentials = HashMap::new();
        for grant in &grants {
            for account_id in &grant.accounts {
                let mut account = new_account(
                    grant.provider,
                    account_id.clone(),
                    temp.path()
                        .join(grant.provider.as_str())
                        .join(account_id.as_str()),
                    Some(
                        crate::profile::EmailAddress::parse(&format!("{account_id}@example.com"))
                            .unwrap(),
                    ),
                );
                if grant.provider == Provider::Codex && account_id == &id("reserve") {
                    account.cooldown_until =
                        Some(time::OffsetDateTime::now_utc().unix_timestamp() + 3600);
                }
                store.upsert_provider_account(&account).await.unwrap();
                credentials.insert(
                    (grant.provider, account_id.clone()),
                    format!("{}-{account_id}-secret", grant.provider),
                );
            }
        }
        let lease = AccountLease {
            grants,
            restricted: false,
        };
        let expected_lease = serde_json::to_vec(&lease).unwrap();
        let restricted_credentials = credentials.clone();
        let prepared = PreparedAccountLease {
            lease,
            credentials,
            unavailable_credentials: HashSet::new(),
            store: Arc::clone(&store),
            restricted: false,
        };
        let broker = AccountLeaseBroker::start(prepared).unwrap();
        assert!(!format!("{broker:?}").contains(&broker.secret));
        let mut inherited = broker.local_handle();
        let mut client = AccountLeaseClient {
            handle: inherited.clone(),
        };
        for _ in 0..3 {
            inherited = AccountLeaseHandle::decode(&inherited.encode().unwrap()).unwrap();
            client = AccountLeaseClient {
                handle: inherited.clone(),
            };
            assert_eq!(
                serde_json::to_vec(&client.describe().unwrap()).unwrap(),
                expected_lease
            );
        }
        // A wrong secret is rejected: the lease is a capability, not the socket.
        let forged = AccountLeaseClient {
            handle: AccountLeaseHandle {
                socket: broker.local_socket().to_path_buf(),
                secret: "not-the-secret".to_string(),
            },
        };
        assert!(!format!("{forged:?}").contains("not-the-secret"));
        assert!(forged
            .describe()
            .unwrap_err()
            .to_string()
            .contains("missing or expired"));
        // A resumed fallback account does not consume the preferred attempt.
        store
            .pin_provider_session_route(Provider::Codex, "fallback-session", &id("primary"))
            .await
            .unwrap();
        let fallback_resume = client
            .resolve(Provider::Codex, Some("fallback-session".to_string()))
            .unwrap();
        assert_eq!(fallback_resume.account_id, id("primary"));
        assert!(fallback_resume.resume_requested_session);
        // Preferred `reserve` still gets its one attempt despite the cooldown.
        let codex = client.resolve(Provider::Codex, None).unwrap();
        assert_eq!(codex.account_id, id("reserve"));
        // A sibling resolution avoids the spent, rate-limited preferred.
        let sibling = client.resolve(Provider::Codex, None).unwrap();
        assert_eq!(sibling.account_id, id("primary"));
        assert!(
            client
                .account_facts(Provider::Claude, &id("personal"))
                .unwrap()
                .credential_available
        );
        assert_eq!(
            client
                .login_email(Provider::Claude, &id("personal"))
                .unwrap(),
            "personal@example.com"
        );
        let facts = client
            .account_facts(Provider::Codex, &id("reserve"))
            .unwrap();
        assert!(facts
            .account
            .is_some_and(|account| account.cooldown_until.is_some()));
        let exact = client
            .resolve_exact(Provider::Claude, &id("work"), None)
            .unwrap();
        assert_eq!(exact.account_id, id("work"));
        assert!(client
            .resolve_exact(Provider::Claude, &id("outside"), None)
            .is_err());
        // Resume stays on the account the store recorded for the session.
        store
            .pin_provider_session_route(Provider::Codex, "existing-session", &id("reserve"))
            .await
            .unwrap();
        let resumed = client
            .resolve(Provider::Codex, Some("existing-session".to_string()))
            .unwrap();
        assert_eq!(resumed.account_id, id("reserve"));
        assert!(resumed.resume_requested_session);
        // A remote-store account outside the grant stays unavailable.
        let remote_only = new_account(
            Provider::Codex,
            id("remote-only"),
            temp.path().join("codex").join("remote-only"),
            None,
        );
        store.upsert_provider_account(&remote_only).await.unwrap();
        store
            .pin_provider_session_route(Provider::Codex, "outside-session", &id("remote-only"))
            .await
            .unwrap();
        let outside = client
            .resolve(Provider::Codex, Some("outside-session".to_string()))
            .unwrap();
        assert_eq!(outside.account_id, id("primary"));
        assert!(!outside.resume_requested_session);
        let restricted = AccountLeaseBroker::start(PreparedAccountLease {
            lease: AccountLease {
                grants: vec![ProviderGrant {
                    provider: Provider::Codex,
                    accounts: vec![id("reserve"), id("primary")],
                    preferred: 2,
                }],
                restricted: true,
            },
            credentials: restricted_credentials,
            unavailable_credentials: HashSet::new(),
            store: Arc::clone(&store),
            restricted: true,
        })
        .unwrap();
        let restricted_client = AccountLeaseClient {
            handle: restricted.local_handle(),
        };
        assert_eq!(
            restricted_client
                .resolve(Provider::Codex, None)
                .unwrap()
                .account_id,
            id("reserve")
        );
        restricted_client
            .record_health(
                Provider::Codex,
                &id("reserve"),
                &RateLimitSignal {
                    utilization_percent: Some(100),
                    resets_at: None,
                    limited: true,
                    reason: "test".to_string(),
                    windows: Vec::new(),
                },
            )
            .unwrap();
        assert_eq!(
            restricted_client
                .resolve(Provider::Codex, None)
                .unwrap()
                .account_id,
            id("primary")
        );
        client
            .record_credential_invalidated(Provider::Codex, &id("reserve"), "token_invalidated")
            .unwrap();
        let invalidated = client
            .account_facts(Provider::Codex, &id("reserve"))
            .unwrap();
        assert!(!invalidated.credential_available);
        assert!(invalidated.account.is_some_and(|account| {
            account.credential_state == crate::store::CredentialState::Missing
                && account.cooldown_reason.as_deref() == Some("token_invalidated")
        }));
        assert!(client
            .resolve_exact(Provider::Codex, &id("reserve"), None)
            .is_err());
        assert_eq!(
            client.resolve(Provider::Codex, None).unwrap().account_id,
            id("primary")
        );
        let _lock = crate::journal::test_env_lock();
        let _restore = RestoreEnv::capture(ACCOUNT_LEASE_ENV);
        std::env::set_var(ACCOUNT_LEASE_ENV, broker.local_handle().encode().unwrap());
        let route = crate::provider_account::resolve_provider_account(Provider::Claude, None)
            .await
            .unwrap()
            .expect("forwarded grant should resolve a route");
        let mut command = std::process::Command::new("claude");
        command.env("CODEX_ACCESS_TOKEN", "ambient-secret");
        route.apply(&mut command);
        assert!(command.get_envs().any(|(name, value)| {
            name == "CLAUDE_CODE_OAUTH_TOKEN"
                && value.is_some_and(|value| {
                    let value = value.to_string_lossy();
                    value.starts_with("claude-") && value.ends_with("-secret")
                })
        }));
        assert!(command
            .get_envs()
            .any(|(name, value)| name == "CODEX_ACCESS_TOKEN" && value.is_none()));
        probe_forwarded_authority().unwrap();
        std::env::set_var(ACCOUNT_LEASE_ENV, "malformed");
        assert!(probe_forwarded_authority().is_err());
        std::env::remove_var(ACCOUNT_LEASE_ENV);
        assert!(probe_forwarded_authority().is_err());
        let socket = broker.local_socket().to_path_buf();
        drop(broker);
        assert!(!socket.exists());
        assert!(client
            .describe()
            .unwrap_err()
            .to_string()
            .contains("unavailable or expired"));
    }
}
