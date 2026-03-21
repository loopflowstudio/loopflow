use std::path::Path;

use anyhow::Result;
use time::OffsetDateTime;

use crate::engine::flow::ConcreteStep;
use crate::lfd::executor::helpers::build_agent_for_step;
use crate::lfd::id::LfdId;
use crate::lfd::types::{AgentStatus, Event};

use super::WaveExecutor;
use crate::lfd::executor::AgentRunContext;

#[derive(Debug, Clone)]
pub(super) struct AgentLaunchRequest {
    pub wave_id: LfdId,
    pub wave_run_id: LfdId,
    pub branch: Option<String>,
    pub repo: String,
    pub worktree: String,
    pub step: ConcreteStep,
    pub agent: String,
    pub cmd: Vec<String>,
    pub output_prefix: Option<String>,
    pub extra_env: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub(super) struct AgentLaunchOutcome {
    #[allow(dead_code)]
    pub agent_id: LfdId,
    pub exit_code: i32,
}

impl WaveExecutor {
    pub(super) async fn launch_agent(
        &self,
        request: AgentLaunchRequest,
    ) -> Result<AgentLaunchOutcome> {
        let AgentLaunchRequest {
            wave_id,
            wave_run_id,
            branch,
            repo,
            worktree,
            step,
            agent,
            cmd,
            output_prefix,
            extra_env,
        } = request;
        let agent = build_agent_for_step(
            &wave_run_id,
            &repo,
            &worktree,
            &step,
            AgentStatus::Running,
            &agent,
        );
        let agent_id = agent.id.clone();
        self.store.start_agent(&agent).await?;
        self.event_hub.send(Event::agent_started(
            agent_id.clone(),
            step.step.name.clone(),
            worktree.clone(),
        ));

        let exit_code = self
            .runner
            .run(
                cmd,
                Path::new(&worktree),
                AgentRunContext {
                    wave_id: wave_id.to_string(),
                    agent_id: agent_id.to_string(),
                    wave_run_id: wave_run_id.to_string(),
                    branch,
                    output: self.output.clone(),
                    output_prefix,
                    extra_env,
                },
            )
            .await;

        let (status, exit_code) = match exit_code {
            Ok(0) => (AgentStatus::Completed, 0),
            Ok(code) => (AgentStatus::Failed, code),
            Err(err) => {
                self.store
                    .end_agent(
                        &agent_id,
                        AgentStatus::Failed.as_i32(),
                        OffsetDateTime::now_utc().unix_timestamp(),
                    )
                    .await?;
                self.event_hub
                    .send(Event::agent_ended(agent_id.clone(), AgentStatus::Failed));
                return Err(err);
            }
        };
        self.store
            .end_agent(
                &agent_id,
                status.as_i32(),
                OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await?;
        self.event_hub
            .send(Event::agent_ended(agent_id.clone(), status));

        Ok(AgentLaunchOutcome {
            agent_id,
            exit_code,
        })
    }
}
