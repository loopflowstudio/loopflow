use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::durable::{
    Ask, AskId, AskOrigin, AskResult, AskSession, AskState, AskTarget, RunId, WorkRef,
};
use crate::engine::wave_home::HomeRoute;
use crate::store::SharedStore;

const POLL_INTERVAL: Duration = Duration::from_millis(250);
const RETRY_DELAY: Duration = Duration::from_secs(5);
const LAUNCH_GRACE_SECONDS: i64 = 5;

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
    pub(crate) surface: Option<AskSession>,
    pub(crate) attention: AttentionState,
}

pub(crate) async fn request_intervention(
    store: &SharedStore,
    origin: AskOrigin,
    prompt: &str,
    user: bool,
) -> Result<Ask> {
    let ask = store.request_intervention(origin, prompt, user).await?;
    wake(store, &ask.target).await;
    Ok(ask)
}

pub(crate) async fn pending_attention(
    store: &SharedStore,
    target: &AskTarget,
) -> Result<Vec<AskAttention>> {
    let asks = store.pending_asks(target).await?;
    let mut projection = Vec::with_capacity(asks.len());
    for ask in asks {
        projection.push(project_attention(store, ask).await?);
    }
    Ok(projection)
}

pub(crate) async fn project_attention(store: &SharedStore, ask: Ask) -> Result<AskAttention> {
    let surface = active_surface(store, &ask).await?;
    let mut stale = false;
    if absence_is_authoritative(&ask, time::OffsetDateTime::now_utc()) {
        if let Some(active) = surface.as_ref() {
            stale = !matches!(observe_surface(active).await, Ok(true));
        }
    }
    let attention = if stale {
        AttentionState::Stale
    } else {
        attention_state(&ask)
    };
    Ok(AskAttention {
        ask,
        surface,
        attention,
    })
}

pub(crate) async fn prepare_open(store: &SharedStore, ask_id: &AskId) -> Result<AskSession> {
    project_attention(store, store.ask_by_id(ask_id).await?).await?;
    let claim = store.claim_ask(ask_id).await?;
    let ask = store.ask_by_id(ask_id).await?;
    if claim.needs_launch {
        launch_claimed(store, &ask, &claim.run_id, false).await
    } else {
        let session = session_for(store, &ask, &claim.run_id).await?;
        match observe_surface(&session).await {
            Ok(false) => Err(anyhow!(
                "Ask {ask_id} Run {} is still starting",
                claim.run_id
            )),
            Ok(true) | Err(_) => Ok(session),
        }
    }
}

async fn active_surface(store: &SharedStore, ask: &Ask) -> Result<Option<AskSession>> {
    match ask.active_run_id.as_ref() {
        Some(run_id) => Ok(Some(session_for(store, ask, run_id).await?)),
        None => Ok(None),
    }
}

async fn session_for(store: &SharedStore, ask: &Ask, run_id: &RunId) -> Result<AskSession> {
    let home = store
        .home_by_id(&ask.origin.home_id)
        .await?
        .ok_or_else(|| anyhow!("Ask {} Home {} disappeared", ask.id, ask.origin.home_id))?;
    Ok(AskSession {
        ask_id: ask.id.clone(),
        run_id: run_id.clone(),
        home_route: home.route,
        attach_argv: vec![
            "tmux".to_string(),
            "attach-session".to_string(),
            "-t".to_string(),
            session_name(run_id),
        ],
    })
}

fn attention_state(ask: &Ask) -> AttentionState {
    match (ask.state, ask.ready_at, ask.presented_at) {
        (AskState::Queued, _, _) => AttentionState::Queued,
        (AskState::Claimed, _, Some(_)) => AttentionState::Active,
        (AskState::Claimed, Some(_), None) => AttentionState::NotPresented,
        (AskState::Claimed, None, None) => AttentionState::Claimed,
        _ => AttentionState::Queued,
    }
}

fn absence_is_authoritative(ask: &Ask, now: time::OffsetDateTime) -> bool {
    ask.presented_at.is_some()
        || ask
            .ready_at
            .is_some_and(|ready_at| now - ready_at >= time::Duration::seconds(LAUNCH_GRACE_SECONDS))
}

pub(crate) async fn launch_claimed(
    store: &SharedStore,
    ask: &Ask,
    run_id: &RunId,
    headless: bool,
) -> Result<AskSession> {
    let session_name = session_name(run_id);
    let lf = crate::engine::process::resolve_current_home_lf_binary();
    let mut argv = vec![
        lf.to_string_lossy().to_string(),
        "ask".to_string(),
        "serve".to_string(),
        ask.id.to_string(),
        run_id.to_string(),
    ];
    if headless {
        argv.push("--headless".to_string());
    }
    if let Err(error) =
        crate::engine::process::start_home_session(&session_name, &ask.origin.cwd, &argv).await
    {
        let _ = store
            .release_ask(&ask.id, run_id, Some("Ask session failed to start"))
            .await;
        return Err(error.context("start Ask session"));
    }
    if headless {
        store.mark_ask_presented(&ask.id, run_id).await?;
    }
    session_for(store, ask, run_id).await
}

pub(crate) async fn settle(
    store: &SharedStore,
    ask_id: &AskId,
    run_id: &RunId,
    result: AskResult,
) -> Result<Ask> {
    let ask = store.settle_ask(ask_id, run_id, result).await?;
    checkpoint_origin_task(store, &ask, "settle").await;
    resume_flow_step(store, &ask).await?;
    Ok(ask)
}

pub(crate) async fn cancel(store: &SharedStore, ask_id: &AskId, reason: &str) -> Result<Ask> {
    let ask = store.cancel_ask(ask_id, reason).await?;
    checkpoint_origin_task(store, &ask, "cancel").await;
    resume_flow_step(store, &ask).await?;
    Ok(ask)
}

pub(crate) async fn checkpoint_origin_task(store: &SharedStore, ask: &Ask, action: &str) {
    let WorkRef::Task(task_id) = &ask.origin.work else {
        return;
    };
    let task = match store.get_task(task_id).await {
        Ok(Some(task)) => task,
        _ => return,
    };
    if let Err(error) = crate::ops::checkpoint_task_worktree(
        task.worktree.clone(),
        task.plan.identifier.clone(),
        format!("checkpoint: {action} Ask {}", ask.id),
    )
    .await
    {
        tracing::warn!(ask = %ask.id, action, %error, "Ask settled without a pushed checkpoint");
    }
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
    run_id: RunId,
    headless: bool,
) -> Result<()> {
    let cleanup_store = Arc::clone(&store);
    let cleanup_ask_id = ask_id.clone();
    let cleanup_run_id = run_id.clone();
    crate::engine::agent::register_interrupt_cleanup(move || {
        let _ = cleanup_store.interrupt_ask_on_interrupt(&cleanup_ask_id, &cleanup_run_id);
    });
    let ask = wait_until_presented(&store, &ask_id, &run_id).await?;
    if ask.state.is_terminal()
        || ask.state != AskState::Claimed
        || ask.active_run_id.as_ref() != Some(&run_id)
    {
        return Ok(());
    }
    let result = match &ask.request {
        crate::durable::AskBody::FlowStep { .. } => {
            let turn = crate::controller::task::flow_step_harness_turn(&store, &ask).await?;
            run_flow_step_harness(&ask, turn, &run_id, headless).await
        }
        crate::durable::AskBody::Intervention { .. } => {
            let prompt = ask_prompt(&ask);
            let config = crate::engine::load_config_or_default(Some(&ask.origin.cwd));
            let agent = config.agent().to_string();
            run_provider(&ask, &agent, &prompt, &run_id, headless).await
        }
    };
    let current = store.ask_by_id(&ask.id).await?;
    if current.state == AskState::Claimed && current.active_run_id.as_ref() == Some(&run_id) {
        let reason = if interrupted_result(&result) {
            "Ask provider exited on a signal"
        } else {
            "Ask provider exited without settlement"
        };
        store.release_ask(&ask_id, &run_id, Some(reason)).await?;
        eprintln!("Ask Run closed without resolution; {} requeued", current.id);
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

async fn wait_until_presented(store: &SharedStore, ask_id: &AskId, run_id: &RunId) -> Result<Ask> {
    loop {
        let ask = store.ask_by_id(ask_id).await?;
        if ask.state.is_terminal()
            || ask.active_run_id.as_ref() != Some(run_id)
            || ask.presented_at.is_some()
        {
            return Ok(ask);
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn run_provider(
    ask: &Ask,
    agent: &str,
    prompt: &str,
    run_id: &RunId,
    headless: bool,
) -> Result<ExitStatus> {
    let launch = crate::engine::AgentConfig {
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
        cwd: Some(ask.origin.cwd.clone()),
        ..Default::default()
    };
    let (provider, model) = crate::engine::parse_agent(agent);
    let context = crate::trace::PreparedTurnContext::from_prompts(
        &crate::engine::agent::system_prompt_with_structured_replies(&launch),
        &launch.task_prompt,
    );
    let capture = begin_capture(
        ask,
        run_id,
        provider.clone(),
        model.clone(),
        "ask",
        &context,
    )?;
    capture.record_input("initial", prompt);
    let process = crate::engine::ProcessConfig {
        auto: headless,
        stream: false,
        capture: Some(capture.into()),
        ..Default::default()
    };
    let capabilities = crate::engine::AgentCapabilities::default();
    let result = tokio::task::spawn_blocking(move || {
        crate::engine::launch_agent(&launch, &process, &capabilities)
    })
    .await??;
    Ok(ExitStatus::from_raw(result.exit_code << 8))
}

async fn run_flow_step_harness(
    ask: &Ask,
    turn: crate::lf::commands::run::PreparedHarnessTurn,
    run_id: &RunId,
    headless: bool,
) -> Result<ExitStatus> {
    let context = turn.context;
    let mut launch = turn.config;
    launch.task_prompt = turn.input;
    launch.cwd = Some(ask.origin.cwd.clone());
    let agent = launch.agent.clone().unwrap_or_else(|| {
        crate::engine::load_config_or_default(Some(&ask.origin.cwd))
            .agent()
            .to_string()
    });
    let (provider, model) = crate::engine::parse_agent(&agent);
    let capture = begin_capture(
        ask,
        run_id,
        provider.clone(),
        model.clone(),
        "ask-flow-step",
        &context,
    )?;
    capture.record_input("initial", &launch.task_prompt);
    let process = crate::engine::ProcessConfig {
        auto: headless,
        stream: false,
        capture: Some(capture.into()),
        ..Default::default()
    };
    let capabilities = crate::engine::AgentCapabilities::default();
    let result = tokio::task::spawn_blocking(move || {
        crate::engine::launch_agent(&launch, &process, &capabilities)
    })
    .await??;
    Ok(ExitStatus::from_raw(result.exit_code << 8))
}

fn begin_capture(
    ask: &Ask,
    run_id: &RunId,
    provider: String,
    model: Option<String>,
    surface: &str,
    context: &crate::trace::PreparedTurnContext,
) -> Result<crate::run_record::CaptureHandle> {
    let spec = crate::run_record::RunSpec {
        harness: provider,
        model,
        surface: surface.to_string(),
        cwd: ask.origin.cwd.clone(),
        repo: Some(ask.origin.cwd.clone()),
        worktree: Some(ask.origin.cwd.clone()),
        skill: match &ask.request {
            crate::durable::AskBody::FlowStep { skill, .. } => Some(skill.clone()),
            crate::durable::AskBody::Intervention { .. } => None,
        },
        subjects: vec![
            crate::run_record::SubjectAttribution::declared(format!("ask:{}", ask.id)),
            crate::run_record::SubjectAttribution::declared(format!(
                "{}:{}",
                ask.origin.work.kind(),
                ask.origin.work.id()
            )),
        ],
    };
    crate::run_record::CaptureHandle::begin_with_verified_parent_and_context(
        spec,
        run_id.clone(),
        ask.origin.source_run_id.clone(),
        context,
    )
    .map_err(Into::into)
}

fn ask_prompt(ask: &Ask) -> String {
    format!(
        "Resolve durable Ask {id}.\n\nRequest:\n{request}\n\nOrigin: {kind} {work}\nOrigin cwd: {cwd}\nTarget: {target}\n\nYou are responsible only for this intervention; do not adopt the originating Work. Inspect or mutate the origin cwd as needed. Settlement is explicit and mandatory:\n- `lf ask resolve {id} \"<concise verified summary>\"` on success\n- `lf ask decline {id} \"<reason>\"` when the request should not be fulfilled\n- `lf ask release {id} \"<reason>\"` when unfinished\n- for a parent-targeted Ask that genuinely needs the absent User, `lf ask escalate {id} --user`\nA final response, clean exit, Ctrl-D, window close, or process exit never settles the Ask.",
        id = ask.id,
        request = ask.request,
        kind = ask.origin.work.kind(),
        work = ask.origin.work.id(),
        cwd = ask.origin.cwd.display(),
        target = ask.target,
    )
}

fn interrupted_result(result: &Result<ExitStatus>) -> bool {
    result.as_ref().ok().is_some_and(|status| {
        status.signal().is_some() || matches!(status.code(), Some(129 | 130 | 143))
    })
}

pub(crate) fn session_name(run_id: &RunId) -> String {
    format!("lf-ask-{}", &run_id.as_str()[4..16])
}

pub(crate) async fn observe_surface(surface: &AskSession) -> Result<bool> {
    let Some(session) = tmux_target(&surface.attach_argv) else {
        return Err(anyhow!("Ask session does not identify a tmux target"));
    };
    let Some(home) = HomeRoute::parse(&surface.home_route) else {
        return Err(anyhow!("Ask session has an invalid Home route"));
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
            Ok(output) if output.status.success() => Ok(true),
            Ok(output) if output.status.code() == Some(1) => Ok(false),
            Ok(output) => Err(anyhow!(
                "remote tmux session probe failed: {}",
                output.status
            )),
            Err(error) => Err(error.into()),
        }
    } else {
        crate::engine::process::tmux_session_exists(session).await
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
    retry_at: Option<tokio::time::Instant>,
}

impl AskLane {
    pub(crate) fn new(parent: WorkRef) -> Self {
        Self {
            parent,
            retry_at: None,
        }
    }

    pub(crate) async fn reconcile(&mut self, store: &SharedStore) -> Result<bool> {
        let retrying = self
            .retry_at
            .is_some_and(|retry_at| retry_at > tokio::time::Instant::now());
        if !retrying {
            self.retry_at = None;
        }
        let asks = pending_attention(store, &AskTarget::Parent(self.parent.clone()))
            .await?
            .into_iter()
            .map(|attention| attention.ask)
            .collect::<Vec<_>>();
        if asks.iter().any(|ask| ask.state == AskState::Claimed) {
            self.retry_at = None;
            return Ok(true);
        }
        let Some(ask) = asks.into_iter().find(|ask| ask.state == AskState::Queued) else {
            self.retry_at = None;
            return Ok(false);
        };
        if retrying {
            return Ok(true);
        }
        let claim = store.claim_ask(&ask.id).await?;
        if !claim.needs_launch {
            return Ok(true);
        }
        if let Err(error) = launch_claimed(store, &ask, &claim.run_id, true).await {
            self.retry_at = Some(tokio::time::Instant::now() + RETRY_DELAY);
            tracing::warn!(ask_id = %ask.id, %error, "failed to launch parent Ask session");
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    use super::{
        absence_is_authoritative, attention_state, begin_capture, interrupted_result, session_name,
        tmux_target, AttentionState, LAUNCH_GRACE_SECONDS,
    };
    use crate::durable::{
        Ask, AskBody, AskId, AskOrigin, AskState, AskTarget, HomeId, RunId, WorkRef,
    };
    use crate::id::WaveId;

    struct EnvironmentRestore(Vec<(&'static str, Option<OsString>)>);

    impl EnvironmentRestore {
        fn capture(keys: &[&'static str]) -> Self {
            Self(
                keys.iter()
                    .map(|key| (*key, std::env::var_os(key)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvironmentRestore {
        fn drop(&mut self) {
            for (key, value) in self.0.drain(..).rev() {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    fn ask(state: AskState) -> Ask {
        Ask {
            id: AskId::new(),
            origin: AskOrigin {
                work: WorkRef::Wave(WaveId::new()),
                source_run_id: Some(RunId::new()),
                home_id: HomeId::new(),
                cwd: "/tmp".into(),
            },
            target: AskTarget::User,
            request: AskBody::Intervention {
                prompt: "help".to_string(),
            },
            state,
            active_run_id: None,
            ready_at: None,
            presented_at: None,
            result: None,
            terminal_author: None,
            asked_at: time::OffsetDateTime::now_utc(),
            terminal_at: None,
        }
    }

    #[test]
    fn attention_distinguishes_queued_ready_and_active() {
        assert_eq!(
            attention_state(&ask(AskState::Queued)),
            AttentionState::Queued
        );
        let mut claimed = ask(AskState::Claimed);
        assert_eq!(attention_state(&claimed), AttentionState::Claimed);
        claimed.ready_at = Some(time::OffsetDateTime::now_utc());
        assert_eq!(attention_state(&claimed), AttentionState::NotPresented);
        claimed.presented_at = Some(time::OffsetDateTime::now_utc());
        assert_eq!(attention_state(&claimed), AttentionState::Active);
    }

    #[test]
    fn a_new_claim_gets_launch_grace_before_absence_is_stale() {
        let now = time::OffsetDateTime::now_utc();
        let mut claimed = ask(AskState::Claimed);
        claimed.ready_at = Some(now);
        assert!(!absence_is_authoritative(&claimed, now));

        claimed.ready_at = Some(now - time::Duration::seconds(LAUNCH_GRACE_SECONDS));
        assert!(absence_is_authoritative(&claimed, now));
    }

    #[test]
    fn ask_session_and_tmux_attach_are_exact() {
        let run_id = RunId::new();
        let name = session_name(&run_id);
        let argv = vec![
            "tmux".to_string(),
            "attach-session".to_string(),
            "-t".to_string(),
            name.clone(),
        ];
        assert_eq!(tmux_target(&argv), Some(name.as_str()));
    }

    #[test]
    fn ask_capture_uses_only_a_verified_source_run_as_parent() {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let _environment = EnvironmentRestore::capture(&[
            "LF_HOME",
            crate::durable::RUN_ID_ENV,
            crate::run_record::RUN_DIR_ENV,
        ]);
        std::env::set_var("LF_HOME", home.path());
        std::env::remove_var(crate::durable::RUN_ID_ENV);
        std::env::remove_var(crate::run_record::RUN_DIR_ENV);
        let mut ask = ask(AskState::Claimed);
        let parent = crate::run_record::CaptureHandle::begin(crate::run_record::RunSpec {
            harness: "codex".to_string(),
            model: Some("gpt-5".to_string()),
            surface: "parent".to_string(),
            cwd: home.path().to_path_buf(),
            repo: None,
            worktree: None,
            skill: None,
            subjects: Vec::new(),
        })
        .unwrap();
        let parent_run_id = parent.run_id();
        ask.origin.source_run_id = Some(parent_run_id.clone());
        let run_id = RunId::new();
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "task");

        let capture = begin_capture(
            &ask,
            &run_id,
            "codex".to_string(),
            Some("gpt-5".to_string()),
            "ask",
            &context,
        )
        .unwrap();
        let directory = capture.artifact_dir();
        let manifest: crate::run_record::RunManifest =
            serde_json::from_slice(&std::fs::read(directory.join("manifest.json")).unwrap())
                .unwrap();

        assert_eq!(manifest.run_id, run_id);
        assert_eq!(manifest.parent_run_id, Some(parent_run_id));
        assert!(manifest
            .subjects
            .iter()
            .any(|subject| subject.selector == format!("ask:{}", ask.id)));
        assert!(!directory.join("owner.json").exists());
        capture.finish("completed").unwrap();
        parent.finish("completed").unwrap();
        assert!(directory.join("terminal.json").is_file());

        ask.origin.source_run_id = Some(RunId::new());
        let capture = begin_capture(
            &ask,
            &RunId::new(),
            "codex".to_string(),
            Some("gpt-5".to_string()),
            "ask",
            &context,
        )
        .unwrap();
        let manifest: crate::run_record::RunManifest = serde_json::from_slice(
            &std::fs::read(capture.artifact_dir().join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert!(manifest.parent_run_id.is_none());
    }

    #[test]
    fn exiting_interrupts_are_transport_evidence_not_success() {
        for status in [ExitStatus::from_raw(130 << 8), ExitStatus::from_raw(9)] {
            assert!(interrupted_result(&Ok(status)));
        }
        assert!(!interrupted_result(&Ok(ExitStatus::from_raw(0))));
    }
}
