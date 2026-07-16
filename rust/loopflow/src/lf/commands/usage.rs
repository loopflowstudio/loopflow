use std::collections::BTreeMap;

use anyhow::Result;
use time::OffsetDateTime;

use crate::journal::open_ledger;
use crate::lf::commands::runs::{boundary_spans, SpanDto};
use crate::lf::output::{format_int, truncate, Colors};
use crate::provider_account::open_account_store;
use crate::store::{AccountLimitRow, AccountLimitWindow, ProviderAccount};
use crate::subscription::{poll_account, SubscriptionError};

const REPO_WIDTH: usize = 32;
const PROVIDER_WIDTH: usize = 12;
const NUM_WIDTH: usize = 14;
const SHARE_WIDTH: usize = 7;
const ACCOUNT_WIDTH: usize = 30;
const WINDOW_WIDTH: usize = 14;

/// A stored observation older than this is re-polled before display.
const FRESH_SECS: i64 = 15 * 60;

/// `lf usage`: how much of each account's subscription is used, then token
/// spend by repo and provider. Both read local stores; accounts whose stored
/// window observations have gone stale are polled live first.
///
/// `--json` emits one row per *boundary* instead: what each skill, and each
/// terminal run, actually spent. Rows are stored as per-boundary deltas, so
/// this is exactly the grain the dashboard groups by — skill,
/// `provider:model`, repo — and consumers sum, never diff.
pub fn run(json: bool, days: u32, refresh: bool, cached: bool) -> Result<()> {
    let since = if days == 0 {
        0
    } else {
        OffsetDateTime::now_utc().unix_timestamp() - i64::from(days) * 86_400
    };
    let events = open_ledger()?.list_run_events_since(since)?;
    let spend = boundary_spans(&events);
    if json {
        println!("{}", serde_json::to_string(&spend)?);
        return Ok(());
    }

    let runtime = tokio::runtime::Runtime::new()?;
    match runtime.block_on(account_statuses(refresh, cached)) {
        Ok(accounts) => print_accounts(&accounts),
        Err(error) => println!("accounts unavailable: {error}\n"),
    }
    print_report(&aggregate_spend(&spend), days);
    Ok(())
}

// -- Accounts ------------------------------------------------------------------

struct AccountStatus {
    provider: String,
    label: String,
    plan: Option<String>,
    windows: Vec<AccountLimitWindow>,
    observed_at: Option<i64>,
    note: Option<String>,
}

/// Every managed account with its freshest subscription windows: stored
/// observations when recent, a live poll when stale (or `--refresh`), stored
/// again when the poll fails. A revoked credential is reported as the fix
/// (`lf auth connect`), never a blank row.
async fn account_statuses(refresh: bool, cached: bool) -> Result<Vec<AccountStatus>> {
    let store = open_account_store().await?;
    let accounts: Vec<ProviderAccount> = store
        .list_provider_accounts(None)
        .await?
        .into_iter()
        .filter(|account| account.home.is_some())
        .collect();
    let stored = store.provider_account_limits(None).await?;
    let profiles = store.list_profile_provider_accounts(None).await?;
    let now = OffsetDateTime::now_utc().unix_timestamp();

    let polls = accounts.iter().map(|account| {
        let stored_windows = windows_for(&stored, account);
        let fresh = stored_windows
            .iter()
            .map(|row| row.observed_at)
            .max()
            .is_some_and(|observed| now - observed < FRESH_SECS);
        let should_poll = !cached && (refresh || !fresh);
        async move {
            if !should_poll {
                return None;
            }
            Some(poll_account(account).await)
        }
    });
    let polls = futures_util::future::join_all(polls).await;

    let mut statuses = Vec::new();
    for (account, poll) in accounts.iter().zip(polls) {
        let stored_windows = windows_for(&stored, account);
        let mut status = AccountStatus {
            provider: account.provider.clone(),
            label: account_label(account, &profiles),
            plan: account.plan.clone(),
            windows: stored_windows
                .iter()
                .map(|row| AccountLimitWindow {
                    window: row.window.clone(),
                    used_percent: row.used_percent,
                    resets_at: row.resets_at,
                    plan: row.plan.clone(),
                })
                .collect(),
            observed_at: stored_windows.iter().map(|row| row.observed_at).max(),
            note: None,
        };
        match poll {
            Some(Ok(usage)) => {
                store
                    .upsert_provider_account_limits(
                        &account.provider,
                        &account.account_id,
                        &usage.windows,
                        "poll",
                    )
                    .await?;
                status.windows = usage.windows;
                status.observed_at = Some(now);
            }
            Some(Err(SubscriptionError::NeedsLogin(_))) => {
                status.note = Some(format!(
                    "needs re-login: lf auth connect {}",
                    account.provider
                ));
            }
            Some(Err(SubscriptionError::Unavailable(reason))) => {
                status.note = Some(reason);
            }
            None => {}
        }
        if status.plan.is_none() {
            status.plan = status.windows.iter().find_map(|window| window.plan.clone());
        }
        let cooling = account.cooldown_until.filter(|until| *until > now);
        if let Some(until) = cooling {
            let note = format!("cooling until {}", format_reset(until, now));
            status.note = Some(match status.note.take() {
                Some(existing) => format!("{existing} · {note}"),
                None => note,
            });
        }
        statuses.push(status);
    }
    Ok(statuses)
}

fn windows_for<'a>(
    stored: &'a [AccountLimitRow],
    account: &ProviderAccount,
) -> Vec<&'a AccountLimitRow> {
    stored
        .iter()
        .filter(|row| row.provider == account.provider && row.account_id == account.account_id)
        .collect()
}

/// Profiles are how the rest of loopflow names these accounts, so lead with
/// the profile when one is bound; fall back to the login, then the raw id.
fn account_label(
    account: &ProviderAccount,
    profiles: &[crate::profile::ProfileProviderAccount],
) -> String {
    let bound: Vec<&str> = profiles
        .iter()
        .filter(|mapping| {
            mapping.provider.as_str() == account.provider
                && mapping.account_id == account.account_id
        })
        .map(|mapping| mapping.profile_id.as_str())
        .collect();
    if !bound.is_empty() {
        return bound.join(", ");
    }
    account
        .login_email
        .as_ref()
        .map(|email| email.to_string())
        .unwrap_or_else(|| account.account_id.to_string())
}

fn print_accounts(statuses: &[AccountStatus]) {
    if statuses.is_empty() {
        return;
    }
    let colors = Colors::default();
    let now = OffsetDateTime::now_utc().unix_timestamp();
    println!(
        "{bold}{provider:<PROVIDER_WIDTH$}  {account:<ACCOUNT_WIDTH$}  {plan:<6}  {session:<WINDOW_WIDTH$}  {weekly:<WINDOW_WIDTH$}  NOTE{reset}",
        bold = colors.bold,
        reset = colors.reset,
        provider = "PROVIDER",
        account = "ACCOUNT",
        plan = "PLAN",
        session = "SESSION",
        weekly = "WEEKLY",
    );
    for status in statuses {
        let mut note = status.note.clone().unwrap_or_default();
        if status.note.is_none() {
            if let Some(observed) = status.observed_at {
                if now - observed > FRESH_SECS {
                    note = format!("as of {}", format_age(now - observed));
                }
            }
        }
        println!(
            "{provider:<PROVIDER_WIDTH$}  {account:<ACCOUNT_WIDTH$}  {plan:<6}  {session:<WINDOW_WIDTH$}  {weekly:<WINDOW_WIDTH$}  {note}",
            provider = status.provider,
            account = truncate(&status.label, ACCOUNT_WIDTH),
            plan = status.plan.as_deref().unwrap_or("-"),
            session = format_window(&status.windows, "session", now),
            weekly = format_window(&status.windows, "weekly", now),
        );
    }
    println!();
}

/// Render one window group: the group's own window when present, otherwise
/// the tightest model-scoped one (annotated), otherwise a dash.
fn format_window(windows: &[AccountLimitWindow], group: &str, now: i64) -> String {
    let exact = windows.iter().find(|window| window.window == group);
    let scoped = windows
        .iter()
        .filter(|window| window.window.starts_with(&format!("{group}:")))
        .max_by_key(|window| window.used_percent);
    let Some(window) = exact.or(scoped) else {
        return "-".to_string();
    };
    let mut rendered = format!("{}%", window.used_percent);
    if let Some(resets_at) = window.resets_at.filter(|at| *at > now) {
        rendered.push_str(&format!(" → {}", format_reset(resets_at, now)));
    }
    if exact.is_none() {
        if let Some((_, scope)) = window.window.split_once(':') {
            rendered.push_str(&format!(" ({scope})"));
        }
    }
    rendered
}

fn format_reset(resets_at: i64, now: i64) -> String {
    let Some(reset) = chrono::DateTime::from_timestamp(resets_at, 0) else {
        return resets_at.to_string();
    };
    let local = reset.with_timezone(&chrono::Local);
    if resets_at - now < 24 * 3_600 {
        local.format("%H:%M").to_string()
    } else {
        local.format("%b %-d").to_string()
    }
}

fn format_age(seconds: i64) -> String {
    if seconds >= 86_400 {
        format!("{}d ago", seconds / 86_400)
    } else if seconds >= 3_600 {
        format!("{}h ago", seconds / 3_600)
    } else {
        format!("{}m ago", (seconds / 60).max(1))
    }
}

// -- Spend ---------------------------------------------------------------------

/// Rows recorded before the provider dimension existed, and runs whose repo
/// was never resolved, group under a NULL key.
fn or_unattributed(value: Option<&str>) -> &str {
    value.unwrap_or("(unattributed)")
}

/// A running sum over `(repo, provider)` rows — the only grain the ledger
/// reports. Every coarser row in this table is one of these.
#[derive(Default)]
struct Totals {
    input: u64,
    output: u64,
    cache: u64,
}

impl Totals {
    fn add(&mut self, row: &UsageRow) {
        self.input += row.input_tokens;
        self.output += row.output_tokens;
        self.cache += row.cache_read_tokens;
    }

    fn total(&self) -> u64 {
        self.input + self.output
    }

    fn cells(&self, grand_total: u64) -> [String; 5] {
        [
            format_int(self.input),
            format_int(self.output),
            format_int(self.cache),
            format_int(self.total()),
            format_share(self.total(), grand_total),
        ]
    }
}

fn format_share(total: u64, grand_total: u64) -> String {
    if grand_total == 0 {
        return "-".to_string();
    }
    let share = total as f64 * 100.0 / grand_total as f64;
    if total > 0 && share < 1.0 {
        return "<1%".to_string();
    }
    format!("{share:.0}%")
}

/// Spend attributed to one `(repo, provider)` pair, folded from the same
/// per-boundary rows emitted by `--json`: terminal-only SQL cannot assign a
/// flow that uses Claude for one skill and Codex for another.
#[derive(Debug, PartialEq)]
struct UsageRow {
    repo: Option<String>,
    provider: Option<String>,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
}

fn aggregate_spend(spend: &[SpanDto]) -> Vec<UsageRow> {
    let mut rows: BTreeMap<(Option<String>, Option<String>), Totals> = BTreeMap::new();
    for span in spend {
        let input = span.input_tokens.unwrap_or(0).max(0) as u64;
        let output = span.output_tokens.unwrap_or(0).max(0) as u64;
        let cache = span.cache_read_tokens.unwrap_or(0).max(0) as u64;
        if input == 0 && output == 0 && cache == 0 {
            continue;
        }
        let totals = rows
            .entry((span.repo.clone(), span.provider.clone()))
            .or_default();
        totals.input += input;
        totals.output += output;
        totals.cache += cache;
    }
    rows.into_iter()
        .map(|((repo, provider), totals)| UsageRow {
            repo,
            provider,
            input_tokens: totals.input,
            output_tokens: totals.output,
            cache_read_tokens: totals.cache,
        })
        .collect()
}

fn print_report(rows: &[UsageRow], days: u32) {
    let window = if days == 0 {
        "all time".to_string()
    } else {
        format!("last {days} days")
    };
    if rows.is_empty() {
        println!("No token usage recorded ({window}).");
        return;
    }

    let mut by_provider: BTreeMap<Option<&str>, Totals> = BTreeMap::new();
    let mut grand = Totals::default();
    for row in rows {
        by_provider
            .entry(row.provider.as_deref())
            .or_default()
            .add(row);
        grand.add(row);
    }
    let grand_total = grand.total();

    let colors = Colors::default();
    println!("{}SPEND ({window}){}", colors.bold, colors.reset);
    print_row(
        &repo_lead("REPO", "PROVIDER"),
        HEADINGS.map(String::from),
        true,
    );
    for row in rows {
        let mut totals = Totals::default();
        totals.add(row);
        print_row(
            &repo_lead(
                &truncate(
                    &short_repo(or_unattributed(row.repo.as_deref())),
                    REPO_WIDTH,
                ),
                or_unattributed(row.provider.as_deref()),
            ),
            totals.cells(grand_total),
            false,
        );
    }
    println!();

    print_row(&provider_lead("PROVIDER"), HEADINGS.map(String::from), true);
    for (provider, totals) in &by_provider {
        print_row(
            &provider_lead(or_unattributed(*provider)),
            totals.cells(grand_total),
            false,
        );
    }
    println!();

    print_row(&provider_lead("TOTAL"), grand.cells(grand_total), true);
}

const HEADINGS: [&str; 5] = ["INPUT", "OUTPUT", "CACHE READ", "TOTAL", "SHARE"];

fn repo_lead(repo: &str, provider: &str) -> String {
    format!("{repo:<REPO_WIDTH$}  {provider:<PROVIDER_WIDTH$}")
}

fn provider_lead(provider: &str) -> String {
    format!("{provider:<PROVIDER_WIDTH$}")
}

/// Both tables are the same five columns behind a label; only the label differs,
/// so one printer serves headers, rows, and totals.
fn print_row(lead: &str, cells: [String; 5], bold: bool) {
    let colors = Colors::default();
    let (on, off) = if bold {
        (colors.bold, colors.reset)
    } else {
        ("", "")
    };
    let [input, output, cache, total, share] = cells;
    println!(
        "{on}{lead}  {input:>num_w$}  {output:>num_w$}  {cache:>num_w$}  {total:>num_w$}  {share:>share_w$}{off}",
        num_w = NUM_WIDTH,
        share_w = SHARE_WIDTH,
    );
}

/// Show just the final path component when the repo is an absolute path, so
/// the table stays scannable; keep short names or slugs verbatim.
fn short_repo(repo: &str) -> String {
    repo.rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(repo)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_spend, format_share, format_window, or_unattributed, short_repo, Totals, UsageRow,
    };
    use crate::lf::commands::runs::SpanDto;
    use crate::store::AccountLimitWindow;

    fn row(repo: Option<&str>, provider: Option<&str>, input: u64) -> UsageRow {
        UsageRow {
            repo: repo.map(str::to_string),
            provider: provider.map(str::to_string),
            input_tokens: input,
            output_tokens: 1,
            cache_read_tokens: 2,
        }
    }

    #[test]
    fn short_repo_takes_last_path_segment() {
        assert_eq!(short_repo("/Users/jack/src/loopflow"), "loopflow");
        assert_eq!(short_repo("loopflow"), "loopflow");
        assert_eq!(short_repo("/Users/jack/src/cadenza/"), "cadenza");
    }

    #[test]
    fn null_repo_and_provider_read_as_unattributed() {
        assert_eq!(or_unattributed(None), "(unattributed)");
        assert_eq!(or_unattributed(Some("claude")), "claude");
    }

    /// The per-provider table is a fold of the repo rows, so a run with no repo
    /// still reaches the provider's total — and the grand total is every row.
    #[test]
    fn a_repoless_run_still_lands_in_the_provider_rollup() {
        let rows = [
            row(Some("/src/loopflow"), Some("claude"), 100),
            row(None, Some("claude"), 50),
            row(Some("/src/cadenza"), None, 7),
        ];

        let mut claude = Totals::default();
        let mut grand = Totals::default();
        for row in &rows {
            if row.provider.as_deref() == Some("claude") {
                claude.add(row);
            }
            grand.add(row);
        }

        assert_eq!(claude.input, 150);
        assert_eq!(grand.input, 157);
    }

    #[test]
    fn share_is_of_input_plus_output() {
        assert_eq!(format_share(50, 200), "25%");
        assert_eq!(format_share(0, 0), "-");
        assert_eq!(format_share(1, 1_000_000), "<1%", "small spend stays visible");
        assert_eq!(format_share(0, 200), "0%");
    }

    fn window(name: &str, percent: u8) -> AccountLimitWindow {
        AccountLimitWindow {
            window: name.to_string(),
            used_percent: percent,
            resets_at: None,
            plan: None,
        }
    }

    #[test]
    fn window_rendering_prefers_the_group_and_falls_back_to_scoped() {
        let windows = vec![window("session", 22), window("weekly:fable", 11)];
        assert_eq!(format_window(&windows, "session", 0), "22%");
        assert_eq!(format_window(&windows, "weekly", 0), "11% (fable)");
        assert_eq!(format_window(&[], "session", 0), "-");
    }

    fn boundary(process: &str, seq: i64, provider: &str, input: i64) -> SpanDto {
        SpanDto {
            run_id: "trace".to_string(),
            process_id: process.to_string(),
            parent_process_id: None,
            seq,
            node: "skill".to_string(),
            name: Some("implement".to_string()),
            repo: Some("/src/loopflow".to_string()),
            wave: None,
            flow: Some("ship".to_string()),
            skill: Some("implement".to_string()),
            started_at: seq,
            ended_at: Some(seq),
            status: "completed".to_string(),
            input_tokens: Some(input),
            output_tokens: Some(0),
            cache_read_tokens: Some(0),
            cost_usd: Some(input as f64 / 100.0),
            duration_secs: None,
            provider: Some(provider.to_string()),
            model: None,
        }
    }

    #[test]
    fn mixed_provider_flow_spend_stays_with_each_provider() {
        let rows = aggregate_spend(&[
            boundary("process", 1, "claude", 100),
            boundary("process", 2, "codex", 25),
        ]);

        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter()
                .find(|row| row.provider.as_deref() == Some("claude"))
                .expect("claude row")
                .input_tokens,
            100
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.provider.as_deref() == Some("codex"))
                .expect("codex row")
                .input_tokens,
            25
        );
    }

    #[test]
    fn processes_sharing_a_trace_remain_additive() {
        let rows = aggregate_spend(&[
            boundary("parent", 1, "claude", 100),
            boundary("child", 1, "claude", 5),
        ]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].input_tokens, 105);
    }
}
