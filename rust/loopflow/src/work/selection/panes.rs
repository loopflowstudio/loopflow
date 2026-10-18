//! Portable pane intent. Native incarnations, attachments and liveness stay in the host.
use super::{ArrangementAxis, ViewLocation, WorkWorkspaceKey, WorkWorkspaceKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkPaneLayout {
    pub work: WorkWorkspaceKey,
    pub revision: i64,
    pub tree: Option<PaneTree>,
    pub focused: Option<String>,
    pub zoomed: Option<String>,
    pub hidden: BTreeSet<String>,
    pub drafts: Vec<FileDraft>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PaneTree {
    Pane {
        id: String,
        content: PaneIntent,
    },
    Split {
        axis: ArrangementAxis,
        ratio: f64,
        first: Box<Self>,
        second: Box<Self>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum PaneIntent {
    Empty,
    Session {
        session_id: String,
        location: Option<ViewLocation>,
    },
    Shell {
        tabs: Vec<ShellTab>,
        selected: String,
    },
    Files {
        location: Option<ViewLocation>,
        tabs: Vec<FileTab>,
        selected: Option<String>,
    },
    Log {
        location: Option<ViewLocation>,
    },
}

/// A retained shell identity, independent of its tool pane and current selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShellTab {
    pub id: String,
    pub location: Option<ViewLocation>,
    #[schemars(with = "Option<String>")]
    pub lf_process_id: Option<crate::id::LfProcessId>,
}

/// References the Task's document owner; drafts are stored once on WorkPaneLayout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileTab {
    pub id: String,
    pub path: String,
}

/// Recovery data, not permission to overwrite a file. The host compares the base on reopen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileDraft {
    pub location: ViewLocation,
    pub path: String,
    pub base_revision: String,
    pub base_content: String,
    pub text: String,
}

impl PaneTree {
    pub fn pane(&self, target: &str) -> Option<&PaneIntent> {
        match self {
            Self::Pane { id, content } => (id == target).then_some(content),
            Self::Split { first, second, .. } => first.pane(target).or_else(|| second.pane(target)),
        }
    }
}

impl WorkPaneLayout {
    pub fn validate(&self) -> Result<(), String> {
        fn visit<'a>(
            tree: &'a PaneTree,
            ids: &mut BTreeSet<&'a str>,
            tabs_seen: &mut BTreeSet<&'a str>,
            tools: &mut BTreeSet<&'static str>,
            task: bool,
        ) -> Result<(), String> {
            match tree {
                PaneTree::Pane { id, content } => {
                    if id.is_empty() || !ids.insert(id.as_str()) {
                        return Err("pane IDs must be nonempty and unique".into());
                    }
                    match content {
                        PaneIntent::Session {
                            session_id,
                            location,
                        } => {
                            if session_id.is_empty() {
                                return Err("Session ID must be nonempty".into());
                            }
                            if let Some(location) = location {
                                validate_location(location)?;
                            }
                        }
                        PaneIntent::Shell { tabs, selected } => {
                            if !task || !tools.insert("shell") {
                                return Err(
                                    "shells require one Shell pane in a Task workspace".into()
                                );
                            }
                            validate_tabs(
                                tabs.iter().map(|tab| tab.id.as_str()),
                                Some(selected),
                                tabs_seen,
                            )?;
                            for tab in tabs {
                                if let Some(location) = &tab.location {
                                    validate_location(location)?;
                                }
                            }
                        }
                        PaneIntent::Files {
                            location,
                            tabs,
                            selected,
                        } => {
                            if !task || !tools.insert("files") {
                                return Err(
                                    "files require one Files pane in a Task workspace".into()
                                );
                            }
                            validate_tabs(
                                tabs.iter().map(|tab| tab.id.as_str()),
                                selected.as_deref(),
                                tabs_seen,
                            )?;
                            if let Some(location) = location {
                                validate_location(location)?;
                            }
                            let mut documents = BTreeSet::new();
                            for tab in tabs {
                                validate_path(&tab.path)?;
                                if !documents.insert(&tab.path) {
                                    return Err("a document can have only one file tab".into());
                                }
                            }
                        }
                        PaneIntent::Log {
                            location: Some(location),
                        } => validate_location(location)?,
                        PaneIntent::Empty | PaneIntent::Log { location: None } => {}
                    }
                }
                PaneTree::Split {
                    ratio,
                    first,
                    second,
                    ..
                } => {
                    if !ratio.is_finite() || !(0.1..=0.9).contains(ratio) {
                        return Err("split ratio must be between 0.1 and 0.9".into());
                    }
                    visit(first, ids, tabs_seen, tools, task)?;
                    visit(second, ids, tabs_seen, tools, task)?;
                }
            }
            Ok(())
        }
        let mut ids = BTreeSet::new();
        if let Some(tree) = &self.tree {
            visit(
                tree,
                &mut ids,
                &mut BTreeSet::new(),
                &mut BTreeSet::new(),
                self.work.kind == WorkWorkspaceKind::Task,
            )?;
        }
        if self.hidden.iter().any(|id| !ids.contains(id.as_str()))
            || self
                .focused
                .iter()
                .chain(self.zoomed.iter())
                .any(|id| !ids.contains(id.as_str()) || self.hidden.contains(id))
        {
            return Err("focus, zoom and hidden panes must refer to retained panes; focus and zoom must be visible".into());
        }
        let mut drafts = BTreeSet::new();
        for draft in &self.drafts {
            validate_location(&draft.location)?;
            validate_path(&draft.path)?;
            if draft.base_revision.is_empty()
                || !drafts.insert((&draft.location.machine_id, &draft.location.cwd, &draft.path))
            {
                return Err(
                    "file drafts require a base revision and unique recorded location/path".into(),
                );
            }
        }
        Ok(())
    }
}

fn validate_location(location: &ViewLocation) -> Result<(), String> {
    if location.machine_id.is_empty() || location.cwd.is_empty() {
        return Err("recorded locations require Machine identity and cwd".into());
    }
    Ok(())
}

fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || std::path::Path::new(path)
            .components()
            .any(|part| !matches!(part, std::path::Component::Normal(_)))
    {
        return Err("file paths must be relative to their recorded checkout".into());
    }
    Ok(())
}

fn validate_tabs<'a>(
    tabs: impl Iterator<Item = &'a str>,
    selected: Option<&str>,
    seen: &mut BTreeSet<&'a str>,
) -> Result<(), String> {
    let ids: Vec<_> = tabs.collect();
    if ids.iter().any(|id| id.is_empty() || !seen.insert(*id)) {
        return Err("tab IDs must be nonempty and unique".into());
    }
    if (ids.is_empty() && selected.is_some())
        || (!ids.is_empty() && !selected.is_some_and(|id| ids.contains(&id)))
    {
        return Err("selected tab must name a retained tab; empty tools have no selection".into());
    }
    Ok(())
}
