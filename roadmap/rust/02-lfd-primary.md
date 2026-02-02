# 02: lfd as Primary Execution Path

Wire loopflow-engine into lfd so the daemon actually executes waves.

## Context

lfd exists with:
- gRPC API (40+ methods)
- Storage backends (SQLite, Postgres)
- Background loops (loop, watch, cron, recovery)
- Scheduler for slot management

But these don't actually execute anything. The loops poll but don't drive real agent execution.

Meanwhile, loopflow-engine has `tick_flow()` for flow execution but nothing uses it in lfd.

## Goal

lfd becomes the execution engine for waves:
1. `RunWave` RPC triggers `tick_flow()` from loopflow-engine
2. Background loops (loop, watch, cron) drive real flow execution
3. Agent lifecycle tracked via `StartAgent`/`EndAgent`
4. Output streaming works via `StreamOutput`
5. Interactive steps pause, `ConnectWave` resumes with PTY

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│ lfd                                                                 │
│                                                                     │
│  ┌──────────────┐     ┌──────────────────────────────────────────┐ │
│  │ gRPC Server  │────▶│ Wave Executor                            │ │
│  │              │     │                                          │ │
│  │ RunWave ─────┼────▶│  tick_flow() from loopflow-engine        │ │
│  │ StopWave ────┼────▶│                                          │ │
│  │ ConnectWave ─┼────▶│  launch_agent() → claude/codex/gemini    │ │
│  │              │     │                                          │ │
│  └──────────────┘     │  Output capture → StreamOutput           │ │
│                       │                                          │ │
│  ┌──────────────┐     │  Status updates → Store                  │ │
│  │ Background   │────▶│                                          │ │
│  │ Loops        │     └──────────────────────────────────────────┘ │
│  │              │                                                   │
│  │ loop_ticker  │  Polls waves with STIMULUS_LOOP                  │
│  │ watch_poller │  Polls git for changes on main                   │
│  │ cron_poller  │  Evaluates cron schedules                        │
│  │ recovery     │  Cleans up stuck agents                          │
│  └──────────────┘                                                   │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Implementation

### Wave Executor

```rust
// rust/lfd/src/execution/mod.rs

pub struct WaveExecutor {
    store: SharedStore,
    scheduler: Arc<Scheduler>,
    output_channels: DashMap<String, broadcast::Sender<Bytes>>,
}

impl WaveExecutor {
    pub async fn run_wave(&self, wave_id: &LfdId) -> Result<()> {
        // Load wave from store
        let wave = self.store.get_wave(wave_id)?
            .ok_or_else(|| anyhow!("wave not found"))?;

        // Acquire scheduler slot
        if !self.scheduler.acquire(&wave.id) {
            return Err(anyhow!("no slots available"));
        }

        // Update status
        self.store.update_wave_status(wave_id, WaveStatus::WaveRunning)?;

        // Build WaveRun from Wave
        let mut run = self.build_wave_run(&wave)?;

        // Execute
        let result = self.execute_flow(&mut run).await;

        // Release slot
        self.scheduler.release(&wave.id);

        // Update final status
        match &result {
            Ok(_) => {
                self.store.update_wave_status(wave_id, WaveStatus::WaveIdle)?;
                self.store.reset_consecutive_failures(wave_id)?;
            }
            Err(e) => {
                self.store.update_wave_error(wave_id, &e.to_string())?;
                self.store.increment_consecutive_failures(wave_id)?;
            }
        }

        result
    }

    async fn execute_flow(&self, run: &mut WaveRun) -> Result<()> {
        loop {
            let store_adapter = StoreAdapter::new(self.store.clone());

            match loopflow_engine::runtime::tick_flow(run, &store_adapter)? {
                TickResult::Continue => continue,
                TickResult::WaitingForAgent => {
                    // Agent is running, wait for completion
                    self.wait_for_current_agent(run).await?;
                }
                TickResult::WaitingForConnect => {
                    // Interactive step - pause execution
                    self.store.update_wave_status(
                        &run.id.parse()?,
                        WaveStatus::WaveWaiting
                    )?;
                    return Ok(());
                }
                TickResult::Completed => return Ok(()),
                TickResult::Failed(e) => return Err(anyhow!(e)),
            }
        }
    }

    fn build_wave_run(&self, wave: &Wave) -> Result<WaveRun> {
        let repo = PathBuf::from(&wave.repo);
        let flow = loopflow_engine::flow::load_flow(&repo, &wave.flow)?;

        Ok(WaveRun {
            id: wave.id.clone(),
            flow,
            directions: wave.directions.clone(),
            areas: wave.areas.iter().map(PathBuf::from).collect(),
            repo,
            status: WaveRunStatus::Running,
            step_index: 0,
            worktree: None,
            current_step: None,
            error: None,
        })
    }
}
```

### Agent Spawning with Output Capture

```rust
// rust/lfd/src/execution/spawner.rs

impl WaveExecutor {
    async fn spawn_agent(&self, step: &FlowItem, run: &WaveRun) -> Result<String> {
        // Create agent record
        let agent_id = LfdId::new().to_string();
        let agent = Agent {
            id: agent_id.clone(),
            wave_id: run.id.clone(),
            step: step.name().to_string(),
            status: AgentStatus::AgentRunning as i32,
            started_at: Utc::now().timestamp(),
            ended_at: None,
            exit_code: None,
        };
        self.store.create_agent(&agent)?;

        // Create output channel
        let (tx, _) = broadcast::channel(1024);
        self.output_channels.insert(agent_id.clone(), tx.clone());

        // Gather context
        let config = loopflow_engine::config::load_config(&run.repo)?;
        let context = loopflow_engine::prompt::gather_context(&ContextConfig {
            repo: &run.repo,
            areas: &run.areas,
            directions: &run.directions,
            diff: true,
            clipboard: false,
            token_budget: config.token_budget,
        })?;

        // Format prompt
        let prompt = loopflow_engine::prompt::format_prompt(&context, step)?;

        // Build command
        let agent_config = AgentConfig {
            backend: config.backend,
            model: config.agent_model,
            prompt,
            working_dir: run.repo.clone(),
            auto_mode: !step.is_interactive(),
            streaming: true,
            chrome: config.chrome,
            skip_permissions: true,  // Daemon runs in auto mode
        };

        // Spawn process
        let mut child = loopflow_engine::agent::spawn_agent_process(&agent_config)?;

        // Stream output in background
        let stdout = child.stdout.take().expect("stdout piped");
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut buf = vec![0u8; 4096];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => { let _ = tx_clone.send(Bytes::copy_from_slice(&buf[..n])); }
                    Err(_) => break,
                }
            }
        });

        // Wait for completion in background
        let store = self.store.clone();
        let aid = agent_id.clone();
        tokio::spawn(async move {
            let status = child.wait().await;
            let exit_code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
            let _ = store.end_agent(&aid.parse().unwrap(), exit_code);
        });

        Ok(agent_id)
    }

    pub fn subscribe_output(&self, agent_id: &str) -> Result<broadcast::Receiver<Bytes>> {
        self.output_channels
            .get(agent_id)
            .map(|tx| tx.subscribe())
            .ok_or_else(|| anyhow!("agent not found"))
    }
}
```

### Store Adapter

Bridge lfd's Store to loopflow-engine's RunStore:

```rust
// rust/lfd/src/execution/store_adapter.rs

pub struct StoreAdapter {
    store: SharedStore,
}

impl StoreAdapter {
    pub fn new(store: SharedStore) -> Self {
        Self { store }
    }
}

impl loopflow_engine::store::RunStore for StoreAdapter {
    fn get_run(&self, id: &str) -> Result<Option<WaveRun>> {
        let wave = self.store.get_wave(&id.parse()?)?;
        wave.map(|w| wave_to_run(&w)).transpose()
    }

    fn update_run(&self, run: &WaveRun) -> Result<()> {
        let wave = run_to_wave(run)?;
        self.store.update_wave(&wave)
    }

    fn create_agent(&self, agent: &loopflow_engine::runtime::Agent) -> Result<()> {
        let proto = engine_agent_to_proto(agent);
        self.store.create_agent(&proto)
    }

    fn list_fork_runs(&self, run_id: &str) -> Result<Vec<ForkRun>> {
        self.store.list_fork_runs(run_id)
    }

    fn upsert_fork_run(&self, fork_run: &ForkRun) -> Result<()> {
        self.store.upsert_fork_run(fork_run)
    }

    fn delete_fork_runs(&self, run_id: &str) -> Result<()> {
        self.store.delete_fork_runs(run_id)
    }
}
```

### Background Loop Integration

Update loop_ticker to actually execute:

```rust
// rust/lfd/src/loops/loop_ticker.rs

pub fn spawn_loop_ticker(
    store: SharedStore,
    executor: Arc<WaveExecutor>,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("loop_ticker shutting down");
                    break;
                }
                _ = interval.tick() => {
                    tick_loop_waves(&store, &executor).await;
                }
            }
        }
    })
}

async fn tick_loop_waves(store: &SharedStore, executor: &WaveExecutor) {
    // Find IDLE waves with STIMULUS_LOOP that aren't paused
    let stimuli = match store.list_stimuli_by_kind(StimulusKind::StimulusLoop as i32) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to list loop stimuli");
            return;
        }
    };

    for stimulus in stimuli {
        if !stimulus.enabled {
            continue;
        }

        let wave_id = match LfdId::parse(&stimulus.wave_id) {
            Ok(id) => id,
            Err(_) => continue,
        };

        let wave = match store.get_wave(&wave_id) {
            Ok(Some(w)) => w,
            _ => continue,
        };

        // Only run IDLE waves
        if wave.status != WaveStatus::WaveIdle as i32 || wave.paused {
            continue;
        }

        // Check failure threshold
        if wave.consecutive_failures >= 3 {
            tracing::warn!(wave_id = %wave.id, "wave paused due to consecutive failures");
            continue;
        }

        // Run wave (non-blocking)
        let executor = executor.clone();
        tokio::spawn(async move {
            if let Err(e) = executor.run_wave(&wave_id).await {
                tracing::error!(wave_id = %wave_id, error = %e, "loop execution failed");
            }
        });
    }
}
```

### Watch Poller with Real Activation

```rust
// rust/lfd/src/loops/watch.rs

async fn check_watch_stimuli(store: &SharedStore, executor: &WaveExecutor) {
    let stimuli = match store.list_stimuli_by_kind(StimulusKind::StimulusWatch as i32) {
        Ok(s) => s,
        Err(_) => return,
    };

    for stimulus in stimuli {
        if !stimulus.enabled {
            continue;
        }

        let wave_id = match LfdId::parse(&stimulus.wave_id) {
            Ok(id) => id,
            Err(_) => continue,
        };

        let wave = match store.get_wave(&wave_id) {
            Ok(Some(w)) => w,
            _ => continue,
        };

        if wave.paused {
            continue;
        }

        // Check for changes on main
        let current_sha = match get_main_sha(&wave.repo) {
            Ok(sha) => sha,
            Err(_) => continue,
        };

        if stimulus.last_main_sha == current_sha {
            continue;  // No changes
        }

        // Update last_main_sha
        let mut updated_stimulus = stimulus.clone();
        updated_stimulus.last_main_sha = current_sha.clone();
        let _ = store.update_stimulus(&updated_stimulus);

        // If wave is IDLE, run it
        if wave.status == WaveStatus::WaveIdle as i32 {
            let executor = executor.clone();
            tokio::spawn(async move {
                if let Err(e) = executor.run_wave(&wave_id).await {
                    tracing::error!(wave_id = %wave_id, error = %e, "watch execution failed");
                }
            });
        } else {
            // Wave is busy, queue activation
            let stimulus_id = LfdId::parse(&stimulus.id).unwrap();
            queue_pending_activation(store, &wave_id, &stimulus_id, &stimulus.last_main_sha, &current_sha);
        }
    }
}
```

### Interactive Step Connect

```rust
// rust/lfd/src/server.rs

async fn connect_wave(
    &self,
    request: Request<Streaming<ConnectRequest>>,
) -> Result<Response<Self::ConnectWaveStream>, Status> {
    let mut stream = request.into_inner();

    // First message has wave_id
    let first = stream.next().await
        .ok_or_else(|| Status::invalid_argument("empty stream"))??;
    let wave_id: LfdId = first.wave_id.parse()
        .map_err(|_| Status::invalid_argument("invalid wave_id"))?;

    // Verify wave is WAITING
    let wave = self.store.get_wave(&wave_id)
        .map_err(|e| Status::internal(e.to_string()))?
        .ok_or_else(|| Status::not_found("wave not found"))?;

    if wave.status != WaveStatus::WaveWaiting as i32 {
        return Err(Status::failed_precondition("wave not waiting for connect"));
    }

    // Create PTY for the interactive step
    let working_dir = PathBuf::from(&wave.repo);
    let step_name = wave.current_step.as_ref()
        .ok_or_else(|| Status::internal("no current step"))?;

    let pty_session = self.executor.create_pty_session(&working_dir, step_name)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    let (tx, rx) = mpsc::channel(32);

    // PTY reader → client
    let mut pty_reader = pty_session.reader;
    let tx_out = tx.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 4096];
        loop {
            match pty_reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let _ = tx_out.send(Ok(ConnectResponse {
                        data: buf[..n].to_vec(),
                    })).await;
                }
                Err(_) => break,
            }
        }
    });

    // Client → PTY writer
    let mut pty_writer = pty_session.writer;
    tokio::spawn(async move {
        while let Some(Ok(req)) = stream.next().await {
            if !req.input.is_empty() {
                let _ = pty_writer.write_all(&req.input);
            }
        }
    });

    // When PTY exits, resume wave execution
    let executor = self.executor.clone();
    let exit_rx = pty_session.exit_rx;
    tokio::spawn(async move {
        let _ = exit_rx.await;
        // Resume wave execution
        let _ = executor.run_wave(&wave_id).await;
    });

    Ok(Response::new(ReceiverStream::new(rx)))
}
```

### gRPC StreamOutput

```rust
async fn stream_output(
    &self,
    request: Request<StreamOutputRequest>,
) -> Result<Response<Self::StreamOutputStream>, Status> {
    let agent_id = request.into_inner().agent_id;

    let rx = self.executor.subscribe_output(&agent_id)
        .map_err(|e| Status::not_found(e.to_string()))?;

    let stream = BroadcastStream::new(rx)
        .filter_map(|result| async move {
            result.ok().map(|bytes| Ok(StreamOutputResponse {
                data: bytes.to_vec(),
            }))
        });

    Ok(Response::new(Box::pin(stream)))
}
```

## Testing

```rust
#[tokio::test]
async fn test_wave_execution_completes() {
    let store = create_test_store();
    let executor = WaveExecutor::new(store.clone(), Scheduler::new(4));

    // Create wave with simple flow
    let wave = create_test_wave(&store, "ship");

    // Mock the agent execution to complete immediately
    // (or use a test flow that doesn't spawn real agents)

    executor.run_wave(&wave.id.parse().unwrap()).await.unwrap();

    let updated = store.get_wave(&wave.id.parse().unwrap()).unwrap().unwrap();
    assert_eq!(updated.status, WaveStatus::WaveIdle as i32);
}

#[tokio::test]
async fn test_interactive_step_pauses() {
    let store = create_test_store();
    let executor = WaveExecutor::new(store.clone(), Scheduler::new(4));

    // Create wave with interactive step
    let wave = create_test_wave(&store, "design");  // design is interactive

    executor.run_wave(&wave.id.parse().unwrap()).await.unwrap();

    let updated = store.get_wave(&wave.id.parse().unwrap()).unwrap().unwrap();
    assert_eq!(updated.status, WaveStatus::WaveWaiting as i32);
}

#[tokio::test]
async fn test_loop_stimulus_triggers() {
    let store = create_test_store();
    let executor = WaveExecutor::new(store.clone(), Scheduler::new(4));

    // Create wave with loop stimulus
    let wave = create_test_wave(&store, "ship");
    create_loop_stimulus(&store, &wave.id);

    // Run the loop ticker once
    tick_loop_waves(&store, &executor).await;

    // Wave should be running
    let updated = store.get_wave(&wave.id.parse().unwrap()).unwrap().unwrap();
    assert_eq!(updated.status, WaveStatus::WaveRunning as i32);
}
```

## Done When

- [ ] `RunWave` RPC executes flows via `tick_flow()`
- [ ] Agent processes spawn with proper config
- [ ] `StreamOutput` streams agent stdout/stderr
- [ ] `StopWave` terminates running agents (SIGTERM, then SIGKILL)
- [ ] `ConnectWave` provides PTY for interactive steps
- [ ] Wave resumes after interactive step completes
- [ ] Loop ticker runs waves with `STIMULUS_LOOP`
- [ ] Watch poller triggers on main branch changes
- [ ] Cron poller triggers on schedule
- [ ] Agent lifecycle tracked (start, running, completed/failed)
- [ ] Wave status transitions correctly
- [ ] Fork steps run branches (parallel or sequential)
- [ ] Consecutive failure tracking pauses waves
- [ ] Pending activations coalesce during busy waves
- [ ] Integration tests pass

## Dependencies

- loopflow-engine (tick_flow, launch_agent, gather_context)
- Enables: waves actually work, lfd becomes useful
