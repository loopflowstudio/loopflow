//! `lf top` — one-hour provider throughput and live Loopflow activity.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::lf::output::{format_int, truncate};
use crate::store::{sqlite::SqliteStore, TurnSpendRow};

const WINDOW_MINUTES: usize = 60;
const BUCKET_SECONDS: i64 = 60;
const GRAPH_HEIGHT: usize = 8;
const PROCESS_COMMAND_WIDTH: usize = 88;

#[derive(Debug, PartialEq)]
struct RunningProcess {
    pid: u32,
    elapsed: String,
    kind: ProcessKind,
    command: String,
    workspace: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ProcessKind {
    Lf,
    Codex,
    Claude,
    Gemini,
    OpenCode,
}

impl ProcessKind {
    fn label(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Gemini => "gemini",
            Self::OpenCode => "opencode",
        }
    }
}

pub fn run() -> Result<()> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let home = dirs::home_dir().ok_or_else(|| anyhow!("home directory unavailable"))?;
    let ledger_path = home.join(".lf/loopflow.db");
    let spend = read_turn_spend(&ledger_path, now - (WINDOW_MINUTES as i64 * BUCKET_SECONDS))?;
    let (mut buckets, has_codex_activity) = codex_token_buckets(&codex_session_roots(&home), now);
    add_ledger_tokens(&mut buckets, &spend, now, !has_codex_activity);
    let processes = running_loopflow_processes()?;

    print!("{}", render_dashboard(&buckets, &processes));
    Ok(())
}

/// Best-effort snapshot of directories currently owned by a live process.
///
/// Worktree cleanup uses this as a second ownership signal alongside durable
/// Task Sessions. An empty set is returned when `lsof` is unavailable so the
/// daemon can still rely on its registry on minimal hosts.
pub fn running_workspace_paths() -> HashSet<PathBuf> {
    let output = Command::new("lsof").args(["-d", "cwd", "-Fn"]).output();
    let Ok(output) = output else {
        return HashSet::new();
    };
    if !output.status.success() {
        return HashSet::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .collect()
}

fn read_turn_spend(path: &Path, since: i64) -> Result<Vec<TurnSpendRow>> {
    SqliteStore::open_run_ledger_read_only(path)
        .and_then(|store| store.turn_spend_since(since))
        .map_err(|error| anyhow!("failed to read run ledger {}: {error}", path.display()))
}

fn add_ledger_tokens(
    buckets: &mut [u64; WINDOW_MINUTES],
    spend: &[TurnSpendRow],
    now: i64,
    include_codex: bool,
) {
    for turn in spend {
        // Codex's session log reports incremental usage throughout a turn. Its
        // turn receipt contains the same tokens and would count them twice here.
        if !include_codex && turn.provider == "codex" {
            continue;
        }
        let Some(index) = bucket_index(turn.at, now) else {
            continue;
        };
        let output = turn.output_tokens.unwrap_or(0).max(0) as u64;
        buckets[index] = buckets[index].saturating_add(output);
    }
}

fn codex_token_buckets(roots: &[PathBuf], now: i64) -> ([u64; WINDOW_MINUTES], bool) {
    let mut buckets = [0_u64; WINDOW_MINUTES];
    let mut has_activity = false;
    let mut files = Vec::new();
    for root in roots {
        collect_recent_jsonl(root, now, &mut files);
    }
    for path in files {
        has_activity |= add_codex_session_tokens(&mut buckets, &path, now);
    }
    (buckets, has_activity)
}

fn codex_session_roots(home: &Path) -> Vec<PathBuf> {
    let mut roots = vec![home.join(".codex/sessions")];
    let accounts = home.join(".lf/accounts/codex");
    let Ok(entries) = fs::read_dir(accounts) else {
        return roots;
    };
    for entry in entries.flatten() {
        let sessions = entry.path().join("sessions");
        if sessions.is_dir() {
            roots.push(sessions);
        }
    }
    roots
}

fn collect_recent_jsonl(directory: &Path, now: i64, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let window_start = UNIX_EPOCH
        + Duration::from_secs(
            (now - WINDOW_MINUTES as i64 * BUCKET_SECONDS)
                .max(0)
                .try_into()
                .unwrap_or(0),
        );
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_recent_jsonl(&path, now, files);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("jsonl") {
            continue;
        }
        let is_recent = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified >= window_start);
        if is_recent {
            files.push(path);
        }
    }
}

fn add_codex_session_tokens(buckets: &mut [u64; WINDOW_MINUTES], path: &Path, now: i64) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let mut previous_total = None;
    let mut has_activity = false;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if !line.contains("\"token_count\"") {
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if record.get("type").and_then(Value::as_str) != Some("event_msg")
            || record.pointer("/payload/type").and_then(Value::as_str) != Some("token_count")
        {
            continue;
        }
        let Some(total) = record
            .pointer("/payload/info/total_token_usage/output_tokens")
            .and_then(Value::as_u64)
        else {
            continue;
        };
        let delta = previous_total.map_or_else(
            || {
                record
                    .pointer("/payload/info/last_token_usage/output_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(total)
            },
            |previous| {
                if total >= previous {
                    total - previous
                } else {
                    record
                        .pointer("/payload/info/last_token_usage/output_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(total)
                }
            },
        );
        previous_total = Some(total);
        let Some(timestamp) = record
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
            .map(OffsetDateTime::unix_timestamp)
        else {
            continue;
        };
        let Some(index) = bucket_index(timestamp, now) else {
            continue;
        };
        has_activity = true;
        buckets[index] = buckets[index].saturating_add(delta);
    }
    has_activity
}

fn bucket_index(timestamp: i64, now: i64) -> Option<usize> {
    let start = now - WINDOW_MINUTES as i64 * BUCKET_SECONDS + 1;
    (start..=now)
        .contains(&timestamp)
        .then_some(((timestamp - start) / BUCKET_SECONDS) as usize)
}

fn render_dashboard(buckets: &[u64; WINDOW_MINUTES], processes: &[RunningProcess]) -> String {
    let mut output = String::new();
    output.push_str("LOOPFLOW THROUGHPUT · LAST 60 MINUTES\n");
    output.push_str("provider-reported output tokens/s · one-minute activity buckets\n\n");
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

fn running_loopflow_processes() -> Result<Vec<RunningProcess>> {
    let output = Command::new("ps")
        .args(["-axo", "pid=,etime=,command="])
        .output()
        .map_err(|error| anyhow!("failed to inspect running processes: {error}"))?;
    if !output.status.success() {
        return Err(anyhow!("failed to inspect running processes with ps"));
    }
    let mut processes =
        parse_processes(&String::from_utf8_lossy(&output.stdout), std::process::id());
    let workspaces = process_workspaces(&processes);
    for process in &mut processes {
        if process.kind != ProcessKind::Lf {
            process.workspace = workspaces.get(&process.pid).cloned();
        }
    }
    Ok(processes)
}

fn parse_processes(output: &str, current_pid: u32) -> Vec<RunningProcess> {
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
            let kind = process_kind(&command)?;
            if pid == current_pid {
                return None;
            }
            Some(RunningProcess {
                pid,
                elapsed,
                kind,
                command,
                workspace: None,
            })
        })
        .collect::<Vec<_>>();
    processes.sort_by_key(|process| std::cmp::Reverse(elapsed_seconds(&process.elapsed)));
    processes
}

fn process_kind(command: &str) -> Option<ProcessKind> {
    let executable = command
        .split_whitespace()
        .next()
        .and_then(|word| Path::new(word).file_name())
        .and_then(|name| name.to_str())?;
    match executable {
        "lf" => Some(ProcessKind::Lf),
        "codex" => Some(ProcessKind::Codex),
        "claude" => Some(ProcessKind::Claude),
        "gemini" => Some(ProcessKind::Gemini),
        "opencode" => Some(ProcessKind::OpenCode),
        _ => None,
    }
}

fn process_workspaces(processes: &[RunningProcess]) -> HashMap<u32, String> {
    let pids = processes
        .iter()
        .filter(|process| process.kind != ProcessKind::Lf)
        .map(|process| process.pid.to_string())
        .collect::<Vec<_>>()
        .join(",");
    if pids.is_empty() {
        return HashMap::new();
    }
    let output = Command::new("lsof")
        .args(["-a", "-p", &pids, "-d", "cwd", "-Fn"])
        .output();
    let Ok(output) = output else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }
    parse_lsof_workspaces(&String::from_utf8_lossy(&output.stdout))
}

fn parse_lsof_workspaces(output: &str) -> HashMap<u32, String> {
    let mut workspaces = HashMap::new();
    let mut pid = None;
    for line in output.lines() {
        if let Some(value) = line.strip_prefix('p') {
            pid = value.parse::<u32>().ok();
        } else if let (Some(pid), Some(path)) = (pid, line.strip_prefix('n')) {
            if let Some(name) = Path::new(path).file_name().and_then(|name| name.to_str()) {
                workspaces.insert(pid, name.to_string());
            }
        }
    }
    workspaces
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

fn render_processes(processes: &[RunningProcess]) -> String {
    if processes.is_empty() {
        return "RUNNING LF + PROVIDER PROCESSES\nnone\n".to_string();
    }

    let mut output = format!("RUNNING LF + PROVIDER PROCESSES ({})\n", processes.len());
    output.push_str("     PID  ELAPSED       KIND      COMMAND / WORKTREE\n");
    for process in processes {
        let description = if process.kind == ProcessKind::Lf {
            &process.command
        } else {
            process.workspace.as_deref().unwrap_or(process.kind.label())
        };
        output.push_str(&format!(
            "{:>8}  {:<12}  {:<8}  {}\n",
            process.pid,
            process.elapsed,
            process.kind.label(),
            truncate(description, PROCESS_COMMAND_WIDTH),
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        add_codex_session_tokens, add_ledger_tokens, codex_session_roots, parse_lsof_workspaces,
        parse_processes, read_turn_spend, render_dashboard, ProcessKind, RunningProcess,
        WINDOW_MINUTES,
    };
    use crate::store::{sqlite::SqliteStore, TurnSpendRow};

    fn spend(recorded_at: i64, output: i64, provider: &str) -> TurnSpendRow {
        TurnSpendRow {
            run_id: "run".to_string(),
            process_id: "process".to_string(),
            repo: "/src/loopflow".to_string(),
            wave: None,
            flow: None,
            skill: None,
            provider: provider.to_string(),
            model: None,
            at: recorded_at,
            input_tokens: Some(0),
            output_tokens: Some(output),
            cache_read_tokens: Some(0),
            cost_usd: None,
        }
    }

    #[test]
    fn telemetry_reader_ignores_newer_migration_history_without_writing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("loopflow.db");
        drop(SqliteStore::new(&path).unwrap());
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations (version, applied_at)
                 VALUES ('99.99.999_future', unixepoch())",
                [],
            )
            .unwrap();
        drop(connection);

        // `lf top` reads a ledger a newer `lf` may have migrated past. It must
        // answer from what it can read, and must not migrate anything away.
        read_turn_spend(&path, 0).unwrap();

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
    fn ledger_buckets_non_codex_completion_tokens() {
        let now = 10_000;
        let mut buckets = [0; WINDOW_MINUTES];
        add_ledger_tokens(
            &mut buckets,
            &[
                spend(now - 3_599, 30, "claude"),
                spend(now - 1, 60, "gemini"),
                spend(now, 9_999, "codex"),
                spend(now - 3_600, 9_999, "claude"),
            ],
            now,
            false,
        );

        assert_eq!(buckets[0], 30);
        assert_eq!(buckets[WINDOW_MINUTES - 1], 60);
        assert_eq!(buckets.iter().sum::<u64>(), 90);
    }

    #[test]
    fn codex_buckets_incremental_session_tokens() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rollout.jsonl");
        std::fs::write(
            &path,
            concat!(
                "{\"timestamp\":\"2026-07-15T22:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"output_tokens\":100},\"last_token_usage\":{\"output_tokens\":100}}}}\n",
                "{\"timestamp\":\"2026-07-15T22:59:30Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"output_tokens\":160},\"last_token_usage\":{\"output_tokens\":60}}}}\n",
                "{\"timestamp\":\"2026-07-15T23:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"output_tokens\":190},\"last_token_usage\":{\"output_tokens\":30}}}}\n"
            ),
        )
        .unwrap();
        let now = time::OffsetDateTime::parse(
            "2026-07-15T23:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
        .unix_timestamp();
        let mut buckets = [0; WINDOW_MINUTES];

        assert!(add_codex_session_tokens(&mut buckets, &path, now));

        assert_eq!(buckets[WINDOW_MINUTES - 1], 90);
        assert_eq!(buckets.iter().sum::<u64>(), 90);
    }

    #[test]
    fn codex_roots_include_default_and_managed_accounts() {
        let home = tempfile::tempdir().unwrap();
        let managed = home.path().join(".lf/accounts/codex/engineering/sessions");
        std::fs::create_dir_all(&managed).unwrap();

        let roots = codex_session_roots(home.path());

        assert_eq!(roots, vec![home.path().join(".codex/sessions"), managed]);
    }

    #[test]
    fn dashboard_shows_rate_summary_and_live_processes() {
        let mut buckets = [0; WINDOW_MINUTES];
        buckets[WINDOW_MINUTES - 1] = 120;
        let rendered = render_dashboard(
            &buckets,
            &[RunningProcess {
                pid: 42,
                elapsed: "01:12".to_string(),
                kind: ProcessKind::Lf,
                command: "lf __task ts_123 --generation 1".to_string(),
                workspace: None,
            }],
        );

        assert!(rendered.contains("LOOPFLOW THROUGHPUT · LAST 60 MINUTES"));
        assert!(rendered.contains("provider-reported output tokens/s"));
        assert!(rendered
            .contains("total 120 tokens · avg 0.03 tok/s · peak 2.0 tok/s · current 2.0 tok/s"));
        assert!(rendered.contains("RUNNING LF + PROVIDER PROCESSES (1)"));
        assert!(rendered.contains("42  01:12"));
        assert!(rendered.contains("lf __task ts_123 --generation 1"));
    }

    #[test]
    fn process_snapshot_keeps_lf_and_provider_commands() {
        let processes = parse_processes(
            "  10  01:00 /usr/local/bin/lf wave infrastructure\n  11  00:01 /usr/bin/ps -axo pid=,etime=,command=\n  12  00:02 lf top\n  13  00:03 /bin/zsh -lc lf task run INF-1\n  14  02:00 lf __task ts_123\n  15  03:00 /opt/codex app-server\n",
            12,
        );

        assert_eq!(
            processes,
            vec![
                RunningProcess {
                    pid: 15,
                    elapsed: "03:00".to_string(),
                    kind: ProcessKind::Codex,
                    command: "/opt/codex app-server".to_string(),
                    workspace: None,
                },
                RunningProcess {
                    pid: 14,
                    elapsed: "02:00".to_string(),
                    kind: ProcessKind::Lf,
                    command: "lf __task ts_123".to_string(),
                    workspace: None,
                },
                RunningProcess {
                    pid: 10,
                    elapsed: "01:00".to_string(),
                    kind: ProcessKind::Lf,
                    command: "/usr/local/bin/lf wave infrastructure".to_string(),
                    workspace: None,
                },
            ]
        );
    }

    // `lf top` is the machine-health view of the exact tree the OpenCode
    // reaper manages: the `lf __resident` body, the `opencode serve` provider
    // leader, and the descendants (MCP servers, model proxies) that live in the
    // server's process group. The snapshot must keep the leader — that is the
    // unit the reaper reaps, and showing it is accurate evidence of an orphan
    // before a resident's boot sweep takes it down — and must filter the
    // descendants, so the view never double-counts the tree as separate
    // providers. After a reap the leader line disappears; this is the
    // classification that keeps that honest.
    #[test]
    fn process_snapshot_classifies_the_opencode_tree_the_reaper_manages() {
        let processes = parse_processes(
            "  20  05:00 /usr/local/bin/lf __resident infrastructure\n  21  04:59 /usr/local/bin/opencode serve --port 33421\n  22  04:58 node /Users/jack/.opencode/mcp-servers/filesystem.js\n  23  04:57 /usr/bin/python -m opencode_proxy\n  24  00:01 /bin/ps -axo pid=,etime=,command=\n",
            24,
        );

        assert_eq!(
            processes,
            vec![
                RunningProcess {
                    pid: 20,
                    elapsed: "05:00".to_string(),
                    kind: ProcessKind::Lf,
                    command: "/usr/local/bin/lf __resident infrastructure".to_string(),
                    workspace: None,
                },
                RunningProcess {
                    pid: 21,
                    elapsed: "04:59".to_string(),
                    kind: ProcessKind::OpenCode,
                    command: "/usr/local/bin/opencode serve --port 33421".to_string(),
                    workspace: None,
                },
            ]
        );
    }

    #[test]
    fn process_workspaces_use_each_provider_cwd() {
        let workspaces = parse_lsof_workspaces(
            "p15\nfcwd\nn/Users/jack/src/loopflow.context-lab\np16\nfcwd\nn/Users/jack/src/manabot\n",
        );

        assert_eq!(
            workspaces.get(&15).map(String::as_str),
            Some("loopflow.context-lab")
        );
        assert_eq!(workspaces.get(&16).map(String::as_str), Some("manabot"));
    }
}
