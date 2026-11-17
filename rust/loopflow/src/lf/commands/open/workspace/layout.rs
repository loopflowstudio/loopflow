//! One Task in a host's native panes: its Task view, beside its changes when
//! `--diff` asks. Hosts own the result; nothing here is persisted.
use std::path::Path;

use anyhow::Result;
use serde_json::Value;

use super::{lf_argv, tool_cwd, Native, SqliteStore, ViewContent};
use loopflow_storage::work::selection::SplitAxis;

/// The native layout for one Task.
pub(super) fn layout(
    store: &SqliteStore,
    content: &ViewContent,
    repo: &Path,
    diff: bool,
    host: &Native,
) -> Result<Value> {
    let cwd = tool_cwd(content, repo);
    let id = content.work.id.as_str();
    let task_view = host.block("Task & workflow", cwd, lf_argv(cwd, &["task", "view", id])?);
    let Some(location) = &content.location else {
        return Ok(task_view);
    };
    anyhow::ensure!(
        location.machine_id
            == loopflow_execution::store::machines::local_machine(store)?
                .id
                .as_str(),
        "Task runs on Machine {}; no local tool was created",
        location.machine_id
    );
    anyhow::ensure!(
        cwd.is_dir(),
        "Recorded Task checkout is unavailable; no home-directory fallback"
    );
    if !diff {
        return Ok(task_view);
    }
    let changes = host.block(
        "Changes",
        cwd,
        lf_argv(cwd, &["task", "diff", id, "--base", "parent"])?,
    );
    Ok(host.split(SplitAxis::Vertical, 0.618, task_view, changes))
}
