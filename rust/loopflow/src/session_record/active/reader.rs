use super::{ActiveSession, ActiveSessionsSnapshot, DiscoveryState};
use crate::durable::WorkRef;
use crate::store::SharedStore;
use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

#[derive(Debug)]
pub(crate) struct ActiveSessionReader {
    home: PathBuf,
    cancel: CancellationToken,
}

impl ActiveSessionReader {
    pub(crate) fn start(home: &Path, _continuous: bool, cancel: CancellationToken) -> Result<Self> {
        Ok(Self {
            home: home.to_owned(),
            cancel,
        })
    }
    pub(crate) fn home(&self) -> &Path {
        &self.home
    }
    pub(crate) fn invalidate(&mut self) {}
    pub(crate) async fn observe(
        &mut self,
        store: &SharedStore,
        task: Option<WorkRef>,
    ) -> ActiveSessionsSnapshot {
        let mut result = ActiveSessionsSnapshot {
            home: self.home.clone(),
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            task,
            discovery: DiscoveryState::Ready,
            sessions: Vec::new(),
            gaps: Vec::new(),
        };
        let read = (|| -> Result<()> {
            anyhow::ensure!(
                !self.cancel.is_cancelled(),
                "active Session observation cancelled"
            );
            let agents = store.sqlite.agent_processes()?;
            let os = crate::lf::commands::top::sample_processes(result.observed_at)?;
            for agent in &agents {
                let Some(process) = crate::lf::commands::top::observe_agent_process(&os, agent)
                else {
                    continue;
                };
                let Some(id) = agent.process.agent_session_id.as_deref() else {
                    continue;
                };
                let Some(session) = store.sqlite.session(id)? else {
                    continue;
                };
                let work = session
                    .task_id
                    .clone()
                    .map(WorkRef::Task)
                    .or_else(|| session.wave_id.clone().map(WorkRef::Wave));
                if result
                    .task
                    .as_ref()
                    .is_some_and(|task| Some(task) != work.as_ref())
                {
                    continue;
                }
                if let Some(active) = result.sessions.iter_mut().find(|active| active.id == id) {
                    active.processes.push(process);
                } else {
                    result.sessions.push(ActiveSession {
                        id: id.into(),
                        title: session.title,
                        work,
                        processes: vec![process],
                    });
                }
            }
            if agents != store.sqlite.agent_processes()? {
                result.discovery = DiscoveryState::Scanning;
                result.sessions.clear();
            }
            Ok(())
        })();
        if let Err(error) = read {
            result.discovery = DiscoveryState::Unavailable;
            result.sessions.clear();
            result.gaps.push(error.to_string());
        }
        result
    }
}
