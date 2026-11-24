//! Fresh native workspaces. Every `lf open --workspace` launches a repository's
//! or Wave's started Task tabs, once, in a new Rex session, Herdr workspace or
//! cmux group: each tab is the Task's Session. The host owns them afterwards: LF
//! records no host, container, tab, layout or creation state.
use loopflow_execution::store::selections;

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use super::{
    cmux, herdr, rex::Rex, OpenArgs, OpenHost, OpenReceipt, OpenReport, OpenStatus, Resolved,
};
use loopflow_model::durable::{MachineId, RepositoryId, TaskId};
use loopflow_storage::sqlite::SqliteStore;
use loopflow_storage::work::selection::{
    Selection, SelectionExpression, SelectionFilter, SelectionId, SelectionState, ViewContent,
    ViewPlan, WorkWorkspaceKind,
};

/// Where a Task's tab opens: its recorded checkout, else the repository.
fn tool_cwd<'a>(content: &'a ViewContent, repo: &'a Path) -> &'a Path {
    content
        .location
        .as_ref()
        .map(|location| Path::new(&location.cwd))
        .filter(|cwd| cwd.is_dir())
        .unwrap_or(repo)
}

fn label(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(80).collect()
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
    let mut plan = selections::plan_selection(
        store,
        repository,
        inventory,
        crate::lf::navigation::command_merges,
    )?;
    plan.selection = selected;
    Ok(plan)
}

// ---------------------------------------------------------------------------
// Native hosts
// ---------------------------------------------------------------------------

/// One host and its caller context, resolved for this command only.
enum Native {
    Rex(Rex),
    Herdr(herdr::Herdr),
    Cmux(cmux::Cmux),
}

/// The outer container for this launch: a Rex session, Herdr workspace or cmux
/// group. `None` in cmux means the caller's ungrouped window.
#[derive(Debug, Clone)]
struct Container {
    id: Option<String>,
    /// Herdr creates a workspace with one empty tab; the first Task fills it.
    spare_tab: Option<String>,
}

impl Container {
    fn existing(id: impl Into<String>) -> Self {
        Self {
            id: Some(id.into()),
            spare_tab: None,
        }
    }
}

impl Native {
    fn resolve(host: OpenHost, server: Option<&str>) -> Result<Self> {
        Ok(match host {
            OpenHost::Rex => Self::Rex(Rex::local(server)?),
            OpenHost::Herdr => Self::Herdr(herdr::Herdr::local()?),
            OpenHost::Cmux => Self::Cmux(cmux::Cmux::local()?),
            other => bail!("{} has no native workspace renderer", other.label()),
        })
    }

    fn open_host(&self) -> OpenHost {
        match self {
            Self::Rex(_) => OpenHost::Rex,
            Self::Herdr(_) => OpenHost::Herdr,
            Self::Cmux(_) => OpenHost::Cmux,
        }
    }

    fn block(&self, label: &str, cwd: &Path, argv: Vec<String>) -> Value {
        match self {
            Self::Rex(rex) => rex.block(label, cwd, argv),
            Self::Herdr(_) => herdr::terminal(label, cwd, argv),
            Self::Cmux(_) => cmux::terminal(label, cwd, argv),
        }
    }

    /// The container the caller is in, when there is one.
    fn here(&self) -> Result<Option<Container>> {
        Ok(match self {
            Self::Rex(rex) => rex
                .caller
                .as_ref()
                .map(|caller| Container::existing(&caller.session)),
            Self::Herdr(herdr) => herdr.here()?.map(Container::existing),
            // The caller's group, else its window (cmux's default placement).
            Self::Cmux(cmux) => Some(Container {
                id: cmux.caller_group()?,
                spare_tab: None,
            }),
        })
    }

    fn create_container(&self, name: &str, cwd: &Path) -> Result<Container> {
        let name = label(name);
        match self {
            Self::Rex(rex) => Ok(Container::existing(rex.create_session(&name)?)),
            Self::Herdr(herdr) => {
                let (workspace, tab) = herdr.create_workspace(&name, cwd)?;
                Ok(Container {
                    id: Some(workspace),
                    spare_tab: Some(tab),
                })
            }
            Self::Cmux(cmux) => Ok(Container::existing(cmux.create_group(&name, cwd)?)),
        }
    }

    /// Add one Task tab (or Wave conversation tab) with its splits.
    fn add_tab(
        &self,
        container: &mut Container,
        name: &str,
        cwd: &Path,
        layout: Value,
    ) -> Result<String> {
        let name = label(name);
        match self {
            Self::Rex(rex) => {
                let session = container
                    .id
                    .as_deref()
                    .context("Rex session is unavailable")?;
                rex.create_window(session, &name, layout)
            }
            Self::Herdr(herdr) => {
                let tab = match container.spare_tab.take() {
                    Some(tab) => tab,
                    None => herdr.create_tab(
                        container
                            .id
                            .as_deref()
                            .context("Herdr workspace is unavailable")?,
                        &name,
                        cwd,
                    )?,
                };
                herdr.apply(&tab, &name, layout)
            }
            Self::Cmux(cmux) => cmux.create_workspace(container.id.as_deref(), &name, cwd, &layout),
        }
    }

    /// Add one Task's tab: its Session, attached by `lf open --task <id> --here`'s
    /// rules, so an ended Session starts its provider as the tab opens.
    fn add_task_tab(
        &self,
        repo: &Path,
        container: &mut Container,
        content: &ViewContent,
    ) -> Result<String> {
        let cwd = tool_cwd(content, repo);
        let argv = lf_argv(cwd, &["--task", content.work.id.as_str(), "open", "--here"])?;
        let layout = self.block("Conversation", cwd, argv);
        self.add_tab(container, &content.title, cwd, layout)
    }

    fn focus(&self, container: &Container, tab: &str) -> String {
        let result = match self {
            Self::Rex(rex) => rex.focus(
                container.id.as_deref().expect("Rex tabs have a session"),
                tab,
            ),
            Self::Herdr(herdr) => herdr.focus(tab).map(|()| "Selected in Herdr".into()),
            Self::Cmux(cmux) => cmux.focus(tab).map(|()| "Selected in cmux".into()),
        };
        result.unwrap_or_else(|error| format!("Created; display selection failed: {error:#}"))
    }
}

// ---------------------------------------------------------------------------
// Opening
// ---------------------------------------------------------------------------

pub(super) fn run(
    cli: &crate::lf::Cli,
    args: &OpenArgs,
    target: &Resolved,
    host: OpenHost,
) -> Result<()> {
    loopflow_execution::store::read_existing_registry()?.context("Work registry is unavailable")?;
    let store = loopflow_execution::store::open_sqlite(
        &loopflow_execution::store::database_path_from_env()?,
    )?;
    let report = open_machines(&store, host, target, args).unwrap_or_else(|error| OpenReport {
        receipts: vec![failed(target, host, format!("{error:#}"))],
        unknown: Vec::new(),
    });
    if args.selection.json || cli.json {
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

fn failed(target: &Resolved, host: OpenHost, reason: String) -> OpenReceipt {
    target.receipt(Some(host), None, OpenStatus::Failed, Some(reason))
}

fn open_machines(
    store: &SqliteStore,
    host: OpenHost,
    target: &Resolved,
    args: &OpenArgs,
) -> Result<OpenReport> {
    let repository = target
        .repository_id
        .as_deref()
        .context("Repository has no Work identity")?;
    let repository = RepositoryId::parse(repository)?;
    let selected = scope(store, target)?;
    let local = loopflow_execution::store::machines::local_machine(store)?.id;
    let plan = workspace_plan(store, &repository, selected.clone())?;
    let routed = route(store, plan.contents.iter(), &local)?;
    let mut receipts = Vec::new();
    let mut unknown = plan.resolution.unavailable.clone();
    unknown.extend(routed.unknown);
    if !args.local_workspace {
        for peer in routed.peers {
            match remote_open(store, host, &peer, repository.as_str(), &selected, args) {
                Ok(mut report) => {
                    receipts.append(&mut report.receipts);
                    unknown.append(&mut report.unknown);
                }
                Err(error) => receipts.push(failed(
                    target,
                    host,
                    format!("Machine {peer}: {error:#}; no local fallback"),
                )),
            }
        }
    }
    if !routed.local.is_empty() {
        let result = Native::resolve(host, args.selection.server.as_deref())
            .and_then(|native| launch(store, &native, target, &selected, args, &routed.local));
        match result {
            Ok(mut opened) => receipts.append(&mut opened),
            Err(error) => receipts.push(failed(target, host, format!("{error:#}"))),
        }
    } else if receipts.is_empty() {
        receipts.push(target.receipt(
            Some(host),
            None,
            OpenStatus::Opening,
            Some("No started, unfinished Tasks to open here; nothing was created".into()),
        ));
    }
    Ok(OpenReport { receipts, unknown })
}

/// A selection's started, unfinished Tasks by the Machine that runs them. Another
/// Machine decides for itself which of its Tasks started; an unknown route never
/// becomes a local tab.
#[derive(Default)]
struct Routed<'a> {
    local: Vec<&'a ViewContent>,
    peers: BTreeSet<String>,
    unknown: Vec<String>,
}

fn route<'a>(
    store: &SqliteStore,
    contents: impl Iterator<Item = &'a ViewContent>,
    local: &MachineId,
) -> Result<Routed<'a>> {
    let mut routed = Routed::default();
    for content in contents.filter(|content| content.work.kind == WorkWorkspaceKind::Task) {
        if content.states.contains(&SelectionState::Done) {
            continue;
        }
        let task = TaskId::parse(&content.work.id)?;
        match loopflow_execution::store::ownership::task_execution_route(store, &task) {
            Ok(route) if route.machine_id == *local => {
                if started(content) {
                    routed.local.push(content);
                }
            }
            Ok(route) => {
                routed.peers.insert(route.machine_id.to_string());
            }
            Err(error) => routed.unknown.push(format!(
                "{}: execution Machine unavailable: {error}",
                content.title
            )),
        }
    }
    Ok(routed)
}

/// Ask `machine` for its part of the overview.
fn remote_open(
    store: &SqliteStore,
    host: OpenHost,
    machine: &str,
    repository: &str,
    selection: &Selection,
    args: &OpenArgs,
) -> Result<OpenReport> {
    let machine = loopflow_execution::store::machines::machines(store)?
        .into_iter()
        .find(|entry| entry.id.as_str() == machine)
        .context("Task execution Machine has no configured route")?;
    let mut argv = vec![
        "--repo-id".into(),
        repository.into(),
        "open".into(),
        "--workspace".into(),
        "--host".into(),
        host.label().to_lowercase(),
        "--local-workspace".into(),
        "--json".into(),
    ];
    match &selection.expression {
        SelectionExpression::Wave { wave } => argv.extend(["--wave".into(), wave.to_string()]),
        SelectionExpression::Repository => {}
        _ => bail!("This workspace selection cannot be routed"),
    }
    if let Some(agent) = loopflow_execution::config::agent_override() {
        argv.extend(["--agent".into(), agent]);
    }
    if args.no_focus {
        argv.push("--no-focus".into());
    }
    let bytes = tokio::runtime::Runtime::new()?.block_on(
        loopflow_execution::machine::ssh::request(&machine, &argv, None),
    )?;
    serde_json::from_slice(&bytes)
        .context("Remote workspace receipt is unavailable; creation may have succeeded")
}

fn scope(store: &SqliteStore, target: &Resolved) -> Result<Selection> {
    let expression = if let Some(wave) = &target.wave {
        SelectionExpression::Wave {
            wave: store
                .resolve_wave_id(wave, Some(&target.repository_path.to_string_lossy()))?
                .context("Wave is unavailable")?,
        }
    } else {
        SelectionExpression::Repository
    };
    Ok(Selection {
        id: SelectionId("workspace".into()),
        expression,
    })
}

/// The container's name: the Wave, else the repository.
fn container_name(store: &SqliteStore, target: &Resolved, selected: &Selection) -> Result<String> {
    let wave = match &selected.expression {
        SelectionExpression::Wave { wave } => Some(wave.clone()),
        _ => None,
    };
    Ok(
        match wave
            .map(|wave| store.get_wave(&wave))
            .transpose()?
            .flatten()
        {
            Some(wave) => wave.slug().to_owned(),
            None => target.repository_path.file_name().map_or_else(
                || target.label.clone(),
                |name| name.to_string_lossy().into_owned(),
            ),
        },
    )
}

/// One conversation tab, independent of any saved Task layout.
pub(super) fn conversation_tab(
    target: &Resolved,
    host: OpenHost,
    server: Option<&str>,
    argv: Vec<String>,
    focus: bool,
) -> Result<OpenReceipt> {
    let native = Native::resolve(host, server)?;
    let cwd = target.cwd();
    let mut container = match native.here()? {
        Some(here) => here,
        None => native.create_container(&target.label, cwd)?,
    };
    let layout = native.block("Conversation", cwd, argv);
    let tab = native.add_tab(&mut container, &target.destination_name(), cwd, layout)?;
    let reason = focus.then(|| native.focus(&container, &tab));
    let mut receipt = target.receipt(Some(host), Some(tab), OpenStatus::Opening, reason);
    receipt.container = container.id;
    Ok(receipt)
}
fn launch(
    store: &SqliteStore,
    native: &Native,
    target: &Resolved,
    selected: &Selection,
    args: &OpenArgs,
    local_tasks: &[&ViewContent],
) -> Result<Vec<OpenReceipt>> {
    let host = native.open_host();
    let repo = target.repository_path.as_path();
    let mut container = native.create_container(&container_name(store, target, selected)?, repo)?;
    let mut receipts = Vec::new();
    let mut focused = args.no_focus;
    for content in local_tasks {
        let task = store
            .task(&TaskId::parse(&content.work.id)?)?
            .context("Task is unavailable")?;
        let mut receipt = target.receipt(Some(host), None, OpenStatus::Opening, None);
        receipt.label = format!("{} · {}", task.identifier, task.title);
        receipt.url = loopflow_execution::work::link::WorkUrl::new(
            target.url.repository.clone(),
            loopflow_execution::work::link::WorkTarget::Task(task.identifier.clone()),
        )
        .to_string();
        let opened = native
            .add_task_tab(repo, &mut container, content)
            .inspect(|tab| {
                let showing = if focused {
                    "Focus preserved".into()
                } else {
                    focused = true;
                    native.focus(&container, tab)
                };
                receipt.reason = Some(format!("Created a new Task tab; {showing}"));
            });
        settle_receipt(&mut receipt, opened);
        receipts.push(receipt);
    }
    for receipt in &mut receipts {
        receipt.container = container.id.clone();
    }
    Ok(receipts)
}

/// Record a tab's native result. A lost reply is reported, never retried.
fn settle_receipt(receipt: &mut OpenReceipt, opened: Result<String>) {
    match opened {
        Ok(tab) => {
            receipt.destination = Some(tab);
            if receipt.reason.is_none() {
                receipt.reason = Some("Created".into());
            }
        }
        Err(error) => {
            receipt.status = OpenStatus::Failed;
            receipt.reason = Some(format!(
                "{error:#}; nothing was retried, so inspect the host before opening again"
            ));
        }
    }
}

fn started(content: &ViewContent) -> bool {
    content.states.iter().any(|state| {
        matches!(
            state,
            SelectionState::Idle
                | SelectionState::Active
                | SelectionState::Moving
                | SelectionState::Paused
        )
    })
}

fn lf_argv(cwd: &Path, words: &[&str]) -> Result<Vec<String>> {
    // Tools belong to this opening. Keep an explicit private-Machine runtime, but
    // never hand a dev build's real-Machine panes to the installed release.
    let binary = if loopflow_execution::store::custom_home_selected() {
        loopflow_execution::os_process::resolve_lf_binary()?
    } else {
        std::env::current_exe()?
    };
    let mut argv = vec![
        binary.to_string_lossy().into_owned(),
        "--cwd".into(),
        cwd.to_string_lossy().into_owned(),
    ];
    argv.extend(words.iter().map(|word| (*word).into()));
    Ok(with_machine_home(argv))
}

fn with_machine_home(mut argv: Vec<String>) -> Vec<String> {
    if loopflow_execution::store::custom_home_selected() {
        argv.splice(
            0..0,
            [
                "env".into(),
                format!(
                    "LF_HOME={}",
                    loopflow_execution::store::lf_home_dir().display()
                ),
            ],
        );
    }
    argv
}

#[cfg(test)]
mod tests {
    use super::started;
    use loopflow_storage::work::selection::{
        SelectionState, ViewContent, WorkWorkspaceKey, WorkWorkspaceKind,
    };

    #[test]
    fn real_machine_tools_keep_the_build_that_opened_them() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        std::env::set_var("LF_BIN", "/old/installed/lf");
        let cwd = std::env::current_dir().unwrap();
        let argv = super::lf_argv(&cwd, &["resume", "existing"]).unwrap();
        assert_eq!(argv[0], std::env::current_exe().unwrap().to_string_lossy());
        assert_eq!(&argv[3..], &["resume", "existing"]);
    }

    #[test]
    fn only_started_unfinished_tasks_open_as_tabs() {
        let mut content = ViewContent {
            working_group: None,
            work: WorkWorkspaceKey {
                repository: "r".into(),
                kind: WorkWorkspaceKind::Task,
                id: "t".into(),
            },
            title: "Task".into(),
            states: Default::default(),
            identifier: None,
            graph_step: None,
            primary_session: None,
            location: None,
            argv: None,
            unavailable: vec![],
        };
        for (state, expected) in [
            (SelectionState::Unstarted, false),
            (SelectionState::Idle, true),
            (SelectionState::Paused, true),
            (SelectionState::Moving, true),
            (SelectionState::Done, false),
        ] {
            content.states = std::collections::BTreeSet::from([state]);
            assert_eq!(started(&content), expected);
        }
    }
}
