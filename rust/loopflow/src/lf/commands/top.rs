//! `lf top` — one-hour output throughput and live Loopflow processes.

use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};

use crate::lf::commands::runs::{boundary_spans, own_spend, SpanDto};
use crate::lf::output::{format_int, truncate};
use crate::store::{sqlite::SqliteStore, RunEventRow};

const WINDOW_MINUTES: usize = 60;
const BUCKET_SECONDS: i64 = 60;
const GRAPH_HEIGHT: usize = 8;
const PROCESS_COMMAND_WIDTH: usize = 88;
const MAX_PROCESSES: usize = 20;

#[derive(Debug, PartialEq)]
struct LfProcess {
    pid: u32,
    elapsed: String,
    command: String,
}

pub fn run() -> Result<()> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    // Boundary usage is cumulative within each process, so the hour needs the
    // earlier boundary to avoid charging a long-running process's history now.
    let ledger_path = dirs::home_dir()
        .ok_or_else(|| anyhow!("home directory unavailable"))?
        .join(".lf/loopflow.db");
    let events = read_run_events(&ledger_path)?;
    let spend = own_spend(&boundary_spans(&events));
    let buckets = token_buckets(&spend, now);
    let processes = running_lf_processes()?;

    print!("{}", render_dashboard(&buckets, &processes));
    Ok(())
}

fn read_run_events(path: &Path) -> Result<Vec<RunEventRow>> {
    SqliteStore::open_run_ledger_read_only(path)
        .and_then(|store| store.list_run_events_since(0))
        .map_err(|error| anyhow!("failed to read run ledger {}: {error}", path.display()))
}

fn token_buckets(spend: &[SpanDto], now: i64) -> [u64; WINDOW_MINUTES] {
    let mut buckets = [0_u64; WINDOW_MINUTES];
    let start = now - WINDOW_MINUTES as i64 * BUCKET_SECONDS + 1;
    for span in spend {
        let Some(recorded_at) = span.ended_at else {
            continue;
        };
        if !(start..=now).contains(&recorded_at) {
            continue;
        }
        let index = ((recorded_at - start) / BUCKET_SECONDS) as usize;
        let output = span.output_tokens.unwrap_or(0).max(0) as u64;
        buckets[index] = buckets[index].saturating_add(output);
    }
    buckets
}

fn render_dashboard(buckets: &[u64; WINDOW_MINUTES], processes: &[LfProcess]) -> String {
    let mut output = String::new();
    output.push_str("LOOPFLOW THROUGHPUT · LAST 60 MINUTES\n");
    output.push_str("recorded output tokens/s · one-minute completion buckets\n\n");
    output.push_str(&render_graph(buckets));
    output.push('\n');
    output.push_str(&render_processes(processes));
    output
}

fn render_graph(buckets: &[u64; WINDOW_MINUTES]) -> String {
    let rates = buckets.map(|tokens| tokens as f64 / BUCKET_SECONDS as f64);
    let peak = rates.iter().copied().fold(0.0_f64, f64::max);
    let total = buckets.iter().copied().sum::<u64>();
    let average = total as f64 / (WINDOW_MINUTES as i64 * BUCKET_SECONDS) as f64;
    let current = rates[WINDOW_MINUTES - 1];
    let mut output = String::new();

    for level in (1..=GRAPH_HEIGHT).rev() {
        let label = if level == GRAPH_HEIGHT {
            format_rate(peak)
        } else {
            String::new()
        };
        let threshold = peak * level as f64 / GRAPH_HEIGHT as f64;
        let bars = rates
            .iter()
            .map(|rate| {
                if *rate > 0.0 && *rate >= threshold {
                    '█'
                } else {
                    ' '
                }
            })
            .collect::<String>();
        output.push_str(&format!("{label:>8} │{bars}\n"));
    }
    output.push_str(&format!("{:>8} └{}\n", "0.0", "─".repeat(WINDOW_MINUTES)));
    output.push_str(&format!("          60m ago{:>53}\n", "now"));
    output.push_str(&format!(
        "total {} tokens · avg {} tok/s · peak {} tok/s · current {} tok/s\n",
        format_int(total),
        format_rate(average),
        format_rate(peak),
        format_rate(current),
    ));
    output
}

fn format_rate(rate: f64) -> String {
    if rate > 0.0 && rate < 0.1 {
        format!("{rate:.2}")
    } else {
        format!("{rate:.1}")
    }
}

fn running_lf_processes() -> Result<Vec<LfProcess>> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,etime=,command="])
        .output()
        .map_err(|error| anyhow!("failed to inspect running processes: {error}"))?;
    if !output.status.success() {
        return Err(anyhow!("failed to inspect running processes with ps"));
    }
    Ok(parse_processes(
        &String::from_utf8_lossy(&output.stdout),
        std::process::id(),
    ))
}

fn parse_processes(output: &str, current_pid: u32) -> Vec<LfProcess> {
    let mut processes = output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let pid = fields.next()?.parse::<u32>().ok()?;
            let elapsed = fields.next()?.trim().to_string();
            let command = fields.collect::<Vec<_>>().join(" ");
            if command.is_empty() {
                return None;
            }
            if pid == current_pid || !is_lf_command(&command) {
                return None;
            }
            Some(LfProcess {
                pid,
                elapsed,
                command,
            })
        })
        .collect::<Vec<_>>();
    processes.sort_by_key(|process| std::cmp::Reverse(elapsed_seconds(&process.elapsed)));
    processes
}

fn is_lf_command(command: &str) -> bool {
    command.split_whitespace().next().is_some_and(|word| {
        std::path::Path::new(word)
            .file_name()
            .and_then(|name| name.to_str())
            == Some("lf")
    })
}

fn elapsed_seconds(elapsed: &str) -> u64 {
    let (days, clock) = elapsed
        .split_once('-')
        .map_or((0, elapsed), |(days, clock)| {
            (days.parse::<u64>().unwrap_or(0), clock)
        });
    let parts = clock
        .split(':')
        .filter_map(|part| part.parse::<u64>().ok())
        .collect::<Vec<_>>();
    let clock_seconds = match parts.as_slice() {
        [minutes, seconds] => minutes.saturating_mul(60).saturating_add(*seconds),
        [hours, minutes, seconds] => hours
            .saturating_mul(3_600)
            .saturating_add(minutes.saturating_mul(60))
            .saturating_add(*seconds),
        _ => 0,
    };
    days.saturating_mul(86_400).saturating_add(clock_seconds)
}

fn render_processes(processes: &[LfProcess]) -> String {
    if processes.is_empty() {
        return "RUNNING LF PROCESSES\nnone\n".to_string();
    }

    let heading = if processes.len() > MAX_PROCESSES {
        format!(
            "RUNNING LF PROCESSES ({}; {MAX_PROCESSES} oldest shown)\n",
            processes.len()
        )
    } else {
        format!("RUNNING LF PROCESSES ({})\n", processes.len())
    };
    let mut output = heading;
    output.push_str("     PID  ELAPSED       COMMAND\n");
    for process in processes.iter().take(MAX_PROCESSES) {
        output.push_str(&format!(
            "{:>8}  {:<12}  {}\n",
            process.pid,
            process.elapsed,
            truncate(&process.command, PROCESS_COMMAND_WIDTH),
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        parse_processes, read_run_events, render_dashboard, token_buckets, LfProcess,
        WINDOW_MINUTES,
    };
    use crate::lf::commands::runs::SpanDto;
    use crate::store::{sqlite::SqliteStore, RunEventRow};

    fn spend(recorded_at: i64, input: i64, output: i64, cache: i64) -> SpanDto {
        SpanDto {
            run_id: "run".to_string(),
            process_id: "process".to_string(),
            parent_process_id: None,
            seq: recorded_at,
            node: "run".to_string(),
            name: Some("lf implement".to_string()),
            repo: Some("/src/loopflow".to_string()),
            wave: None,
            flow: None,
            skill: None,
            started_at: recorded_at,
            ended_at: Some(recorded_at),
            status: "completed".to_string(),
            input_tokens: Some(input),
            output_tokens: Some(output),
            cache_read_tokens: Some(cache),
            cost_usd: None,
            duration_secs: None,
            provider: Some("codex".to_string()),
            model: None,
        }
    }

    #[test]
    fn telemetry_reader_ignores_newer_migration_history_without_writing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        let store = SqliteStore::new(&path).unwrap();
        store
            .insert_run_event(&RunEventRow {
                run_id: "run".to_string(),
                process_id: "process".to_string(),
                parent_process_id: None,
                seq: 0,
                ts: 1,
                repo: Some("/repo".to_string()),
                worktree: Some("/repo".to_string()),
                wave: None,
                node: "run".to_string(),
                event: "completed".to_string(),
                command: Some("lf top".to_string()),
                flow: None,
                skill: None,
                step_index: None,
                error: None,
                input_tokens: Some(10),
                output_tokens: Some(5),
                cache_read_tokens: None,
                cost_usd: None,
                duration_secs: None,
                provider: Some("codex".to_string()),
                model: None,
            })
            .unwrap();
        drop(store);
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at)
                 VALUES ('99.99.999_future', unixepoch())",
                [],
            )
            .unwrap();
        drop(connection);

        let events = read_run_events(&path).unwrap();

        assert_eq!(events.len(), 1);
        let connection = rusqlite::Connection::open(&path).unwrap();
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM schema_migrations WHERE version = '99.99.999_future'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
    }

    #[test]
    fn buckets_last_hour_output_tokens() {
        let now = 10_000;
        let buckets = token_buckets(
            &[
                spend(now - 3_599, 60, 30, 1_000),
                spend(now - 1, 120, 60, 2_000),
                spend(now, 30, 30, 0),
                spend(now - 3_600, 9_999, 9_999, 0),
            ],
            now,
        );

        assert_eq!(buckets[0], 30);
        assert_eq!(buckets[WINDOW_MINUTES - 1], 90);
        assert_eq!(buckets.iter().sum::<u64>(), 120);
    }

    #[test]
    fn dashboard_shows_rate_summary_and_live_processes() {
        let mut buckets = [0; WINDOW_MINUTES];
        buckets[WINDOW_MINUTES - 1] = 120;
        let rendered = render_dashboard(
            &buckets,
            &[LfProcess {
                pid: 42,
                elapsed: "01:12".to_string(),
                command: "lf __task ts_123 --generation 1".to_string(),
            }],
        );

        assert!(rendered.contains("LOOPFLOW THROUGHPUT · LAST 60 MINUTES"));
        assert!(rendered.contains("recorded output tokens/s"));
        assert!(rendered
            .contains("total 120 tokens · avg 0.03 tok/s · peak 2.0 tok/s · current 2.0 tok/s"));
        assert!(rendered.contains("RUNNING LF PROCESSES (1)"));
        assert!(rendered.contains("42  01:12"));
        assert!(rendered.contains("lf __task ts_123 --generation 1"));
    }

    #[test]
    fn process_snapshot_keeps_only_other_lf_commands() {
        let processes = parse_processes(
            "  10  01:00 /usr/local/bin/lf wave infrastructure\n  11  00:01 /usr/bin/ps -axo pid=,etime=,command=\n  12  00:02 lf top\n  13  00:03 /bin/zsh -lc lf task run INF-1\n  14  02:00 lf __task ts_123\n",
            12,
        );

        assert_eq!(
            processes,
            vec![
                LfProcess {
                    pid: 14,
                    elapsed: "02:00".to_string(),
                    command: "lf __task ts_123".to_string(),
                },
                LfProcess {
                    pid: 10,
                    elapsed: "01:00".to_string(),
                    command: "/usr/local/bin/lf wave infrastructure".to_string(),
                },
            ]
        );
    }
}
