//! Render one Task's layout into a host's native split format. The source is the
//! Task's saved Desktop layout, else its configured `task_layout`, else the
//! builtin. Hosts own the result; nothing here is persisted.
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::{tool_cwd, viewer_argv, Presentation, SqliteStore, ViewContent};
use crate::transport::local_directory_report;
use crate::work::selection::{
    BrowserTab, ChangeComparison, PaneIntent, PaneTree, SplitAxis, TaskLayoutTemplate,
};

// Render each host directly. No host's wire format is shared layout state.
#[derive(Debug, Clone, Copy)]
pub(super) enum Host<'a> {
    /// A Rex server endpoint, carried into every tool's environment.
    Rex(&'a str),
    Herdr,
    Cmux,
}

impl Host<'_> {
    /// Use the ordinary conversation command in the native terminal.
    pub(super) fn conversation(self, cwd: &Path, words: &[&str]) -> Result<Value> {
        Ok(self.block("Conversation", cwd, viewer_argv(cwd, words)?))
    }

    /// A tool pane. Rex's login wrapper stays in the foreground of a program
    /// without job control and hides its cwd, so the program's report is the
    /// only directory a native split can inherit.
    pub(super) fn block(self, label: &str, cwd: &Path, argv: Vec<String>) -> Value {
        let argv = match (self, local_directory_report(cwd)) {
            (Self::Rex(_), Some(report)) => [
                "/bin/sh".into(),
                "-c".into(),
                r#"printf %s "$1"; shift; exec "$@""#.into(),
                "lf-directory".into(),
                report,
            ]
            .into_iter()
            .chain(argv)
            .collect(),
            _ => argv,
        };
        self.terminal(label, cwd, argv)
    }

    /// The pane with no report. A shell uses this directly: a host reads its
    /// process cwd, and a report made at launch would outlive its first `cd`.
    fn terminal(self, label: &str, cwd: &Path, mut argv: Vec<String>) -> Value {
        match self {
            Self::Rex(server) => {
                // Rex omits its endpoint from native child environments. Carry it
                // through argv so tools and their shells return to this server.
                argv.splice(0..0, ["env".into(), format!("REX_SERVER={server}")]);
                json!({"block":{"flavor":"com.superlogical.terminal.shell","label":label,"options":{"cwd":cwd,"shell":"none","command":argv}}})
            }
            Self::Herdr => json!({"type":"pane","label":label,"cwd":cwd,"command":argv,"env":{}}),
            // cmux types a surface command into a fresh shell. Enter the
            // checkout first and leave that shell behind when the tool exits.
            Self::Cmux => json!({"pane":{"surfaces":[{"type":"terminal","name":label,
                "command":format!("cd {} && {}", shell_quote(&cwd.to_string_lossy()),
                    argv.iter().map(|word| shell_quote(word)).collect::<Vec<_>>().join(" "))}]}}),
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
            Self::Cmux => json!({
                "direction": if axis == SplitAxis::Vertical { "horizontal" } else { "vertical" },
                "split": ratio, "children": [before, after],
            }),
        }
    }
}

fn shell_quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

struct Tools<'a> {
    host: Host<'a>,
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
    host: Host<'_>,
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
        let shell = tools.tool("shell", None, None)?;
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
        self.host.conversation(self.cwd, &words)
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
        let argv = viewer_argv(self.cwd, &args)?;
        Ok(if name == "shell" {
            self.host.terminal(name, self.cwd, argv)
        } else {
            self.host.block(name, self.cwd, argv)
        })
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
                resume: true,
            };
            for (hidden, expected_shells) in [(saved.hidden.clone(), 0), (Default::default(), 1)] {
                let mut shells = 0;
                let native = tools
                    .render_tree(&saved.tree, &hidden, &mut shells)
                    .unwrap()
                    .unwrap();
                assert_eq!(shells, expected_shells);
                let (conversation, secondary, files) = match host {
                    Host::Rex(..) => {
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
                    Host::Cmux => unreachable!("cmux has its own test"),
                };
                let reports = |pane: &serde_json::Value| {
                    pane["command"].as_array().unwrap().iter().any(|word| {
                        let word = word.as_str().unwrap();
                        word.starts_with("\x1b]7;file://") && word.ends_with("/repo/task/\x1b\\")
                    })
                };
                for pane in [conversation, files] {
                    assert_eq!(pane["cwd"], "/repo/task");
                    let command = pane["command"].as_array().unwrap();
                    assert_eq!(
                        command[0] == "env" && command[1] == "REX_SERVER=unix:///task.sock",
                        matches!(host, Host::Rex(..))
                    );
                    assert_eq!(reports(pane), matches!(host, Host::Rex(..)));
                }
                if let (Host::Rex(..), 1) = (host, shells) {
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

    #[test]
    fn cmux_surfaces_enter_the_checkout_with_quoted_argv() {
        let pane = Host::Cmux.terminal(
            "Files",
            Path::new("/repo/it's"),
            vec!["lf".into(), "task".into(), "a b".into()],
        );
        assert_eq!(
            pane["pane"]["surfaces"][0]["command"],
            r"cd '/repo/it'\''s' && 'lf' 'task' 'a b'"
        );
        let split = Host::Cmux.split(
            crate::work::selection::SplitAxis::Vertical,
            0.6,
            pane.clone(),
            pane,
        );
        assert_eq!(split["direction"], "horizontal");
        assert_eq!(split["children"].as_array().unwrap().len(), 2);
    }
}
