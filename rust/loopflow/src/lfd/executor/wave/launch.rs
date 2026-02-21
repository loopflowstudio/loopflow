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
    pub repo: String,
    pub worktree: String,
    pub step: ConcreteStep,
    pub model: String,
    pub cmd: Vec<String>,
    pub output_prefix: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct AgentLaunchOutcome {
    pub agent_id: LfdId,
    pub exit_code: i32,
}

impl WaveExecutor {
    pub(super) async fn launch_agent(
        &self,
        request: AgentLaunchRequest,
    ) -> Result<AgentLaunchOutcome> {
        let agent = build_agent_for_step(
            &request.wave_run_id,
            &request.repo,
            &request.worktree,
            &request.step,
            AgentStatus::Running,
            &request.model,
        );
        let agent_id = agent.id.clone();
        self.store.start_agent(&agent).await?;
        self.event_hub.send(Event::agent_started(
            agent_id.clone(),
            request.step.step.name.clone(),
            request.worktree.clone(),
        ));

        let exit_code = self
            .runner
            .run(
                request.cmd,
                Path::new(&request.worktree),
                AgentRunContext {
                    wave_id: request.wave_id.as_str(),
                    agent_id: agent_id.as_str(),
                    wave_run_id: request.wave_run_id.as_str(),
                    output: &self.output,
                    output_prefix: request.output_prefix.as_deref(),
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
