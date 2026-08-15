use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::durable::{
    AgentInvocation, AgentInvocationId, Ask, AskId, AskResult, AskState, AskTarget, BoundaryState,
    ContainmentObservation, ControlCtx, InvocationRoute, InvocationSurface, RunLease, WorkRef,
};
use crate::engine::wave_home::HomeRoute;
use crate::store::SharedStore;

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const RETRY_DELAY: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AttentionState {
    Queued,
    Claimed,
    NotPresented,
    Active,
    Stale,
}

impl std::fmt::Display for AttentionState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => formatter.write_str("queued"),
            Self::Claimed => formatter.write_str("claimed"),
            Self::NotPresented => formatter.write_str("not-presented"),
            Self::Active => formatter.write_str("active"),
            Self::Stale => formatter.write_str("stale"),
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct AskAttention {
    pub(crate) ask: Ask,
    pub(crate) surface: Option<InvocationSurface>,
    pub(crate) attention: AttentionState,
}

pub(crate) async fn request_intervention(
    store: &SharedStore,
    lease: &RunLease,
    invocation_id: &AgentInvocationId,
    prompt: &str,
    user: bool,
) -> Result<Ask> {
    let ask = store
        .request_intervention(lease, invocation_id, prompt, user)
        .await?;
    wake(store, &ask.target).await;
    Ok(ask)
}

pub(crate) async fn pending_attention(
    store: &SharedStore,
    context: &ControlCtx<'_>,
    target: &AskTarget,
) -> Result<Vec<AskAttention>> {
    let asks = store.pending_asks(context, target).await?;
    let mut projection = Vec::with_capacity(asks.len());
    for ask in asks {
        projection.push(project_attention(store, ask).await?);
    }
    Ok(projection)
}

pub(crate) async fn project_attention(store: &SharedStore, mut ask: Ask) -> Result<AskAttention> {
    let mut surface = active_surface(store, &ask).await?;
    let mut presentation = match ask.active_invocation_id.as_ref() {
        Some(invocation_id) => store.ask_presentation(invocation_id).await?,
        None => (false, false),
    };
    let mut stale = false;
    if let Some(invocation_surface) = surface.as_ref() {
        if presentation.0 && invocation_surface.invocation.ended_at.is_none() {
            match observe_surface(invocation_surface).await {
                ContainmentObservation::Absent => {
                    ask = store
                        .reconcile_ask(
                            &invocation_surface.invocation.id,
                            ContainmentObservation::Absent,
                        )
                        .await?;
                    surface = active_surface(store, &ask).await?;
                    presentation = match ask.active_invocation_id.as_ref() {
                        Some(invocation_id) => store.ask_presentation(invocation_id).await?,
                        None => (false, false),
                    };
                }
                ContainmentObservation::Unprovable => stale = true,
                ContainmentObservation::Present => {}
            }
        }
    }
    let attention = if stale {
        AttentionState::Stale
    } else {
        attention_state(
            &ask,
            surface.as_ref().map(|surface| &surface.invocation),
            presentation,
        )
    };
    Ok(AskAttention {
        ask,
        surface,
        attention,
    })
}

pub(crate) async fn prepare_open(
    store: &SharedStore,
    context: &ControlCtx<'_>,
    ask_id: &AskId,
) -> Result<InvocationSurface> {
    let ask = project_attention(store, store.ask_by_id(ask_id).await?)
        .await?
        .ask;
    let (route, surface_kind) = launch_identity(&ask, false);
    let claim = store
        .claim_ask(context, ask_id, route, surface_kind)
        .await?;
    let ask = store.ask_by_id(ask_id).await?;
    let surface = if claim.needs_launch {
        launch_claimed(store, &ask, &claim.invocation_id, false).await?
    } else {
        if !store.ask_presentation(&claim.invocation_id).await?.0 {
            return Err(anyhow!(
                "Ask {ask_id} Invocation {} is starting and has no attach route yet",
                claim.invocation_id
            ));
        }
        active_surface(store, &ask).await?.ok_or_else(|| {
            anyhow!(
                "Ask {ask_id} Invocation {} is starting and has no attach route yet",
                claim.invocation_id
            )
        })?
    };
    Ok(surface)
}

async fn active_surface(store: &SharedStore, ask: &Ask) -> Result<Option<InvocationSurface>> {
    match ask.active_invocation_id.as_ref() {
        Some(invocation_id) => Ok(store.invocation_surface(invocation_id).await?),
        None => Ok(None),
    }
}

fn attention_state(
    ask: &Ask,
    invocation: Option<&AgentInvocation>,
    presentation: (bool, bool),
) -> AttentionState {
    match (ask.state, invocation, presentation) {
        (AskState::Queued, _, _) => AttentionState::Queued,
        (AskState::Claimed, Some(invocation), (_, true)) if invocation.ended_at.is_none() => {
            AttentionState::Active
        }
        (AskState::Claimed, Some(invocation), (true, _)) if invocation.ended_at.is_none() => {
            AttentionState::NotPresented
        }
        (AskState::Claimed, _, _) => AttentionState::Claimed,
        _ => AttentionState::Queued,
    }
}

fn launch_identity(ask: &Ask, headless: bool) -> (InvocationRoute, &'static str) {
    let config = crate::engine::load_config_or_default(Some(&ask.origin.cwd));
    let agent = config.agent().to_string();
    let (provider, model) = crate::engine::parse_agent(&agent);
    let surface = if headless { "ask_headless" } else { "ask_tui" };
    let route = InvocationRoute {
        provider,
        model,
        account_id: None,
    };
    (route, surface)
}

pub(crate) async fn launch_claimed(
    store: &SharedStore,
    ask: &Ask,
    invocation_id: &AgentInvocationId,
    headless: bool,
) -> Result<InvocationSurface> {
    let flow_run_lease = store
        .claim_flow_step_run_lease(&ask.id, invocation_id)
        .await?;
    let session_name = session_name(invocation_id);
    let lf = crate::engine::process::resolve_lf_binary();
    let mut argv = vec![
        lf.to_string_lossy().to_string(),
        "ask".to_string(),
        "serve".to_string(),
        ask.id.to_string(),
    ];
    if headless {
        argv.push("--headless".to_string());
    }
    let mut environment = vec![
        (crate::durable::AGENT_INVOCATION_ENV, invocation_id.as_str()),
        (crate::durable::RUN_CONTEXT_ENV, "ask"),
    ];
    if let Some(run_lease) = flow_run_lease.as_ref() {
        environment.push((crate::durable::RUN_LEASE_ENV, run_lease.env_value()));
    }
    if let Err(error) = crate::engine::process::start_lf_session_with_env(
        &session_name,
        &ask.origin.cwd,
        &argv,
        &environment,
    )
    .await
    {
        let _ = store
            .close_ask_invocation(
                &ask.id,
                invocation_id,
                Some("Ask session failed to start"),
                BoundaryState::Failed,
            )
            .await;
        return Err(error.context("start Ask session"));
    }
    store.mark_ask_ready(&ask.id, invocation_id).await?;
    if headless {
        store.mark_ask_presented(&ask.id, invocation_id).await?;
    }
    store
        .invocation_surface(invocation_id)
        .await?
        .ok_or_else(|| anyhow!("Ask Invocation {invocation_id} has no surface"))
}

pub(crate) async fn settle(
    store: &SharedStore,
    ask_id: &AskId,
    invocation_id: &AgentInvocationId,
    result: AskResult,
) -> Result<Ask> {
    let ask = store.settle_ask(ask_id, invocation_id, result).await?;
    resume_flow_step(store, &ask).await?;
    Ok(ask)
}

pub(crate) async fn cancel(
    store: &SharedStore,
    context: &ControlCtx<'_>,
    ask_id: &AskId,
    reason: &str,
) -> Result<Ask> {
    let ask = store.cancel_ask(context, ask_id, reason).await?;
    resume_flow_step(store, &ask).await?;
    Ok(ask)
}

async fn resume_flow_step(store: &SharedStore, ask: &Ask) -> Result<()> {
    if !matches!(ask.request, crate::durable::AskBody::FlowStep { .. }) {
        return Ok(());
    }
    let WorkRef::Task(task_id) = &ask.origin.work else {
        return Err(anyhow!(
            "flow-step Ask {} does not belong to a Task",
            ask.id
        ));
    };
    let mut task = store
        .get_task(task_id)
        .await?
        .ok_or_else(|| anyhow!("Task {task_id} disappeared"))?;
    crate::ops::task::relaunch_inactive_process(store, &mut task)
        .await
        .map_err(|error| anyhow!(error.to_string()))
}

pub(crate) async fn serve(
    store: SharedStore,
    ask_id: AskId,
    invocation_id: AgentInvocationId,
    headless: bool,
) -> Result<()> {
    let cleanup_store = Arc::clone(&store);
    let cleanup_ask_id = ask_id.clone();
    let cleanup_invocation_id = invocation_id.clone();
    crate::engine::agent::register_interrupt_cleanup(move || {
        let _ = cleanup_store.interrupt_ask_on_interrupt(&cleanup_ask_id, &cleanup_invocation_id);
    });
    let ask = wait_until_presented(&store, &ask_id, &invocation_id).await?;
    if ask.state.is_terminal() {
        return Ok(());
    }
    let prompt = ask_prompt(&store, &ask).await?;
    let config = crate::engine::load_config_or_default(Some(&ask.origin.cwd));
    let agent = config.agent().to_string();
    let result = run_provider(
        &ask.origin.cwd,
        &agent,
        &prompt,
        &invocation_id,
        headless,
        matches!(ask.request, crate::durable::AskBody::FlowStep { .. }),
    )
    .await;
    let current = store.ask_by_id(&ask.id).await?;
    if current.state == AskState::Claimed
        && current.active_invocation_id.as_ref() == Some(&invocation_id)
    {
        if interrupted_result(&result) {
            let _ = store
                .close_ask_invocation(
                    &ask_id,
                    &invocation_id,
                    Some("Ask provider exited on a signal"),
                    BoundaryState::Interrupted,
                )
                .await?;
            eprintln!("Ask Invocation interrupted; {} requeued", current.id);
        } else {
            let outcome = if result.as_ref().is_ok_and(|status| status.success()) {
                BoundaryState::Unknown
            } else {
                BoundaryState::Failed
            };
            let _ = store
                .close_ask_invocation(
                    &ask_id,
                    &invocation_id,
                    Some("Ask provider exited without settlement"),
                    outcome,
                )
                .await?;
            eprintln!(
                "Invocation closed without resolution; {} requeued",
                current.id
            );
        }
    }
    if matches!(current.state, AskState::Resolved | AskState::Declined) {
        Ok(())
    } else {
        match result {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(anyhow!("Ask provider exited with {status}")),
            Err(error) => Err(error),
        }
    }
}

async fn wait_until_presented(
    store: &SharedStore,
    ask_id: &AskId,
    invocation_id: &AgentInvocationId,
) -> Result<Ask> {
    loop {
        let ask = store.ask_by_id(ask_id).await?;
        if ask.state.is_terminal() {
            return Ok(ask);
        }
        let invocation = store
            .invocation_surface(invocation_id)
            .await?
            .ok_or_else(|| anyhow!("Ask Invocation {invocation_id} disappeared"))?
            .invocation;
        if store.ask_presentation(&invocation.id).await?.1 {
            return Ok(ask);
        }
        if invocation.ended_at.is_none() {
            tokio::time::sleep(POLL_INTERVAL).await;
        } else {
            return Ok(ask);
        }
    }
}

async fn run_provider(
    cwd: &Path,
    agent: &str,
    prompt: &str,
    invocation_id: &AgentInvocationId,
    headless: bool,
    flow_step: bool,
) -> Result<ExitStatus> {
    let mut launch = crate::engine::AgentConfig {
        system_prompt: if headless {
            format!(
                "{}\n\n{}",
                crate::engine::builtins::SURFACE_HEADLESS,
                crate::engine::builtins::LOOPFLOW_DOC
            )
        } else {
            String::new()
        },
        task_prompt: prompt.to_string(),
        agent: Some(agent.to_string()),
        cwd: Some(cwd.to_path_buf()),
        authority: if flow_step {
            crate::engine::agent::AgentAuthority::Inherit
        } else {
            crate::engine::agent::AgentAuthority::Detached
        },
        ..Default::default()
    };
    launch.env.insert(
        crate::durable::AGENT_INVOCATION_ENV.to_string(),
        invocation_id.as_str().to_string(),
    );
    let process = crate::engine::ProcessConfig {
        auto: headless,
        stream: false,
        ..Default::default()
    };
    let capabilities = crate::engine::AgentCapabilities::default();
    let result = tokio::task::spawn_blocking(move || {
        crate::engine::launch_agent(&launch, &process, &capabilities)
    })
    .await??;
    Ok(ExitStatus::from_raw(result.exit_code << 8))
}

async fn ask_prompt(store: &SharedStore, ask: &Ask) -> Result<String> {
    if matches!(ask.request, crate::durable::AskBody::FlowStep { .. }) {
        return crate::task::runner::flow_step_ask_prompt(store, ask).await;
    }
    let invocations = store.ask_invocations(&ask.id).await?;
    let mut history = Vec::with_capacity(invocations.len());
    for invocation in invocations {
        history.push(store.invocation_surface(&invocation.id).await?);
    }
    Ok(format!(
        "Resolve durable Ask {id}.\n\nRequest:\n{request}\n\nOrigin: {kind} {work}\nOrigin cwd: {cwd}\nTarget: {target}\nPrevious Ask Invocations:\n{invocations}\n\nYou are responsible only for this intervention; do not adopt the originating Work. Inspect or mutate the origin cwd as needed. Settlement is explicit and mandatory:\n- `lf ask resolve {id} \"<concise verified summary>\"` on success\n- `lf ask decline {id} \"<reason>\"` when the request should not be fulfilled\n- `lf ask release {id} \"<reason>\"` when unfinished\n- for a parent-targeted Ask that genuinely needs the absent User, `lf ask escalate {id} --user`\nA final response, clean exit, Ctrl-D, window close, or process exit never settles the Ask.",
        id = ask.id,
        request = ask.request,
        kind = ask.origin.work.kind(),
        work = ask.origin.work.id(),
        cwd = ask.origin.cwd.display(),
        target = ask.target,
        invocations = serde_json::to_string_pretty(&history)?,
    ))
}

fn interrupted_result(result: &Result<ExitStatus>) -> bool {
    result.as_ref().ok().is_some_and(|status| {
        status.signal().is_some() || matches!(status.code(), Some(129 | 130 | 143))
    })
}

pub(crate) fn session_name(invocation_id: &AgentInvocationId) -> String {
    format!("lf-ask-{}", &invocation_id.as_str()[11..23])
}

pub(crate) async fn observe_surface(surface: &InvocationSurface) -> ContainmentObservation {
    let Some(attach) = surface.attach_argv.as_ref() else {
        return ContainmentObservation::Unprovable;
    };
    let Some(session) = tmux_target(attach) else {
        return ContainmentObservation::Unprovable;
    };
    let Some(home) = HomeRoute::parse(&surface.home_route) else {
        return ContainmentObservation::Unprovable;
    };
    if let Some(destination) = home.ssh_destination() {
        let mut command = tokio::process::Command::new("ssh");
        command.args(crate::engine::wave_home::bounded_ssh_args(
            &destination,
            home.ssh_port(),
        ));
        let output = command
            .args(["--", "tmux", "has-session", "-t", session])
            .stdin(Stdio::null())
            .output()
            .await;
        match output {
            Ok(output) if output.status.success() => ContainmentObservation::Present,
            Ok(output) if output.status.code() == Some(1) => ContainmentObservation::Absent,
            _ => ContainmentObservation::Unprovable,
        }
    } else {
        match crate::engine::process::tmux_session_exists(session).await {
            Ok(true) => ContainmentObservation::Present,
            Ok(false) => ContainmentObservation::Absent,
            Err(_) => ContainmentObservation::Unprovable,
        }
    }
}

pub(crate) async fn wake(store: &SharedStore, target: &AskTarget) {
    if let Err(error) = wake_parent(store, target).await {
        tracing::warn!(%error, "Ask parent wake failed; durable attention remains queued");
    }
}

async fn wake_parent(store: &SharedStore, target: &AskTarget) -> Result<()> {
    let AskTarget::Parent(parent) = target else {
        return Ok(());
    };
    match parent {
        WorkRef::Project(project_id) => crate::ops::project::wake_project(project_id)
            .await
            .map_err(|error| anyhow!(error.to_string())),
        WorkRef::Wave(wave_id) => {
            let wave = store
                .get_wave(wave_id)
                .await?
                .ok_or_else(|| anyhow!("parent Wave {wave_id} is not registered"))?;
            let placement = store.placement(parent).await?;
            crate::lfd::ensure(&placement.home_id, Path::new(wave.repo())).await?;
            let outcomes =
                crate::lfd::start_waves(&placement.home_id, vec![wave_id.clone()]).await?;
            match outcomes.as_slice() {
                [crate::wave_host::WaveStartOutcome {
                    state: crate::wave_host::WaveStartState::Live { .. },
                    ..
                }] => Ok(()),
                [crate::wave_host::WaveStartOutcome {
                    state: crate::wave_host::WaveStartState::Failed { reason },
                    ..
                }] => Err(anyhow!(reason.clone())),
                _ => Err(anyhow!("lfd returned no outcome for parent Wave {wave_id}")),
            }
        }
        WorkRef::Task(task_id) => Err(anyhow!(
            "Task {task_id} cannot own child Work and is not an Ask parent"
        )),
    }
}

pub(crate) fn tmux_target(argv: &[String]) -> Option<&str> {
    (argv.first().map(String::as_str) == Some("tmux"))
        .then_some(())
        .and_then(|_| argv.windows(2).find(|pair| pair[0] == "-t"))
        .map(|pair| pair[1].as_str())
}

pub(crate) struct AskLane {
    parent: WorkRef,
    lease: RunLease,
    retry_at: Option<tokio::time::Instant>,
}

impl AskLane {
    pub(crate) fn new(parent: WorkRef, lease: RunLease) -> Self {
        Self {
            parent,
            lease,
            retry_at: None,
        }
    }

    /// Reconcile the next parent-directed Ask and report whether any remain unresolved.
    pub(crate) async fn reconcile(&mut self, store: &SharedStore) -> Result<bool> {
        let retrying = self
            .retry_at
            .is_some_and(|retry_at| retry_at > tokio::time::Instant::now());
        if !retrying {
            self.retry_at = None;
        }
        let asks = pending_attention(
            store,
            &ControlCtx::Run(&self.lease),
            &AskTarget::Parent(self.parent.clone()),
        )
        .await?
        .into_iter()
        .map(|attention| attention.ask)
        .collect::<Vec<_>>();
        if let Some(ask) = asks.iter().find(|ask| ask.state == AskState::Claimed) {
            self.retry_at = None;
            ask.active_invocation_id
                .as_ref()
                .ok_or_else(|| anyhow!("claimed Ask {} has no active Invocation", ask.id))?;
            return Ok(true);
        }
        let Some(ask) = asks.into_iter().find(|ask| ask.state == AskState::Queued) else {
            self.retry_at = None;
            return Ok(false);
        };
        if retrying {
            return Ok(true);
        }
        let (route, surface) = launch_identity(&ask, true);
        let claim = store
            .claim_ask(&ControlCtx::Run(&self.lease), &ask.id, route, surface)
            .await?;
        if !claim.needs_launch {
            return Ok(true);
        }
        if let Err(error) = launch_claimed(store, &ask, &claim.invocation_id, true).await {
            self.retry_at = Some(tokio::time::Instant::now() + RETRY_DELAY);
            tracing::warn!(ask_id = %ask.id, %error, "failed to launch parent Ask session");
            return Ok(true);
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    use super::{attention_state, interrupted_result, session_name, tmux_target, AttentionState};
    use crate::durable::{
        AgentInvocation, AgentInvocationId, Ask, AskBody, AskId, AskOrigin, AskState, AskTarget,
        HomeId, InvocationRoute, RunId, WorkRef,
    };
    use crate::id::WaveId;

    fn invocation() -> AgentInvocation {
        AgentInvocation {
            id: AgentInvocationId::new(),
            supervising_run_id: Some(RunId::new()),
            answer_ask_id: Some(AskId::new()),
            route: InvocationRoute {
                provider: "codex".to_string(),
                model: None,
                account_id: None,
            },
            surface: "ask_tui".to_string(),
            resume_token: None,
            started_at: time::OffsetDateTime::now_utc(),
            ended_at: None,
        }
    }

    fn ask(state: AskState) -> Ask {
        Ask {
            id: AskId::new(),
            origin: AskOrigin {
                work: WorkRef::Wave(WaveId::new()),
                run_id: RunId::new(),
                turn_id: None,
                invocation_id: None,
                home_id: HomeId::new(),
                cwd: "/tmp".into(),
            },
            target: AskTarget::User,
            request: AskBody::Intervention {
                prompt: "help".to_string(),
            },
            state,
            active_invocation_id: None,
            result: None,
            terminal_author: None,
            asked_at: time::OffsetDateTime::now_utc(),
            terminal_at: None,
        }
    }

    #[test]
    fn attention_distinguishes_queued_ready_and_active() {
        assert_eq!(
            attention_state(&ask(AskState::Queued), None, (false, false)),
            AttentionState::Queued
        );
        let invocation = invocation();
        assert_eq!(
            attention_state(&ask(AskState::Claimed), Some(&invocation), (true, false)),
            AttentionState::NotPresented
        );
        assert_eq!(
            attention_state(&ask(AskState::Claimed), Some(&invocation), (true, true)),
            AttentionState::Active
        );
        assert_eq!(
            attention_state(&ask(AskState::Claimed), Some(&invocation), (false, false)),
            AttentionState::Claimed
        );
    }

    #[test]
    fn ask_session_and_tmux_attach_are_exact() {
        let session = AgentInvocationId::new();
        let name = session_name(&session);
        assert!(name.starts_with("lf-ask-"));
        let argv = vec![
            "tmux".to_string(),
            "attach-session".to_string(),
            "-t".to_string(),
            name.clone(),
        ];
        assert_eq!(tmux_target(&argv), Some(name.as_str()));
    }

    #[test]
    fn exiting_interrupts_are_transport_evidence_not_success() {
        for status in [ExitStatus::from_raw(130 << 8), ExitStatus::from_raw(9)] {
            assert!(interrupted_result(&Ok(status)));
        }
        assert!(!interrupted_result(&Ok(ExitStatus::from_raw(0))));
    }
}
