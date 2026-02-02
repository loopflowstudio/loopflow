use std::pin::Pin;
use std::sync::Arc;

use crate::id::LfdId;
use crate::proto::control::control_service_server::ControlService;
use crate::proto::control::{
    AcquireSlotRequest, AcquireSlotResponse, Agent, AgentStatus, CloneWaveRequest,
    CloneWaveResponse, ConnectWaveRequest, ConnectWaveResponse, CreateStimulusRequest,
    CreateStimulusResponse, CreateWaveRequest, CreateWaveResponse, DeleteStimulusRequest,
    DeleteStimulusResponse, DeleteWaveRequest, DeleteWaveResponse, EndAgentRequest,
    EndAgentResponse, Event, GetHealthRequest, GetHealthResponse, GetSchedulerStatusRequest,
    GetSchedulerStatusResponse, GetStatusRequest, GetStatusResponse, GetStimulusRequest,
    GetStimulusResponse, HealthChecks, HealthMetrics, ListAgentsRequest, ListAgentsResponse,
    ListFlowsRequest, ListFlowsResponse, ListStimuliRequest, ListStimuliResponse, ListWavesRequest,
    ListWavesResponse, ListWorktreesRequest, ListWorktreesResponse, MergeMode, NotifyRequest,
    NotifyResponse, NotifyWorktreeChangedRequest, NotifyWorktreeChangedResponse, ProtocolVersion,
    ReleaseSlotRequest, ReleaseSlotResponse, RunWaveRequest, RunWaveResponse, Stimulus,
    StopWaveRequest, StopWaveResponse, StreamOutputRequest, StreamOutputResponse,
    UpdateStimulusRequest, UpdateStimulusResponse, UpdateWaveRequest, UpdateWaveResponse, Wave,
    WaveStatus,
};
use crate::scheduler::Scheduler;
use crate::sessions::{run_pty_command, PtyCommand};
use crate::store::{SharedStore, StoreError, StoreResult};
use prost_types::Timestamp;
use time::OffsetDateTime;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct ControlServer {
    store: SharedStore,
    scheduler: Arc<Scheduler>,
    started_at: OffsetDateTime,
}

impl std::fmt::Debug for ControlServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ControlServer")
            .field("store", &"RunStore")
            .field("scheduler", &self.scheduler)
            .field("started_at", &self.started_at)
            .finish()
    }
}

impl ControlServer {
    pub fn new(store: SharedStore, scheduler: Arc<Scheduler>) -> Self {
        Self {
            store,
            scheduler,
            started_at: OffsetDateTime::now_utc(),
        }
    }

    #[allow(clippy::result_large_err)]
    fn parse_id(value: &str, name: &str) -> Result<LfdId, Status> {
        LfdId::parse(value)
            .map_err(|err| Status::invalid_argument(format!("invalid {name}: {err}")))
    }

    async fn run_store<T, F>(&self, func: F) -> Result<T, Status>
    where
        T: Send + 'static,
        F: FnOnce(SharedStore) -> StoreResult<T> + Send + 'static,
    {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || func(store))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)
    }

    fn status_counts(waves: &[Wave], agents: &[Agent]) -> (u32, u32, u32) {
        let waves_defined = waves.len() as u32;
        let waves_running = waves
            .iter()
            .filter(|wave| wave.status == WaveStatus::WaveRunning as i32)
            .count() as u32;
        let agents_active = agents
            .iter()
            .filter(|run| {
                run.status == AgentStatus::AgentRunning as i32
                    || run.status == AgentStatus::AgentWaiting as i32
            })
            .count() as u32;
        (waves_defined, waves_running, agents_active)
    }

    fn now_timestamp() -> Timestamp {
        let now = OffsetDateTime::now_utc();
        Timestamp {
            seconds: now.unix_timestamp(),
            nanos: 0,
        }
    }

    fn store_error(err: StoreError) -> Status {
        match err {
            StoreError::NotFound => Status::not_found("not found"),
            StoreError::InvalidData(message) => Status::invalid_argument(message),
            StoreError::Serde(err) => Status::internal(format!("serialization error: {err}")),
            StoreError::Sqlite(err) => Status::internal(format!("database error: {err}")),
            StoreError::Postgres(err) => Status::internal(format!("database error: {err}")),
            StoreError::PostgresPool(err) => Status::internal(format!("database error: {err}")),
        }
    }

    async fn list_waves_inner(&self, repo: Option<String>) -> Result<Vec<Wave>, Status> {
        self.run_store(move |store| store.list_waves(repo.as_deref()))
            .await
    }

    async fn list_agents_inner(&self) -> Result<Vec<Agent>, Status> {
        self.run_store(|store| store.list_agents()).await
    }
}

#[tonic::async_trait]
impl ControlService for ControlServer {
    async fn get_status(
        &self,
        _request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        let waves = self.list_waves_inner(None).await?;
        let agents = self.list_agents_inner().await?;
        let (waves_defined, waves_running, agents_active) = Self::status_counts(&waves, &agents);

        Ok(Response::new(GetStatusResponse {
            pid: std::process::id(),
            waves_defined,
            waves_running,
            agents_active,
        }))
    }

    async fn get_health(
        &self,
        _request: Request<GetHealthRequest>,
    ) -> Result<Response<GetHealthResponse>, Status> {
        let schema_version = self.run_store(|store| store.schema_version()).await?;
        let database_ok = self.run_store(|store| store.health_check()).await.is_ok();

        let waves = self.list_waves_inner(None).await?;
        let agents = self.list_agents_inner().await?;
        let (waves_defined, waves_running, agents_active) = Self::status_counts(&waves, &agents);

        let uptime = OffsetDateTime::now_utc() - self.started_at;

        Ok(Response::new(GetHealthResponse {
            version: env!("CARGO_PKG_VERSION").to_string(),
            schema_version,
            uptime_seconds: uptime.whole_seconds() as u64,
            checks: Some(HealthChecks {
                database: database_ok,
                socket: true,
            }),
            metrics: Some(HealthMetrics {
                waves_total: waves_defined,
                waves_running,
                agents_active,
                wave_runs_total: 0,
            }),
            protocol_version: Some(ProtocolVersion {
                major: 1,
                minor: 0,
                patch: 0,
            }),
        }))
    }

    async fn list_waves(
        &self,
        request: Request<ListWavesRequest>,
    ) -> Result<Response<ListWavesResponse>, Status> {
        let repo = request.into_inner().repo;
        let waves = if repo.is_empty() {
            self.list_waves_inner(None).await?
        } else {
            self.list_waves_inner(Some(repo)).await?
        };
        Ok(Response::new(ListWavesResponse { waves }))
    }

    async fn get_wave(
        &self,
        request: Request<crate::proto::control::GetWaveRequest>,
    ) -> Result<Response<crate::proto::control::GetWaveResponse>, Status> {
        let wave_id = request.into_inner().wave_id;
        let wave_id = Self::parse_id(&wave_id, "wave_id")?;
        let wave = self
            .run_store(move |store| store.get_wave(&wave_id))
            .await?
            .ok_or_else(|| Status::not_found("wave not found"))?;
        Ok(Response::new(crate::proto::control::GetWaveResponse {
            wave: Some(wave),
        }))
    }

    async fn create_wave(
        &self,
        request: Request<CreateWaveRequest>,
    ) -> Result<Response<CreateWaveResponse>, Status> {
        let req = request.into_inner();
        let id = LfdId::new().to_string();
        let name = req.name.unwrap_or_else(|| format!("wave-{id}"));
        let flow = req.flow.unwrap_or_else(|| "ship".to_string());

        let wave = Wave {
            id,
            name,
            repo: req.repo,
            flow,
            direction: req.direction,
            area: req.area,
            paused: false,
            status: WaveStatus::WaveIdle as i32,
            iteration: 0,
            step_index: 0,
            worktree: String::new(),
            branch: String::new(),
            pr_limit: 0,
            merge_mode: MergeMode::MergePr as i32,
            pid: None,
            created_at: Some(Self::now_timestamp()),
            consecutive_failures: 0,
            pending_activations: 0,
        };

        let wave_clone = wave.clone();
        self.run_store(move |store| store.create_wave(&wave_clone))
            .await?;

        Ok(Response::new(CreateWaveResponse { wave: Some(wave) }))
    }

    async fn update_wave(
        &self,
        request: Request<UpdateWaveRequest>,
    ) -> Result<Response<UpdateWaveResponse>, Status> {
        let req = request.into_inner();
        let wave_id = req.wave_id;
        let wave_id = Self::parse_id(&wave_id, "wave_id")?;
        let mut wave = self
            .run_store(move |store| store.get_wave(&wave_id))
            .await?
            .ok_or_else(|| Status::not_found("wave not found"))?;

        if let Some(flow) = req.flow {
            wave.flow = flow;
        }
        if !req.direction.is_empty() {
            wave.direction = req.direction;
        }
        if !req.area.is_empty() {
            wave.area = req.area;
        }
        // Note: stimulus is now managed via separate Stimulus RPCs
        if let Some(paused) = req.paused {
            wave.paused = paused;
        }

        let wave_clone = wave.clone();
        self.run_store(move |store| store.update_wave(&wave_clone))
            .await?;

        Ok(Response::new(UpdateWaveResponse { wave: Some(wave) }))
    }

    async fn delete_wave(
        &self,
        request: Request<DeleteWaveRequest>,
    ) -> Result<Response<DeleteWaveResponse>, Status> {
        let wave_id = request.into_inner().wave_id;
        let wave_id = Self::parse_id(&wave_id, "wave_id")?;
        self.run_store(move |store| store.delete_wave(&wave_id))
            .await?;
        Ok(Response::new(DeleteWaveResponse {}))
    }

    async fn clone_wave(
        &self,
        request: Request<CloneWaveRequest>,
    ) -> Result<Response<CloneWaveResponse>, Status> {
        let req = request.into_inner();
        let wave_id = Self::parse_id(&req.wave_id, "wave_id")?;
        let mut wave = self
            .run_store(move |store| store.get_wave(&wave_id))
            .await?
            .ok_or_else(|| Status::not_found("wave not found"))?;

        let new_id = LfdId::new().to_string();
        wave.id = new_id;
        wave.name = req.name.unwrap_or_else(|| format!("{}-copy", wave.name));
        wave.created_at = Some(Self::now_timestamp());

        let wave_clone = wave.clone();
        self.run_store(move |store| store.create_wave(&wave_clone))
            .await?;

        Ok(Response::new(CloneWaveResponse { wave: Some(wave) }))
    }

    async fn run_wave(
        &self,
        request: Request<RunWaveRequest>,
    ) -> Result<Response<RunWaveResponse>, Status> {
        let req = request.into_inner();
        let wave_id = req.wave_id;
        let wave_id = Self::parse_id(&wave_id, "wave_id")?;
        let mut wave = self
            .run_store(move |store| store.get_wave(&wave_id))
            .await?
            .ok_or_else(|| Status::not_found("wave not found"))?;

        wave.status = WaveStatus::WaveRunning as i32;
        wave.step_index = 0;
        wave.paused = false;
        if let Some(flow) = req.flow {
            wave.flow = flow;
        }
        if !req.direction.is_empty() {
            wave.direction = req.direction;
        }
        if !req.area.is_empty() {
            wave.area = req.area;
        }
        // Note: stimulus is now managed via separate Stimulus RPCs

        let wave_clone = wave.clone();
        self.run_store(move |store| store.update_wave(&wave_clone))
            .await?;

        Ok(Response::new(RunWaveResponse {
            started: true,
            wave_id: wave.id,
        }))
    }

    async fn stop_wave(
        &self,
        request: Request<StopWaveRequest>,
    ) -> Result<Response<StopWaveResponse>, Status> {
        let wave_id = request.into_inner().wave_id;
        let wave_id = Self::parse_id(&wave_id, "wave_id")?;
        let mut wave = self
            .run_store(move |store| store.get_wave(&wave_id))
            .await?
            .ok_or_else(|| Status::not_found("wave not found"))?;

        wave.status = WaveStatus::WaveIdle as i32;
        wave.paused = true;

        let wave_clone = wave.clone();
        self.run_store(move |store| store.update_wave(&wave_clone))
            .await?;

        Ok(Response::new(StopWaveResponse { stopped: true }))
    }

    // Stimulus management

    async fn list_stimuli(
        &self,
        request: Request<ListStimuliRequest>,
    ) -> Result<Response<ListStimuliResponse>, Status> {
        let req = request.into_inner();
        let wave_id = req.wave_id.as_deref().filter(|value| !value.is_empty());
        let wave_id = match wave_id {
            Some(value) => Some(Self::parse_id(value, "wave_id")?),
            None => None,
        };

        let stimuli = if let Some(kind) = req.kind {
            self.run_store(move |store| store.list_stimuli_by_kind(kind))
                .await?
        } else {
            self.run_store(move |store| store.list_stimuli(wave_id.as_ref()))
                .await?
        };

        Ok(Response::new(ListStimuliResponse { stimuli }))
    }

    async fn get_stimulus(
        &self,
        request: Request<GetStimulusRequest>,
    ) -> Result<Response<GetStimulusResponse>, Status> {
        let stimulus_id = request.into_inner().stimulus_id;
        let stimulus_id = Self::parse_id(&stimulus_id, "stimulus_id")?;
        let stimulus = self
            .run_store(move |store| store.get_stimulus(&stimulus_id))
            .await?
            .ok_or_else(|| Status::not_found("stimulus not found"))?;
        Ok(Response::new(GetStimulusResponse {
            stimulus: Some(stimulus),
        }))
    }

    async fn create_stimulus(
        &self,
        request: Request<CreateStimulusRequest>,
    ) -> Result<Response<CreateStimulusResponse>, Status> {
        let req = request.into_inner();

        // Verify wave exists
        let wave_id = Self::parse_id(&req.wave_id, "wave_id")?;
        self.run_store(move |store| store.get_wave(&wave_id))
            .await?
            .ok_or_else(|| Status::not_found("wave not found"))?;

        let stimulus = Stimulus {
            id: LfdId::new().to_string(),
            wave_id: req.wave_id,
            kind: req.kind,
            cron: req.cron,
            last_main_sha: None,
            last_triggered_at: None,
            enabled: true,
            created_at: Some(Self::now_timestamp()),
        };

        let stimulus_clone = stimulus.clone();
        self.run_store(move |store| store.create_stimulus(&stimulus_clone))
            .await?;

        Ok(Response::new(CreateStimulusResponse {
            stimulus: Some(stimulus),
        }))
    }

    async fn update_stimulus(
        &self,
        request: Request<UpdateStimulusRequest>,
    ) -> Result<Response<UpdateStimulusResponse>, Status> {
        let req = request.into_inner();
        let stimulus_id = req.stimulus_id;
        let stimulus_id = Self::parse_id(&stimulus_id, "stimulus_id")?;
        let mut stimulus = self
            .run_store(move |store| store.get_stimulus(&stimulus_id))
            .await?
            .ok_or_else(|| Status::not_found("stimulus not found"))?;

        if let Some(cron) = req.cron {
            stimulus.cron = cron;
        }
        if let Some(enabled) = req.enabled {
            stimulus.enabled = enabled;
        }

        let stimulus_clone = stimulus.clone();
        self.run_store(move |store| store.update_stimulus(&stimulus_clone))
            .await?;

        Ok(Response::new(UpdateStimulusResponse {
            stimulus: Some(stimulus),
        }))
    }

    async fn delete_stimulus(
        &self,
        request: Request<DeleteStimulusRequest>,
    ) -> Result<Response<DeleteStimulusResponse>, Status> {
        let stimulus_id = request.into_inner().stimulus_id;
        let stimulus_id = Self::parse_id(&stimulus_id, "stimulus_id")?;
        self.run_store(move |store| store.delete_stimulus(&stimulus_id))
            .await?;
        Ok(Response::new(DeleteStimulusResponse {}))
    }

    async fn connect_wave(
        &self,
        request: Request<ConnectWaveRequest>,
    ) -> Result<Response<ConnectWaveResponse>, Status> {
        let wave_id = request.into_inner().wave_id;
        let wave_id = Self::parse_id(&wave_id, "wave_id")?;
        let mut wave = self
            .run_store({
                let wave_id = wave_id.clone();
                move |store| store.get_wave(&wave_id)
            })
            .await?
            .ok_or_else(|| Status::not_found("wave not found"))?;

        if !self.scheduler.register_session(wave_id.as_str()) {
            return Err(Status::failed_precondition("session already active"));
        }

        let agent = match self
            .run_store({
                let wave_id = wave_id.clone();
                move |store| store.get_waiting_agent(&wave_id)
            })
            .await
        {
            Ok(Some(agent)) => agent,
            Ok(None) => {
                self.scheduler.unregister_session(wave_id.as_str());
                return Err(Status::not_found("no waiting step run"));
            }
            Err(err) => {
                self.scheduler.unregister_session(wave_id.as_str());
                return Err(err);
            }
        };

        let agent_id = agent.id.clone();
        if let Err(err) = self
            .run_store({
                let agent_id = agent_id.clone();
                move |store| {
                    store.update_agent_status(
                        &LfdId::from_raw(agent_id),
                        AgentStatus::AgentRunning as i32,
                        None,
                    )
                }
            })
            .await
        {
            self.scheduler.unregister_session(wave_id.as_str());
            return Err(err);
        }

        wave.status = WaveStatus::WaveRunning as i32;
        let wave_clone = wave.clone();
        if let Err(err) = self
            .run_store(move |store| store.update_wave(&wave_clone))
            .await
        {
            self.scheduler.unregister_session(wave_id.as_str());
            return Err(err);
        }

        let store = self.store.clone();
        let scheduler = self.scheduler.clone();
        let directions = wave.direction.clone();
        let worktree = agent.worktree.clone();
        let step = agent.step.clone();
        let wave_id_task = wave.id.clone();
        let wave_id_task_id =
            LfdId::parse(&wave.id).unwrap_or_else(|_| LfdId::from_raw(wave.id.clone()));

        tokio::spawn(async move {
            let mut command = PtyCommand::new("lf")
                .arg("run")
                .arg("--interactive")
                .arg(step.clone())
                .cwd(&worktree);
            if !directions.is_empty() {
                command = command.arg("--direction").arg(directions.join(","));
            }

            let exit_code = match tokio::task::spawn_blocking(move || run_pty_command(command))
                .await
            {
                Ok(Ok(code)) => code,
                Ok(Err(err)) => {
                    tracing::error!(wave_id = %wave_id_task, error = %err, "session failed");
                    1
                }
                Err(err) => {
                    tracing::error!(wave_id = %wave_id_task, error = %err, "session join failed");
                    1
                }
            };

            let status = if exit_code == 0 {
                AgentStatus::AgentCompleted
            } else {
                AgentStatus::AgentFailed
            };

            let ended_at = OffsetDateTime::now_utc().unix_timestamp();
            let agent_id = LfdId::from_raw(agent_id);
            if let Err(err) = store.end_agent(&agent_id, status as i32, ended_at) {
                tracing::warn!(wave_id = %wave_id_task, error = %err, "failed to end step run");
            }

            if let Ok(Some(mut wave)) = store.get_wave(&wave_id_task_id) {
                if status == AgentStatus::AgentCompleted {
                    wave.step_index += 1;
                    wave.consecutive_failures = 0;
                    wave.status = WaveStatus::WaveRunning as i32;
                } else {
                    wave.consecutive_failures += 1;
                    if wave.consecutive_failures >= 3 {
                        wave.status = WaveStatus::WaveError as i32;
                    } else {
                        wave.status = WaveStatus::WaveRunning as i32;
                    }
                }
                if let Err(err) = store.update_wave(&wave) {
                    tracing::warn!(wave_id = %wave.id, error = %err, "failed to update wave");
                }
            }

            scheduler.unregister_session(&wave_id_task);
        });

        Ok(Response::new(ConnectWaveResponse {
            worktree: agent.worktree,
            step: agent.step,
            agent_id: agent.id,
            prompt_file: String::new(),
            wave_run_id: agent.wave_run_id,
            step_index: wave.step_index,
        }))
    }

    async fn list_flows(
        &self,
        _request: Request<ListFlowsRequest>,
    ) -> Result<Response<ListFlowsResponse>, Status> {
        Ok(Response::new(ListFlowsResponse {
            flows: Vec::new(),
            steps: Vec::new(),
        }))
    }

    async fn list_worktrees(
        &self,
        _request: Request<ListWorktreesRequest>,
    ) -> Result<Response<ListWorktreesResponse>, Status> {
        Ok(Response::new(ListWorktreesResponse {
            worktrees: Vec::new(),
        }))
    }

    async fn notify_worktree_changed(
        &self,
        request: Request<NotifyWorktreeChangedRequest>,
    ) -> Result<Response<NotifyWorktreeChangedResponse>, Status> {
        let req = request.into_inner();
        Ok(Response::new(NotifyWorktreeChangedResponse {
            branch: req.branch,
            reason: req.reason.unwrap_or_else(|| "updated".to_string()),
        }))
    }

    async fn get_scheduler_status(
        &self,
        _request: Request<GetSchedulerStatusRequest>,
    ) -> Result<Response<GetSchedulerStatusResponse>, Status> {
        Ok(Response::new(GetSchedulerStatusResponse {
            slots_used: self.scheduler.slots_used(),
            slots_total: self.scheduler.max_slots() as u32,
            outstanding: 0,
            outstanding_limit: 0,
            running: Vec::new(),
        }))
    }

    async fn acquire_slot(
        &self,
        request: Request<AcquireSlotRequest>,
    ) -> Result<Response<AcquireSlotResponse>, Status> {
        let run_id = request.into_inner().run_id;
        let (acquired, reason) = self.scheduler.acquire(&run_id).await;
        Ok(Response::new(AcquireSlotResponse {
            acquired,
            reason,
            slots_used: self.scheduler.slots_used(),
        }))
    }

    async fn release_slot(
        &self,
        request: Request<ReleaseSlotRequest>,
    ) -> Result<Response<ReleaseSlotResponse>, Status> {
        let run_id = request.into_inner().run_id;
        let slots_used = self.scheduler.release(&run_id);
        Ok(Response::new(ReleaseSlotResponse { slots_used }))
    }

    async fn list_agents(
        &self,
        _request: Request<ListAgentsRequest>,
    ) -> Result<Response<ListAgentsResponse>, Status> {
        let runs = self.list_agents_inner().await?;
        Ok(Response::new(ListAgentsResponse { agents: runs }))
    }

    async fn get_agent_history(
        &self,
        request: Request<crate::proto::control::GetAgentHistoryRequest>,
    ) -> Result<Response<crate::proto::control::GetAgentHistoryResponse>, Status> {
        let req = request.into_inner();
        let worktree = req.worktree.clone();
        let repo = req.repo.clone();
        let limit = req.limit;
        let runs = self
            .run_store(move |store| {
                store.list_agent_history(worktree.as_deref(), repo.as_deref(), limit)
            })
            .await?;

        Ok(Response::new(
            crate::proto::control::GetAgentHistoryResponse { agents: runs },
        ))
    }

    async fn start_agent(
        &self,
        request: Request<crate::proto::control::StartAgentRequest>,
    ) -> Result<Response<crate::proto::control::StartAgentResponse>, Status> {
        let agent = request
            .into_inner()
            .agent
            .ok_or_else(|| Status::invalid_argument("missing agent"))?;
        let id = if agent.id.is_empty() {
            LfdId::new().to_string()
        } else {
            agent.id.clone()
        };

        let mut stored = agent.clone();
        stored.id = id.clone();
        if stored.started_at.is_none() {
            stored.started_at = Some(Self::now_timestamp());
        }

        let stored_clone = stored.clone();
        self.run_store(move |store| store.start_agent(&stored_clone))
            .await?;

        Ok(Response::new(crate::proto::control::StartAgentResponse {
            id,
        }))
    }

    async fn end_agent(
        &self,
        request: Request<EndAgentRequest>,
    ) -> Result<Response<EndAgentResponse>, Status> {
        let req = request.into_inner();
        let agent_id = req.agent_id.clone();
        let agent_id_for_end = LfdId::from_raw(agent_id.clone());
        let ended_at = Self::now_timestamp().seconds;
        self.run_store(move |store| store.end_agent(&agent_id_for_end, req.status, ended_at))
            .await?;

        if let Ok(Some(agent)) = self
            .run_store({
                let agent_id = LfdId::from_raw(agent_id.clone());
                move |store| store.get_agent(&agent_id)
            })
            .await
        {
            if let Some(wave_id) = agent.wave_id {
                let wave_id = Self::parse_id(&wave_id, "wave_id")?;
                let wave = self
                    .run_store(move |store| store.get_wave(&wave_id))
                    .await?;

                if let Some(mut wave) = wave {
                    if req.status == AgentStatus::AgentCompleted as i32 {
                        wave.step_index += 1;
                        wave.consecutive_failures = 0;
                        wave.status = WaveStatus::WaveRunning as i32;
                    } else if req.status == AgentStatus::AgentFailed as i32 {
                        wave.consecutive_failures += 1;
                        if wave.consecutive_failures >= 3 {
                            wave.status = WaveStatus::WaveError as i32;
                        } else {
                            wave.status = WaveStatus::WaveRunning as i32;
                        }
                    }
                    let wave_clone = wave.clone();
                    let _ = self
                        .run_store(move |store| store.update_wave(&wave_clone))
                        .await;
                }
            }
        }

        Ok(Response::new(EndAgentResponse { id: req.agent_id }))
    }

    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<Event, Status>> + Send>>;

    async fn subscribe(
        &self,
        _request: Request<crate::proto::control::SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let stream = tokio_stream::empty();
        Ok(Response::new(Box::pin(stream)))
    }

    async fn notify(
        &self,
        request: Request<NotifyRequest>,
    ) -> Result<Response<NotifyResponse>, Status> {
        let req = request.into_inner();
        Ok(Response::new(NotifyResponse { event: req.event }))
    }

    async fn stream_output(
        &self,
        _request: Request<StreamOutputRequest>,
    ) -> Result<Response<StreamOutputResponse>, Status> {
        Ok(Response::new(StreamOutputResponse {}))
    }
}
