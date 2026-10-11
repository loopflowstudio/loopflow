//! Account selection carried across an lf process boundary.
use crate::provider_account::{account_login, match_account, AccountMatch, ProviderAccountError};
use crate::provider_auth::Provider;
use crate::store::{ProviderAccount, ProviderAccountId};
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
pub const ACCOUNT_SELECTION_ENV: &str = "LF_ACCOUNT_SELECTION";

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

/// Account selection at one CLI boundary, carried to children through
/// [`ACCOUNT_SELECTION_ENV`].
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

/// Explicit login preferences or restrictions within a machine catalog.
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

    pub(crate) fn from_flags_or_env(
        preferred: &[String],
        restricted: &[String],
    ) -> Result<Self, ProviderAccountError> {
        if preferred.is_empty() && restricted.is_empty() {
            Self::from_env()
        } else {
            Self::from_flags(preferred, restricted)
        }
    }

    pub(crate) fn activate(&self) -> Result<AccountSelectionGuard, ProviderAccountError> {
        let previous = std::env::var_os(ACCOUNT_SELECTION_ENV);
        std::env::set_var(ACCOUNT_SELECTION_ENV, self.env_value()?);
        Ok(AccountSelectionGuard(previous))
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
            .map_err(|error| ProviderAccountError::AccountSelection(error.to_string()))?;
        Ok(BASE64_URL_SAFE_NO_PAD.encode(bytes))
    }

    pub(crate) fn from_env() -> Result<Self, ProviderAccountError> {
        let Some(value) = std::env::var_os(ACCOUNT_SELECTION_ENV) else {
            return Ok(Self::default());
        };
        let value = value.into_string().map_err(|_| {
            ProviderAccountError::AccountSelection(
                "LF_ACCOUNT_SELECTION is not valid UTF-8".to_string(),
            )
        })?;
        let bytes = BASE64_URL_SAFE_NO_PAD
            .decode(value.trim())
            .map_err(|error| ProviderAccountError::AccountSelection(error.to_string()))?;
        serde_json::from_slice(&bytes)
            .map_err(|error| ProviderAccountError::AccountSelection(error.to_string()))
    }

    pub(crate) fn resolved_accounts(
        &self,
        catalog: &[ProviderAccount],
    ) -> Result<Vec<(Provider, ProviderAccountId)>, ProviderAccountError> {
        resolve_selectors(catalog, self.selectors())
    }
}

#[derive(Debug)]
pub(crate) struct AccountSelectionGuard(Option<std::ffi::OsString>);

impl Drop for AccountSelectionGuard {
    fn drop(&mut self) {
        match &self.0 {
            Some(value) => std::env::set_var(ACCOUNT_SELECTION_ENV, value),
            None => std::env::remove_var(ACCOUNT_SELECTION_ENV),
        }
    }
}

fn resolve_selectors(
    catalog: &[ProviderAccount],
    selectors: &[ProviderAccountSelector],
) -> Result<Vec<(Provider, ProviderAccountId)>, ProviderAccountError> {
    let mut resolved = Vec::new();
    let mut seen = HashSet::new();
    for selector in selectors {
        let mut selector_matches = Vec::new();
        for provider in [Provider::Claude, Provider::Codex] {
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
                AccountMatch::One(account) => {
                    selector_matches.push((provider, account.account_id.clone()))
                }
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
                "no managed {provider}account matches '{}'; see `lf account`",
                selector.account
            )));
        }
        for (provider, account_id) in selector_matches {
            if !seen.insert((provider, account_id.clone())) {
                return Err(ProviderAccountError::Runtime(format!(
                    "account selector duplicates {provider}/{}",
                    selector.account
                )));
            }
            resolved.push((provider, account_id));
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::AccountSelection;
    use crate::profile::EmailAddress;
    use crate::provider_account::{new_account, parse_account_id};
    use crate::provider_auth::Provider;
    use crate::store::ProviderAccount;

    fn account(provider: Provider, name: &str) -> ProviderAccount {
        new_account(
            provider,
            parse_account_id(name).unwrap(),
            format!("/accounts/{provider}/{name}").into(),
            Some(EmailAddress::parse(&format!("{name}@example.com")).unwrap()),
        )
    }

    #[test]
    fn account_selectors_resolve_across_providers_or_within_one_provider() {
        let catalog = [
            account(Provider::Claude, "work"),
            account(Provider::Codex, "work"),
        ];
        for (selector, providers) in [
            ("work@", vec![Provider::Claude, Provider::Codex]),
            ("codex=work@", vec![Provider::Codex]),
        ] {
            let selection = AccountSelection::from_flags(&[], &[selector.into()]).unwrap();
            assert!(selection.is_restricted());
            assert_eq!(
                selection.resolved_accounts(&catalog).unwrap(),
                providers
                    .into_iter()
                    .map(|provider| (provider, parse_account_id("work").unwrap()))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn ambiguous_duplicate_and_missing_logins_fail_before_launch() {
        let catalog = [
            account(Provider::Codex, "work-one"),
            account(Provider::Codex, "work-two"),
        ];
        for (selectors, diagnostic) in [
            (vec!["codex=work"], "matches several accounts"),
            (vec!["codex=work-one@", "codex=work-one@"], "duplicates"),
            (vec!["claude=work-one@"], "no managed claude account"),
        ] {
            let selectors = selectors
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();
            let selection = AccountSelection::from_flags(&selectors, &[]).unwrap();
            assert!(selection
                .resolved_accounts(&catalog)
                .unwrap_err()
                .to_string()
                .contains(diagnostic));
        }
    }
}
