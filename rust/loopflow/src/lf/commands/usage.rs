use std::collections::BTreeMap;

use anyhow::Result;
use time::OffsetDateTime;

use crate::journal::open_ledger;
use crate::lf::output::{format_int, truncate, Colors};
use crate::provider_account::open_account_store;
use crate::store::{AccountLimitRow, AccountLimitWindow, AttributedTurnUsage, ProviderAccount};
use crate::subscription::{poll_account, SubscriptionError};

const REPO_WIDTH: usize = 32;
const PROVIDER_WIDTH: usize = 12;
const NUM_WIDTH: usize = 14;
const ACCOUNT_WIDTH: usize = 30;
const WINDOW_WIDTH: usize = 14;

/// A stored observation older than this is re-polled before display.
const FRESH_SECS: i64 = 15 * 60;

/// `lf usage`: how much of each account's subscription is used, then token
/// usage by repo and provider. Both read local stores; accounts whose stored
/// window observations have gone stale are polled live first.
///
/// `--json` emits the canonical fixed-window usage snapshot consumed by every
/// CLI and UI surface. Provider account limits remain a separate subscription
/// concern and are rendered only in the human report.
pub fn run(json: bool, days: u32, refresh: bool, cached: bool) -> Result<()> {
    let ledger = open_ledger()?;
    if json {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        println!(
            "{}",
            serde_json::to_string(&crate::usage::snapshot(&ledger, now)?)?
        );
        return Ok(());
    }
    let since = if days == 0 {
        0
    } else {
        OffsetDateTime::now_utc().unix_timestamp() - i64::from(days) * 86_400
    };
    let usage = ledger.attributed_turn_usage_since(since)?;

    let runtime = tokio::runtime::Runtime::new()?;
    match runtime.block_on(account_statuses(refresh, cached)) {
        Ok(accounts) => print_accounts(&accounts),
        Err(error) => println!("accounts unavailable: {error}\n"),
    }
    print_report(&aggregate_usage(&usage), days);
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
    let forwarded_client = crate::provider_account::lease::AccountLeaseClient::from_env()?;
    let store = open_account_store().await?;
    let accounts: Vec<ProviderAccount> = store
        .list_provider_accounts(None)
        .await?
        .into_iter()
        .filter(|account| account.home.is_some())
        .collect();
    let stored = store.provider_account_limits(None).await?;
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
            label: if forwarded_client.is_some() {
                format!("{} (local)", account_label(account))
            } else {
                account_label(account)
            },
            plan: None,
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
                    "needs re-login: lf auth connect {} {}",
                    account.provider,
                    crate::provider_account::account_login(account)
                ));
            }
            Some(Err(SubscriptionError::Unavailable(reason))) => {
                status.note = Some(reason);
            }
            None => {}
        }
        // The plan is what the provider reports (poll planType, claude's
        // subscriptionType), never a hand-entered label; the stored lifecycle
        // column is only a fallback for accounts never yet observed.
        status.plan = status
            .windows
            .iter()
            .find_map(|window| window.plan.clone())
            .or_else(|| account.plan.clone());
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
    if let Some(client) = forwarded_client {
        for grant in client.describe()?.grants {
            for account_id in grant.accounts {
                let facts = client.account_facts(grant.provider, &account_id)?;
                let Some(account) = facts.account else {
                    continue;
                };
                let observed_at = facts.limits.iter().map(|row| row.observed_at).max();
                statuses.push(AccountStatus {
                    provider: account.provider.clone(),
                    label: format!("{} (forwarded)", account_label(&account)),
                    plan: facts.limits.iter().find_map(|row| row.plan.clone()),
                    windows: facts
                        .limits
                        .into_iter()
                        .map(|row| AccountLimitWindow {
                            window: row.window,
                            used_percent: row.used_percent,
                            resets_at: row.resets_at,
                            plan: row.plan,
                        })
                        .collect(),
                    observed_at,
                    note: (refresh && !cached)
                        .then(|| "forwarded · refresh on origin".to_string())
                        .or_else(|| Some("forwarded · cached".to_string())),
                });
            }
        }
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

fn account_label(account: &ProviderAccount) -> String {
    crate::provider_account::account_login(account).to_string()
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
        session = "SESSION USED",
        weekly = "WEEKLY USED",
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

/// A running sum over measured `(repo, provider)` fields. Optional counters
/// stay absent until at least one provider reports them.
#[derive(Default)]
struct Totals {
    input: Option<u64>,
    cache_read: Option<u64>,
    cache_write: Option<u64>,
    output: Option<u64>,
    reasoning: Option<u64>,
    cost_usd: Option<f64>,
}

impl Totals {
    fn add(&mut self, row: &UsageRow) {
        add_optional(&mut self.input, row.input_tokens);
        add_optional(&mut self.cache_read, row.cache_read_tokens);
        add_optional(&mut self.cache_write, row.cache_write_tokens);
        add_optional(&mut self.output, row.output_tokens);
        add_optional(&mut self.reasoning, row.reasoning_tokens);
        add_optional_f64(&mut self.cost_usd, row.cost_usd);
    }

    fn cells(&self) -> [String; 6] {
        [
            format_optional(self.input),
            format_optional(self.cache_read),
            format_optional(self.cache_write),
            format_optional(self.output),
            format_optional(self.reasoning),
            self.cost_usd
                .map(|cost| format!("${cost:.2}"))
                .unwrap_or_else(|| "-".to_string()),
        ]
    }
}

fn add_optional(total: &mut Option<u64>, value: Option<u64>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or(0).saturating_add(value));
    }
}

fn add_optional_f64(total: &mut Option<f64>, value: Option<f64>) {
    if let Some(value) = value {
        *total = Some(total.unwrap_or(0.0) + value);
    }
}

fn format_optional(value: Option<u64>) -> String {
    value.map(format_int).unwrap_or_else(|| "-".to_string())
}

/// Usage attributed to one `(repo, provider)` pair. A Turn reaches this report
/// through the Invocation that ran it, so a mixed-provider flow stays exact.
///
/// Global unattributed output remains visible in the canonical snapshot; this
/// historical table only groups provider Turns with a repository.
#[derive(Debug, PartialEq)]
struct UsageRow {
    repo: String,
    provider: String,
    input_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    output_tokens: Option<u64>,
    reasoning_tokens: Option<u64>,
    cost_usd: Option<f64>,
}

fn aggregate_usage(usage: &[AttributedTurnUsage]) -> Vec<UsageRow> {
    let mut rows: BTreeMap<(String, String), Totals> = BTreeMap::new();
    for turn in usage {
        let input = turn.usage.input_tokens;
        let cache_read = turn.usage.cache_read_tokens;
        let cache_write = turn.usage.cache_write_tokens;
        let output = turn.usage.output_tokens;
        let reasoning = turn.usage.reasoning_tokens;
        if input.is_none()
            && cache_read.is_none()
            && cache_write.is_none()
            && output.is_none()
            && reasoning.is_none()
            && turn.usage.cost_usd.is_none()
        {
            continue;
        }
        let totals = rows
            .entry((turn.repo.clone(), turn.provider.clone()))
            .or_default();
        totals.add(&UsageRow {
            repo: turn.repo.clone(),
            provider: turn.provider.clone(),
            input_tokens: input,
            cache_read_tokens: cache_read,
            cache_write_tokens: cache_write,
            output_tokens: output,
            reasoning_tokens: reasoning,
            cost_usd: turn.usage.cost_usd,
        });
    }
    rows.into_iter()
        .map(|((repo, provider), totals)| UsageRow {
            repo,
            provider,
            input_tokens: totals.input,
            cache_read_tokens: totals.cache_read,
            cache_write_tokens: totals.cache_write,
            output_tokens: totals.output,
            reasoning_tokens: totals.reasoning,
            cost_usd: totals.cost_usd,
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

    let mut by_provider: BTreeMap<&str, Totals> = BTreeMap::new();
    let mut grand = Totals::default();
    for row in rows {
        by_provider
            .entry(row.provider.as_str())
            .or_default()
            .add(row);
        grand.add(row);
    }
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
            &repo_lead(&truncate(&short_repo(&row.repo), REPO_WIDTH), &row.provider),
            totals.cells(),
            false,
        );
    }
    println!();

    print_row(&provider_lead("PROVIDER"), HEADINGS.map(String::from), true);
    for (provider, totals) in &by_provider {
        print_row(&provider_lead(provider), totals.cells(), false);
    }
    println!();

    print_row(&provider_lead("TOTAL"), grand.cells(), true);
}

const HEADINGS: [&str; 6] = [
    "INPUT",
    "CACHE READ",
    "CACHE WRITE",
    "OUTPUT",
    "REASONING",
    "COST",
];

fn repo_lead(repo: &str, provider: &str) -> String {
    format!("{repo:<REPO_WIDTH$}  {provider:<PROVIDER_WIDTH$}")
}

fn provider_lead(provider: &str) -> String {
    format!("{provider:<PROVIDER_WIDTH$}")
}

/// Both tables are the same six columns behind a label; only the label differs,
/// so one printer serves headers, rows, and totals.
fn print_row(lead: &str, cells: [String; 6], bold: bool) {
    let colors = Colors::default();
    let (on, off) = if bold {
        (colors.bold, colors.reset)
    } else {
        ("", "")
    };
    let [input, cache_read, cache_write, output, reasoning, cost] = cells;
    println!(
        "{on}{lead}  {input:>num_w$}  {cache_read:>num_w$}  {cache_write:>num_w$}  {output:>num_w$}  {reasoning:>num_w$}  {cost:>num_w$}{off}",
        num_w = NUM_WIDTH,
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
    use super::{account_label, aggregate_usage, format_window, short_repo, Totals, UsageRow};
    use crate::chat::types::TurnUsage;
    use crate::profile::EmailAddress;
    use crate::store::{
        sqlite::SqliteStore, AccountLimitWindow, AttributedTurnUsage, CredentialState,
        ProviderAccount, ProviderAccountId, RoutingState, TurnUsageSample,
    };
    use crate::trace::{AgentInvocationRow, AgentTurnRow};

    #[test]
    fn account_label_uses_login_email_without_internal_id() {
        let account = ProviderAccount {
            provider: "codex".to_string(),
            account_id: ProviderAccountId::parse("engineering").unwrap(),
            home: None,
            login_email: Some(EmailAddress::parse("loopflow-eng@loopflow.studio").unwrap()),
            credential_state: CredentialState::Connected,
            routing_state: RoutingState::Automatic,
            plan: None,
            paid_through: None,
            utilization_percent: None,
            cooldown_until: None,
            cooldown_reason: None,
            last_selected_at: None,
            created_at: 1,
            updated_at: 1,
        };

        assert_eq!(account_label(&account), "loopflow-eng@loopflow.studio");
    }

    fn invocation(id: &str) -> AgentInvocationRow {
        AgentInvocationRow {
            id: format!("invocation-{id}"),
            run_id: format!("trace-{id}"),
            answer_ask_id: None,
            process_id: format!("exec-{id}"),
            started_at: 100,
            ended_at: Some(110),
            repo: "/src/loopflow".to_string(),
            worktree: "/src/loopflow".to_string(),
            wave: None,
            flow: Some("build".to_string()),
            skill: Some("gate".to_string()),
            project: None,
            task: None,
            provider: "claude".to_string(),
            model: Some("opus".to_string()),
            surface: "headless".to_string(),
            capture_status: "complete".to_string(),
            incomplete_reason: None,
            outcome: "completed".to_string(),
            artifact_dir: "traces/invocation".to_string(),
            conversation_path: "traces/invocation/conversation.jsonl".to_string(),
            provider_events_path: None,
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: 1,
            conversation_bytes: 1,
            supervision: None,
        }
    }

    fn measured_turn(invocation: &AgentInvocationRow) -> AgentTurnRow {
        AgentTurnRow {
            id: invocation.id.replacen("invocation", "turn", 1),
            invocation_id: invocation.id.clone(),
            ordinal: 1,
            provider_turn_id: None,
            started_at: 100,
            ended_at: Some(110),
            status: "completed".to_string(),
            input_op: "initial".to_string(),
            context_coverage: "unknown".to_string(),
            tokenizer: "o200k_base".to_string(),
            system_prompt_path: None,
            task_prompt_path: "prompt.md".to_string(),
            system_tokens: 0,
            task_tokens: 0,
            supplied_context_tokens: 0,
            usage: None,
            context_gather_ms: 0,
            context_render_ms: 0,
            context_persist_ms: 0,
            first_event_seq: None,
            last_event_seq: None,
            root_output: None,
            basis: None,
        }
    }

    #[test]
    fn spend_query_keeps_zero_and_cache_only_but_omits_absent_usage() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = SqliteStore::new(&directory.path().join("loopflow.db")).expect("store");
        store
            .apply_migration_for_test("add_turn_usage_samples")
            .expect("usage sample migration");
        for id in ["absent", "zero", "cache", "write"] {
            let invocation = invocation(id);
            let turn = measured_turn(&invocation);
            store
                .insert_trace_capture(&invocation, &turn, &[], &[])
                .expect("insert capture");
            let usage = match id {
                "zero" => Some(TurnUsage {
                    output_tokens: Some(0),
                    ..Default::default()
                }),
                "cache" => Some(TurnUsage {
                    cache_read_tokens: Some(150),
                    ..Default::default()
                }),
                "write" => Some(TurnUsage {
                    cache_write_tokens: Some(75),
                    ..Default::default()
                }),
                _ => None,
            };
            if let Some(usage) = usage {
                store
                    .record_turn_usage_sample(&TurnUsageSample {
                        turn_id: turn.id,
                        observed_at: 110,
                        final_receipt: true,
                        usage,
                    })
                    .expect("record usage");
            }
        }

        let rows = store.attributed_turn_usage_since(0).expect("turn usage");

        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|row| row.turn_id != "turn-absent"));
        let zero = rows
            .iter()
            .find(|row| row.turn_id == "turn-zero")
            .expect("zero report");
        assert_eq!(zero.usage.output_tokens, Some(0));
        let cache = rows
            .iter()
            .find(|row| row.turn_id == "turn-cache")
            .expect("cache-only report");
        assert_eq!(cache.usage.input_tokens, None);
        assert_eq!(cache.usage.output_tokens, None);
        assert_eq!(cache.usage.cache_read_tokens, Some(150));
        let write = rows
            .iter()
            .find(|row| row.turn_id == "turn-write")
            .expect("cache-write-only report");
        assert_eq!(write.usage.cache_write_tokens, Some(75));
    }

    #[test]
    fn usage_checkpoints_are_monotonic_final_and_bounded() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = SqliteStore::new(&directory.path().join("loopflow.db")).expect("store");
        let invocation = invocation("progress");
        let turn = measured_turn(&invocation);
        store
            .insert_trace_capture(&invocation, &turn, &[], &[])
            .expect("insert capture");

        let checkpoint = |at, output, reasoning, final_receipt| TurnUsageSample {
            turn_id: turn.id.clone(),
            observed_at: at,
            final_receipt,
            usage: TurnUsage {
                output_tokens: Some(output),
                reasoning_tokens: reasoning,
                ..Default::default()
            },
        };
        store
            .record_turn_usage_sample(&checkpoint(0, 10, Some(4), false))
            .expect("first checkpoint");
        store
            .record_turn_usage_sample(&checkpoint(1, 20, Some(8), false))
            .expect("progress checkpoint");

        let decreased = store
            .record_turn_usage_sample(&checkpoint(2, 19, Some(8), false))
            .expect_err("cumulative output cannot decrease");
        assert!(decreased.to_string().contains("output_tokens decreased"));
        let invalid_breakdown = store
            .record_turn_usage_sample(&checkpoint(2, 20, Some(21), false))
            .expect_err("reasoning is included in output");
        assert!(invalid_breakdown
            .to_string()
            .contains("reasoning tokens exceed inclusive output"));

        store
            .record_turn_usage_sample(&checkpoint(90_000, 30, Some(12), true))
            .expect("final receipt");
        let reopened = store
            .record_turn_usage_sample(&checkpoint(90_001, 31, Some(12), false))
            .expect_err("a final receipt cannot become provisional");
        assert!(reopened.to_string().contains("cannot become provisional"));

        let retained = store
            .turn_usage_samples_since(0)
            .expect("retained checkpoints");
        assert_eq!(retained.len(), 1);
        assert_eq!(retained[0].observed_at, 90_000);
        assert!(retained[0].final_receipt);
        assert_eq!(retained[0].usage.output_tokens, Some(30));
    }

    fn row(repo: &str, provider: &str, input: u64) -> UsageRow {
        UsageRow {
            repo: repo.to_string(),
            provider: provider.to_string(),
            input_tokens: Some(input),
            cache_read_tokens: Some(2),
            cache_write_tokens: None,
            output_tokens: Some(1),
            reasoning_tokens: None,
            cost_usd: None,
        }
    }

    #[test]
    fn short_repo_takes_last_path_segment() {
        assert_eq!(short_repo("/Users/jack/src/loopflow"), "loopflow");
        assert_eq!(short_repo("loopflow"), "loopflow");
        assert_eq!(short_repo("/Users/jack/src/cadenza/"), "cadenza");
    }

    /// The per-provider table is a fold of the repo rows: one provider's usage
    /// across every repo reaches its total, and the grand total is every row.
    #[test]
    fn provider_rollup_folds_every_repo_row() {
        let rows = [
            row("/src/loopflow", "claude", 100),
            row("/src/cadenza", "claude", 50),
            row("/src/cadenza", "codex", 7),
        ];

        let mut claude = Totals::default();
        let mut grand = Totals::default();
        for row in &rows {
            if row.provider == "claude" {
                claude.add(row);
            }
            grand.add(row);
        }

        assert_eq!(claude.input, Some(150));
        assert_eq!(grand.input, Some(157));
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

    fn turn(process: &str, at: i64, provider: &str, input: i64) -> AttributedTurnUsage {
        AttributedTurnUsage {
            turn_id: format!("turn-{process}-{at}"),
            invocation_id: format!("invocation-{process}"),
            exec_id: process.to_string(),
            repo: "/src/loopflow".to_string(),
            wave: None,
            flow: Some("ship".to_string()),
            skill: Some("implement".to_string()),
            provider: provider.to_string(),
            at,
            usage: TurnUsage {
                input_tokens: Some(input as u64),
                total_input_tokens: Some(input as u64),
                output_tokens: Some(0),
                cache_read_tokens: Some(0),
                cost_usd: Some(input as f64 / 100.0),
                ..Default::default()
            },
        }
    }

    #[test]
    fn mixed_provider_flow_spend_stays_with_each_provider() {
        let rows = aggregate_usage(&[
            turn("process", 1, "claude", 100),
            turn("process", 2, "codex", 25),
        ]);

        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter()
                .find(|row| row.provider == "claude")
                .expect("claude row")
                .input_tokens,
            Some(100)
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.provider == "codex")
                .expect("codex row")
                .input_tokens,
            Some(25)
        );
    }

    #[test]
    fn processes_sharing_a_trace_remain_additive() {
        let rows = aggregate_usage(&[
            turn("parent", 1, "claude", 100),
            turn("child", 1, "claude", 5),
        ]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].input_tokens, Some(105));
    }
}
