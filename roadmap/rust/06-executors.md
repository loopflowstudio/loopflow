# 06: Executor Abstraction

Enable lfd to run agents in containers or Kubernetes in addition to local processes.

## Context

Phase 1 lfd spawns agents as local processes using `~/.claude` credentials.

For self-hosted and hosted deployments, we need:
- Container isolation
- Credential injection
- Resource limits
- Multi-tenant safety

## Goal

1. `AgentExecutor` trait abstracts execution backend
2. `LocalProcessExecutor` - current behavior
3. `ContainerExecutor` - Docker containers
4. `KubernetesExecutor` - K8s Jobs
5. Config selects executor type

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│ lfd                                                                 │
│                                                                     │
│  ┌────────────────────────────────────────────────────────────────┐│
│  │ WaveExecutor                                                   ││
│  │                                                                ││
│  │  tick_flow() ──▶ spawn_agent() ──▶ AgentExecutor               ││
│  │                                           │                    ││
│  │                                           ▼                    ││
│  │                        ┌──────────────────────────────────┐    ││
│  │                        │ LocalProcessExecutor             │    ││
│  │                        │ ContainerExecutor                │    ││
│  │                        │ KubernetesExecutor               │    ││
│  │                        └──────────────────────────────────┘    ││
│  │                                                                ││
│  └────────────────────────────────────────────────────────────────┘│
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Executor Trait

```rust
// rust/lfd/src/executor/mod.rs

#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Spawn an agent to execute a step
    async fn spawn(&self, config: AgentSpawnConfig) -> Result<AgentHandle>;

    /// Check agent status
    async fn status(&self, handle: &AgentHandle) -> Result<AgentStatus>;

    /// Terminate an agent
    async fn terminate(&self, handle: &AgentHandle) -> Result<()>;

    /// Stream agent output
    async fn logs(&self, handle: &AgentHandle) -> Result<LogStream>;

    /// Wait for agent completion
    async fn wait(&self, handle: &AgentHandle) -> Result<i32>;
}

#[derive(Debug, Clone)]
pub struct AgentSpawnConfig {
    pub agent_id: String,
    pub wave_id: String,
    pub step_name: String,
    pub prompt: String,
    pub repo_path: PathBuf,
    pub worktree_path: Option<PathBuf>,
    pub model: AgentModel,
    pub auto_mode: bool,
    pub timeout: Option<Duration>,
    pub env: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct AgentHandle {
    pub id: String,
    pub executor_type: ExecutorType,
    /// Executor-specific identifier (PID, container ID, Job name)
    pub backend_id: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ExecutorType {
    LocalProcess,
    Container,
    Kubernetes,
}

pub type LogStream = Pin<Box<dyn Stream<Item = Bytes> + Send>>;
```

## LocalProcessExecutor

Current behavior - fork and exec:

```rust
// rust/lfd/src/executor/local.rs

pub struct LocalProcessExecutor {
    claude_credentials: PathBuf,
    processes: DashMap<String, Child>,
}

impl LocalProcessExecutor {
    pub fn new() -> Self {
        Self {
            claude_credentials: dirs::home_dir().unwrap().join(".claude"),
            processes: DashMap::new(),
        }
    }
}

#[async_trait]
impl AgentExecutor for LocalProcessExecutor {
    async fn spawn(&self, config: AgentSpawnConfig) -> Result<AgentHandle> {
        let agent_config = loopflow_engine::agent::AgentConfig {
            backend: config.model.backend,
            model: config.model.name,
            prompt: config.prompt,
            working_dir: config.worktree_path.unwrap_or(config.repo_path),
            auto_mode: config.auto_mode,
            streaming: true,
            chrome: false,
            skip_permissions: config.auto_mode,
        };

        let mut child = loopflow_engine::agent::spawn_agent_process(&agent_config)?;
        let pid = child.id().unwrap();

        self.processes.insert(config.agent_id.clone(), child);

        Ok(AgentHandle {
            id: config.agent_id,
            executor_type: ExecutorType::LocalProcess,
            backend_id: pid.to_string(),
        })
    }

    async fn status(&self, handle: &AgentHandle) -> Result<AgentStatus> {
        let child = self.processes.get(&handle.id)
            .ok_or_else(|| anyhow!("process not found"))?;

        match child.try_wait()? {
            Some(status) => {
                if status.success() {
                    Ok(AgentStatus::Completed)
                } else {
                    Ok(AgentStatus::Failed)
                }
            }
            None => Ok(AgentStatus::Running),
        }
    }

    async fn terminate(&self, handle: &AgentHandle) -> Result<()> {
        if let Some(mut child) = self.processes.remove(&handle.id) {
            // Try SIGTERM first
            child.1.kill()?;
        }
        Ok(())
    }

    async fn logs(&self, handle: &AgentHandle) -> Result<LogStream> {
        let child = self.processes.get_mut(&handle.id)
            .ok_or_else(|| anyhow!("process not found"))?;

        let stdout = child.stdout.take()
            .ok_or_else(|| anyhow!("stdout already taken"))?;

        let stream = ReaderStream::new(BufReader::new(stdout))
            .map(|r| r.map(Bytes::from).unwrap_or_default());

        Ok(Box::pin(stream))
    }

    async fn wait(&self, handle: &AgentHandle) -> Result<i32> {
        let mut child = self.processes.remove(&handle.id)
            .ok_or_else(|| anyhow!("process not found"))?.1;

        let status = child.wait().await?;
        Ok(status.code().unwrap_or(-1))
    }
}
```

## ContainerExecutor

Run agents in Docker containers:

```rust
// rust/lfd/src/executor/container.rs

pub struct ContainerExecutor {
    docker: Docker,
    image: String,
    network: Option<String>,
    claude_credentials: PathBuf,
    containers: DashMap<String, String>,  // agent_id -> container_id
}

impl ContainerExecutor {
    pub fn new(config: ContainerConfig) -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()?;

        Ok(Self {
            docker,
            image: config.image.unwrap_or_else(|| "loopflow/agent:latest".to_string()),
            network: config.network,
            claude_credentials: config.claude_credentials,
            containers: DashMap::new(),
        })
    }
}

#[async_trait]
impl AgentExecutor for ContainerExecutor {
    async fn spawn(&self, config: AgentSpawnConfig) -> Result<AgentHandle> {
        // Build container config
        let container_config = ContainerCreateConfig {
            Image: Some(self.image.clone()),
            Cmd: Some(vec![
                "claude".to_string(),
                "--print".to_string(),
                "--model".to_string(),
                config.model.name.clone(),
                "-p".to_string(),
                config.prompt.clone(),
            ]),
            WorkingDir: Some("/workspace".to_string()),
            Env: Some(vec![
                "HOME=/home/agent".to_string(),
            ]),
            HostConfig: Some(HostConfig {
                Binds: Some(vec![
                    // Mount Claude credentials read-only
                    format!("{}:/home/agent/.claude:ro", self.claude_credentials.display()),
                    // Mount repo
                    format!("{}:/workspace:rw", config.repo_path.display()),
                ]),
                NetworkMode: self.network.clone(),
                Memory: Some(4 * 1024 * 1024 * 1024),  // 4GB
                CpuShares: Some(1024),
                ..Default::default()
            }),
            ..Default::default()
        };

        // Create container
        let create_resp = self.docker.create_container(
            Some(CreateContainerOptions {
                name: format!("lfd-agent-{}", config.agent_id),
                platform: None,
            }),
            container_config,
        ).await?;

        let container_id = create_resp.id;

        // Start container
        self.docker.start_container::<String>(&container_id, None).await?;

        self.containers.insert(config.agent_id.clone(), container_id.clone());

        Ok(AgentHandle {
            id: config.agent_id,
            executor_type: ExecutorType::Container,
            backend_id: container_id,
        })
    }

    async fn status(&self, handle: &AgentHandle) -> Result<AgentStatus> {
        let inspect = self.docker.inspect_container(&handle.backend_id, None).await?;

        let state = inspect.state.ok_or_else(|| anyhow!("no state"))?;

        if state.running.unwrap_or(false) {
            Ok(AgentStatus::Running)
        } else {
            let exit_code = state.exit_code.unwrap_or(-1);
            if exit_code == 0 {
                Ok(AgentStatus::Completed)
            } else {
                Ok(AgentStatus::Failed)
            }
        }
    }

    async fn terminate(&self, handle: &AgentHandle) -> Result<()> {
        self.docker.stop_container(&handle.backend_id, Some(StopContainerOptions {
            t: 10,  // 10 second timeout before SIGKILL
        })).await?;

        self.docker.remove_container(&handle.backend_id, None).await?;
        self.containers.remove(&handle.id);

        Ok(())
    }

    async fn logs(&self, handle: &AgentHandle) -> Result<LogStream> {
        let stream = self.docker.logs::<String>(
            &handle.backend_id,
            Some(LogsOptions {
                follow: true,
                stdout: true,
                stderr: true,
                ..Default::default()
            }),
        );

        let stream = stream.map(|r| {
            r.map(|output| Bytes::from(output.into_bytes()))
                .unwrap_or_default()
        });

        Ok(Box::pin(stream))
    }

    async fn wait(&self, handle: &AgentHandle) -> Result<i32> {
        let result = self.docker.wait_container::<String>(&handle.backend_id, None)
            .try_collect::<Vec<_>>().await?;

        let exit_code = result.first()
            .and_then(|r| r.status_code)
            .unwrap_or(-1) as i32;

        // Cleanup
        self.docker.remove_container(&handle.backend_id, None).await?;
        self.containers.remove(&handle.id);

        Ok(exit_code)
    }
}
```

## KubernetesExecutor

Run agents as K8s Jobs:

```rust
// rust/lfd/src/executor/kubernetes.rs

pub struct KubernetesExecutor {
    client: kube::Client,
    namespace: String,
    image: String,
    claude_secret: String,
    service_account: String,
}

impl KubernetesExecutor {
    pub async fn new(config: KubernetesConfig) -> Result<Self> {
        let client = kube::Client::try_default().await?;

        Ok(Self {
            client,
            namespace: config.namespace,
            image: config.image.unwrap_or_else(|| "loopflow/agent:latest".to_string()),
            claude_secret: config.claude_secret,
            service_account: config.service_account.unwrap_or_else(|| "default".to_string()),
        })
    }
}

#[async_trait]
impl AgentExecutor for KubernetesExecutor {
    async fn spawn(&self, config: AgentSpawnConfig) -> Result<AgentHandle> {
        let jobs: Api<Job> = Api::namespaced(self.client.clone(), &self.namespace);

        let job_name = format!("lfd-agent-{}", &config.agent_id[..8]);

        let job = Job {
            metadata: ObjectMeta {
                name: Some(job_name.clone()),
                labels: Some(BTreeMap::from([
                    ("app".to_string(), "loopflow-agent".to_string()),
                    ("wave-id".to_string(), config.wave_id.clone()),
                    ("agent-id".to_string(), config.agent_id.clone()),
                ])),
                ..Default::default()
            },
            spec: Some(JobSpec {
                backoff_limit: Some(0),  // No retries
                ttl_seconds_after_finished: Some(3600),  // Cleanup after 1 hour
                template: PodTemplateSpec {
                    spec: Some(PodSpec {
                        service_account_name: Some(self.service_account.clone()),
                        restart_policy: Some("Never".to_string()),
                        containers: vec![Container {
                            name: "agent".to_string(),
                            image: Some(self.image.clone()),
                            command: Some(vec!["claude".to_string()]),
                            args: Some(vec![
                                "--print".to_string(),
                                "--model".to_string(),
                                config.model.name.clone(),
                                "-p".to_string(),
                                config.prompt.clone(),
                            ]),
                            working_dir: Some("/workspace".to_string()),
                            env: Some(vec![
                                EnvVar {
                                    name: "HOME".to_string(),
                                    value: Some("/home/agent".to_string()),
                                    ..Default::default()
                                },
                            ]),
                            volume_mounts: Some(vec![
                                VolumeMount {
                                    name: "claude-credentials".to_string(),
                                    mount_path: "/home/agent/.claude".to_string(),
                                    read_only: Some(true),
                                    ..Default::default()
                                },
                                VolumeMount {
                                    name: "workspace".to_string(),
                                    mount_path: "/workspace".to_string(),
                                    ..Default::default()
                                },
                            ]),
                            resources: Some(ResourceRequirements {
                                requests: Some(BTreeMap::from([
                                    ("memory".to_string(), Quantity("512Mi".to_string())),
                                    ("cpu".to_string(), Quantity("500m".to_string())),
                                ])),
                                limits: Some(BTreeMap::from([
                                    ("memory".to_string(), Quantity("4Gi".to_string())),
                                    ("cpu".to_string(), Quantity("2".to_string())),
                                ])),
                            }),
                            ..Default::default()
                        }],
                        volumes: Some(vec![
                            Volume {
                                name: "claude-credentials".to_string(),
                                secret: Some(SecretVolumeSource {
                                    secret_name: Some(self.claude_secret.clone()),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                            Volume {
                                name: "workspace".to_string(),
                                empty_dir: Some(EmptyDirVolumeSource::default()),
                                ..Default::default()
                            },
                        ]),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        };

        jobs.create(&PostParams::default(), &job).await?;

        Ok(AgentHandle {
            id: config.agent_id,
            executor_type: ExecutorType::Kubernetes,
            backend_id: job_name,
        })
    }

    async fn status(&self, handle: &AgentHandle) -> Result<AgentStatus> {
        let jobs: Api<Job> = Api::namespaced(self.client.clone(), &self.namespace);

        let job = jobs.get(&handle.backend_id).await?;
        let status = job.status.unwrap_or_default();

        if status.active.unwrap_or(0) > 0 {
            return Ok(AgentStatus::Running);
        }

        if status.succeeded.unwrap_or(0) > 0 {
            return Ok(AgentStatus::Completed);
        }

        if status.failed.unwrap_or(0) > 0 {
            return Ok(AgentStatus::Failed);
        }

        Ok(AgentStatus::Pending)
    }

    async fn terminate(&self, handle: &AgentHandle) -> Result<()> {
        let jobs: Api<Job> = Api::namespaced(self.client.clone(), &self.namespace);

        jobs.delete(&handle.backend_id, &DeleteParams {
            propagation_policy: Some(PropagationPolicy::Background),
            ..Default::default()
        }).await?;

        Ok(())
    }

    async fn logs(&self, handle: &AgentHandle) -> Result<LogStream> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);

        // Find pod for this job
        let pod_list = pods.list(&ListParams::default()
            .labels(&format!("job-name={}", handle.backend_id))
        ).await?;

        let pod = pod_list.items.first()
            .ok_or_else(|| anyhow!("no pod found for job"))?;

        let pod_name = pod.metadata.name.as_ref()
            .ok_or_else(|| anyhow!("pod has no name"))?;

        let stream = pods.log_stream(pod_name, &LogParams {
            follow: true,
            ..Default::default()
        }).await?;

        Ok(Box::pin(stream.map(|r| r.unwrap_or_default())))
    }

    async fn wait(&self, handle: &AgentHandle) -> Result<i32> {
        let jobs: Api<Job> = Api::namespaced(self.client.clone(), &self.namespace);

        // Watch for completion
        let lp = ListParams::default()
            .fields(&format!("metadata.name={}", handle.backend_id));

        let mut stream = watcher(jobs.clone(), lp).applied_objects().boxed();

        while let Some(job) = stream.try_next().await? {
            let status = job.status.unwrap_or_default();

            if status.succeeded.unwrap_or(0) > 0 {
                return Ok(0);
            }

            if status.failed.unwrap_or(0) > 0 {
                return Ok(1);
            }
        }

        Ok(-1)
    }
}
```

## Configuration

```yaml
# ~/.lf/lfd.yaml

executor:
  # Local processes (default)
  type: local

  # Or: Docker containers
  type: container
  container:
    image: loopflow/agent:latest
    network: loopflow
    claude_credentials: /home/user/.claude

  # Or: Kubernetes Jobs
  type: kubernetes
  kubernetes:
    namespace: loopflow
    image: loopflow/agent:latest
    claude_secret: claude-credentials
    service_account: loopflow-agent
```

## Done When

- [ ] `AgentExecutor` trait defined
- [ ] `LocalProcessExecutor` works (existing behavior)
- [ ] `ContainerExecutor` spawns Docker containers
- [ ] `ContainerExecutor` mounts credentials correctly
- [ ] `KubernetesExecutor` creates Jobs
- [ ] `KubernetesExecutor` mounts Secret for credentials
- [ ] All executors implement `logs()` streaming
- [ ] All executors implement `terminate()`
- [ ] All executors implement `wait()`
- [ ] Config selects executor type
- [ ] Integration tests for each executor

## Dependencies

- Requires: 02-lfd-primary
- Enables: 07-deployment (container/K8s deployment)
