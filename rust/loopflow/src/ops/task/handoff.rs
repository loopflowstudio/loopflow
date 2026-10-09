use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{block_on_task, lock_task_pr_mutation, task_error, task_for_checkout, task_store};
use crate::engine::git::{rev_parse, worktree_root};
use crate::engine::worktrees::git_common_dir;
use crate::ops::error::OpsResult;
use crate::work::task::{Task, TaskId};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct DesignHandoff {
    source_task: Option<TaskId>,
    source_issue: Option<String>,
    source_path: PathBuf,
    source_commit: String,
    content_sha256: String,
    content: String,
}

pub(super) fn read_design(caller: &Path, path: &Path) -> OpsResult<DesignHandoff> {
    // Capture before checkout creation clears inherited scratch or changes placement.
    let source_path = caller.join(path).canonicalize()?;
    let root = worktree_root(caller)?.canonicalize()?;
    if !source_path.starts_with(&root) {
        return Err(task_error(
            "--design must name a file in the caller's checkout",
        ));
    }
    let content = fs::read_to_string(&source_path)?;
    let source_task = block_on_task(async {
        let store = task_store().await?;
        task_for_checkout(&store, caller).await
    })?;
    Ok(DesignHandoff {
        source_issue: source_task
            .as_ref()
            .map(|task| task.plan.identifier.clone()),
        source_task: source_task.map(|task| task.id),
        source_path,
        source_commit: rev_parse(caller, "HEAD")?,
        content_sha256: hex::encode(Sha256::digest(content.as_bytes())),
        content,
    })
}

pub(super) fn write_design(task: &Task, design: &DesignHandoff) -> OpsResult<()> {
    let _mutation = lock_task_pr_mutation(task.worktree()?)?;
    // Common metadata survives removal of the child's worktree registration.
    let root = git_common_dir(task.worktree()?)?
        .join("loopflow-design-handoffs")
        .join(task.id.as_str());
    // HEAD may advance between retries; the selected source and bytes identify input.
    let identity = serde_json::to_vec(&(
        &task.id,
        &design.source_task,
        &design.source_path,
        &design.content_sha256,
    ))
    .map_err(task_error)?;
    let receipt_dir = root.join(hex::encode(Sha256::digest(identity)));
    fs::create_dir_all(&receipt_dir)?;
    let receipt = receipt_dir.join("receipt.json");
    if !receipt.exists() {
        let bytes = serde_json::to_vec_pretty(design).map_err(task_error)?;
        write_new(&receipt, &bytes)?;
    }
    let applied = receipt_dir.join("applied");
    if applied.exists() {
        // Even deleted or edited child notes belong to the child after handoff.
        return Ok(());
    }

    let scratch = task.worktree()?.join("scratch");
    match fs::create_dir(&scratch) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    if !fs::symlink_metadata(&scratch)?.file_type().is_dir() {
        return Err(task_error(format!(
            "design handoff conflict: {} is not a directory; selected content retained in {}",
            scratch.display(),
            receipt.display()
        )));
    }
    let destination = scratch.join(format!("{}.md", task.workspace_slug));
    match write_new(&destination, design.content.as_bytes()) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let regular = fs::symlink_metadata(&destination)?.file_type().is_file();
            if !regular || fs::read(&destination)? != design.content.as_bytes() {
                return Err(task_error(format!(
                    "design handoff conflict: {} already contains child work; selected content retained in {}",
                    destination.display(),
                    receipt.display()
                )));
            }
        }
        Err(error) => return Err(error.into()),
    }
    write_new(&applied, b"")?;
    Ok(())
}

// Publish complete bytes without ever truncating an occupied destination.
fn write_new(path: &Path, content: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().expect("handoff paths have a parent");
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(content)?;
    file.as_file().sync_all()?;
    file.persist_noclobber(path).map_err(|error| error.error)?;
    Ok(())
}
