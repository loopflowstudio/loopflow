//! Native Rex workspaces. The registry keeps locators and creation receipts;
//! Rex alone owns geometry and ordinary processes. Reads never execute resume argv.
mod herdr;
mod hold;
mod layout;
mod observer;

pub(super) use herdr::open_herdr;
use layout::{layout, Host};
pub(crate) use observer::ObserverReport;

use std::collections::{BTreeSet, HashMap};
use std::io::{IsTerminal, Read, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{
    ConversationReadiness, ObserverState, OpenArgs, OpenHost, OpenReceipt, OpenReport, OpenStatus,
    Opening, Resolved, ViewHold,
};
use crate::durable::{RepositoryId, TaskId};
use crate::id::WaveId;
use crate::journal::OsProcess;
use crate::store::sqlite::{
    host_workspaces::{HostCreation, HostOccurrence, HostWorkspace},
    SqliteStore,
};
use crate::work::selection::{
    Selection, SelectionExpression, SelectionFilter, SelectionId, SelectionState, ViewContent,
    ViewPlan, WorkWorkspaceKey, WorkWorkspaceKind,
};

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}
/// Where a Task's tools run: its recorded checkout, else the repository.
fn tool_cwd<'a>(content: &'a ViewContent, repo: &'a Path) -> &'a Path {
    content
        .location
        .as_ref()
        .map_or(repo, |location| Path::new(&location.cwd))
}

fn identity(value: &str) -> String {
    hex::encode(&Sha256::digest(value.as_bytes())[..16])
}
fn selection(expression: SelectionExpression) -> Selection {
    Selection {
        id: SelectionId("workspace".into()),
        expression,
    }
}

fn workspace_plan(
    store: &SqliteStore,
    repository: &RepositoryId,
    selected: Selection,
) -> Result<ViewPlan> {
    // Working-set projections know only local execution. Include backlog in the
    // inventory so the recorded remote Machine can decide whether its Task started.
    let inventory = Selection {
        id: selected.id.clone(),
        expression: match &selected.expression {
            SelectionExpression::Repository => SelectionExpression::Filter {
                filter: SelectionFilter::Kind(WorkWorkspaceKind::Task),
            },
            SelectionExpression::Wave { wave } => SelectionExpression::Filter {
                filter: SelectionFilter::Wave(wave.to_string()),
            },
            expression => expression.clone(),
        },
    };
    let mut plan = store.plan_selection(repository, inventory)?;
    plan.selection = selected;
    Ok(plan)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct RexLocation {
    pub server: String,
    pub session: String,
    pub block: String,
}

impl RexLocation {
    pub(super) fn current() -> Result<Self> {
        let server = match std::env::var("REX_SERVER").ok() {
            Some(server) => server,
            None => caller_server()?.context(
                "Rex caller server is unavailable from its process ancestry; select it with --server",
            )?,
        };
        Self {
            server,
            session: std::env::var("REX_SESSION").context("Rex caller session is missing")?,
            block: std::env::var("REX_BLOCK").context("Rex caller block is missing")?,
        }
        .resolve()
    }

    fn resolve(mut self) -> Result<Self> {
        self.server = Rex::endpoint(Some(&self.server))?;
        Ok(self)
    }
}

/// Native Rex shells descend from the serving process, even through login and
/// tool shells. Its explicit listener is destination evidence; a running-app or
/// socket inventory is not. Current conversation placement is resolved first.
fn caller_server() -> Result<Option<String>> {
    let mut pid = std::process::id();
    let mut seen = BTreeSet::new();
    while pid > 1 && seen.insert(pid) {
        let Some(process) = OsProcess::read(pid)? else {
            break;
        };
        if OsProcess::executable(pid)
            .is_some_and(|path| path.file_name().is_some_and(|name| name == "rex"))
        {
            let args = OsProcess::arguments(pid)?
                .context("Rex caller server arguments are unavailable")?;
            if args.windows(2).any(|pair| pair == ["server", "run"]) {
                return Ok(listening_endpoint(&args));
            }
        }
        pid = process.ppid;
    }
    Ok(None)
}

fn listening_endpoint(args: &[String]) -> Option<String> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--listen" {
            return args.next().cloned();
        }
        if let Some(endpoint) = arg.strip_prefix("--listen=") {
            return Some(endpoint.into());
        }
    }
    None
}

struct Rex {
    server: String,
    caller: Option<RexLocation>,
}

#[derive(Clone, Copy, Default)]
struct Presentation<'a> {
    passive: bool,
    omit_shells: bool,
    session: Option<&'a str>,
    diff: bool,
    retry_observers: bool,
}

#[derive(Debug)]
struct Materialized {
    window: String,
    created: bool,
    shells: usize,
    observers: String,
    retried: usize,
}

#[derive(Clone, Copy)]
enum Follow {
    Local,
    All,
}

fn following(args: &OpenArgs) -> Option<Follow> {
    if args.arrive_task.is_some() {
        None
    } else if args.local_workspace {
        Some(Follow::Local)
    } else {
        Some(Follow::All)
    }
}
impl Rex {
    fn call(&self, method: &str, args: Value) -> Result<Value> {
        let mut command = Command::new("rex");
        command.args(["--autostart=false", "--timeout", "2s", "-S", &self.server]);
        command.args(["api", "call", method, &serde_json::to_string(&args)?]);
        let output = super::run_host(command, "Rex")
            .map_err(|error| anyhow::anyhow!("Rex {method}: {}", error.reason()))?;
        serde_json::from_str(&output).context("Rex reply is unreadable; outcome is unknown")
    }
    fn local(server: Option<&str>) -> Result<Self> {
        let viewed = super::viewed_placement();
        let location = if server.is_some() {
            None
        } else {
            match viewed {
                Some((OpenHost::Rex, value)) => Some(
                    serde_json::from_str::<RexLocation>(&value)
                        .context("Current Rex viewer placement is unreadable")?
                        .resolve()?,
                ),
                Some(_) => None,
                None if super::in_rex() => {
                    anyhow::ensure!(
                        crate::journal::agent_caller().is_none(),
                        "Current conversation viewer is unavailable; refusing stale Rex startup placement"
                    );
                    Some(RexLocation::current()?)
                }
                None => None,
            }
        };
        let endpoint = match &location {
            Some(location) => location.server.clone(),
            None => Self::endpoint(server)?,
        };
        let rex = Self {
            server: endpoint,
            caller: location,
        };
        if let Some(location) = &rex.caller {
            let windows = rex.windows(&location.session)?;
            let caller_exists = windows
                .iter()
                .flat_map(|window| window["layers"].as_array().into_iter().flatten())
                .flat_map(|layer| layer["blocks"].as_array().into_iter().flatten())
                .any(|block| block["block_id"] == location.block);
            anyhow::ensure!(
                caller_exists,
                "The current Rex caller block is unavailable; nothing was opened"
            );
        }
        Ok(rex)
    }
    fn endpoint(server: Option<&str>) -> Result<String> {
        let mut command = Command::new("rex");
        for name in ["REX_SERVER", "REX_SESSION", "REX_BLOCK"] {
            command.env_remove(name);
        }
        if let Some(server) = server {
            command.args(["-S", server]);
        }
        command.args(["--autostart=false", "--timeout", "2s", "whoami", "--json"]);
        let output = super::run_host(command, "Rex")
            .map_err(|error| anyhow::anyhow!("{}", error.reason()))?;
        let who: Value = serde_json::from_str(&output)?;
        let server = who["endpoint"]
            .as_str()
            .context("Rex did not identify its server")?;
        // Unix peer authentication identifies this Machine's account. A remotely
        // selected default is never accepted as the local Task execution host.
        anyhow::ensure!(server.starts_with("unix://") && who["principal"]["transport"] == "unix", "Rex is connected to another Machine; run lf open on the Task's recorded Machine with its local Rex server");
        Ok(server.into())
    }
    fn sessions(&self) -> Result<Vec<Value>> {
        array(self.call("session.list", json!({}))?, "sessions")
    }
    fn windows(&self, container: &str) -> Result<Vec<Value>> {
        let view = self.call("session.view", json!({"session_id":container}))?;
        // Rex omits windows for an empty session. Its identity and revision
        // distinguish a complete empty view from an incomplete reply.
        if view.get("windows").is_none()
            && view["session_id"] == container
            && view["revision"].is_u64()
        {
            return Ok(Vec::new());
        }
        array(view, "windows")
    }
    /// Add a pane to a retained window, leaving its native edits and focus alone.
    fn split_beside(&self, container: &str, window: &str, layout: Value) -> Result<Value> {
        self.call("session.new_split", json!({"session_id":container,"window_id":window,"direction":"horizontal","side":"after","focus":false,"layout":layout}))
    }
    fn focus(&self, container: &str, window: &str) -> Result<String> {
        self.call(
            "session.focus_window",
            json!({"session_id":container,"window_id":window}),
        )?;
        let clients = array(self.call("client.list", json!({}))?, "clients")?;
        let mut candidates = Vec::new();
        for client in clients {
            let Some(id) = client["client_id"].as_str() else {
                continue;
            };
            let detail = self.call("client.inspect", json!({"client_id":id}))?;
            if detail["actions"].as_array().is_some_and(|actions| {
                actions
                    .iter()
                    .any(|action| action["name"] == "session.select")
            }) {
                candidates.push(id.to_string());
            }
        }
        if candidates.len() != 1 {
            return Ok(
                "Materialized in Rex; app visibility is unconfirmed (no unique capable client)"
                    .into(),
            );
        }
        self.call("client.perform", json!({"client_id":candidates[0],"action":"session.select","args":{"session_id":container,"window_id":window}}))?;
        Ok(
            "Rex accepted display selection; conversation input remains with its current viewer"
                .into(),
        )
    }
}

fn array(mut value: Value, key: &str) -> Result<Vec<Value>> {
    match value.as_object_mut().and_then(|object| object.remove(key)) {
        Some(Value::Array(items)) => Ok(items),
        Some(Value::Null) => Ok(Vec::new()),
        _ => bail!("Host inventory omitted {key}; absence is unknown"),
    }
}

/// The item recorded under `key` as `id`, else the one LF labelled `[lf:<owner>]`.
/// Native labels may change, so the recorded identity wins over the marker.
fn find_native<'a>(
    items: &'a [Value],
    key: &str,
    id: Option<&str>,
    owner: &str,
) -> Option<&'a Value> {
    let marker = format!("[lf:{owner}]");
    items
        .iter()
        .find(|item| id.is_some_and(|id| item[key] == id))
        .or_else(|| {
            items.iter().find(|item| {
                item["label"]
                    .as_str()
                    .is_some_and(|label| label.ends_with(&marker))
            })
        })
}

pub(super) fn run(cli: &crate::lf::Cli, args: &OpenArgs) -> Result<()> {
    anyhow::ensure!(
        args.copy
            .as_ref()
            .is_none_or(|copy| !copy.trim().is_empty()),
        "A workspace copy needs a name"
    );
    if let Some(id) = &args.watch_workspace {
        return follow(id, args);
    }
    let (targets, unknown) = super::targets(cli, args)?;
    crate::store::read_existing_registry()?.context("Work registry is unavailable")?;
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let mut report = OpenReport {
        receipts: Vec::new(),
        unknown,
    };
    for target in &targets {
        // The same opening lock protects ordinary and workspace opening.
        let _opening = Opening::lock(target)?;
        match open_machines(&store, target, args) {
            Ok(opened) => {
                report.receipts.extend(opened.receipts);
                report.unknown.extend(opened.unknown);
            }
            Err(error) => report.receipts.push(target.receipt(
                Some(OpenHost::Rex),
                None,
                OpenStatus::Failed,
                Some(format!("{error:#}")),
            )),
        }
    }
    if args.json || cli.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for receipt in &report.receipts {
            println!(
                "{}: {}{}",
                receipt.label,
                receipt.reason.as_deref().unwrap_or("opened"),
                receipt
                    .destination
                    .as_ref()
                    .map(|id| format!(" ({id})"))
                    .unwrap_or_default()
            );
            if let Some(hold) = receipt.rex_hold {
                println!("  Rex views: {}", hold_text(hold));
            }
            for conversation in &receipt.conversations {
                print_conversation(conversation);
            }
        }
        super::print_unknown(&report.unknown);
    }
    // The inter-Machine reader transports per-item failure receipts as data.
    if args.local_workspace {
        return Ok(());
    }
    anyhow::ensure!(
        !report
            .receipts
            .iter()
            .any(|r| r.status == OpenStatus::Failed),
        "Some workspace items could not be opened; successful native work was retained"
    );
    Ok(())
}

fn hold_text(hold: ViewHold) -> &'static str {
    match hold {
        ViewHold::Held => "present",
        ViewHold::Released => "confirmed closed",
        ViewHold::Unknown => "unreadable",
    }
}

fn print_conversation(conversation: &ConversationReadiness) {
    let state = match conversation.state {
        ObserverState::Connected => "connected",
        ObserverState::History => "history",
        ObserverState::Ended => "ended",
        ObserverState::Failed => "failed",
        ObserverState::Uncertain => "uncertain",
    };
    println!(
        "  Conversation {}: {state}{}",
        conversation.session,
        conversation
            .reason
            .as_ref()
            .map(|reason| format!("; {reason}"))
            .unwrap_or_default()
    );
    if !conversation.retry_argv.is_empty() {
        println!("  To retry: {}", conversation.retry_argv.join(" "));
    }
}

/// Repeats this opening with an explicit retry. Retries never take focus.
fn retry_argv(target: &Resolved, args: &OpenArgs, server: Option<&str>) -> Vec<String> {
    let mut argv = vec![
        "lf".into(),
        "open".into(),
        target.url.to_string(),
        "--host".into(),
        "rex".into(),
    ];
    if let Some(session) = &target.session {
        argv.extend(["--session".into(), session.clone()]);
    }
    if let Some(copy) = &args.copy {
        argv.extend(["--copy".into(), copy.clone()]);
    }
    if let Some(server) = server {
        argv.extend(["--server".into(), server.into()]);
    }
    argv.extend(["--no-focus".into(), "--retry-observers".into()]);
    argv
}

/// Launch a new observer for each failed Conversation at a destination. The
/// retained pane keeps its error; the new one starts passive, so it takes no
/// input. A lost reply stays pending and fences a duplicate.
fn retry_conversations(
    rex: &Rex,
    key: &str,
    cwd: &Path,
    mut launch: impl FnMut(Value) -> Result<Value>,
) -> Result<usize> {
    let mut retried = 0;
    for failed in observer::read(key).iter().filter(|o| o.retryable()) {
        launch(Host::Rex(&rex.server, Some(key)).conversation(cwd, &failed.session)?)?;
        observer::confirm(key);
        retried += 1;
    }
    Ok(retried)
}

fn open_machines(store: &SqliteStore, target: &Resolved, args: &OpenArgs) -> Result<OpenReport> {
    let repository = target
        .repository_id
        .as_deref()
        .context("Repository has no Work identity")?;
    let selected = scope(store, target)?;
    let plan = workspace_plan(store, &RepositoryId::parse(repository)?, selected.clone())?;
    let local = store.local_machine()?.id;
    let mut local_tasks = Vec::new();
    let mut peers = BTreeSet::new();
    let mut receipts = Vec::new();
    let mut unknown = plan.resolution.unavailable.clone();
    for content in plan
        .contents
        .iter()
        .filter(|content| content.work.kind == WorkWorkspaceKind::Task)
        .filter(|content| target.task.is_some() || !content.states.contains(&SelectionState::Done))
        .filter(|content| {
            args.arrive_task
                .as_ref()
                .is_none_or(|id| *id == content.work.id)
        })
    {
        match store.task_execution_route(&TaskId::parse(&content.work.id)?) {
            Ok(route) if route.machine_id == local => {
                if eligible(content, target.task.is_some() && args.arrive_task.is_none()) {
                    local_tasks.push(content);
                }
            }
            Ok(route) => {
                peers.insert(route.machine_id.to_string());
            }
            Err(error) => unknown.push(format!(
                "{}: execution Machine unavailable: {error}",
                content.title
            )),
        }
    }
    anyhow::ensure!(
        args.arrive_task.is_none() || peers.is_empty(),
        "Arriving Task belongs to another Machine; no local fallback"
    );
    if !args.local_workspace {
        for peer in peers {
            let mut remote_args = args.clone();
            remote_args.session = target.session.clone();
            remote_args.diff |= target.url.diff;
            let result = remote_open(store, peer.as_str(), repository, &selected, &remote_args);
            match result {
                Ok(mut report) => {
                    // The peer's own command means nothing on this Machine.
                    let retry = retry_argv(target, args, args.server.as_deref());
                    report
                        .receipts
                        .iter_mut()
                        .flat_map(|receipt| &mut receipt.conversations)
                        .filter(|conversation| !conversation.retry_argv.is_empty())
                        .for_each(|conversation| conversation.retry_argv = retry.clone());
                    receipts.append(&mut report.receipts);
                    unknown.append(&mut report.unknown);
                }
                Err(error) => receipts.push(target.receipt(
                    Some(OpenHost::Rex),
                    None,
                    OpenStatus::Failed,
                    Some(format!("Machine {peer}: {error:#}; no local fallback")),
                )),
            }
        }
    }
    // A broad local workspace keeps watching even when its working set is empty.
    if !local_tasks.is_empty() || (target.task.is_none() && !args.local_workspace) {
        let result = Rex::local(args.server.as_deref())
            .and_then(|rex| open(store, &rex, target, args, &plan.selection, &local_tasks));
        match result {
            Ok(mut opened) => receipts.append(&mut opened),
            Err(error) => receipts.push(target.receipt(
                Some(OpenHost::Rex),
                None,
                OpenStatus::Failed,
                Some(format!("{error:#}")),
            )),
        }
    }
    Ok(OpenReport { receipts, unknown })
}

fn remote_open(
    store: &SqliteStore,
    machine: &str,
    repository: &str,
    selection: &Selection,
    args: &OpenArgs,
) -> Result<OpenReport> {
    let machine = store
        .machines()?
        .into_iter()
        .find(|entry| entry.id.as_str() == machine)
        .context("Task execution Machine has no configured route")?;
    let mut argv = vec![
        "--repository".into(),
        repository.into(),
        "open".into(),
        "--host".into(),
        "rex".into(),
        "--local-workspace".into(),
        "--json".into(),
    ];
    match &selection.expression {
        SelectionExpression::Work { work } => argv.extend(["--task".into(), work.id.clone()]),
        SelectionExpression::Wave { wave } => argv.extend(["--wave".into(), wave.to_string()]),
        SelectionExpression::Repository => {}
        _ => bail!("This workspace selection cannot be routed"),
    }
    if let Some(copy) = &args.copy {
        argv.extend(["--copy".into(), copy.clone()]);
    }
    if args.new_shells {
        argv.push("--new-shells".into());
    }
    if let Some(session) = &args.session {
        argv.extend(["--session".into(), session.clone()]);
    }
    if args.diff {
        argv.push("--diff".into());
    }
    if args.no_focus {
        argv.push("--no-focus".into());
    }
    if args.retry_observers {
        argv.push("--retry-observers".into());
    }
    if let Some(task) = &args.arrive_task {
        argv.extend(["--arrive-task".into(), task.clone()]);
    }
    let bytes = tokio::runtime::Runtime::new()?
        .block_on(super::super::ssh::request(&machine, &argv, None))?;
    serde_json::from_slice(&bytes)
        .context("Remote workspace receipt is unavailable; creation may have succeeded")
}

fn scope(store: &SqliteStore, target: &Resolved) -> Result<Selection> {
    Ok(selection(if let Some(task) = &target.task {
        SelectionExpression::Work {
            work: WorkWorkspaceKey {
                repository: task.repository_id.to_string(),
                kind: WorkWorkspaceKind::Task,
                id: task.id.to_string(),
            },
        }
    } else if let Some(wave) = &target.wave {
        SelectionExpression::Wave {
            wave: store
                .resolve_wave_id(wave, Some(&target.repository_path.to_string_lossy()))?
                .context("Wave is unavailable")?,
        }
    } else {
        SelectionExpression::Repository
    }))
}

fn workspace(
    store: &SqliteStore,
    rex: &Rex,
    repository: &str,
    selected: Selection,
    copy: Option<&str>,
) -> Result<HostWorkspace> {
    let id = identity(&format!(
        "{repository}\n{}\n{}\n{}",
        rex.server,
        serde_json::to_string(&selected.expression)?,
        copy.unwrap_or("")
    ));
    Ok(store.host_workspace(&id)?.unwrap_or(HostWorkspace {
        id,
        repository_id: repository.into(),
        host: "rex".into(),
        server: rex.server.clone(),
        selection: selected,
        copy_name: copy.map(str::to_owned),
        container_id: None,
        creation_pending: false,
        updated_at: now(),
    }))
}

fn ensure_container(
    store: &SqliteStore,
    rex: &Rex,
    workspace: &mut HostWorkspace,
    label: &str,
    follow: Option<Follow>,
    cwd: &Path,
) -> Result<()> {
    let _opening = Opening::lock_key(&format!("host-workspace:{}", workspace.id))?;
    if let Some(current) = store.host_workspace(&workspace.id)? {
        *workspace = current;
    }
    let inventory = rex.sessions()?;
    let recorded = workspace.container_id.as_deref();
    let found = find_native(&inventory, "session_id", recorded, &workspace.id);
    let retained = found.filter(|item| recorded.is_some_and(|id| item["session_id"] == id));
    if let Some(item) = found {
        workspace.container_id = Some(
            item["session_id"]
                .as_str()
                .context("Rex session has no identity")?
                .into(),
        );
    } else {
        anyhow::ensure!(
            !workspace.creation_pending || workspace.container_id.is_some(),
            "Workspace creation outcome is unknown; inspect Rex before retrying this occurrence"
        );
        workspace.container_id = None;
        workspace.creation_pending = true;
        workspace.updated_at = now();
        store.save_host_workspace(workspace)?;
        let label = match &workspace.copy_name {
            Some(copy) => format!("{label} · {copy}"),
            None => label.into(),
        };
        let label: String = label.chars().filter(|c| !c.is_control()).take(80).collect();
        let mut windows = Vec::new();
        if let SelectionExpression::Wave { wave } = &workspace.selection.expression {
            if let Some(session) =
                store.primary_session(&crate::session::PrimaryScope::Wave(wave.clone()))?
            {
                observer::clear(&workspace.id)?;
                let host = Host::Rex(&rex.server, Some(&workspace.id));
                windows.push(json!({"window_label":"Conversation","layout":host.conversation(cwd,session.id.as_str())?}));
            }
        }
        let result = rex.call(
            "session.create",
            json!({"label":format!("{label} [lf:{}]", workspace.id),"initial_windows":windows}),
        )?;
        workspace.container_id = Some(
            result["session_id"]
                .as_str()
                .context("Rex create reply omitted session identity; outcome is unknown")?
                .into(),
        );
        observer::confirm(&workspace.id);
    }
    if retained.is_none() {
        workspace.creation_pending = false;
        workspace.updated_at = now();
        store.save_host_workspace(workspace)?;
    }
    if let Some(follow) = follow {
        ensure_follower(rex, workspace, cwd, follow)?;
    }
    Ok(())
}

fn open(
    store: &SqliteStore,
    rex: &Rex,
    target: &Resolved,
    args: &OpenArgs,
    selected: &Selection,
    local_tasks: &[&ViewContent],
) -> Result<Vec<OpenReceipt>> {
    let repo = target
        .repository_id
        .as_deref()
        .context("Repository has no Work identity")?;
    let mut root = workspace(store, rex, repo, selected.clone(), args.copy.as_deref())?;
    let explicit = target.task.is_some();
    if let Some(task) = target
        .task
        .as_ref()
        .filter(|_| root.container_id.is_none() && !root.creation_pending)
    {
        let inventory = rex.sessions()?;
        for existing in store.task_host_workspaces(&task.id, "rex", &rex.server)? {
            if existing.copy_name != args.copy {
                continue;
            }
            if let Some(container) = &existing.container_id {
                if inventory
                    .iter()
                    .any(|item| item["session_id"] == *container)
                {
                    root = existing;
                    break;
                }
            }
        }
        if root.container_id.is_none() && args.copy.is_none() {
            if let Some(caller) = &rex.caller {
                root.container_id = Some(caller.session.clone());
                store.save_host_workspace(&root)?;
            }
        }
    }
    ensure_container(
        store,
        rex,
        &mut root,
        &target.label,
        if explicit { None } else { following(args) },
        &target.repository_path,
    )?;
    let mut receipts = Vec::new();
    let mut groups = HashMap::new();
    let mut launched = Vec::new();
    let mut observed = Vec::new();
    for content in local_tasks {
        let task = store
            .task(&TaskId::parse(&content.work.id)?)?
            .context("Task is unavailable")?;
        // Explicit opening may reuse an occurrence in a repository or Wave
        // container. Its current membership must not move that native view.
        let destination = if explicit {
            Ok(root.clone())
        } else {
            task_workspace(
                store,
                rex,
                &root,
                task.wave_id.as_ref(),
                &target.repository_path,
                &mut groups,
                following(args),
            )
        };
        let result = destination.and_then(|destination| {
            materialize(
                store,
                rex,
                &destination,
                content,
                &target.repository_path,
                Presentation {
                    passive: args.arrive_task.is_some(),
                    omit_shells: args.copy.is_some() && !args.new_shells,
                    session: target.session.as_deref(),
                    diff: args.diff || target.url.diff,
                    retry_observers: args.retry_observers,
                },
            )
            .map(|opened| (destination, opened))
        });
        let mut receipt = target.receipt(Some(OpenHost::Rex), None, OpenStatus::Opening, None);
        receipt.label = format!("{} · {}", task.identifier, task.title);
        receipt.url = crate::work::link::WorkUrl::new(
            target.url.repository.clone(),
            crate::work::link::WorkTarget::Task(task.identifier),
        )
        .to_string();
        match result {
            Ok((destination, opened)) => {
                let window = opened.window;
                receipt.destination = Some(window.clone());
                if opened.created || opened.retried > 0 {
                    launched.push(opened.observers.clone());
                }
                observed.push((receipts.len(), opened.observers));
                let focus = !args.no_focus && args.arrive_task.is_none() && receipts.is_empty();
                let showing = if focus {
                    rex.focus(
                        destination
                            .container_id
                            .as_deref()
                            .expect("container created"),
                        &window,
                    )
                    .unwrap_or_else(|error| {
                        format!("Materialized; display selection failed: {error}")
                    })
                } else {
                    "Focus preserved".into()
                };
                let retried = match opened.retried {
                    0 => String::new(),
                    count => format!("; {count} new Conversation observers beside retained panes"),
                };
                receipt.reason = Some(format!(
                    "{}; {} new native shells{retried}; {showing}",
                    if opened.created {
                        "Created"
                    } else {
                        "Reused native layout without replaying commands"
                    },
                    opened.shells
                ));
            }
            Err(error) => {
                receipt.status = OpenStatus::Failed;
                receipt.reason = Some(format!("{error:#}"));
            }
        }
        receipt.rex_hold = hold::task_hold(store, &task.id).unwrap_or_else(|error| {
            tracing::debug!(%error, "Task occurrences are unreadable; the hold stays");
            Some(ViewHold::Unknown)
        });
        receipts.push(receipt);
    }
    if receipts.is_empty() && !explicit {
        receipts.push(target.receipt(
            Some(OpenHost::Rex),
            root.container_id.clone(),
            OpenStatus::Opening,
            Some("Workspace is following started Tasks; no eligible local Tasks yet".into()),
        ));
    }
    // Wave containers carry their own conversation; a Task opened explicitly
    // inside one reports only itself.
    if !explicit {
        for workspace in std::iter::once(&root).chain(groups.values()) {
            let Some(container) = workspace.container_id.as_deref() else {
                continue;
            };
            let key = &workspace.id;
            let mut receipt = target.receipt(
                Some(OpenHost::Rex),
                Some(container.into()),
                OpenStatus::Opening,
                None,
            );
            receipt.label = "Wave conversation".into();
            if args.retry_observers {
                match retry_conversations(rex, key, &target.repository_path, |layout| {
                    rex.call("session.new_window", json!({"session_id":container,"window_label":"Conversation","focus":false,"layout":layout}))
                }) {
                    Ok(0) => {}
                    Ok(_) => launched.push(key.clone()),
                    Err(error) => {
                        receipt.status = OpenStatus::Failed;
                        receipt.reason = Some(format!("{error:#}"));
                    }
                }
            }
            if !observer::read(key).is_empty() {
                observed.push((receipts.len(), key.clone()));
                receipts.push(receipt);
            }
        }
    }
    observer::settle(&launched);
    let retry = retry_argv(target, args, Some(&rex.server));
    for (index, key) in observed {
        receipts[index].conversations = observer::read(&key)
            .iter()
            .map(|observer| observer.readiness(&retry))
            .collect();
    }
    Ok(receipts)
}

/// Repository selection groups Tasks by Wave; explicit Task/Wave selections stay put.
fn task_workspace(
    store: &SqliteStore,
    rex: &Rex,
    root: &HostWorkspace,
    wave: Option<&WaveId>,
    repo: &Path,
    groups: &mut HashMap<WaveId, HostWorkspace>,
    follow: Option<Follow>,
) -> Result<HostWorkspace> {
    if !matches!(root.selection.expression, SelectionExpression::Repository) {
        return Ok(root.clone());
    }
    let Some(wave) = wave else {
        return Ok(root.clone());
    };
    if let Some(group) = groups.get(wave) {
        return Ok(group.clone());
    }
    let mut group = workspace(
        store,
        rex,
        &root.repository_id,
        selection(SelectionExpression::Wave { wave: wave.clone() }),
        root.copy_name.as_deref(),
    )?;
    let label = store
        .get_wave(wave)?
        .map(|wave| wave.slug().to_owned())
        .unwrap_or_else(|| wave.to_string());
    ensure_container(store, rex, &mut group, &label, follow, repo)?;
    groups.insert(wave.clone(), group.clone());
    Ok(group)
}

fn eligible(content: &ViewContent, explicit: bool) -> bool {
    content.work.kind == WorkWorkspaceKind::Task
        && (explicit
            || content.states.iter().any(|state| {
                matches!(
                    state,
                    SelectionState::Idle
                        | SelectionState::Active
                        | SelectionState::Moving
                        | SelectionState::Shipping
                )
            }))
}

fn materialize(
    store: &SqliteStore,
    rex: &Rex,
    workspace: &HostWorkspace,
    content: &ViewContent,
    repo: &Path,
    presentation: Presentation<'_>,
) -> Result<Materialized> {
    let container = workspace
        .container_id
        .as_deref()
        .context("Workspace is unavailable")?;
    let id = identity(&format!(
        "{}\n{}\n{}\n{}",
        workspace.id,
        content.work.id,
        presentation.session.unwrap_or(""),
        presentation.diff
    ));
    let _opening = Opening::lock_key(&format!("host-occurrence:{id}"))?;
    let observers = id.clone();
    let mut occurrence = store.host_occurrence(&id)?.unwrap_or(HostOccurrence {
        id,
        workspace_id: workspace.id.clone(),
        task_id: TaskId::parse(&content.work.id)?,
        destination_id: None,
        creation: HostCreation::Complete,
    });
    let inventory = rex.windows(container)?;
    let recorded = occurrence.destination_id.as_deref();
    let existing = find_native(&inventory, "window_id", recorded, &occurrence.id);
    let retained = existing.filter(|window| recorded.is_some_and(|id| window["window_id"] == id));
    if retained.is_none() && presentation.passive && recorded.is_some() {
        bail!("Task view was explicitly closed; following leaves it closed");
    }
    if let Some(window) = existing {
        if retained.is_none() {
            occurrence.destination_id = Some(
                window["window_id"]
                    .as_str()
                    .context("Window has no identity")?
                    .to_string(),
            );
            // Recover identity without turning an uncertain creation into replay permission.
            store.save_host_occurrence(&occurrence)?;
        }
        anyhow::ensure!(
            occurrence.creation != HostCreation::ToolsPending,
            "Tool activation outcome is unknown; no duplicate was attempted"
        );
        if presentation.passive
            || occurrence.creation != HostCreation::Passive
            || content.location.is_none()
        {
            let window = occurrence
                .destination_id
                .expect("retained window has an identity");
            let retried = if presentation.retry_observers {
                retry_conversations(rex, &observers, tool_cwd(content, repo), |layout| {
                    rex.split_beside(container, &window, layout)
                })?
            } else {
                0
            };
            return Ok(Materialized {
                window,
                created: false,
                shells: 0,
                observers,
                retried,
            });
        }
    } else {
        anyhow::ensure!(
            occurrence.creation != HostCreation::Pending || occurrence.destination_id.is_some(),
            "Task window creation outcome is unknown; no duplicate was attempted"
        );
        observer::clear(&observers)?;
    }

    let host = Host::Rex(&rex.server, Some(&observers));
    let (layout, shells) = layout(store, content, repo, presentation, host)?;
    let id = if existing.is_some() {
        let id = occurrence
            .destination_id
            .as_deref()
            .expect("retained window has an identity");
        occurrence.creation = HostCreation::ToolsPending;
        store.save_host_occurrence(&occurrence)?;
        rex.split_beside(container, id, layout)?;
        id.to_owned()
    } else {
        occurrence.destination_id = None;
        occurrence.creation = HostCreation::Pending;
        store.save_host_occurrence(&occurrence)?;
        let label: String = content
            .title
            .chars()
            .filter(|c| !c.is_control())
            .take(80)
            .collect();
        let result = rex.call("session.new_window", json!({"session_id":container,"window_label":format!("{label} [lf:{}]", occurrence.id),"focus":false,"layout":layout}))?;
        result["window_id"]
            .as_str()
            .context("Rex create reply omitted window identity; outcome is unknown")?
            .to_owned()
    };
    observer::confirm(&observers);
    occurrence.destination_id = Some(id.clone());
    occurrence.creation = if presentation.passive || content.location.is_none() {
        HostCreation::Passive
    } else {
        HostCreation::Complete
    };
    store.save_host_occurrence(&occurrence)?;
    Ok(Materialized {
        window: id,
        created: true,
        shells,
        observers,
        retried: 0,
    })
}

// The OS lock proves follower liveness, including after a host restart. Native
// window/block identities alone cannot prove that their commands survived.
fn follower_file(id: &str) -> Result<std::fs::File> {
    let directory = crate::store::lf_home_dir().join("open");
    std::fs::create_dir_all(&directory)?;
    Ok(std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(format!("{}.follower", identity(id))))?)
}

fn ensure_follower(rex: &Rex, workspace: &HostWorkspace, cwd: &Path, follow: Follow) -> Result<()> {
    use fs2::FileExt;
    let mut file = follower_file(&workspace.id)?;
    match file.try_lock_exclusive() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    let mut pending = String::new();
    file.read_to_string(&mut pending)?;
    anyhow::ensure!(
        pending.is_empty(),
        "Workspace follower launch is pending or unknown; no duplicate was attempted"
    );
    file.write_all(b"pending")?;
    file.sync_all()?;
    let mut words = vec!["open", "--host", "rex", "--watch-workspace", &workspace.id];
    if matches!(follow, Follow::Local) {
        words.push("--local-workspace");
    }
    rex.call("session.new_window", json!({"session_id":workspace.container_id,"window_label":"Workspace","focus":false,"layout":Host::Rex(&rex.server,None).block("Workspace",cwd,viewer_argv(cwd,&words)?)}))?;
    Ok(())
}

fn viewer_argv(cwd: &Path, words: &[&str]) -> Result<Vec<String>> {
    let mut argv = vec![
        crate::os_process::resolve_lf_binary()?
            .to_string_lossy()
            .into_owned(),
        "--cwd".into(),
        cwd.to_string_lossy().into_owned(),
    ];
    argv.extend(words.iter().map(|word| (*word).into()));
    if crate::store::custom_home_selected() {
        argv.splice(
            0..0,
            [
                "env".into(),
                format!("LF_HOME={}", crate::store::lf_home_dir().display()),
            ],
        );
    }
    Ok(argv)
}

fn follow(id: &str, args: &OpenArgs) -> Result<()> {
    use fs2::FileExt;
    let follower = follower_file(id)?;
    // The creator briefly owns this lock until the native launch returns.
    follower.lock_exclusive()?;
    follower.set_len(0)?;
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    // Creation records the pending occurrence before starting this native block.
    let mut workspace = store
        .host_workspace(id)?
        .context("Workspace occurrence is unavailable")?;
    let rex = Rex {
        server: workspace.server.clone(),
        caller: None,
    };
    println!("Loopflow workspace · {id}\nNewly started Tasks appear without taking focus. Close this tab to stop following.");
    let repo = store
        .repository_path(&RepositoryId::parse(&workspace.repository_id)?)?
        .context("Repository location is unavailable")?;
    let selected = workspace.selection.clone();
    let repository = RepositoryId::parse(&workspace.repository_id)?;
    let subscription = super::super::watch::start(
        selected,
        super::super::watch::Watched::every(super::super::watch::ACTIVITY_CLOCK),
        move |store, selected| {
            let repository = repository.clone();
            async move {
                let store = super::super::watch::require_store(store)?;
                workspace_plan(&store.sqlite, &repository, selected)
            }
        },
        super::super::watch::Transport::Pull(Box::new(|| {})),
    )?
    .context("Workspace watch did not start")?;
    let mut arrived = BTreeSet::new();
    let mut refresh = std::time::Instant::now();
    let mut request_id = 0_u64;
    loop {
        workspace = store
            .host_workspace(id)?
            .context("Workspace occurrence was removed")?;
        if let Some(container) = &workspace.container_id {
            if !rex
                .sessions()?
                .iter()
                .any(|session| session["session_id"] == *container)
            {
                return Ok(());
            }
            if let Some(bytes) = subscription.pull() {
                let frame: Value = serde_json::from_slice(&bytes)?;
                if let Some(error) = frame["unavailable"].as_str() {
                    eprintln!("Work selection unavailable: {error}");
                    continue;
                }
                let plan: ViewPlan = serde_json::from_value(frame["body"].clone())?;
                let mut groups = HashMap::new();
                for content in plan
                    .contents
                    .iter()
                    .filter(|content| content.work.kind == WorkWorkspaceKind::Task)
                    .filter(|content| !content.states.contains(&SelectionState::Done))
                {
                    if arrived.contains(&content.work.id) {
                        continue;
                    }
                    match arrive(
                        &store,
                        &rex,
                        &workspace,
                        content,
                        Path::new(&repo),
                        &mut groups,
                        args,
                    ) {
                        Ok(true) => {
                            arrived.insert(content.work.id.clone());
                        }
                        Ok(false) => {}
                        Err(error) => eprintln!("{}: {error:#}", content.title),
                    }
                }
            }
        }
        // Remote starts and recovered connections need not change this Machine's
        // Work snapshot. Re-read periodically; creation receipts fence retries.
        if refresh.elapsed() >= std::time::Duration::from_secs(15) {
            request_id += 1;
            subscription.request(format!("{{\"id\":{request_id}}}").as_bytes());
            refresh = std::time::Instant::now();
        }
        let mut fd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one initialized poll descriptor, bounded wait, no retained pointer.
        if unsafe { libc::poll(&mut fd, 1, 1000) } > 0 && fd.revents != 0 {
            let mut byte = [0];
            if std::io::stdin().read(&mut byte)? == 0 || matches!(byte[0], b'q' | 3) {
                return Ok(());
            }
        }
        if !std::io::stdin().is_terminal() {
            return Ok(());
        }
    }
}

/// Report arrival only after materialization; failed/unknown receipts remain retryable.
fn arrive(
    store: &SqliteStore,
    rex: &Rex,
    workspace: &HostWorkspace,
    content: &ViewContent,
    repo: &Path,
    groups: &mut HashMap<WaveId, HostWorkspace>,
    args: &OpenArgs,
) -> Result<bool> {
    let task_id = TaskId::parse(&content.work.id)?;
    let route = store
        .task_execution_route(&task_id)
        .context("execution Machine unavailable")?;
    if route.machine_id != store.local_machine()?.id {
        if args.local_workspace {
            return Ok(false);
        }
        let arrival = OpenArgs {
            copy: workspace.copy_name.clone(),
            no_focus: true,
            arrive_task: Some(content.work.id.clone()),
            ..args.clone()
        };
        let report = remote_open(
            store,
            route.machine_id.as_str(),
            &workspace.repository_id,
            &workspace.selection,
            &arrival,
        )
        .with_context(|| format!("Machine {}; no local fallback", route.machine_id))?;
        let delivered = report
            .receipts
            .iter()
            .any(|receipt| receipt.status != OpenStatus::Failed && receipt.destination.is_some());
        for receipt in report.receipts {
            println!(
                "{}: {}",
                receipt.label,
                receipt.reason.as_deref().unwrap_or("opened")
            );
        }
        super::print_unknown(&report.unknown);
        return Ok(delivered);
    }
    if !eligible(content, false) {
        return Ok(false);
    }
    let task = store.task(&task_id)?.context("Task is unavailable")?;
    let destination = task_workspace(
        store,
        rex,
        workspace,
        task.wave_id.as_ref(),
        repo,
        groups,
        following(args),
    )?;
    let opened = materialize(
        store,
        rex,
        &destination,
        content,
        repo,
        Presentation {
            passive: true,
            omit_shells: workspace.copy_name.is_some(),
            ..Presentation::default()
        },
    )?;
    if opened.created {
        println!("Added {}", content.title);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{array, eligible, listening_endpoint};
    use crate::work::selection::{
        SelectionState, ViewContent, WorkWorkspaceKey, WorkWorkspaceKind,
    };
    use serde_json::json;

    #[test]
    fn caller_listener_preserves_argument_boundaries() {
        for args in [
            vec![
                "rex",
                "server",
                "run",
                "--listen",
                "unix:///Task servers/rex.sock",
            ],
            vec![
                "rex",
                "server",
                "run",
                "--listen=unix:///Task servers/rex.sock",
            ],
        ] {
            assert_eq!(
                listening_endpoint(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()),
                Some("unix:///Task servers/rex.sock".into())
            );
        }
        assert_eq!(
            listening_endpoint(&["--config".into(), "/rex.lua".into()]),
            None
        );
    }

    #[test]
    fn only_started_unfinished_tasks_arrive_automatically() {
        let mut content = ViewContent {
            pane_revision: None,
            face: None,
            work: WorkWorkspaceKey {
                repository: "r".into(),
                kind: WorkWorkspaceKind::Task,
                id: "t".into(),
            },
            title: "Task".into(),
            states: Default::default(),
            primary_session: None,
            location: None,
            argv: None,
            unavailable: vec![],
        };
        for (state, expected) in [
            (SelectionState::Unstarted, false),
            (SelectionState::Idle, true),
            (SelectionState::Moving, true),
            (SelectionState::Done, false),
        ] {
            content.states = std::collections::BTreeSet::from([state]);
            assert_eq!(eligible(&content, false), expected);
            assert!(eligible(&content, true));
        }
    }
    #[test]
    fn missing_inventory_is_not_an_empty_host() {
        assert!(array(json!({}), "windows").is_err());
        assert!(array(json!({"windows":null}), "windows")
            .unwrap()
            .is_empty());
    }
}
