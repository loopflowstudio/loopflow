//! Saved intent and file images for Wave lifecycle effects.
use crate::work::wave::Wave;
use loopflow_model::id::WaveId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: WaveId,
    pub slug: String,
}

impl From<&Wave> for Identity {
    fn from(wave: &Wave) -> Self {
        Self {
            id: wave.id().clone(),
            slug: wave.slug().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderChange {
    pub wave: String,
    pub initiative: String,
    pub title: Option<String>,
    pub done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    /// Repository where the request is retried, even if the old checkout is gone.
    pub repo: String,
    pub source_repo: String,
    pub kind: String,
    pub from: String,
    pub to: String,
    pub target_repo: String,
    pub waves: Vec<Identity>,
    pub destination: Option<WaveId>,
    pub delete_backlog: bool,
    pub providers: Vec<ProviderChange>,
    pub changes: Vec<FileChange>,
    pub planning_done: bool,
    pub applied: bool,
    pub files_done: bool,
}

impl Operation {
    pub fn complete(&self) -> bool {
        self.planning_done && self.files_done && self.providers.iter().all(|provider| provider.done)
    }

    pub fn matches(&self, name: &str, target: Option<&str>) -> bool {
        (self.from == name || self.waves[0].id.as_str() == name)
            && target.is_none_or(|target| self.to == target)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    pub bytes: Vec<u8>,
    pub mode: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub owner: Option<Identity>,
    pub base: Option<Image>,
    pub index: Option<Image>,
    pub before: Option<Image>,
    pub after: Option<Image>,
}
