//! Read-only Task views: status snapshot, changed files, diffs and file contents.

use super::{
    block_on_task, comparison_context, completion_pending, completion_refusal, file_context,
    file_save, file_store, owning_wave, repository_for_task, resolve_task_agent,
    task_configuration_refusal, task_error, task_event_process_refusal, task_store,
    task_worktree_blocker, TaskChangedFile, TaskChangesSnapshot, TaskComparison, TaskDiffSnapshot,
    TaskFileSnapshot, TaskFileState, TaskSnapshot, TaskWorkspace,
};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::child::ChildRef;
use crate::config::parse_agent;
use crate::ops::error::{OpsError, OpsResult};
use crate::ops::task_actions::{derive_task_actions, TaskActionEvidence, TaskActionModel};
use crate::store::SharedStore;
use crate::work::task::{PrPhase, Task, TaskPr};
use sha2::{Digest, Sha256};

pub fn task_snapshot(task: &Task) -> OpsResult<TaskSnapshot> {
    block_on_task(async {
        let store = task_store().await?;
        read_task_snapshot(&store, task.clone()).await
    })
}

pub(super) async fn read_task_snapshot(store: &SharedStore, task: Task) -> OpsResult<TaskSnapshot> {
    let wave = owning_wave(store, &task).await?;
    let repo = repository_for_task(store, &task)?;
    let personal =
        crate::config::PersonalSettings::read(Some(&repo), wave.as_ref().map(|w| w.slug()));
    let project = match &task.project_id {
        Some(id) => Some(
            store
                .get_project(id)
                .await
                .map_err(task_error)?
                .ok_or_else(|| task_error(format!("owning Project {id} is missing")))?,
        ),
        None => None,
    };
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .map_err(|error| task_error(format!("failed to resolve Task Work: {error}")))?;
    let latest_event = store
        .latest_task_event(&task.id)
        .await
        .map_err(|error| task_error(format!("failed to read latest task event: {error}")))?;
    let pr = store.active_task_pr(&task.id).await.map_err(task_error)?;
    let active = pr.as_ref().filter(|pr| pr.is_active());
    let work_status = store
        .work_status(&work)
        .await
        .map_err(|error| task_error(format!("failed to derive Task Work status: {error}")))?;
    let execution = crate::ops::task_execution::task_execution(store, &task)
        .await
        .map_err(task_error)?;
    let work_set = store.sqlite.task_work(&task.id).map_err(task_error)?;
    let status = store.task_state(&task.id).await.map_err(task_error)?;
    let actions = if store.sqlite.task_deleted(&task).map_err(task_error)? {
        TaskActionModel {
            recommended: Some(crate::ops::task_actions::TaskAction::NoAction),
            reason: "Task was deleted; retained history is read-only".into(),
        }
    } else if status == crate::durable::TaskState::Deferred {
        TaskActionModel {
            recommended: Some(crate::ops::task_actions::TaskAction::NoAction),
            reason: format!(
                "Task is deferred; `lf task resume {}` clears the hold without starting execution",
                task.identifier
            ),
        }
    } else {
        let predecessor_phase = match task.parent_pr_id.as_ref() {
            Some(parent_id) => store
                .get_task_pr(parent_id)
                .await
                .map_err(|error| task_error(format!("failed to read parent PR: {error}")))?
                .map(|pr| pr.phase()),
            None => None,
        };
        let completion_refusal = completion_refusal(pr.as_ref(), &task.identifier);
        let worktree_blocker = task_worktree_blocker(store, &task).await?;
        let resume_refusal = worktree_blocker
            .as_ref()
            .map(|blocker| blocker.reason.clone());
        let launch_refusal = if worktree_blocker.is_some() {
            None
        } else {
            task_configuration_refusal(&task, wave.as_ref().map(|w| w.slug()), &personal, None)
                .or_else(|| task_event_process_refusal(latest_event.as_ref()).map(str::to_string))
        };
        let action_evidence = TaskActionEvidence {
            status: work_status.clone(),
            execution: Some(&execution),
            latest_pr_phase: pr.as_ref().map(TaskPr::phase),
            latest_pr_merge_request: pr.as_ref().and_then(TaskPr::merge_request),
            latest_pr_presentation_current: pr
                .as_ref()
                .filter(|pr| pr.phase() == PrPhase::Open)
                .map(|pr| pr.presentation().is_some()),
            completion_refusal: completion_refusal.as_deref(),
            resume_refusal: resume_refusal.as_deref(),
            ci: active.and_then(|pr| pr.fresh_ci()),
            predecessor_phase,
            launch_refusal: launch_refusal.as_deref(),
        };
        derive_task_actions(&action_evidence)
    };
    let agent = resolve_task_agent(
        task.worktree.as_deref().unwrap_or(&repo),
        wave.as_ref().map(|w| w.slug()),
        &personal,
        task.agent.as_deref(),
        None,
    );
    let (provider, _) = parse_agent(&agent);
    let machine_id = store
        .task_checkout(&task.id)
        .await
        .map_err(task_error)?
        .and_then(|row| row.machine_id);
    let local_machine = store.local_machine().await.map_err(task_error)?;
    let worktree = task.worktree.as_ref().map(|path| {
        if machine_id.as_ref() == Some(&local_machine.id) {
            crate::git::worktree_root(path)
                .ok()
                .and_then(|root| root.canonicalize().ok())
                .unwrap_or_else(|| path.clone())
        } else {
            path.clone()
        }
    });
    let completion_pending = completion_pending(store, &task)?;
    let starter = |process: Option<&crate::id::LfProcessId>| {
        super::ship::starter(&store.sqlite, process).map_err(task_error)
    };
    let mark = |at, by: Option<&crate::id::LfProcessId>| {
        Ok::<_, OpsError>(super::ShipMark {
            at,
            by: starter(by)?,
        })
    };
    let started_by = store
        .sqlite
        .latest_task_graphs(&[&task.id])
        .map_err(task_error)?
        .get(&task.id)
        .map(|graph| starter(Some(graph.id())))
        .transpose()?;
    let ship_approval = store
        .sqlite
        .task_shipping(&task.id)
        .map_err(task_error)?
        .map(|shipping| mark(shipping.shipped_at, shipping.approved_by.as_ref()))
        .transpose()?;
    let ship_stop = store
        .sqlite
        .task_ship_stop(&task.id)
        .map_err(task_error)?
        .map(|stop| mark(stop.at, stop.by.as_ref()))
        .transpose()?;
    Ok(TaskSnapshot {
        machine_id,
        issue_id: task.linear_id.as_ref().map(|id| id.as_str().to_string()),
        issue_identifier: task.identifier,
        task_id: task.id.to_string(),
        external_project_id: project
            .as_ref()
            .and_then(|p| p.linear_id.as_ref())
            .map(|id| id.as_str().to_string()),
        project: project.map(|p| p.slug),
        pm_snapshot_synced_at: task.pm_snapshot_synced_at,
        pm_writeback: task.pm_writeback,
        wave: wave.as_ref().map(|w| w.slug().to_string()),
        project_id: task.project_id.as_ref().map(ToString::to_string),
        repository_id: task.repository_id.to_string(),
        status,
        completion_pending,
        execution,
        work: work_set,
        worktree: worktree.map(|path| path.display().to_string()),
        workspace_slug: task.workspace_slug,
        branch: task.worktree.as_ref().map(|_| task.branch.clone()),
        base_commit: task.worktree.as_ref().map(|_| task.base_commit.clone()),
        agent: task.agent,
        provider,
        pr,
        latest_event,
        created_at: task.created_at,
        updated_at: task.updated_at,
        observation: task.observation,
        actions,
        started_by,
        ship_approval,
        ship_stop,
    })
}

/// Maximum byte length for Task file content, drafts and returned patches.
pub const MAX_FILE_BYTES: usize = 1_000_000;

pub fn task_changes(issue: &str, base: &str) -> OpsResult<TaskChangesSnapshot> {
    let (checkout, base) = comparison_context(issue, base)?;
    changes_snapshot(TaskComparison {
        checkout: TaskWorkspace::from(&checkout),
        base_commit: &base,
    })
}

pub(super) fn changes_snapshot(workspace: TaskComparison<'_>) -> OpsResult<TaskChangesSnapshot> {
    let mut files = BTreeMap::<String, TaskChangedFile>::new();
    record_changed_paths(
        workspace.checkout.worktree,
        &[
            "diff",
            "--name-only",
            "-z",
            &format!("{}..HEAD", workspace.base_commit),
        ],
        &mut files,
        |file| file.committed = true,
    )?;
    record_changed_paths(
        workspace.checkout.worktree,
        &["diff", "--cached", "--name-only", "-z"],
        &mut files,
        |file| file.staged = true,
    )?;
    record_changed_paths(
        workspace.checkout.worktree,
        &["diff", "--name-only", "-z"],
        &mut files,
        |file| file.unstaged = true,
    )?;
    record_changed_paths(
        workspace.checkout.worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
        &mut files,
        |file| file.untracked = true,
    )?;
    let head_commit = git_output(workspace.checkout.worktree, &["rev-parse", "HEAD"])?
        .trim()
        .to_string();
    let net = net_changed_paths(workspace)?;
    files.retain(|path, file| {
        file.old_path = net.get(path).cloned().flatten();
        net.contains_key(path) || file.untracked
    });
    let (scratch, scratch_truncated) = scratch_paths(workspace.checkout.worktree)?;
    let recovery = file_save::recovery_directory(workspace.checkout)?;
    Ok(TaskChangesSnapshot {
        recovery_directory: recovery
            .exists()
            .then(|| recovery.to_string_lossy().into_owned()),
        issue_identifier: workspace.checkout.issue_identifier.to_string(),
        task_id: workspace.checkout.task_id.to_string(),
        base_commit: workspace.base_commit.to_string(),
        head_commit,
        files: files.into_values().collect(),
        scratch,
        scratch_truncated,
    })
}

pub fn task_diff(
    issue: &str,
    path: Option<&str>,
    base: &str,
    draft: Option<&str>,
) -> OpsResult<TaskDiffSnapshot> {
    let (checkout, base) = comparison_context(issue, base)?;
    let workspace = TaskComparison {
        checkout: TaskWorkspace::from(&checkout),
        base_commit: &base,
    };
    match draft {
        Some(draft) => draft_snapshot(
            workspace,
            path.ok_or_else(|| task_error("Draft diff requires a path"))?,
            draft,
        ),
        None => diff_snapshot(workspace, path),
    }
}

pub(super) fn diff_snapshot(
    workspace: TaskComparison<'_>,
    path: Option<&str>,
) -> OpsResult<TaskDiffSnapshot> {
    let relative = path.map(validate_task_relative_path).transpose()?;
    let old_path = match &relative {
        Some(path) => net_changed_paths(workspace)?.remove(path).flatten(),
        None => None,
    };
    let mut args = vec![
        "diff",
        "--no-ext-diff",
        "--no-color",
        workspace.base_commit,
        "--",
    ];
    // Both sides of a rename are needed for Git to preserve its identity.
    if let Some(old) = &old_path {
        args.push(old);
    }
    if let Some(path) = &relative {
        args.push(path);
    }
    let mut patch = git_output_bytes(workspace.checkout.worktree, &args)?;
    let untracked = untracked_paths(workspace.checkout.worktree)?;
    let include_untracked = untracked
        .into_iter()
        .filter(|candidate| relative.as_ref().is_none_or(|path| path == candidate));
    for path in include_untracked {
        let output = Command::new("git")
            .current_dir(workspace.checkout.worktree)
            .args(["diff", "--no-index", "--no-color", "--", "/dev/null", &path])
            .output()
            .map_err(|error| {
                task_error(format!("failed to diff untracked file {path}: {error}"))
            })?;
        if !output.status.success() && output.status.code() != Some(1) {
            return Err(task_error(format!(
                "failed to diff untracked file {path}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        patch.extend_from_slice(&output.stdout);
    }
    Ok(patch_snapshot(workspace, relative, &patch))
}

pub(crate) fn task_workspace_context(task: &Task) -> OpsResult<String> {
    const MAX_PATCH_TOKENS: usize = 15_000;

    let workspace = TaskComparison::new(task)?;
    let changes = changes_snapshot(workspace)?;
    let diff = diff_snapshot(workspace, None)?;
    // This inventory locates changes; it is not a content receipt. Avoid reading
    // and hashing every file again, especially when the patch is too large.
    let files = changes
        .files
        .iter()
        .map(|file| {
            let states = [
                (file.committed, "committed"),
                (file.staged, "staged"),
                (file.unstaged, "unstaged"),
                (file.untracked, "untracked"),
            ]
            .into_iter()
            .filter_map(|(present, state)| present.then_some(state))
            .collect::<Vec<_>>()
            .join(", ");
            let path = serde_json::to_string(&file.path).expect("a file path is a JSON string");
            format!("{path}: {states}")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let include_patch = !diff.binary
        && !diff.truncated
        && crate::prompt::count_tokens(&diff.patch) < MAX_PATCH_TOKENS;
    let patch = if include_patch {
        diff.patch.as_str()
    } else {
        "Patch omitted from prompt because it is binary, truncated, or exceeds 15,000 tokens. Read the named worktree paths for exact bytes."
    };
    Ok(format!(
        "<lf:task-workspace>\nCheckout base: {}\nCurrent HEAD: {}\nChanges across the checkout base, index, worktree, and untracked files:\n{files}\n\nPatch (included={include_patch}, binary={}, truncated={}):\n{patch}\n</lf:task-workspace>",
        changes.base_commit, changes.head_commit, diff.binary, diff.truncated
    ))
}

pub fn task_file(issue: &str, path: &str, inspect_recovery: bool) -> OpsResult<TaskFileSnapshot> {
    let checkout = file_context(&file_store()?, issue)?;
    let workspace = TaskWorkspace::from(&checkout);
    let mut file = file_snapshot(workspace, path)?;
    if inspect_recovery {
        file.recoveries = file_save::recoveries(workspace, &file.path)?;
    }
    Ok(file)
}

pub(super) fn file_snapshot(
    workspace: TaskWorkspace<'_>,
    path: &str,
) -> OpsResult<TaskFileSnapshot> {
    let relative = validate_task_relative_path(path)?;
    let root = workspace
        .worktree
        .canonicalize()
        .map_err(|error| task_error(format!("cannot resolve Task worktree: {error}")))?;
    let mut snapshot = TaskFileSnapshot {
        recoveries: Vec::new(),
        issue_identifier: workspace.issue_identifier.to_string(),
        task_id: workspace.task_id.to_string(),
        path: relative.clone(),
        content: None,
        state: TaskFileState::Missing,
        revision: None,
        read_only_reason: None,
        size_bytes: 0,
    };
    let absolute = match root.join(&relative).canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(snapshot),
        Err(error) => {
            return Err(task_error(format!(
                "cannot open Task file {relative:?}: {error}"
            )))
        }
    };
    if !absolute.starts_with(&root) || !absolute.is_file() {
        return Err(task_error(format!(
            "Task file {relative:?} does not resolve to a file inside the Task worktree"
        )));
    }
    let mut component_path = root.clone();
    for component in Path::new(&relative).components() {
        component_path.push(component);
        if std::fs::symlink_metadata(&component_path)?
            .file_type()
            .is_symlink()
        {
            snapshot.read_only_reason =
                Some("Files reached through a symlink are read-only.".into());
            break;
        }
    }
    if Path::new(&relative)
        .components()
        .any(|part| part.as_os_str() == ".git")
    {
        snapshot.read_only_reason = Some("Git metadata is read-only.".into());
    }
    let file = File::open(&absolute)
        .map_err(|error| task_error(format!("cannot read Task file {relative:?}: {error}")))?;
    snapshot.size_bytes = file.metadata()?.len();
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    snapshot.state = if bytes.len() > MAX_FILE_BYTES {
        TaskFileState::Truncated
    } else if bytes.contains(&0) {
        TaskFileState::Binary
    } else if let Ok(content) = String::from_utf8(bytes) {
        snapshot.revision = Some(hex::encode(Sha256::digest(content.as_bytes())));
        snapshot.content = Some(content);
        TaskFileState::Text
    } else {
        TaskFileState::UnsupportedEncoding
    };
    Ok(snapshot)
}

pub(super) fn resolve_file_base(
    worktree: &Path,
    parent: &str,
    selection: &str,
) -> OpsResult<String> {
    let reference = match selection {
        "parent" => parent,
        "head" => "HEAD",
        sha if sha.len() == 40 && sha.bytes().all(|byte| byte.is_ascii_hexdigit()) => sha,
        _ => {
            return Err(task_error(
                "Comparison must be parent, head, or a resolved commit SHA",
            ))
        }
    };
    Ok(git_output(
        worktree,
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )?
    .trim()
    .to_string())
}

fn net_changed_paths(workspace: TaskComparison<'_>) -> OpsResult<BTreeMap<String, Option<String>>> {
    let output = git_output_bytes(
        workspace.checkout.worktree,
        &[
            "diff",
            "--name-status",
            "--no-ext-diff",
            "--find-renames",
            "-z",
            workspace.base_commit,
            "--",
        ],
    )?;
    let entries = nul_paths(&output);
    let mut entries = entries.into_iter();
    let mut paths = BTreeMap::new();
    while let Some(status) = entries.next() {
        let source = entries
            .next()
            .ok_or_else(|| task_error("Missing Git path"))?;
        if status.starts_with('R') || status.starts_with('C') {
            let destination = entries
                .next()
                .ok_or_else(|| task_error("Missing Git rename destination"))?;
            paths.insert(destination, Some(source));
        } else {
            paths.insert(source, None);
        }
    }
    Ok(paths)
}

pub(super) fn scratch_paths(root: &Path) -> OpsResult<(Vec<String>, bool)> {
    // Bound traversal as well as returned files. Never descend into symlinks.
    let mut pending = vec![root.join("scratch")];
    let mut paths = Vec::new();
    let mut visited = 0;
    while let Some(directory) = pending.pop() {
        match std::fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            visited += 1;
            if visited > 2_000 {
                paths.sort();
                return Ok((paths, true));
            }
            let kind = entry.file_type()?;
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                paths.push(
                    entry
                        .path()
                        .strip_prefix(root)
                        .expect("scratch is under worktree")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    paths.sort();
    Ok((paths, false))
}

fn patch_snapshot(
    workspace: TaskComparison<'_>,
    path: Option<String>,
    bytes: &[u8],
) -> TaskDiffSnapshot {
    let truncated = bytes.len() > MAX_FILE_BYTES;
    let patch = String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_FILE_BYTES)]).into_owned();
    // Git's binary markers start a line; patch content is always prefixed.
    let binary = patch
        .lines()
        .any(|line| line.starts_with("Binary files ") || line == "GIT binary patch");
    TaskDiffSnapshot {
        issue_identifier: workspace.checkout.issue_identifier.to_string(),
        task_id: workspace.checkout.task_id.to_string(),
        base_commit: workspace.base_commit.to_string(),
        path,
        patch,
        binary,
        truncated,
    }
}

pub(super) fn draft_snapshot(
    workspace: TaskComparison<'_>,
    path: &str,
    draft: &str,
) -> OpsResult<TaskDiffSnapshot> {
    if draft.len() > MAX_FILE_BYTES {
        return Err(task_error("Draft exceeds 1 MB"));
    }
    let path = validate_task_relative_path(path)?;
    let changes = net_changed_paths(workspace)?;
    let old_path = changes
        .get(&path)
        .and_then(Option::as_deref)
        .unwrap_or(&path);
    let spec = format!("{}:{old_path}", workspace.base_commit);
    let exists = !git_output_bytes(
        workspace.checkout.worktree,
        &["ls-tree", "-z", workspace.base_commit, "--", old_path],
    )?
    .is_empty();
    let original = if exists {
        git_output_bytes(workspace.checkout.worktree, &["show", &spec])?
    } else {
        Vec::new()
    };
    // Private temporary files only: draft comparisons never touch the worktree or index.
    // The a/ and b/ directories stand in for Git's prefixes so headers name the real paths.
    let directory = tempfile::tempdir()?;
    let before = format!("a/{old_path}");
    let after = format!("b/{path}");
    for (relative, bytes) in [(&before, original.as_slice()), (&after, draft.as_bytes())] {
        let file = directory.path().join(relative);
        std::fs::create_dir_all(file.parent().expect("prefixed path has a parent"))?;
        std::fs::write(file, bytes)?;
    }
    let output = Command::new("git")
        .current_dir(directory.path())
        .args([
            "diff",
            "--no-index",
            "--no-ext-diff",
            "--no-color",
            "--no-prefix",
            "--",
            &before,
            &after,
        ])
        .output()?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(task_error(String::from_utf8_lossy(&output.stderr)));
    }
    Ok(patch_snapshot(workspace, Some(path), &output.stdout))
}

pub(super) fn validate_task_relative_path(path: &str) -> OpsResult<String> {
    let path = Path::new(path);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(task_error(
            "Task paths must stay relative to the Task worktree",
        ));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            _ => {
                return Err(task_error(
                    "Task paths must stay relative to the Task worktree",
                ))
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(task_error("Task paths must name a file"));
    }
    Ok(normalized.to_string_lossy().to_string())
}

fn record_changed_paths(
    worktree: &Path,
    args: &[&str],
    files: &mut BTreeMap<String, TaskChangedFile>,
    mark: impl Fn(&mut TaskChangedFile),
) -> OpsResult<()> {
    for path in nul_paths(&git_output_bytes(worktree, args)?) {
        let file = files
            .entry(path)
            .or_insert_with_key(|path| TaskChangedFile {
                path: path.clone(),
                old_path: None,
                committed: false,
                staged: false,
                unstaged: false,
                untracked: false,
            });
        mark(file);
    }
    Ok(())
}

fn untracked_paths(worktree: &Path) -> OpsResult<Vec<String>> {
    Ok(nul_paths(&git_output_bytes(
        worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?))
}

fn nul_paths(output: &[u8]) -> Vec<String> {
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect()
}

pub(super) fn git_output(worktree: &Path, args: &[&str]) -> OpsResult<String> {
    Ok(String::from_utf8_lossy(&git_output_bytes(worktree, args)?).into_owned())
}

pub(super) fn git_output_bytes(worktree: &Path, args: &[&str]) -> OpsResult<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(worktree)
        .args(args)
        .output()
        .map_err(|error| task_error(format!("failed to run git {}: {error}", args.join(" "))))?;
    if !output.status.success() {
        return Err(task_error(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}
