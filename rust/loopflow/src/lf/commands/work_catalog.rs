//! The Work hierarchy Activity renders and filters by.

use std::collections::HashMap;
use std::path::Path;
use crate::store::SqliteStore;

use anyhow::{anyhow, Result};

use crate::durable::WorkRef;
use crate::lf::commands::WorkFilter;
use crate::store::sqlite::WorkIdentity;

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
        let store = SqliteStore::open_read_only(path)?;
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
}
