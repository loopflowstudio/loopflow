//! `lf d` — registry reads and writes as verbs, straight through lfdb.
//!
//! Every subcommand talks to the store directly (sqlite/postgres via the
//! [`crate::lfdb::Store`] facade) — zero HTTP, no daemon in the path. This is
//! sequence step (a) of the collapse: once these verbs exist, every caller
//! can leave the daemon's mutation routes behind.
//!
//! Reads open the store only if it already exists (`open_existing_store`);
//! a machine with no registry yet prints so and exits 0. Writes bootstrap
//! the registry (`storage_config_from_env` + migrate + open) — creation
//! verbs are how a registry comes to exist.
//!
//! Output is the domain types themselves: tables for humans, `--json`
//! serializes the persisted types directly. No wire DTOs here — the DTO
//! discipline is for the HTTP boundary, and these verbs never cross it.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use time::OffsetDateTime;

use crate::engine::worktree::remove_worktree;
use crate::engine::worktrees::worktree_path;
use crate::lf::output::Colors;
use crate::lf::{DCommand, DRepoCommand, DWaveCommand};
use crate::lfd::executor::ensure_wave_worktree;
use crate::lfd::github::github_repo_from_local;
use crate::lfd::http::routes::repos::{repo_name_from_path, would_create_cycle};
use crate::lfd::http::routes::wave_config::read_wave_config;
use crate::lfd::id::LfdId;
use crate::lfd::storage_config_from_env;
use crate::lfd::types::{
    AttentionItem, AttentionStatus, Repo, RepoEdge, RepoId, RepoWork, RunStatus, Wave, WaveMode,
    WaveStatus, LIVE_SESSION_STATUSES,
};
use crate::lfdb::{migrate_store, open_existing_store, open_store, Store};

pub fn run(cmd: &DCommand) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(dispatch(cmd))
}

async fn dispatch(cmd: &DCommand) -> Result<()> {
    match cmd {
        DCommand::Waves { json } => {
            let Some(store) = read_store().await else {
                return Ok(());
            };
            println!("{}", waves_output(&store, *json).await?);
            Ok(())
        }
        DCommand::Wave { name, cmd } => match (name, cmd) {
            (_, Some(cmd)) => wave_write(cmd).await,
            (Some(name), None) => {
                let Some(store) = read_store().await else {
                    return Ok(());
                };
                println!("{}", wave_detail_output(&store, name, false).await?);
                Ok(())
            }
            (None, None) => Err(anyhow!(
                "usage: lf d wave <name> | lf d wave <create|update|delete> ..."
            )),
        },
        DCommand::Runs { wave, limit, json } => {
            let Some(store) = read_store().await else {
                return Ok(());
            };
            println!(
                "{}",
                runs_output(&store, wave.as_deref(), *limit, *json).await?
            );
            Ok(())
        }
        DCommand::Sessions { wave, live, json } => {
            let Some(store) = read_store().await else {
                return Ok(());
            };
            println!(
                "{}",
                sessions_output(&store, wave.as_deref(), *live, *json).await?
            );
            Ok(())
        }
        DCommand::Attention { wave, json } => {
            let Some(store) = read_store().await else {
                return Ok(());
            };
            println!(
                "{}",
                attention_output(&store, wave.as_deref(), *json).await?
            );
            Ok(())
        }
        DCommand::Repo { cmd } => {
            let store = write_store().await?;
            match cmd {
                DRepoCommand::Add { path, parent, json } => {
                    let repo = add_repo(&store, path, parent.as_deref()).await?;
                    if *json {
                        println!("{}", serde_json::to_string_pretty(&repo)?);
                    } else {
                        println!("registered repo {} at {}", repo.repo_id, repo.path);
                        if let Some(parent) = parent {
                            println!("  parent {parent}");
                        }
                    }
                    Ok(())
                }
                DRepoCommand::Remove { path } => {
                    let removed = remove_repo(&store, path).await?;
                    println!("unregistered {removed}");
                    Ok(())
                }
            }
        }
    }
}

async fn wave_write(cmd: &DWaveCommand) -> Result<()> {
    let store = write_store().await?;
    match cmd {
        DWaveCommand::Create {
            name,
            repo,
            mode,
            flow,
            goal,
            json,
        } => {
            let repo = match repo {
                Some(repo) => PathBuf::from(repo),
                None => default_repo()?,
            };
            let mode = parse_mode(mode.as_deref())?;
            let wave =
                create_wave(&store, name, &repo, mode, flow.as_deref(), goal.as_deref()).await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&wave)?);
            } else {
                println!(
                    "created wave '{}' ({}, mode {}, repo {})",
                    wave.name(),
                    wave.id(),
                    wave.mode().as_str(),
                    wave.repo()
                );
            }
            Ok(())
        }
        DWaveCommand::Update {
            name,
            mode,
            flow,
            goal,
            paused,
            json,
        } => {
            let mode = parse_mode(mode.as_deref())?;
            let wave = update_wave(
                &store,
                name,
                WaveUpdate {
                    mode,
                    flow: flow.clone(),
                    goal: goal.clone(),
                    paused: *paused,
                },
            )
            .await?;
            if *json {
                println!("{}", serde_json::to_string_pretty(&wave)?);
            } else {
                println!(
                    "updated wave '{}' (mode {}, paused {}, flow {})",
                    wave.name(),
                    wave.mode().as_str(),
                    wave.paused,
                    wave.primary_flow()
                );
            }
            Ok(())
        }
        DWaveCommand::Delete { name, force } => {
            let removed = delete_wave(&store, name, *force).await?;
            println!("deleted wave '{name}'");
            for worktree in removed {
                println!("  removed worktree {worktree}");
            }
            Ok(())
        }
    }
}

/// Reads never create a registry: no store on this machine is an empty
/// answer, not an error.
async fn read_store() -> Option<Store> {
    let store = open_existing_store().await;
    if store.is_none() {
        println!("no registry on this machine yet");
    }
    store
}

/// Writes bootstrap the registry: resolve config from env, apply migrations,
/// open. Creation verbs are how ~/.lf/lfd.db comes to exist.
async fn write_store() -> Result<Store> {
    let cfg = storage_config_from_env().map_err(|err| anyhow!("registry config: {err}"))?;
    migrate_store(&cfg, false)
        .await
        .map_err(|err| anyhow!("registry migration failed: {err}"))?;
    open_store(&cfg)
        .await
        .map_err(|err| anyhow!("failed to open registry: {err}"))
}

fn default_repo() -> Result<PathBuf> {
    let root = crate::lf::commands::util::find_repo_root()?;
    crate::engine::worktrees::main_repo_root(&root)
        .map_err(|err| anyhow!("cannot resolve main repo (pass --repo): {err}"))
}

fn parse_mode(mode: Option<&str>) -> Result<Option<WaveMode>> {
    mode.map(|value| value.parse::<WaveMode>().map_err(|err| anyhow!(err)))
        .transpose()
}

async fn resolve_wave(store: &Store, name: &str) -> Result<Wave> {
    store
        .get_wave_by_name(name)
        .await?
        .ok_or_else(|| anyhow!("wave '{name}' not found in the registry"))
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

pub(crate) async fn waves_output(store: &Store, json: bool) -> Result<String> {
    let waves = store.list_waves(None).await?;
    if json {
        return Ok(serde_json::to_string_pretty(&waves)?);
    }
    if waves.is_empty() {
        return Ok("no waves in the registry".to_string());
    }
    let colors = Colors::default();
    let mut out = format!(
        "{bold}{name:<16}  {mode:<7}  {paused:<7}  {brain:<6}  REPOS{reset}",
        bold = colors.bold,
        reset = colors.reset,
        name = "NAME",
        mode = "MODE",
        paused = "PAUSED",
        brain = "BRAIN",
    );
    for wave in &waves {
        // The live-brain probe: one non-terminal WaveAgent session per wave
        // is the single fact one-brain enforcement keys on.
        let brain = store.live_wave_agent_session(wave.id()).await?;
        let repos = wave
            .repos
            .iter()
            .map(|rw| repo_name_from_path(Path::new(&rw.repo)))
            .collect::<Vec<_>>()
            .join(",");
        out.push_str(&format!(
            "\n{name:<16}  {mode:<7}  {paused:<7}  {brain:<6}  {repos}",
            name = truncate(wave.name(), 16),
            mode = wave.mode().as_str(),
            paused = if wave.paused { "yes" } else { "-" },
            brain = if brain.is_some() { "live" } else { "-" },
        ));
    }
    Ok(out)
}

pub(crate) async fn wave_detail_output(store: &Store, name: &str, json: bool) -> Result<String> {
    let wave = resolve_wave(store, name).await?;
    if json {
        return Ok(serde_json::to_string_pretty(&wave)?);
    }
    let colors = Colors::default();
    let mut out = format!(
        "{bold}wave {name}{reset}  {id}",
        bold = colors.bold,
        reset = colors.reset,
        name = wave.name(),
        id = wave.id(),
    );
    out.push_str(&format!("\n  mode      {}", wave.mode().as_str()));
    out.push_str(&format!("\n  status    {}", wave.status().as_str()));
    out.push_str(&format!("\n  paused    {}", wave.paused));
    out.push_str(&format!("\n  flow      {}", wave.primary_flow()));
    out.push_str(&format!("\n  goal      {}", wave.goal()));
    out.push_str(&format!("\n  workers   {}", wave.workers()));
    if !wave.direction().is_empty() {
        out.push_str(&format!("\n  direction {}", wave.direction().join(", ")));
    }
    if !wave.area().is_empty() {
        out.push_str(&format!("\n  area      {}", wave.area().join(", ")));
    }
    out.push_str(&format!("\n  created   {}", fmt_ts(wave.created_at())));
    for rw in &wave.repos {
        out.push_str(&format!(
            "\n  repo      {}  {}  iter {}",
            rw.repo,
            rw.status.as_str(),
            rw.iteration
        ));
        if !rw.worktree.is_empty() {
            out.push_str(&format!(
                "\n    worktree {}  branch {}",
                rw.worktree, rw.branch
            ));
        }
    }
    let crons = store.list_wave_crons(wave.id()).await?;
    for cron in &crons {
        out.push_str(&format!("\n  cron      {}  {}", cron.schedule, cron.flow));
    }
    match store.live_wave_agent_session(wave.id()).await? {
        Some(session) => out.push_str(&format!(
            "\n  brain     live  session {}  tmux {}",
            session.id, session.tmux_name
        )),
        None => out.push_str("\n  brain     -"),
    }
    Ok(out)
}

pub(crate) async fn runs_output(
    store: &Store,
    wave: Option<&str>,
    limit: u32,
    json: bool,
) -> Result<String> {
    let wave_id = match wave {
        Some(name) => Some(resolve_wave(store, name).await?.id().clone()),
        None => None,
    };
    let runs = store.list_runs(wave_id.as_ref(), Some(limit)).await?;
    if json {
        return Ok(serde_json::to_string_pretty(&runs)?);
    }
    if runs.is_empty() {
        return Ok("no runs in the registry".to_string());
    }
    let wave_names = wave_name_index(store).await?;
    let colors = Colors::default();
    let mut out = format!(
        "{bold}{started:<17}  {wave:<12}  {flow:<16}  {status:<9}  {dur:>8}  ID{reset}",
        bold = colors.bold,
        reset = colors.reset,
        started = "STARTED",
        wave = "WAVE",
        flow = "FLOW",
        status = "STATUS",
        dur = "DURATION",
    );
    for run in &runs {
        out.push_str(&format!(
            "\n{started:<17}  {wave:<12}  {flow:<16}  {status:<9}  {dur:>8}  {id}",
            started = fmt_ts(run.started_at),
            wave = truncate(
                wave_names
                    .get(run.wave_id.as_str())
                    .map(String::as_str)
                    .unwrap_or("-"),
                12
            ),
            flow = truncate(&run.flow, 16),
            status = run_status_str(run.status),
            dur = fmt_duration(run.started_at, run.ended_at),
            id = short_id(run.id.as_str()),
        ));
    }
    Ok(out)
}

pub(crate) async fn sessions_output(
    store: &Store,
    wave: Option<&str>,
    live: bool,
    json: bool,
) -> Result<String> {
    let wave_id = match wave {
        Some(name) => Some(resolve_wave(store, name).await?.id().clone()),
        None => None,
    };
    let statuses = live.then_some(LIVE_SESSION_STATUSES);
    let sessions = store
        .list_control_sessions(wave_id.as_ref(), statuses)
        .await?;
    if json {
        return Ok(serde_json::to_string_pretty(&sessions)?);
    }
    if sessions.is_empty() {
        return Ok(if live {
            "no live sessions".to_string()
        } else {
            "no sessions in the registry".to_string()
        });
    }
    let wave_names = wave_name_index(store).await?;
    let colors = Colors::default();
    let mut out = format!(
        "{bold}{created:<17}  {wave:<12}  {use_:<10}  {step:<18}  {status:<9}  ID{reset}",
        bold = colors.bold,
        reset = colors.reset,
        created = "CREATED",
        wave = "WAVE",
        use_ = "USE",
        step = "STEP",
        status = "STATUS",
    );
    for session in &sessions {
        out.push_str(&format!(
            "\n{created:<17}  {wave:<12}  {use_:<10}  {step:<18}  {status:<9}  {id}",
            created = fmt_ts(Some(session.created_at)),
            wave = truncate(
                wave_names
                    .get(session.wave_id.as_str())
                    .map(String::as_str)
                    .unwrap_or("-"),
                12
            ),
            use_ = session.session_use.as_str(),
            step = truncate(&session.step, 18),
            status = session.status.as_str(),
            id = short_id(session.id.as_str()),
        ));
    }
    Ok(out)
}

pub(crate) async fn attention_output(
    store: &Store,
    wave: Option<&str>,
    json: bool,
) -> Result<String> {
    let wave_id = match wave {
        Some(name) => Some(resolve_wave(store, name).await?.id().clone()),
        None => None,
    };
    // Open items only: same default as the list route (unresolved).
    let mut items: Vec<AttentionItem> = store
        .list_attention_items(None, None)
        .await?
        .into_iter()
        .filter(|item| item.status != AttentionStatus::Resolved)
        .collect();
    if let Some(wave_id) = &wave_id {
        items.retain(|item| item.wave_id == *wave_id);
    }
    if json {
        return Ok(serde_json::to_string_pretty(&items)?);
    }
    if items.is_empty() {
        return Ok("no open attention items".to_string());
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.surfaced_at));
    let wave_names = wave_name_index(store).await?;
    let colors = Colors::default();
    let mut out = format!(
        "{bold}{surfaced:<17}  {wave:<12}  {kind:<11}  {status:<8}  TITLE{reset}",
        bold = colors.bold,
        reset = colors.reset,
        surfaced = "SURFACED",
        wave = "WAVE",
        kind = "KIND",
        status = "STATUS",
    );
    for item in &items {
        out.push_str(&format!(
            "\n{surfaced:<17}  {wave:<12}  {kind:<11}  {status:<8}  {title}",
            surfaced = fmt_ts(Some(item.surfaced_at)),
            wave = truncate(
                wave_names
                    .get(item.wave_id.as_str())
                    .map(String::as_str)
                    .unwrap_or("-"),
                12
            ),
            kind = item.kind.as_str(),
            status = item.status.as_str(),
            title = item.title,
        ));
    }
    Ok(out)
}

async fn wave_name_index(store: &Store) -> Result<std::collections::HashMap<String, String>> {
    Ok(store
        .list_waves(None)
        .await?
        .into_iter()
        .map(|wave| (wave.id().to_string(), wave.name().clone()))
        .collect())
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

/// Port of `create_wave_handler`'s honest core: the wave row + repos + the
/// wave worktree. GOAL.md frontmatter still seeds flow/goal/workers/metrics.
///
/// Deliberately NOT ported (both die in wave 4 of the collapse, with the
/// trigger/activation organs): the default repo/ci-fix triggers, and the
/// `run: true` immediate-activation arm. Mode defaults to manual — a wave
/// loops only when someone says so (`--mode loop`).
pub(crate) async fn create_wave(
    store: &Store,
    name: &str,
    repo: &Path,
    mode: Option<WaveMode>,
    flow: Option<&str>,
    goal: Option<&str>,
) -> Result<Wave> {
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow!("wave name is required"));
    }
    if store.get_wave_by_name(name).await?.is_some() {
        return Err(anyhow!("wave '{name}' already exists"));
    }
    if !repo.is_dir() {
        return Err(anyhow!("repo path does not exist: {}", repo.display()));
    }

    let wave_config = read_wave_config(repo, name);
    let primary_flow = flow
        .map(str::to_string)
        .or_else(|| wave_config.as_ref().and_then(|c| c.primary_flow.clone()))
        .unwrap_or_else(|| "ship-roadmap".to_string());
    let goal = goal
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| wave_config.as_ref().and_then(|c| c.goal.clone()))
        .unwrap_or_else(|| "ship-roadmap".to_string());
    let workers = wave_config.as_ref().and_then(|c| c.workers).unwrap_or(1);
    let metrics = wave_config
        .as_ref()
        .and_then(|c| c.metrics.clone())
        .unwrap_or_default();

    let wave = Wave {
        id: LfdId::new(),
        name: name.to_string(),
        mode: mode.unwrap_or(WaveMode::Manual),
        primary_flow,
        goal,
        metrics,
        crons: Vec::new(),
        repos: vec![RepoWork {
            repo: repo.display().to_string(),
            worktree: String::new(),
            branch: String::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            position: 0,
        }],
        direction: Vec::new(),
        area: Vec::new(),
        paused: false,
        created_at: Some(OffsetDateTime::now_utc()),
        workers,
        parent_wave_id: None,
    };
    store.create_wave(&wave).await?;

    // Wave-dir workspace, compensating like the route did: a wave whose
    // worktree can't be created should not exist in the registry.
    for repo_work in &wave.repos {
        if let Err(err) = ensure_wave_worktree(Path::new(&repo_work.repo), wave.name()) {
            let _ = store.delete_wave(wave.id()).await;
            return Err(anyhow!("failed to create wave worktree: {err}"));
        }
    }
    Ok(wave)
}

#[derive(Debug, Default)]
pub(crate) struct WaveUpdate {
    pub mode: Option<WaveMode>,
    pub flow: Option<String>,
    pub goal: Option<String>,
    pub paused: Option<bool>,
}

/// Port of `update_wave_handler`'s core: mutate the row, write it back.
/// Rename (worktree move + branch rename), cron replacement, workers, and
/// the per-step agent config writes are not verbs here — rename and agent
/// config are worktree surgery, not registry writes.
pub(crate) async fn update_wave(store: &Store, name: &str, update: WaveUpdate) -> Result<Wave> {
    let mut wave = resolve_wave(store, name).await?;
    if let Some(mode) = update.mode {
        wave.mode = mode;
    }
    if let Some(flow) = update.flow {
        wave.primary_flow = flow;
    }
    if let Some(goal) = update.goal.as_deref().map(str::trim) {
        if !goal.is_empty() {
            wave.goal = goal.to_string();
        }
    }
    if let Some(paused) = update.paused {
        wave.set_status(if paused {
            WaveStatus::Paused
        } else {
            WaveStatus::Idle
        });
    }
    store.update_wave(&wave).await?;
    Ok(wave)
}

/// Port of `delete_wave_handler` minus the executor arms: no agent killing
/// (the executor dies in the collapse), no in-process reconcile-lock cleanup,
/// no event emission (the doorman's store-poll bridge carries deletions).
/// What survives is the honest core: refuse while a live brain holds the
/// wave (unless --force), cascade-delete the rows, remove the worktrees.
/// Returns the worktree paths it removed.
pub(crate) async fn delete_wave(store: &Store, name: &str, force: bool) -> Result<Vec<String>> {
    let wave = resolve_wave(store, name).await?;
    if let Some(session) = store.live_wave_agent_session(wave.id()).await? {
        if !force {
            return Err(anyhow!(
                "wave '{name}' has a live brain (session {}, tmux '{}') — stop it or rerun with --force",
                session.id,
                session.tmux_name
            ));
        }
    }

    store.delete_wave(wave.id()).await?;

    let mut removed = Vec::new();
    for repo_work in &wave.repos {
        let worktree = worktree_path(Path::new(&repo_work.repo), wave.name());
        if worktree.exists() {
            match remove_worktree(&worktree, false) {
                Ok(()) => removed.push(worktree.display().to_string()),
                Err(err) => eprintln!(
                    "warning: failed to remove worktree {}: {err}",
                    worktree.display()
                ),
            }
        }
    }
    Ok(removed)
}

/// Port of `add_repo_handler`'s store logic, plus the optional parent edge
/// (the route splits that into `PUT /repos/{owner}/{repo}/children/...`;
/// here `--parent <path>` is the same edge addressed by path).
pub(crate) async fn add_repo(store: &Store, path: &str, parent: Option<&str>) -> Result<Repo> {
    let path = canonical_repo_path(path)?;
    if !is_git_repo(Path::new(&path)) {
        return Err(anyhow!("path is not a git repository: {path}"));
    }
    let repo_id = github_repo_from_local(Path::new(&path))
        .ok_or_else(|| anyhow!("repo must have a GitHub origin remote: {path}"))?;
    let repo_id = RepoId::parse(&repo_id).map_err(|err| anyhow!("invalid GitHub remote: {err}"))?;

    if let Some(existing) = store.get_repo_by_repo_id(&repo_id).await? {
        if existing.path != path {
            return Err(anyhow!(
                "repo {repo_id} is already registered at a different path: {}",
                existing.path
            ));
        }
    }

    let existing = store.get_repo(&path).await?;
    let repo = Repo {
        name: repo_name_from_path(Path::new(&path)),
        path: path.clone(),
        repo_id,
        added_at: existing
            .as_ref()
            .map(|repo| repo.added_at)
            .unwrap_or_else(OffsetDateTime::now_utc),
    };
    store.upsert_repo(&repo).await?;

    if let Some(parent) = parent {
        let parent_path = canonical_repo_path(parent)?;
        let parent_repo = store
            .get_repo(&parent_path)
            .await?
            .ok_or_else(|| anyhow!("parent repo is not registered: {parent_path}"))?;
        if parent_repo.repo_id == repo.repo_id {
            return Err(anyhow!("repo cannot be its own parent"));
        }
        let edges = store.list_edges().await?;
        if would_create_cycle(&edges, &parent_repo.repo_id, &repo.repo_id) {
            return Err(anyhow!("edge would create a cycle"));
        }
        store
            .add_edge(&RepoEdge {
                parent_repo_id: parent_repo.repo_id,
                child_repo_id: repo.repo_id.clone(),
            })
            .await?;
    }
    Ok(repo)
}

/// Port of `remove_repo_handler`: canonicalize when the path still exists
/// (a deleted checkout can still be unregistered), delete the row. Returns
/// the normalized path that was removed.
pub(crate) async fn remove_repo(store: &Store, path: &str) -> Result<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("path cannot be empty"));
    }
    let raw = PathBuf::from(trimmed);
    if !raw.is_absolute() {
        return Err(anyhow!("path must be absolute"));
    }
    let normalized = if raw.exists() {
        std::fs::canonicalize(&raw)?
    } else {
        raw
    };
    let normalized = normalized.display().to_string();
    store.delete_repo(&normalized).await?;
    Ok(normalized)
}

fn canonical_repo_path(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("path cannot be empty"));
    }
    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(anyhow!("path must be absolute"));
    }
    let canonical =
        std::fs::canonicalize(&path).map_err(|_| anyhow!("path does not exist: {trimmed}"))?;
    Ok(canonical.display().to_string())
}

fn is_git_repo(path: &Path) -> bool {
    let git_entry = path.join(".git");
    git_entry.is_dir() || git_entry.is_file()
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn run_status_str(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Unspecified => "-",
        RunStatus::Pending => "pending",
        RunStatus::Running => "running",
        RunStatus::Waiting => "waiting",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
    }
}

fn fmt_ts(ts: Option<OffsetDateTime>) -> String {
    match ts {
        Some(t) => format!(
            "{:04}-{:02}-{:02} {:02}:{:02}",
            t.year(),
            t.month() as u8,
            t.day(),
            t.hour(),
            t.minute()
        ),
        None => "-".to_string(),
    }
}

fn fmt_duration(started: Option<OffsetDateTime>, ended: Option<OffsetDateTime>) -> String {
    let (Some(started), Some(ended)) = (started, ended) else {
        return "…".to_string();
    };
    let secs = (ended - started).whole_seconds().max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        let mut out: String = value.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::process::Command;
    use std::sync::Arc;

    use loopflow_test_support::TestRepo;

    use crate::lfd::types::{Run, Session, SessionStatus, SessionUse};
    use crate::lfdb::{open_store, SharedStore, StorageConfig};

    async fn temp_store(tmp: &Path) -> SharedStore {
        Arc::new(
            open_store(&StorageConfig::sqlite(tmp.join("lfd.db")))
                .await
                .expect("open sqlite store"),
        )
    }

    fn seed_wave(name: &str, repo: &str) -> Wave {
        let mut wave = Wave::new(LfdId::new(), name.to_string(), repo.to_string());
        wave.mode = WaveMode::Manual;
        wave
    }

    fn live_brain_session(wave: &Wave) -> Session {
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::WaveAgent,
            step: "wave".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp".to_string(),
            argv: vec!["lf".to_string(), "wave".to_string()],
            env: Default::default(),
            source: "wave_server".to_string(),
            tmux_name: "lf-brain".to_string(),
            status: SessionStatus::Running,
            attached_at: None,
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        }
    }

    fn init_github_repo(path: &Path, remote: &str) {
        std::fs::create_dir_all(path).expect("create repo dir");
        for args in [vec!["init"], vec!["remote", "add", "origin", remote]] {
            let status = Command::new("git")
                .args(&args)
                .current_dir(path)
                .status()
                .expect("run git");
            assert!(status.success());
        }
    }

    #[tokio::test]
    async fn waves_render_table_and_json() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let mut paused = seed_wave("goals", "/tmp/repo-a");
        paused.set_status(WaveStatus::Paused);
        store.create_wave(&paused).await.expect("seed paused");
        store
            .create_wave(&seed_wave("systems", "/tmp/repo-b"))
            .await
            .expect("seed second");

        let table = waves_output(&store, false).await.expect("render");
        assert!(table.contains("goals"));
        assert!(table.contains("systems"));
        assert!(table.contains("manual"));
        assert!(table.contains("yes"), "paused wave shows in PAUSED column");

        let json = waves_output(&store, true).await.expect("json");
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).expect("parseable");
        assert_eq!(parsed.len(), 2);
    }

    #[tokio::test]
    async fn wave_detail_shows_live_brain() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = seed_wave("goals", "/tmp/repo-a");
        store.create_wave(&wave).await.expect("seed wave");
        store
            .register_session(&live_brain_session(&wave))
            .await
            .expect("register brain");

        let detail = wave_detail_output(&store, "goals", false)
            .await
            .expect("render");
        assert!(detail.contains("wave goals"));
        assert!(detail.contains("brain     live"));

        let json = wave_detail_output(&store, "goals", true)
            .await
            .expect("json");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parseable");
        assert_eq!(parsed["name"], "goals");

        assert!(wave_detail_output(&store, "ghost", false).await.is_err());
    }

    #[tokio::test]
    async fn create_round_trips_through_get() {
        let repo = TestRepo::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;

        let created = create_wave(
            &store,
            "engbot",
            repo.path(),
            Some(WaveMode::Loop),
            Some("ship-roadmap"),
            Some("Ship the roadmap."),
        )
        .await
        .expect("create wave");

        let fetched = store
            .get_wave_by_name("engbot")
            .await
            .expect("get")
            .expect("wave exists");
        assert_eq!(fetched.id(), created.id());
        assert_eq!(fetched.mode(), WaveMode::Loop);
        assert_eq!(fetched.goal(), "Ship the roadmap.");
        assert!(!fetched.paused);
        assert_eq!(fetched.repos.len(), 1);

        // Wave-dir workspace exists: the sibling <repo>.<wave> worktree.
        let worktree = worktree_path(repo.path(), "engbot");
        assert!(worktree.exists(), "wave worktree created");

        // Duplicate names are refused.
        let err = create_wave(&store, "engbot", repo.path(), None, None, None)
            .await
            .expect_err("duplicate refused");
        assert!(err.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn create_defaults_to_manual_mode() {
        let repo = TestRepo::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;

        create_wave(&store, "quiet", repo.path(), None, None, None)
            .await
            .expect("create wave");
        let fetched = store
            .get_wave_by_name("quiet")
            .await
            .expect("get")
            .expect("wave exists");
        assert_eq!(fetched.mode(), WaveMode::Manual);
        // The dying arms were not ported: no triggers were created.
        let triggers = store
            .list_triggers(Some(fetched.id()))
            .await
            .expect("list triggers");
        assert!(triggers.is_empty(), "no default triggers on lf d create");
    }

    #[tokio::test]
    async fn update_applies_flags_and_round_trips() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        store
            .create_wave(&seed_wave("goals", "/tmp/repo-a"))
            .await
            .expect("seed wave");

        update_wave(
            &store,
            "goals",
            WaveUpdate {
                mode: Some(WaveMode::Loop),
                flow: Some("architecture".to_string()),
                goal: Some("New goal.".to_string()),
                paused: Some(true),
            },
        )
        .await
        .expect("update");

        let fetched = store
            .get_wave_by_name("goals")
            .await
            .expect("get")
            .expect("wave exists");
        assert_eq!(fetched.mode(), WaveMode::Loop);
        assert_eq!(fetched.primary_flow(), "architecture");
        assert_eq!(fetched.goal(), "New goal.");
        assert!(fetched.paused);

        // Unpause round-trips too.
        update_wave(
            &store,
            "goals",
            WaveUpdate {
                paused: Some(false),
                ..Default::default()
            },
        )
        .await
        .expect("unpause");
        let fetched = store
            .get_wave_by_name("goals")
            .await
            .expect("get")
            .expect("wave exists");
        assert!(!fetched.paused);
    }

    #[tokio::test]
    async fn delete_refuses_on_live_brain_unless_forced() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = seed_wave("goals", "/tmp/repo-a");
        store.create_wave(&wave).await.expect("seed wave");
        let brain = live_brain_session(&wave);
        store.register_session(&brain).await.expect("brain");

        let err = delete_wave(&store, "goals", false)
            .await
            .expect_err("live brain refused");
        assert!(
            err.to_string().contains(brain.id.as_str()),
            "refusal names the session: {err}"
        );
        assert!(store
            .get_wave_by_name("goals")
            .await
            .expect("get")
            .is_some());

        delete_wave(&store, "goals", true).await.expect("forced");
        assert!(store
            .get_wave_by_name("goals")
            .await
            .expect("get")
            .is_none());
    }

    #[tokio::test]
    async fn runs_sessions_and_attention_render() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = seed_wave("goals", "/tmp/repo-a");
        store.create_wave(&wave).await.expect("seed wave");

        let mut run = Run::new(LfdId::new(), wave.id().clone());
        run.flow = "implement".to_string();
        run.status = RunStatus::Running;
        store.create_run(&run).await.expect("seed run");

        store
            .register_session(&live_brain_session(&wave))
            .await
            .expect("seed session");

        store
            .upsert_attention_item(&AttentionItem {
                id: LfdId::new(),
                wave_id: wave.id().clone(),
                run_id: Some(run.id.clone()),
                kind: crate::lfd::types::AttentionKind::Algedonic,
                status: AttentionStatus::Surfaced,
                title: "Step failed: gate".to_string(),
                summary: "boom".to_string(),
                context: serde_json::json!({}),
                surfaced_at: OffsetDateTime::now_utc(),
                viewed_at: None,
                resolved_at: None,
            })
            .await
            .expect("seed attention");

        let runs = runs_output(&store, Some("goals"), 10, false)
            .await
            .expect("runs");
        assert!(runs.contains("implement"));
        assert!(runs.contains("goals"));

        let runs_json = runs_output(&store, None, 10, true).await.expect("json");
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&runs_json).expect("parseable");
        assert_eq!(parsed.len(), 1);

        let sessions = sessions_output(&store, Some("goals"), true, false)
            .await
            .expect("sessions");
        assert!(sessions.contains("wave_agent"));
        assert!(sessions.contains("running"));
        let sessions_json = sessions_output(&store, None, false, true)
            .await
            .expect("json");
        assert!(serde_json::from_str::<Vec<serde_json::Value>>(&sessions_json).is_ok());

        let attention = attention_output(&store, Some("goals"), false)
            .await
            .expect("attention");
        assert!(attention.contains("Step failed: gate"));
        assert!(attention.contains("algedonic"));
        let attention_json = attention_output(&store, None, true).await.expect("json");
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&attention_json).expect("parseable");
        assert_eq!(parsed.len(), 1);
    }

    #[tokio::test]
    async fn repo_add_and_remove_round_trip_with_parent_edge() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;

        let parent_path = tmp.path().join("studio");
        let child_path = tmp.path().join("loopflow");
        init_github_repo(&parent_path, "git@github.com:loopflowstudio/studio.git");
        init_github_repo(&child_path, "git@github.com:loopflowstudio/loopflow.git");

        let parent = add_repo(&store, &parent_path.display().to_string(), None)
            .await
            .expect("add parent");
        assert_eq!(parent.repo_id.as_str(), "loopflowstudio/studio");

        let child = add_repo(
            &store,
            &child_path.display().to_string(),
            Some(&parent_path.display().to_string()),
        )
        .await
        .expect("add child with parent");
        assert_eq!(child.repo_id.as_str(), "loopflowstudio/loopflow");

        let edges = store.list_edges().await.expect("edges");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].parent_repo_id, parent.repo_id);
        assert_eq!(edges[0].child_repo_id, child.repo_id);

        let removed = remove_repo(&store, &child.path).await.expect("remove");
        assert_eq!(removed, child.path);
        assert!(store
            .get_repo(&child.path)
            .await
            .expect("get repo")
            .is_none());
    }

    #[tokio::test]
    async fn repo_add_rejects_non_git_and_non_github() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;

        let plain = tmp.path().join("plain");
        std::fs::create_dir_all(&plain).expect("mkdir");
        let err = add_repo(&store, &plain.display().to_string(), None)
            .await
            .expect_err("non-git refused");
        assert!(err.to_string().contains("not a git repository"));

        let local = tmp.path().join("local-only");
        init_github_repo(&local, "git@example.com:org/repo.git");
        let err = add_repo(&store, &local.display().to_string(), None)
            .await
            .expect_err("non-github refused");
        assert!(err.to_string().contains("GitHub origin remote"));
    }
}
