//! Resolve recorded Work subjects before filtering Runs or rendering Activity.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::durable::WorkRef;
use crate::lf::commands::WorkFilter;
use crate::run_record::RunSnapshot;
use crate::store::sqlite::{SqliteStore, WorkIdentity};

#[derive(Debug, Default)]
pub(crate) struct WorkCatalog {
    pub(crate) owners: HashMap<WorkRef, WorkOwner>,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkOwner {
    pub(crate) work: WorkRef,
    pub(crate) subject: String,
    pub(crate) selectors: Vec<String>,
    pub(crate) created_at: Option<i64>,
}

impl WorkOwner {
    pub(crate) fn matches(&self, filter: WorkFilter<'_>) -> bool {
        [
            ("wave", filter.wave),
            ("project", filter.project),
            ("task", filter.task),
        ]
        .into_iter()
        .all(|(kind, value)| value.is_none_or(|value| self.has_subject(kind, value)))
    }

    fn has_subject(&self, kind: &str, value: &str) -> bool {
        (self.work.kind() == kind && self.work.id() == value)
            || self
                .selectors
                .iter()
                .any(|selector| selector.split_once(':') == Some((kind, value)))
    }
}

impl WorkCatalog {
    pub(crate) fn load() -> Result<Self> {
        Self::load_at(&crate::store::observability_database_path()?)
    }

    pub(crate) fn load_at(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let store = SqliteStore::open_run_ledger_read_only(path)?;
        Self::new(store.work_identities()?)
    }

    pub(crate) fn new(identities: Vec<WorkIdentity>) -> Result<Self> {
        let mut catalog = Self::default();
        // The store returns parents before their children.
        for identity in identities {
            let mut selectors = match &identity.parent {
                Some(parent) => catalog
                    .owners
                    .get(parent)
                    .ok_or_else(|| {
                        anyhow!(
                            "{}:{} has no owning {}:{}",
                            identity.work.kind(),
                            identity.work.id(),
                            parent.kind(),
                            parent.id()
                        )
                    })?
                    .selectors
                    .clone(),
                None => Vec::new(),
            };
            selectors.extend([
                format!("{}:{}", identity.work.kind(), identity.work.id()),
                format!("{}:{}", identity.work.kind(), identity.subject),
            ]);
            if let Some(external_id) = identity.external_id {
                selectors.push(format!("{}:{external_id}", identity.work.kind()));
            }
            catalog.owners.insert(
                identity.work.clone(),
                WorkOwner {
                    work: identity.work,
                    subject: identity.subject,
                    selectors,
                    created_at: identity.created_at,
                },
            );
        }
        Ok(catalog)
    }

    pub(crate) fn resolve_run(&self, run: &RunSnapshot) -> Option<&WorkOwner> {
        let kind = ["task", "project", "wave"]
            .into_iter()
            .find(|kind| run.subject(kind).is_some())?;
        let subject = run.subject(kind)?;
        let candidates = || {
            self.owners
                .values()
                .filter(|owner| owner.work.kind() == kind && owner.has_subject(kind, subject))
        };
        let mut matches = candidates();
        let owner = matches.next()?;
        if matches.next().is_none() {
            // A resolved Work keeps its Runs when an ancestor's label changes.
            return Some(owner);
        }
        // Shared names need the recorded ancestry to select one exact Work.
        let mut matches = candidates().filter(|owner| {
            owner.matches(WorkFilter {
                wave: run.subject("wave"),
                project: run.subject("project"),
                task: run.subject("task"),
            })
        });
        let owner = matches.next()?;
        matches.next().is_none().then_some(owner)
    }

    pub(crate) fn matches_run(&self, run: &RunSnapshot, filter: WorkFilter<'_>) -> bool {
        match self.resolve_run(run) {
            Some(owner) => owner.matches(filter),
            None => filter.matches(
                run.subject("wave"),
                run.subject("project"),
                run.subject("task"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkCatalog, WorkOwner};
    use crate::durable::{ProjectId, WorkRef};
    use crate::run_record::{RunSnapshot, SubjectAttribution};

    #[test]
    fn shared_project_names_need_recorded_ancestry() {
        let mut catalog = WorkCatalog::default();
        for wave in ["product", "infrastructure"] {
            let work = WorkRef::Project(ProjectId::new());
            catalog.owners.insert(
                work.clone(),
                WorkOwner {
                    work,
                    subject: "shared".into(),
                    created_at: None,
                    selectors: vec![format!("wave:{wave}"), "project:shared".into()],
                },
            );
        }
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/wave_detail.json"
        ))
        .unwrap();
        let mut run: RunSnapshot =
            serde_json::from_value(fixture["runs"]["items"][0].clone()).unwrap();
        run.subjects = vec![SubjectAttribution::declared("project:shared".into())];
        assert!(catalog.resolve_run(&run).is_none());
        run.subjects
            .push(SubjectAttribution::declared("wave:product".into()));
        assert!(catalog
            .resolve_run(&run)
            .unwrap()
            .selectors
            .contains(&"wave:product".into()));
    }
}
