//! Native Rex workspaces. The registry keeps locators and creation receipts;
//! Rex alone owns geometry and ordinary processes. Reads never execute resume argv.
mod herdr;
mod layout;

pub(super) use herdr::open_herdr;
use layout::{layout, Host};

use std::collections::{BTreeSet, HashMap};
use std::io::{IsTerminal, Read, Write};
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{OpenArgs, OpenHost, OpenReceipt, OpenReport, OpenStatus, Opening, Resolved};
use crate::durable::{RepositoryId, TaskId};
use crate::id::WaveId;
use crate::store::sqlite::{
    host_workspaces::{HostOccurrence, HostWorkspace},
    SqliteStore,
};
use crate::work::selection::{
    Selection, SelectionExpression, SelectionFilter, SelectionId, SelectionState, ViewContent,
    ViewPlan, WorkWorkspaceKey, WorkWorkspaceKind,
};

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
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
        Ok(Self {
            server: Rex::endpoint(std::env::var("REX_SERVER").ok().as_deref())?,
            session: std::env::var("REX_SESSION").context("Rex caller session is missing")?,
            block: std::env::var("REX_BLOCK").context("Rex caller block is missing")?,
        })
    }
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
                        .context("Current Rex viewer placement is unreadable")?,
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
        let endpoint =
            Self::endpoint(server.or(location.as_ref().map(|place| place.server.as_str())))?;
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
    let (targets, mut unknown) = super::targets(cli, args)?;
    crate::store::read_existing_registry()?.context("Work registry is unavailable")?;
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let mut receipts = Vec::new();
    for target in &targets {
        // The same opening lock protects ordinary and workspace opening.
        let _opening = Opening::lock(target)?;
        let result = open_machines(&store, target, args);
        match result {
            Ok((mut opened, mut missing)) => {
                receipts.append(&mut opened);
                unknown.append(&mut missing);
            }
            Err(error) => receipts.push(target.receipt(
                Some(OpenHost::Rex),
                None,
                OpenStatus::Failed,
                Some(format!("{error:#}")),
            )),
        }
    }
    let report = OpenReport { receipts, unknown };
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

fn open_machines(
    store: &SqliteStore,
    target: &Resolved,
    args: &OpenArgs,
) -> Result<(Vec<OpenReceipt>, Vec<String>)> {
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
    Ok((receipts, unknown))
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
    let retained = inventory.iter().find(|item| {
        workspace
            .container_id
            .as_ref()
            .is_some_and(|id| item["session_id"] == *id)
    });
    let marker = format!("[lf:{}]", workspace.id);
    // Native labels may have changed; the recorded identity wins over a marker.
    if let Some(item) = retained.or_else(|| {
        inventory.iter().find(|item| {
            item["label"]
                .as_str()
                .is_some_and(|label| label.ends_with(&marker))
        })
    }) {
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
                windows.push(json!({"window_label":"Conversation","layout":Host::Rex(&rex.server).block("Conversation",cwd,viewer_argv(cwd,&["session","view",session.id.as_str()])?)}));
            }
        }
        let result = rex.call(
            "session.create",
            json!({"label":format!("{label} {marker}"),"initial_windows":windows}),
        )?;
        workspace.container_id = Some(
            result["session_id"]
                .as_str()
                .context("Rex create reply omitted session identity; outcome is unknown")?
                .into(),
        );
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
    for content in local_tasks {
        let task = store
            .task(&TaskId::parse(&content.work.id)?)?
            .context("Task is unavailable")?;
        let destination = if explicit {
            root.clone()
        } else {
            task_workspace(
                store,
                rex,
                &root,
                task.wave_id.as_ref(),
                &target.repository_path,
                &mut groups,
                following(args),
            )?
        };
        let result = materialize(
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
            },
        );
        let mut receipt = target.receipt(Some(OpenHost::Rex), None, OpenStatus::Opening, None);
        receipt.label = format!("{} · {}", task.identifier, task.title);
        receipt.url = crate::work::link::WorkUrl::new(
            target.url.repository.clone(),
            crate::work::link::WorkTarget::Task(task.identifier),
        )
        .to_string();
        match result {
            Ok((window, created, shells)) => {
                receipt.destination = Some(window.clone());
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
                receipt.reason = Some(format!(
                    "{}; {shells} new native shells; {showing}",
                    if created {
                        "Created"
                    } else {
                        "Reused native layout without replaying commands"
                    }
                ));
            }
            Err(error) => {
                receipt.status = OpenStatus::Failed;
                receipt.reason = Some(format!("{error:#}"));
            }
        }
        receipts.push(receipt);
    }
    if receipts.is_empty() && !explicit {
        receipts.push(target.receipt(
            Some(OpenHost::Rex),
            root.container_id,
            OpenStatus::Opening,
            Some("Workspace is following started Tasks; no eligible local Tasks yet".into()),
        ));
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
) -> Result<(String, bool, usize)> {
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
    let mut occurrence = store.host_occurrence(&id)?.unwrap_or(HostOccurrence {
        id,
        workspace_id: workspace.id.clone(),
        task_id: TaskId::parse(&content.work.id)?,
        destination_id: None,
        creation: "complete".into(),
    });
    let inventory = rex.windows(container)?;
    let retained = occurrence
        .destination_id
        .as_ref()
        .and_then(|id| inventory.iter().find(|window| window["window_id"] == *id));
    if retained.is_none() && presentation.passive && occurrence.destination_id.is_some() {
        bail!("Task view was explicitly closed; following leaves it closed");
    }
    let marker = format!("[lf:{}]", occurrence.id);
    if let Some(window) = retained.or_else(|| {
        inventory.iter().find(|window| {
            window["label"]
                .as_str()
                .is_some_and(|label| label.ends_with(&marker))
        })
    }) {
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
        return activate(
            store,
            rex,
            container,
            &mut occurrence,
            content,
            repo,
            presentation,
        );
    }
    anyhow::ensure!(
        occurrence.creation != "pending" || occurrence.destination_id.is_some(),
        "Task window creation outcome is unknown; no duplicate was attempted"
    );
    let (layout, shells) = layout(store, content, repo, presentation, Host::Rex(&rex.server))?;
    occurrence.destination_id = None;
    occurrence.creation = "pending".into();
    store.save_host_occurrence(&occurrence)?;
    let label: String = content
        .title
        .chars()
        .filter(|c| !c.is_control())
        .take(80)
        .collect();
    let result = rex.call("session.new_window", json!({"session_id":container,"window_label":format!("{label} {marker}"),"focus":false,"layout":layout}))?;
    let id = result["window_id"]
        .as_str()
        .context("Rex create reply omitted window identity; outcome is unknown")?
        .to_string();
    occurrence.destination_id = Some(id.clone());
    occurrence.creation = if presentation.passive || content.location.is_none() {
        "passive"
    } else {
        "complete"
    }
    .into();
    store.save_host_occurrence(&occurrence)?;
    Ok((id, true, shells))
}

// Activation adds the selected tools beside the native arrival. It never replaces
// a block: the arrival may already contain edits or native splits.
fn activate(
    store: &SqliteStore,
    rex: &Rex,
    container: &str,
    occurrence: &mut HostOccurrence,
    content: &ViewContent,
    repo: &Path,
    presentation: Presentation<'_>,
) -> Result<(String, bool, usize)> {
    let id = occurrence
        .destination_id
        .clone()
        .context("Task window is unavailable")?;
    anyhow::ensure!(
        occurrence.creation != "tools_pending",
        "Tool activation outcome is unknown; no duplicate was attempted"
    );
    if presentation.passive || occurrence.creation != "passive" || content.location.is_none() {
        return Ok((id, false, 0));
    }
    let (layout, shells) = layout(store, content, repo, presentation, Host::Rex(&rex.server))?;
    occurrence.creation = "tools_pending".into();
    store.save_host_occurrence(occurrence)?;
    rex.call("session.new_split", json!({"session_id":container,"window_id":id,"direction":"horizontal","side":"after","focus":false,"layout":layout}))?;
    occurrence.creation = "complete".into();
    store.save_host_occurrence(occurrence)?;
    Ok((id, true, shells))
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
    rex.call("session.new_window", json!({"session_id":workspace.container_id,"window_label":"Workspace","focus":false,"layout":Host::Rex(&rex.server).block("Workspace",cwd,viewer_argv(cwd,&words)?)}))?;
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
    let (_, created, _) = materialize(
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
    if created {
        println!("Added {}", content.title);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::{array, eligible};
    use crate::work::selection::{
        SelectionState, ViewContent, WorkWorkspaceKey, WorkWorkspaceKind,
    };
    use serde_json::json;

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
