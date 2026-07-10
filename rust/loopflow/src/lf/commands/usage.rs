use std::collections::BTreeMap;

use anyhow::Result;

use crate::journal::open_ledger;
use crate::lf::commands::runs::{boundary_spans, own_spend};
use crate::lf::output::{format_cost, format_int, truncate, Colors};
use crate::lfdb::RepoProviderUsage;

const REPO_WIDTH: usize = 32;
const PROVIDER_WIDTH: usize = 12;
const NUM_WIDTH: usize = 14;
const COST_WIDTH: usize = 10;

/// Print a repo x provider table of token usage and cost, with per-provider
/// and grand-total rollups. Reads the local run ledger directly — the same
/// store `lf runs` and `lf trace` read, and no running `lfd` required.
///
/// `--json` emits one row per *boundary* instead: what each skill, and each
/// terminal run, actually spent. That is the grain the dashboard groups by —
/// skill, `provider:model`, repo — and it is the only form in which "tokens by
/// skill" is answerable, because a skill row's reading is cumulative and must
/// be diffed against the boundary before it.
pub fn run(json: bool, days: u32) -> Result<()> {
    if json {
        return print_spend_json(days);
    }
    print_report(&open_ledger()?.aggregate_token_usage()?);
    Ok(())
}

fn print_spend_json(days: u32) -> Result<()> {
    let since = time::OffsetDateTime::now_utc().unix_timestamp() - i64::from(days) * 86_400;
    let events = open_ledger()?.list_run_events_since(since)?;
    let spend = own_spend(&boundary_spans(&events));
    println!("{}", serde_json::to_string(&spend)?);
    Ok(())
}

/// Rows recorded before the provider dimension existed, and runs whose repo
/// was never resolved, group under a NULL key.
fn or_unattributed(value: Option<&str>) -> &str {
    value.unwrap_or("(unattributed)")
}

/// A running sum over `(repo, provider)` rows — the only grain the ledger
/// aggregates. Every coarser row in this table is one of these.
#[derive(Default)]
struct Totals {
    input: u64,
    output: u64,
    cache: u64,
    cost: f64,
}

impl Totals {
    fn add(&mut self, row: &RepoProviderUsage) {
        self.input += row.input_tokens;
        self.output += row.output_tokens;
        self.cache += row.cache_read_tokens;
        self.cost += row.cost_usd;
    }

    fn cells(&self) -> [String; 5] {
        [
            format_int(self.input),
            format_int(self.output),
            format_int(self.cache),
            format_int(self.input + self.output),
            format_cost(self.cost),
        ]
    }
}

fn print_report(rows: &[RepoProviderUsage]) {
    if rows.is_empty() {
        println!("No token usage recorded yet.");
        return;
    }

    let mut by_provider: BTreeMap<Option<&str>, Totals> = BTreeMap::new();
    let mut grand = Totals::default();

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
            totals.cells(),
            false,
        );
        by_provider
            .entry(row.provider.as_deref())
            .or_default()
            .add(row);
        grand.add(row);
    }
    println!();

    print_row(&provider_lead("PROVIDER"), HEADINGS.map(String::from), true);
    for (provider, totals) in &by_provider {
        print_row(
            &provider_lead(or_unattributed(*provider)),
            totals.cells(),
            false,
        );
    }
    println!();

    print_row(&provider_lead("TOTAL"), grand.cells(), true);
}

const HEADINGS: [&str; 5] = ["INPUT", "OUTPUT", "CACHE READ", "TOTAL", "COST"];

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
    let [input, output, cache, total, cost] = cells;
    println!(
        "{on}{lead}  {input:>num_w$}  {output:>num_w$}  {cache:>num_w$}  {total:>num_w$}  {cost:>cost_w$}{off}",
        num_w = NUM_WIDTH,
        cost_w = COST_WIDTH,
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
    use super::{or_unattributed, short_repo, Totals};
    use crate::lfdb::RepoProviderUsage;

    fn row(repo: Option<&str>, provider: Option<&str>, input: u64, cost: f64) -> RepoProviderUsage {
        RepoProviderUsage {
            repo: repo.map(str::to_string),
            provider: provider.map(str::to_string),
            input_tokens: input,
            output_tokens: 1,
            cache_read_tokens: 2,
            cost_usd: cost,
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
            row(Some("/src/loopflow"), Some("claude"), 100, 1.0),
            row(None, Some("claude"), 50, 0.5),
            row(Some("/src/cadenza"), None, 7, 0.25),
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
        assert_eq!(claude.cost, 1.5);
        assert_eq!(grand.input, 157);
    }
}
