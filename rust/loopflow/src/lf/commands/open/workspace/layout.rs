//! Resolve shared pane intent into native tool argv; hosts retain their active geometry.
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::{viewer_argv, Presentation, SqliteStore, ViewContent};
use crate::work::selection::{BrowserTab, ChangeComparison, PaneIntent, PaneTree, SplitAxis};

pub(super) fn block(label: &str, cwd: &Path, argv: Vec<String>) -> Value {
    json!({"block":{"flavor":"com.superlogical.terminal.shell","label":label,"options":{"cwd":cwd,"shell":"none","command":argv}}})
}

pub(super) fn layout(
    store: &SqliteStore,
    content: &ViewContent,
    repo: &Path,
    presentation: Presentation<'_>,
) -> Result<(Value, usize)> {
    if let Some(location) = &content.location {
        anyhow::ensure!(
            location.machine_id == store.local_machine()?.id.as_str(),
            "Task runs on Machine {}; no local tool was created",
            location.machine_id
        );
    }
    if presentation.passive || content.location.is_none() {
        return Ok((task_view(content, repo)?, 0));
    }
    let location = content.location.as_ref().expect("location checked");
    let cwd = Path::new(&location.cwd);
    anyhow::ensure!(
        cwd.is_dir(),
        "Recorded Task checkout is unavailable; no home-directory fallback"
    );
    if presentation.session.is_some() || presentation.diff {
        let conversation = conversation(content, cwd, presentation.session)?;
        return Ok((
            if presentation.diff {
                json!({"split":{"direction":"horizontal","ratio":0.618,"before":conversation,"after":tool(content,cwd,"changes",None,Some(&ChangeComparison::WorkingTree { base: "parent".into() }))?}})
            } else {
                conversation
            },
            0,
        ));
    }
    let saved = store.pane_layout(&content.work)?;
    let mut shells = 0;
    let tree = if let Some(saved) = &saved {
        render_tree(
            &saved.tree,
            &saved.hidden,
            content,
            cwd,
            &mut shells,
            !presentation.omit_shells,
        )?
    } else {
        let conversation = conversation(content, cwd, None)?;
        if presentation.omit_shells {
            return Ok((conversation, 0));
        }
        shells = 1;
        Some(
            json!({"split":{"direction":"horizontal","ratio":0.618,"before":conversation,"after":{"split":{"direction":"vertical","ratio":0.618,"before":task_view(content,cwd)?,"after":tool(content,cwd,"shell",None,None)?}}}}),
        )
    };
    Ok((
        match tree {
            Some(tree) => tree,
            None => task_view(content, cwd)?,
        },
        shells,
    ))
}

fn task_view(content: &ViewContent, cwd: &Path) -> Result<Value> {
    Ok(block(
        "Task & workflow",
        cwd,
        viewer_argv(cwd, &["task", "view", &content.work.id])?,
    ))
}

fn conversation(content: &ViewContent, cwd: &Path, session: Option<&str>) -> Result<Value> {
    match session.or(content.primary_session.as_deref()) {
        Some(session) => Ok(block(
            "Conversation",
            cwd,
            viewer_argv(cwd, &["session", "view", session])?,
        )),
        None => task_view(content, cwd),
    }
}

fn tool(
    content: &ViewContent,
    cwd: &Path,
    name: &str,
    path: Option<&str>,
    comparison: Option<&ChangeComparison>,
) -> Result<Value> {
    let mut args = vec![
        "task",
        "tool",
        &content.work.id,
        name,
        "--checkout",
        cwd.to_str().context("Checkout is not UTF-8")?,
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
    Ok(block(name, cwd, viewer_argv(cwd, &args)?))
}

fn render_tree(
    tree: &PaneTree,
    hidden: &std::collections::BTreeSet<String>,
    content: &ViewContent,
    cwd: &Path,
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
                render_tree(first, hidden, content, cwd, shells, new_shells)?,
                render_tree(second, hidden, content, cwd, shells, new_shells)?,
            ) {
                (Some(before), Some(after)) => Some(
                    json!({"split":{"direction":if *axis == SplitAxis::Vertical {"horizontal"} else {"vertical"},"ratio":ratio,"before":before,"after":after}}),
                ),
                (first, second) => first.or(second),
            }
        }
        PaneTree::Pane { content: pane, .. } => {
            Some(match pane {
                PaneIntent::Session { session_id, .. } => {
                    conversation(content, cwd, session_id.as_deref())?
                }
                PaneIntent::Shell { tabs, selected } => {
                    if !new_shells {
                        return Ok(None);
                    }
                    if let Some(tab) = tabs.iter().find(|tab| Some(&tab.id) == selected.as_ref()) {
                        anyhow::ensure!(tab.location.as_ref().is_none_or(|place| Some(place) == content.location.as_ref()), "Saved shell belongs to another location; it remains in its source host");
                    }
                    *shells += 1;
                    tool(content, cwd, "shell", None, None)?
                }
                PaneIntent::Browser {
                    location,
                    tabs,
                    selected,
                } => {
                    anyhow::ensure!(
                        location
                            .as_ref()
                            .is_none_or(|place| Some(place) == content.location.as_ref()),
                        "Saved Files belong to another location; source state was retained"
                    );
                    match tabs.iter().find(|tab| Some(tab.id()) == selected.as_deref()) {
                        Some(BrowserTab::Files {tabs,selected,..}) => tool(content,cwd,"files",tabs.iter().find(|tab| Some(&tab.id) == selected.as_ref()).map(|tab|tab.path.as_str()),None)?,
                        Some(BrowserTab::Changes {comparison,..}) => tool(content,cwd,"changes",None,Some(comparison))?,
                        Some(BrowserTab::Url {url,..}) => bail!("Web page {url} remains in the source host; no terminal renderer is configured"),
                        _ => task_view(content, cwd)?,
                    }
                }
            })
        }
    })
}
