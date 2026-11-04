//! Resolve shared pane intent into native tool argv; hosts retain their active geometry.
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::{viewer_argv, Presentation, SqliteStore, ViewContent};
use crate::work::selection::{BrowserTab, ChangeComparison, PaneIntent, PaneTree, SplitAxis};

// Render each host directly. Neither host's wire format is shared layout state.
#[derive(Debug, Clone, Copy)]
pub(super) enum Host<'a> {
    Rex(&'a str),
    Herdr,
}

impl Host<'_> {
    pub(super) fn block(self, label: &str, cwd: &Path, mut argv: Vec<String>) -> Value {
        match self {
            Self::Rex(server) => {
                // Rex omits its endpoint from native child environments. Carry it
                // through argv so tools and their shells return to this server.
                argv.splice(0..0, ["env".into(), format!("REX_SERVER={server}")]);
                json!({"block":{"flavor":"com.superlogical.terminal.shell","label":label,"options":{"cwd":cwd,"shell":"none","command":argv}}})
            }
            Self::Herdr => json!({"type":"pane","label":label,"cwd":cwd,"command":argv,"env":{}}),
        }
    }

    fn split(self, axis: SplitAxis, ratio: f64, before: Value, after: Value) -> Value {
        match self {
            Self::Rex(_) => json!({"split":{
                "direction": if axis == SplitAxis::Vertical { "horizontal" } else { "vertical" },
                "ratio": ratio, "before": before, "after": after,
            }}),
            Self::Herdr => json!({"type":"split",
                "direction": if axis == SplitAxis::Vertical { "right" } else { "down" },
                "ratio":ratio,"first":before,"second":after,
            }),
        }
    }
}

struct Tools<'a> {
    host: Host<'a>,
    content: &'a ViewContent,
    cwd: &'a Path,
}

pub(super) fn layout(
    store: &SqliteStore,
    content: &ViewContent,
    repo: &Path,
    presentation: Presentation<'_>,
    host: Host<'_>,
) -> Result<(Value, usize)> {
    let cwd = content
        .location
        .as_ref()
        .map_or(repo, |location| Path::new(&location.cwd));
    let tools = Tools { host, content, cwd };
    let Some(location) = &content.location else {
        return Ok((tools.task_view()?, 0));
    };
    anyhow::ensure!(
        location.machine_id == store.local_machine()?.id.as_str(),
        "Task runs on Machine {}; no local tool was created",
        location.machine_id
    );
    anyhow::ensure!(
        cwd.is_dir(),
        "Recorded Task checkout is unavailable; no home-directory fallback"
    );
    if presentation.passive {
        return Ok((tools.task_view()?, 0));
    }
    if presentation.session.is_some() || presentation.diff {
        let conversation = tools.conversation(presentation.session)?;
        return Ok((
            if presentation.diff {
                host.split(
                    SplitAxis::Vertical,
                    0.618,
                    conversation,
                    tools.tool(
                        "changes",
                        None,
                        Some(&ChangeComparison::WorkingTree {
                            base: "parent".into(),
                        }),
                    )?,
                )
            } else {
                conversation
            },
            0,
        ));
    }
    let Some(saved) = store.pane_layout(&content.work)? else {
        let conversation = tools.conversation(None)?;
        if presentation.omit_shells {
            return Ok((conversation, 0));
        }
        let shell = tools.tool("shell", None, None)?;
        // Without a conversation the main pane already shows Task & workflow.
        let secondary = if content.primary_session.is_some() {
            host.split(SplitAxis::Horizontal, 0.618, tools.task_view()?, shell)
        } else {
            shell
        };
        return Ok((
            host.split(SplitAxis::Vertical, 0.618, conversation, secondary),
            1,
        ));
    };
    let mut shells = 0;
    let tree = tools.render_tree(
        &saved.tree,
        &saved.hidden,
        &mut shells,
        !presentation.omit_shells,
    )?;
    Ok((
        match tree {
            Some(tree) => tree,
            None => tools.task_view()?,
        },
        shells,
    ))
}

impl Tools<'_> {
    fn task_view(&self) -> Result<Value> {
        Ok(self.host.block(
            "Task & workflow",
            self.cwd,
            viewer_argv(self.cwd, &["task", "view", &self.content.work.id])?,
        ))
    }

    fn conversation(&self, session: Option<&str>) -> Result<Value> {
        match session.or(self.content.primary_session.as_deref()) {
            Some(session) => Ok(self.host.block(
                "Conversation",
                self.cwd,
                viewer_argv(self.cwd, &["session", "view", session])?,
            )),
            None => self.task_view(),
        }
    }

    fn tool(
        &self,
        name: &str,
        path: Option<&str>,
        comparison: Option<&ChangeComparison>,
    ) -> Result<Value> {
        let mut args = vec![
            "task",
            "tool",
            &self.content.work.id,
            name,
            "--checkout",
            self.cwd.to_str().context("Checkout is not UTF-8")?,
        ];
        if let Some(path) = path {
            args.extend(["--path", path]);
        }
        if let Some(comparison) = comparison {
            args.extend(["--base", comparison.base()]);
            if let Some(head) = comparison.head() {
                args.extend(["--head", head]);
            }
        }
        Ok(self
            .host
            .block(name, self.cwd, viewer_argv(self.cwd, &args)?))
    }

    fn render_tree(
        &self,
        tree: &PaneTree,
        hidden: &std::collections::BTreeSet<String>,
        shells: &mut usize,
        new_shells: bool,
    ) -> Result<Option<Value>> {
        Ok(match tree {
            PaneTree::Pane { id, .. } if hidden.contains(id) => None,
            PaneTree::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                match (
                    self.render_tree(first, hidden, shells, new_shells)?,
                    self.render_tree(second, hidden, shells, new_shells)?,
                ) {
                    (Some(before), Some(after)) => {
                        Some(self.host.split(*axis, *ratio, before, after))
                    }
                    (first, second) => first.or(second),
                }
            }
            PaneTree::Pane { content: pane, .. } => self.render_pane(pane, shells, new_shells)?,
        })
    }

    fn render_pane(
        &self,
        pane: &PaneIntent,
        shells: &mut usize,
        new_shells: bool,
    ) -> Result<Option<Value>> {
        Ok(Some(match pane {
            PaneIntent::Session { session_id, .. } => self.conversation(session_id.as_deref())?,
            PaneIntent::Shell { tabs, selected } => {
                if !new_shells {
                    return Ok(None);
                }
                if let Some(tab) = tabs.iter().find(|tab| Some(&tab.id) == selected.as_ref()) {
                    anyhow::ensure!(
                        tab.location
                            .as_ref()
                            .is_none_or(|place| Some(place) == self.content.location.as_ref()),
                        "Saved shell belongs to another location; it remains in its source host"
                    );
                }
                *shells += 1;
                self.tool("shell", None, None)?
            }
            PaneIntent::Browser {
                location,
                tabs,
                selected,
            } => {
                anyhow::ensure!(
                    location
                        .as_ref()
                        .is_none_or(|place| Some(place) == self.content.location.as_ref()),
                    "Saved Files belong to another location; source state was retained"
                );
                match tabs
                    .iter()
                    .find(|tab| Some(tab.id()) == selected.as_deref())
                {
                    Some(BrowserTab::Files { tabs, selected, .. }) => {
                        let path = tabs
                            .iter()
                            .find(|tab| Some(&tab.id) == selected.as_ref())
                            .map(|tab| tab.path.as_str());
                        self.tool("files", path, None)?
                    }
                    Some(BrowserTab::Changes { comparison, .. }) => {
                        self.tool("changes", None, Some(comparison))?
                    }
                    Some(BrowserTab::Url { url, .. }) => {
                        bail!("Web page {url} remains in the source host; no terminal renderer is configured")
                    }
                    _ => self.task_view()?,
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::{Host, Tools};
    use crate::work::selection::{ViewContent, ViewLocation, WorkPaneLayout};
    use std::path::Path;

    #[test]
    fn native_hosts_render_selected_tools_and_collapse_hidden_splits() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let saved: WorkPaneLayout = serde_json::from_str(include_str!(
            "../../../../../../../tests/fixtures/dto/pane_layout.json"
        ))
        .unwrap();
        let content = ViewContent {
            work: saved.work.clone(),
            title: "Task".into(),
            states: Default::default(),
            primary_session: None,
            location: Some(ViewLocation {
                machine_id: "machine_recorded".into(),
                cwd: "/repo/task".into(),
            }),
            pane_revision: None,
            face: None,
            argv: None,
            unavailable: vec![],
        };
        for host in [Host::Rex("unix:///task.sock"), Host::Herdr] {
            let tools = Tools {
                host,
                content: &content,
                cwd: Path::new("/repo/task"),
            };
            for (hidden, new_shells, expected_shells) in [
                (saved.hidden.clone(), true, 0),
                (Default::default(), false, 0),
                (Default::default(), true, 1),
            ] {
                let mut shells = 0;
                let native = tools
                    .render_tree(&saved.tree, &hidden, &mut shells, new_shells)
                    .unwrap()
                    .unwrap();
                assert_eq!(shells, expected_shells);
                let (conversation, secondary, files) = match host {
                    Host::Rex(_) => {
                        assert_eq!(native["split"]["direction"], "horizontal");
                        let second = &native["split"]["after"];
                        let files = if shells == 1 {
                            assert_eq!(second["split"]["direction"], "vertical");
                            &second["split"]["after"]
                        } else {
                            second
                        };
                        (
                            &native["split"]["before"]["block"]["options"],
                            second,
                            &files["block"]["options"],
                        )
                    }
                    Host::Herdr => {
                        assert_eq!(native["direction"], "right");
                        let second = &native["second"];
                        let files = if shells == 1 {
                            assert_eq!(second["direction"], "down");
                            &second["second"]
                        } else {
                            second
                        };
                        (&native["first"], second, files)
                    }
                };
                for pane in [conversation, files] {
                    assert_eq!(pane["cwd"], "/repo/task");
                    let command = pane["command"].as_array().unwrap();
                    assert_eq!(
                        command[0] == "env" && command[1] == "REX_SERVER=unix:///task.sock",
                        matches!(host, Host::Rex(_))
                    );
                }
                let viewing = conversation["command"].as_array().unwrap();
                assert!(viewing
                    .windows(3)
                    .any(|args| args == ["session", "view", "session_primary"]));
                let editing = files["command"].as_array().unwrap();
                assert!(editing
                    .windows(2)
                    .any(|args| args == ["--path", "src/lib.rs"]));
                assert!(!secondary.to_string().contains("README.md"));
            }
        }
    }
}
