//! Portable pane intent. Native incarnations, attachments and liveness stay in the host.
use super::{SplitAxis, ViewLocation, WorkWorkspaceKey, WorkWorkspaceKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WorkPaneLayout {
    pub work: WorkWorkspaceKey,
    pub revision: i64,
    pub tree: PaneTree,
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
        axis: SplitAxis,
        ratio: f64,
        first: Box<Self>,
        second: Box<Self>,
    },
}

/// A Task workspace holds exactly one of each designated pane; Wave and repository
/// workspaces hold only the session pane. Arrangement is free; membership is not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PaneIntent {
    /// One conversation at a time; none shows the start state.
    Session {
        session_id: Option<String>,
        location: Option<ViewLocation>,
    },
    Shell {
        tabs: Vec<ShellTab>,
        selected: Option<String>,
    },
    Browser {
        location: Option<ViewLocation>,
        tabs: Vec<BrowserTab>,
        selected: Option<String>,
    },
}

impl PaneIntent {
    pub fn kind(&self) -> PaneKind {
        match self {
            Self::Session { .. } => PaneKind::Session,
            Self::Shell { .. } => PaneKind::Shell,
            Self::Browser { .. } => PaneKind::Browser,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneKind {
    Session,
    Shell,
    Browser,
}

/// A browser tab: the Task's Files view (with its own document tabs), its Graph
/// process log, or a web page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BrowserTab {
    Files {
        id: String,
        tabs: Vec<FileTab>,
        selected: Option<String>,
    },
    GraphLog {
        id: String,
    },
    Url {
        id: String,
        url: String,
    },
}

impl BrowserTab {
    pub fn id(&self) -> &str {
        match self {
            Self::Files { id, .. } | Self::GraphLog { id } | Self::Url { id, .. } => id,
        }
    }
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

    pub fn panes(&self) -> Vec<(&str, &PaneIntent)> {
        match self {
            Self::Pane { id, content } => vec![(id.as_str(), content)],
            Self::Split { first, second, .. } => {
                let mut panes = first.panes();
                panes.extend(second.panes());
                panes
            }
        }
    }

    fn validate_ratios(&self) -> Result<(), String> {
        if let Self::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            if !ratio.is_finite() || !(0.1..=0.9).contains(ratio) {
                return Err("split ratio must be between 0.1 and 0.9".into());
            }
            first.validate_ratios()?;
            second.validate_ratios()?;
        }
        Ok(())
    }
}

impl WorkPaneLayout {
    pub fn validate(&self) -> Result<(), String> {
        self.tree.validate_ratios()?;
        let panes = self.tree.panes();
        let mut ids = BTreeSet::new();
        let mut tabs_seen = BTreeSet::new();
        let mut kinds = Vec::new();
        for (id, content) in &panes {
            if id.is_empty() || !ids.insert(*id) {
                return Err("pane IDs must be nonempty and unique".into());
            }
            kinds.push(content.kind());
            match content {
                PaneIntent::Session {
                    session_id,
                    location,
                } => {
                    if session_id.as_deref() == Some("") {
                        return Err("Session ID must be nonempty".into());
                    }
                    if let Some(location) = location {
                        validate_location(location)?;
                    }
                }
                PaneIntent::Shell { tabs, selected } => {
                    validate_tabs(
                        tabs.iter().map(|tab| tab.id.as_str()),
                        selected.as_deref(),
                        &mut tabs_seen,
                    )?;
                    for tab in tabs {
                        if let Some(location) = &tab.location {
                            validate_location(location)?;
                        }
                    }
                }
                PaneIntent::Browser {
                    location,
                    tabs,
                    selected,
                } => {
                    if let Some(location) = location {
                        validate_location(location)?;
                    }
                    validate_tabs(
                        tabs.iter().map(BrowserTab::id),
                        selected.as_deref(),
                        &mut tabs_seen,
                    )?;
                    let mut singletons = BTreeSet::new();
                    for tab in tabs {
                        match tab {
                            BrowserTab::Files { tabs, selected, .. } => {
                                if !singletons.insert("files") {
                                    return Err("a browser holds one Files tab".into());
                                }
                                validate_tabs(
                                    tabs.iter().map(|tab| tab.id.as_str()),
                                    selected.as_deref(),
                                    &mut tabs_seen,
                                )?;
                                let mut documents = BTreeSet::new();
                                for tab in tabs {
                                    validate_path(&tab.path)?;
                                    if !documents.insert(&tab.path) {
                                        return Err("a document can have only one file tab".into());
                                    }
                                }
                            }
                            BrowserTab::GraphLog { .. } => {
                                if !singletons.insert("graph_log") {
                                    return Err("a browser holds one Graph log tab".into());
                                }
                            }
                            BrowserTab::Url { url, .. } => validate_url(url)?,
                        }
                    }
                }
            }
        }
        kinds.sort();
        let expected: &[PaneKind] = match self.work.kind {
            WorkWorkspaceKind::Task => &[PaneKind::Session, PaneKind::Shell, PaneKind::Browser],
            WorkWorkspaceKind::Wave | WorkWorkspaceKind::Repository => &[PaneKind::Session],
        };
        if kinds != expected {
            return Err("a Task workspace holds one session, shell and browser pane; Wave and repository workspaces hold one session pane".into());
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

fn validate_url(url: &str) -> Result<(), String> {
    let host = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .and_then(|rest| rest.split(['/', '?', '#']).next());
    if host.is_some_and(|host| !host.is_empty() && !host.contains(char::is_whitespace)) {
        Ok(())
    } else {
        Err("browser pages must be http or https URLs".into())
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
