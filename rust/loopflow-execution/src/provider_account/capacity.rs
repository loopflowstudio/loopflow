//! Account quota policy over saved provider observations.
use crate::store::{AccountLimitRow, AccountLimitWindow, ProviderAccount, StoreResult};
use loopflow_storage::capacity::{ProviderCapacityReading, ProviderCapacityUpdate};
use loopflow_storage::sqlite::SqliteStore;

fn is_model_weekly_window(window: &str, model: Option<&str>) -> bool {
    model.is_some_and(|model| {
        crate::config::CLAUDE_MODEL_FAMILIES.contains(&model)
            && window.strip_prefix("seven_day_") == Some(model)
    })
}

// The bucket a limit row describes: the name and scope a poll writes for a
// known Claude row, otherwise the row's own.
fn limit_bucket(provider: &str, window: &str, model: Option<&str>) -> (String, Option<String>) {
    (provider == "claude")
        .then(|| crate::provider_account::canonical_claude_window(window, model))
        .flatten()
        .unwrap_or_else(|| (window.to_owned(), model.map(str::to_owned)))
}

// Historical aliases can coexist with canonical observations. Prefer the newer
// reading; at equal second-resolution timestamps retain the stricter reading.
pub(crate) fn reconcile_legacy_account_windows(
    limits: Vec<AccountLimitRow>,
) -> Vec<AccountLimitRow> {
    let mut winners = std::collections::HashMap::new();
    let buckets: Vec<_> = limits
        .iter()
        .map(|row| limit_bucket(&row.provider, &row.window, row.model.as_deref()))
        .collect();
    for (index, (row, bucket)) in limits.iter().zip(&buckets).enumerate() {
        let rank = (
            row.observed_at,
            row.limited_until,
            row.used_percent,
            bucket.0 == row.window,
        );
        let winner = winners
            .entry((&row.provider, &row.account_id, bucket))
            .or_insert((rank, index));
        if rank > winner.0 {
            *winner = (rank, index);
        }
    }
    let kept: std::collections::HashSet<_> =
        winners.into_values().map(|(_, index)| index).collect();
    limits
        .into_iter()
        .zip(buckets)
        .enumerate()
        .filter(|(index, _)| kept.contains(index))
        .map(|(_, (mut row, (window, model)))| {
            row.window = window;
            row.model = model;
            row
        })
        .collect()
}

pub fn limits(store: &SqliteStore, provider: Option<&str>) -> StoreResult<Vec<AccountLimitRow>> {
    store
        .provider_account_limit_rows(provider)
        .map(reconcile_legacy_account_windows)
}

pub fn reconcile(
    store: &SqliteStore,
    account: &ProviderAccount,
    revision: i64,
    windows: &[AccountLimitWindow],
) -> StoreResult<bool> {
    let reading = store.provider_account_capacity(&account.provider, &account.account_id)?;
    if reading.revision != revision {
        return Ok(false);
    }
    let update = capacity_update(account, &reading, windows);
    store.apply_provider_account_capacity(&account.provider, &account.account_id, revision, &update)
}

fn capacity_update(
    account: &ProviderAccount,
    reading: &ProviderCapacityReading,
    windows: &[AccountLimitWindow],
) -> ProviderCapacityUpdate {
    let mut superseded_windows = Vec::new();
    let key = |name: &str, model: Option<&str>| limit_bucket(&account.provider, name, model);
    let cause = reading.cooldown_reason.as_deref().map(|reason| {
        if account.provider == "claude" {
            reason
        } else {
            reason.rsplit(": ").next().unwrap_or(reason)
        }
    });
    let cause_matches = |window: &crate::store::AccountLimitWindow| {
        cause.is_some_and(|cause| {
            key(cause, window.model.as_deref()) == key(&window.window, window.model.as_deref())
        })
    };
    let mut windows = windows.to_vec();
    for window in &mut windows {
        let bucket = key(&window.window, window.model.as_deref());
        let matching: Vec<_> = reading
            .limits
            .iter()
            .filter(|row| key(&row.window, row.model.as_deref()) == bucket)
            .collect();
        if window.used_percent >= 100 && window.limited_until.is_none() {
            window.limited_until = matching
                .iter()
                .filter_map(|row| row.limited_until)
                .max()
                .or_else(|| {
                    cause_matches(window)
                        .then_some(reading.cooldown_until)
                        .flatten()
                });
        }
        superseded_windows.extend(matching.into_iter().map(|row| row.window.clone()));
    }
    let recovered = windows.iter().any(|window| {
        cause_matches(window)
            && (is_model_weekly_window(&window.window, window.model.as_deref())
                || (matches!(
                    window.window.as_str(),
                    "five_hour" | "seven_day" | "weekly" | "session"
                ) && window.model.is_none()
                    && window.used_percent < 100))
    });

    ProviderCapacityUpdate {
        windows,
        superseded_windows,
        recovered,
    }
}
