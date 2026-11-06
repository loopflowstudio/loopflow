//! Render one Task's layout into a host's native split format. The source is the
//! Task's saved Desktop layout, else its configured `task_layout`, else the
//! builtin. Hosts own the result; nothing here is persisted.
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use super::{tool_cwd, viewer_argv, Native, Presentation, SqliteStore, ViewContent};
use crate::work::selection::{
    BrowserTab, ChangeComparison, PaneIntent, PaneTree, SplitAxis, TaskLayoutTemplate,
};

struct Tools<'a> {
    host: &'a Native,
    content: &'a ViewContent,
    cwd: &'a Path,
    resume: bool,
}

/// The native layout for one Task, and how many fresh shells it starts.
pub(super) fn layout(
    store: &SqliteStore,
    content: &ViewContent,
    repo: &Path,
    template: Option<&TaskLayoutTemplate>,
    presentation: Presentation<'_>,
    host: &Native,
) -> Result<(Value, usize)> {
    let cwd = tool_cwd(content, repo);
    let tools = Tools {
        host,
        content,
        cwd,
        resume: presentation.resume,
    };
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
    if presentation.session.is_some() || presentation.diff {
        let conversation = tools.conversation(presentation.session)?;
        return Ok((
            if presentation.diff {
                host.split(
                    SplitAxis::Vertical,
                    0.618,
                    conversation,
                    tools.changes(&ChangeComparison::WorkingTree {
                        base: "parent".into(),
                    })?,
                )
            } else {
                conversation
            },
            0,
        ));
    }
    let source = match store.pane_layout(&content.work)? {
        Some(saved) => Some(saved),
        None => template
            .map(|template| {
                template
                    .instantiate(content.work.clone())
                    .map_err(anyhow::Error::msg)
                    .context("task_layout setting")
            })
            .transpose()?,
    };
    let Some(source) = source else {
        let conversation = tools.conversation(None)?;
        let shell = tools.tool("shell", None)?;
        // Background arrivals show workflow in place of an interactive resume.
        let secondary = if tools.resume {
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
    let tree = tools.render_tree(&source.tree, &source.hidden, &mut shells)?;
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
        if !self.resume {
            return self.task_view();
        }
        let words = match session.or(self.content.primary_session.as_deref()) {
            Some(session) => vec!["resume", session],
            None => vec!["resume", "--task", &self.content.work.id],
        };
        Ok(self
            .host
            .block("Conversation", self.cwd, viewer_argv(self.cwd, &words)?))
    }

    fn tool(&self, name: &str, path: Option<&str>) -> Result<Value> {
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
        let argv = viewer_argv(self.cwd, &args)?;
        Ok(if name == "shell" {
            self.host.terminal(name, self.cwd, argv)
        } else {
            self.host.block(name, self.cwd, argv)
        })
    }

    fn changes(&self, comparison: &ChangeComparison) -> Result<Value> {
        let mut args = vec![
            "task",
            "diff",
            &self.content.work.id,
            "--base",
            comparison.base(),
        ];
        if let Some(head) = comparison.head() {
            args.extend(["--head", head]);
        }
        Ok(self
            .host
            .block("Changes", self.cwd, viewer_argv(self.cwd, &args)?))
    }

    fn render_tree(
        &self,
        tree: &PaneTree,
        hidden: &std::collections::BTreeSet<String>,
        shells: &mut usize,
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
                    self.render_tree(first, hidden, shells)?,
                    self.render_tree(second, hidden, shells)?,
                ) {
                    (Some(before), Some(after)) => {
                        Some(self.host.split(*axis, *ratio, before, after))
                    }
                    (first, second) => first.or(second),
                }
            }
            PaneTree::Pane { content: pane, .. } => self.render_pane(pane, shells)?,
        })
    }

    fn render_pane(&self, pane: &PaneIntent, shells: &mut usize) -> Result<Option<Value>> {
        Ok(Some(match pane {
            PaneIntent::Session { session_id, .. } => self.conversation(session_id.as_deref())?,
            PaneIntent::Shell { tabs, selected } => {
                if let Some(tab) = tabs.iter().find(|tab| Some(&tab.id) == selected.as_ref()) {
                    anyhow::ensure!(
                        tab.location
                            .as_ref()
                            .is_none_or(|place| Some(place) == self.content.location.as_ref()),
                        "Saved shell belongs to another location; it remains in its source host"
                    );
                }
                *shells += 1;
                self.tool("shell", None)?
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
                        self.tool("files", path)?
                    }
                    Some(BrowserTab::Changes { comparison, .. }) => self.changes(comparison)?,
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
    use super::{Native, Tools};
    use crate::lf::commands::open::rex::Rex;
    use crate::work::selection::{ViewContent, ViewLocation, WorkPaneLayout};
    use std::path::Path;

    #[test]
    fn selected_tools_and_hidden_splits_shape_the_task_layout() {
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
        let host = Native::Rex(Rex {
            server: "unix:///task.sock".into(),
            caller: None,
        });
        let tools = Tools {
            host: &host,
            content: &content,
            cwd: Path::new("/repo/task"),
            resume: true,
        };
        for (hidden, expected_shells) in [(saved.hidden.clone(), 0), (Default::default(), 1)] {
            let mut shells = 0;
            let native = tools
                .render_tree(&saved.tree, &hidden, &mut shells)
                .unwrap()
                .unwrap();
            assert_eq!(shells, expected_shells);
            assert_eq!(native["split"]["direction"], "horizontal");
            let secondary = &native["split"]["after"];
            let files = if shells == 1 {
                assert_eq!(secondary["split"]["direction"], "vertical");
                &secondary["split"]["after"]
            } else {
                secondary
            };
            let conversation = &native["split"]["before"]["block"]["options"];
            let files = &files["block"]["options"];
            let reports = |pane: &serde_json::Value| {
                pane["command"].as_array().unwrap().iter().any(|word| {
                    let word = word.as_str().unwrap();
                    word.starts_with("\x1b]7;file://") && word.ends_with("/repo/task/\x1b\\")
                })
            };
            for pane in [conversation, files] {
                assert_eq!(pane["cwd"], "/repo/task");
                let command = pane["command"].as_array().unwrap();
                assert_eq!(command[0], "env");
                assert_eq!(command[1], "REX_SERVER=unix:///task.sock");
                assert!(reports(pane));
            }
            if shells == 1 {
                let shell = &secondary["split"]["before"]["block"]["options"];
                assert!(shell.to_string().contains("\"shell\""));
                assert!(!reports(shell), "a shell's own cwd must follow cd");
            }
            let viewing = conversation["command"].as_array().unwrap();
            assert!(viewing
                .windows(2)
                .any(|args| args == ["resume", "session_primary"]));
            let editing = files["command"].as_array().unwrap();
            assert!(editing
                .windows(2)
                .any(|args| args == ["--path", "src/lib.rs"]));
            assert!(!secondary.to_string().contains("README.md"));
        }
        // Without an existing Session, ordinary resume selects this Task.
        let launch = tools.conversation(None).unwrap().to_string();
        assert!(launch.contains("resume") && launch.contains("--task"));
        // Background arrivals must not move input from an active terminal.
        let background = Tools {
            resume: false,
            ..tools
        };
        let launch = background
            .conversation(Some("session_primary"))
            .unwrap()
            .to_string();
        assert!(launch.contains("Task & workflow") && !launch.contains("resume"));
    }
}
