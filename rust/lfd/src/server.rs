use std::pin::Pin;
use std::sync::Arc;

use prost_types::Timestamp;
use time::OffsetDateTime;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use crate::proto::control::control_service_server::ControlService;
use crate::proto::control::{
    AcquireSlotRequest, AcquireSlotResponse, CloneWaveRequest, CloneWaveResponse,
    ConnectWaveRequest, ConnectWaveResponse, CreateWaveRequest, CreateWaveResponse,
    DeleteWaveRequest, DeleteWaveResponse, EndStepRunRequest, EndStepRunResponse, Event,
    GetHealthRequest, GetHealthResponse, GetSchedulerStatusRequest, GetSchedulerStatusResponse,
    GetStatusRequest, GetStatusResponse, HealthChecks, HealthMetrics, ListFlowsRequest,
    ListFlowsResponse, ListStepRunsRequest, ListStepRunsResponse, ListWavesRequest,
    ListWavesResponse, ListWorktreesRequest, ListWorktreesResponse, MergeMode, NotifyRequest,
    NotifyResponse, NotifyWorktreeChangedRequest, NotifyWorktreeChangedResponse, ProtocolVersion,
    ReleaseSlotRequest, ReleaseSlotResponse, RunWaveRequest, RunWaveResponse, StepRun,
    StepRunStatus, Stimulus, StimulusKind, StopWaveRequest, StopWaveResponse, StreamOutputRequest,
    StreamOutputResponse, UpdateWaveRequest, UpdateWaveResponse, Wave, WaveStatus,
};
use crate::scheduler::Scheduler;
use crate::store::{SharedStore, StoreError};

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

    fn status_counts(waves: &[Wave], step_runs: &[StepRun]) -> (u32, u32, u32) {
        let waves_defined = waves.len() as u32;
        let waves_running = waves
            .iter()
            .filter(|wave| wave.status == WaveStatus::WaveRunning as i32)
            .count() as u32;
        let step_runs_active = step_runs
            .iter()
            .filter(|run| {
                run.status == StepRunStatus::StepRunning as i32
                    || run.status == StepRunStatus::StepWaiting as i32
            })
            .count() as u32;
        (waves_defined, waves_running, step_runs_active)
    }

    fn default_stimulus() -> Stimulus {
        Stimulus {
            kind: StimulusKind::StimulusOnce as i32,
            cron: String::new(),
        }
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
        }
    }

    async fn list_waves_inner(&self, repo: Option<String>) -> Result<Vec<Wave>, Status> {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.list_waves(repo.as_deref()))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)
    }

    async fn list_step_runs_inner(&self) -> Result<Vec<StepRun>, Status> {
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.list_step_runs())
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)
    }
}

#[tonic::async_trait]
impl ControlService for ControlServer {
    async fn get_status(
        &self,
        _request: Request<GetStatusRequest>,
    ) -> Result<Response<GetStatusResponse>, Status> {
        let waves = self.list_waves_inner(None).await?;
        let step_runs = self.list_step_runs_inner().await?;
        let (waves_defined, waves_running, step_runs_active) =
            Self::status_counts(&waves, &step_runs);

        Ok(Response::new(GetStatusResponse {
            pid: std::process::id(),
            waves_defined,
            waves_running,
            step_runs_active,
        }))
    }

    async fn get_health(
        &self,
        _request: Request<GetHealthRequest>,
    ) -> Result<Response<GetHealthResponse>, Status> {
        let store = self.store.clone();
        let schema_version = tokio::task::spawn_blocking(move || store.schema_version())
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?;

        let store = self.store.clone();
        let database_ok = tokio::task::spawn_blocking(move || store.health_check())
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)
            .is_ok();

        let waves = self.list_waves_inner(None).await?;
        let step_runs = self.list_step_runs_inner().await?;
        let (waves_defined, waves_running, step_runs_active) =
            Self::status_counts(&waves, &step_runs);

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
                step_runs_active,
                flow_runs_total: 0,
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
        let store = self.store.clone();
        let wave = tokio::task::spawn_blocking(move || store.get_wave(&wave_id))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?
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
        let id = Uuid::new_v4().to_string();
        let name = req.name.unwrap_or_else(|| format!("wave-{id}"));
        let flow = req.flow.unwrap_or_else(|| "ship".to_string());

        let wave = Wave {
            id,
            name,
            repo: req.repo,
            flow,
            direction: req.direction,
            area: req.area,
            stimulus: Some(Self::default_stimulus()),
            paused: false,
            status: WaveStatus::WaveIdle as i32,
            iteration: 0,
            worktree: String::new(),
            branch: String::new(),
            pr_limit: 0,
            merge_mode: MergeMode::MergePr as i32,
            pid: None,
            created_at: Some(Self::now_timestamp()),
            last_main_sha: None,
            consecutive_failures: 0,
            pending_activations: 0,
        };

        let store = self.store.clone();
        let wave_clone = wave.clone();
        tokio::task::spawn_blocking(move || store.create_wave(&wave_clone))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?;

        Ok(Response::new(CreateWaveResponse { wave: Some(wave) }))
    }

    async fn update_wave(
        &self,
        request: Request<UpdateWaveRequest>,
    ) -> Result<Response<UpdateWaveResponse>, Status> {
        let req = request.into_inner();
        let wave_id = req.wave_id;
        let store = self.store.clone();
        let mut wave = tokio::task::spawn_blocking(move || store.get_wave(&wave_id))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?
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
        if let Some(stimulus) = req.stimulus {
            wave.stimulus = Some(stimulus);
        }
        if let Some(paused) = req.paused {
            wave.paused = paused;
        }

        let store = self.store.clone();
        let wave_clone = wave.clone();
        tokio::task::spawn_blocking(move || store.update_wave(&wave_clone))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?;

        Ok(Response::new(UpdateWaveResponse { wave: Some(wave) }))
    }

    async fn delete_wave(
        &self,
        request: Request<DeleteWaveRequest>,
    ) -> Result<Response<DeleteWaveResponse>, Status> {
        let wave_id = request.into_inner().wave_id;
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.delete_wave(&wave_id))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?;
        Ok(Response::new(DeleteWaveResponse {}))
    }

    async fn clone_wave(
        &self,
        request: Request<CloneWaveRequest>,
    ) -> Result<Response<CloneWaveResponse>, Status> {
        let req = request.into_inner();
        let store = self.store.clone();
        let mut wave = tokio::task::spawn_blocking(move || store.get_wave(&req.wave_id))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?
            .ok_or_else(|| Status::not_found("wave not found"))?;

        let new_id = Uuid::new_v4().to_string();
        wave.id = new_id;
        wave.name = req.name.unwrap_or_else(|| format!("{}-copy", wave.name));
        wave.created_at = Some(Self::now_timestamp());

        let store = self.store.clone();
        let wave_clone = wave.clone();
        tokio::task::spawn_blocking(move || store.create_wave(&wave_clone))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?;

        Ok(Response::new(CloneWaveResponse { wave: Some(wave) }))
    }

    async fn run_wave(
        &self,
        request: Request<RunWaveRequest>,
    ) -> Result<Response<RunWaveResponse>, Status> {
        let req = request.into_inner();
        let wave_id = req.wave_id;
        let store = self.store.clone();
        let mut wave = tokio::task::spawn_blocking(move || store.get_wave(&wave_id))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?
            .ok_or_else(|| Status::not_found("wave not found"))?;

        wave.status = WaveStatus::WaveRunning as i32;
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
        if let Some(stimulus) = req.stimulus {
            wave.stimulus = Some(stimulus);
        }

        let store = self.store.clone();
        let wave_clone = wave.clone();
        tokio::task::spawn_blocking(move || store.update_wave(&wave_clone))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?;

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
        let store = self.store.clone();
        let mut wave = tokio::task::spawn_blocking(move || store.get_wave(&wave_id))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?
            .ok_or_else(|| Status::not_found("wave not found"))?;

        wave.status = WaveStatus::WaveIdle as i32;
        wave.paused = true;

        let store = self.store.clone();
        let wave_clone = wave.clone();
        tokio::task::spawn_blocking(move || store.update_wave(&wave_clone))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?;

        Ok(Response::new(StopWaveResponse { stopped: true }))
    }

    async fn connect_wave(
        &self,
        _request: Request<ConnectWaveRequest>,
    ) -> Result<Response<ConnectWaveResponse>, Status> {
        Err(Status::unimplemented(
            "interactive session connect not implemented",
        ))
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

    async fn list_step_runs(
        &self,
        _request: Request<ListStepRunsRequest>,
    ) -> Result<Response<ListStepRunsResponse>, Status> {
        let runs = self.list_step_runs_inner().await?;
        Ok(Response::new(ListStepRunsResponse { step_runs: runs }))
    }

    async fn get_step_run_history(
        &self,
        request: Request<crate::proto::control::GetStepRunHistoryRequest>,
    ) -> Result<Response<crate::proto::control::GetStepRunHistoryResponse>, Status> {
        let req = request.into_inner();
        let store = self.store.clone();
        let worktree = req.worktree.clone();
        let repo = req.repo.clone();
        let limit = req.limit;
        let runs = tokio::task::spawn_blocking(move || {
            store.list_step_run_history(worktree.as_deref(), repo.as_deref(), limit)
        })
        .await
        .map_err(|err| Status::internal(format!("store task failed: {err}")))?
        .map_err(Self::store_error)?;

        Ok(Response::new(
            crate::proto::control::GetStepRunHistoryResponse { step_runs: runs },
        ))
    }

    async fn start_step_run(
        &self,
        request: Request<crate::proto::control::StartStepRunRequest>,
    ) -> Result<Response<crate::proto::control::StartStepRunResponse>, Status> {
        let step_run = request
            .into_inner()
            .step_run
            .ok_or_else(|| Status::invalid_argument("missing step_run"))?;
        let id = if step_run.id.is_empty() {
            Uuid::new_v4().to_string()
        } else {
            step_run.id.clone()
        };

        let mut stored = step_run.clone();
        stored.id = id.clone();
        if stored.started_at.is_none() {
            stored.started_at = Some(Self::now_timestamp());
        }

        let store = self.store.clone();
        let stored_clone = stored.clone();
        tokio::task::spawn_blocking(move || store.start_step_run(&stored_clone))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?;

        Ok(Response::new(crate::proto::control::StartStepRunResponse {
            id,
        }))
    }

    async fn end_step_run(
        &self,
        request: Request<EndStepRunRequest>,
    ) -> Result<Response<EndStepRunResponse>, Status> {
        let req = request.into_inner();
        let step_run_id = req.step_run_id.clone();
        let ended_at = Self::now_timestamp().seconds;
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || store.end_step_run(&step_run_id, req.status, ended_at))
            .await
            .map_err(|err| Status::internal(format!("store task failed: {err}")))?
            .map_err(Self::store_error)?;

        Ok(Response::new(EndStepRunResponse {
            id: req.step_run_id,
        }))
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
