//! Read-only Task views: status snapshot, changed files, diffs and file contents.

use super::{
    block_on_task, comparison_context, completion_pending, completion_refusal, file_context,
    file_save, file_store, owning_wave, repository_for_task, resolve_task_agent,
    task_configuration_refusal, task_error, task_event_process_refusal, task_store,
    task_worktree_blocker, TaskChangedFile, TaskChangesSnapshot, TaskCommit, TaskComparison,
    TaskDiffSnapshot, TaskFileSnapshot, TaskFileState, TaskSnapshot, TaskWorkspace,
};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::ops::error::OpsResult;
use crate::ops::task_actions::{derive_task_actions, TaskActionEvidence, TaskActionModel};
use crate::store::SharedStore;
use base64::Engine as _;
use loopflow_storage::child::ChildRef;
use loopflow_storage::work::task::{PrPhase, Task, TaskPr};
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
    let mut execution = crate::ops::task_execution::task_execution(store, &task)
        .await
        .map_err(task_error)?;
    let work_set =
        crate::store::task_work::task_work(&store.sqlite, &task.id).map_err(task_error)?;
    let auto = store.sqlite.task_auto(&task.id).map_err(task_error)?;
    if execution.state == crate::ops::task_execution::TaskExecutionState::Idle {
        if let Some(wait) = work_set
            .graph
            .as_ref()
            .and_then(|graph| graph.waiting_to_land(auto.as_ref(), &task.identifier))
        {
            execution.state = crate::ops::task_execution::TaskExecutionState::Waiting;
            execution.reason = wait;
        }
    }
    let status = store.task_state(&task.id).await.map_err(task_error)?;
    let parent_pr = match task.parent_pr_id.as_ref() {
        Some(parent_id) => store
            .get_task_pr(parent_id)
            .await
            .map_err(|error| task_error(format!("failed to read parent PR: {error}")))?,
        None => None,
    };
    let stack_parent = match parent_pr.as_ref() {
        Some(parent) => store
            .get_task(&parent.task_id)
            .await
            .map_err(task_error)?
            .map(|parent_task| super::TaskStackParent {
                identifier: parent_task.identifier,
                branch: parent.branch.clone(),
                pr_number: parent.github().map(|github| u64::from(github.number)),
            }),
        None => None,
    };
    let actions = if store.sqlite.task_deleted(&task).map_err(task_error)? {
        TaskActionModel {
            recommended: Some(crate::ops::task_actions::TaskAction::NoAction),
            reason: "Task was deleted; retained history is read-only".into(),
        }
    } else {
        let predecessor_phase = parent_pr.as_ref().map(TaskPr::phase);
        let completion_refusal = completion_refusal(pr.as_ref(), &task.identifier);
        let worktree_blocker = task_worktree_blocker(store, &task).await?;
        let run_refusal = worktree_blocker
            .as_ref()
            .map(|blocker| blocker.reason.clone());
        let launch_refusal = if worktree_blocker.is_some() {
            None
        } else {
            task_configuration_refusal(&task, &repo, wave.as_ref().map(|w| w.slug()))
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
            at_end: work_set.graph.as_ref().is_some_and(|graph| {
                matches!(
                    &graph.position,
                    crate::task_graph::TaskGraphPosition::At { step } if step == crate::task_graph::END
                )
            }),
            open_children: store
                .sqlite
                .open_task_child_count(&task.id)
                .map_err(task_error)?,
            completion_refusal: completion_refusal.as_deref(),
            run_refusal: run_refusal.as_deref(),
            ci: active.and_then(|pr| pr.fresh_ci()),
            predecessor_phase,
            launch_refusal: launch_refusal.as_deref(),
        };
        derive_task_actions(&action_evidence)
    };
    let agent = resolve_task_agent(
        task.worktree.as_deref().unwrap_or(&repo),
        &repo,
        wave.as_ref().map(|w| w.slug()),
        None,
    );
    let provider = agent.ok().map(|agent| agent.harness.as_str().to_owned());
    // A Task's worktree is its checkout on this Machine.
    let local_machine = store.local_machine().await.map_err(task_error)?;
    let machine_id = task.worktree.is_some().then(|| local_machine.id.clone());
    let worktree = task.worktree.as_ref().map(|path| {
        crate::git::worktree_root(path)
            .ok()
            .and_then(|root| root.canonicalize().ok())
            .unwrap_or_else(|| path.clone())
    });
    let completion_pending = completion_pending(store, &task)?;
    let starter = |session: Option<&loopflow_model::id::LfSessionId>| {
        super::auto::starter(&store.sqlite, &repo, session).map_err(task_error)
    };
    let started_by = store
        .sqlite
        .latest_task_graphs(&[&task.id])
        .map_err(task_error)?
        .get(&task.id)
        .map(|graph| starter(graph.caller_session.as_ref()))
        .transpose()?;
    Ok(TaskSnapshot {
        machine_id,
        issue_id: task.tracker_id.as_ref().map(|id| id.as_str().to_string()),
        issue_identifier: task.identifier,
        task_id: task.id.to_string(),
        external_project_id: project
            .as_ref()
            .and_then(|p| p.tracker_id.as_ref())
            .map(|id| id.as_str().to_string()),
        project: project.map(|p| p.slug),
        tracker_synced_at: task.tracker_synced_at,
        tracker_writeback: task.tracker_writeback,
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
        provider,
        pr,
        latest_event,
        created_at: task.created_at,
        updated_at: task.updated_at,
        observation: task.observation,
        actions,
        started_by,
        auto,
        stack_parent,
    })
}

/// Maximum byte length for Task file content, drafts and returned patches.
pub const MAX_FILE_BYTES: usize = 1_000_000;

pub fn task_changes(issue: &str, base: &str, head: Option<&str>) -> OpsResult<TaskChangesSnapshot> {
    let (checkout, base) = comparison_context(issue, base)?;
    let head = head
        .map(|head| resolve_file_base(&checkout.worktree, &base, head))
        .transpose()?;
    changes_snapshot(TaskComparison {
        checkout: TaskWorkspace::from(&checkout),
        base_commit: &base,
        head_commit: head.as_deref(),
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
            &format!(
                "{}..{}",
                workspace.base_commit,
                workspace.head_commit.unwrap_or("HEAD")
            ),
        ],
        &mut files,
        |file| file.committed = true,
    )?;
    if workspace.head_commit.is_none() {
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
    }
    let head_commit = git_output(
        workspace.checkout.worktree,
        &["rev-parse", workspace.head_commit.unwrap_or("HEAD")],
    )?
    .trim()
    .to_string();
    let commits = git_output(
        workspace.checkout.worktree,
        &[
            "log",
            "--reverse",
            "--first-parent",
            "--no-merges",
            "--format=%H%x1f%P%x1f%s",
            &format!("{}..{head_commit}", workspace.base_commit),
        ],
    )?
    .lines()
    .filter_map(|line| {
        let mut fields = line.splitn(3, '\u{1f}');
        Some(TaskCommit {
            sha: fields.next()?.to_string(),
            parent: fields.next()?.to_string(),
            subject: fields.next()?.to_string(),
        })
    })
    .collect();
    let net = net_changed_paths(workspace)?;
    let counts = change_counts(workspace)?;
    files.retain(|path, file| {
        file.old_path = net.get(path).cloned().flatten();
        file.changes = match counts.get(path) {
            Some(count) => *count,
            None if file.untracked => untracked_changes(&workspace.checkout.worktree.join(path)),
            None => 0,
        };
        net.contains_key(path) || file.untracked
    });
    let (scratch, scratch_truncated) = if workspace.head_commit.is_none() {
        scratch_paths(workspace.checkout.worktree)?
    } else {
        (Vec::new(), false)
    };
    let recovery = file_save::recovery_directory(workspace.checkout)?;
    Ok(TaskChangesSnapshot {
        recovery_directory: recovery
            .exists()
            .then(|| recovery.to_string_lossy().into_owned()),
        issue_identifier: workspace.checkout.issue_identifier.to_string(),
        task_id: workspace.checkout.task_id.to_string(),
        base_commit: workspace.base_commit.to_string(),
        head_commit,
        commits,
        files: files.into_values().collect(),
        scratch,
        scratch_truncated,
    })
}

/// Each tracked file's runs of changed lines, from the patch the Diff reads:
/// with its usual context, since Git aligns some changes differently without.
/// A file with no hunks (binary, a pure rename) is absent.
fn change_counts(workspace: TaskComparison<'_>) -> OpsResult<BTreeMap<String, u64>> {
    Ok(count_changes(&git_output_bytes(
        workspace.checkout.worktree,
        &comparison_diff_args(workspace),
    )?))
}

/// Patch flags pinned against Machine configuration (`color.diff`, external
/// drivers, `diff.noprefix`, `diff.mnemonicPrefix`), so the counts and the Diff
/// read the same patch.
const PINNED_PATCH_FLAGS: [&str; 4] = [
    "--no-color",
    "--no-ext-diff",
    "--src-prefix=a/",
    "--dst-prefix=b/",
];

/// `git diff` over the comparison, renames found whatever `diff.renames` says.
/// Pathspecs follow.
fn comparison_diff_args<'a>(workspace: TaskComparison<'a>) -> Vec<&'a str> {
    let mut args = vec!["diff", "--find-renames"];
    args.extend(PINNED_PATCH_FLAGS);
    args.push(workspace.base_commit);
    args.extend(workspace.head_commit);
    args.push("--");
    args
}

pub(super) fn count_changes(patch: &[u8]) -> BTreeMap<String, u64> {
    let mut counts = BTreeMap::new();
    let mut old_path = None::<String>;
    let mut path = None::<String>;
    // Lines left in the current hunk, old and new side.
    let (mut old, mut new) = (0u64, 0u64);
    let mut previous = b' ';
    for line in patch.split(|byte| *byte == b'\n') {
        if old > 0 || new > 0 {
            match line.first() {
                Some(b'-') => old = old.saturating_sub(1),
                Some(b'+') => new = new.saturating_sub(1),
                Some(b' ') => {
                    old = old.saturating_sub(1);
                    new = new.saturating_sub(1);
                }
                _ => continue,
            }
            let kind = line[0];
            let starts = kind != b' ' && (previous == b' ' || (kind == b'-' && previous == b'+'));
            if starts {
                if let Some(path) = path.as_ref().or(old_path.as_ref()) {
                    *counts.entry(path.clone()).or_insert(0) += 1;
                }
            }
            previous = kind;
        } else if line.starts_with(b"diff --git ") {
            old_path = None;
            path = None;
        } else if let Some(name) = line.strip_prefix(b"--- a/") {
            old_path = Some(patch_path(name));
        } else if let Some(name) = line.strip_prefix(b"+++ b/") {
            path = Some(patch_path(name));
        } else if line.starts_with(b"@@ ") {
            (old, new) = hunk_lengths(line);
            previous = b' ';
        }
    }
    counts
}

/// A path as Git writes it after `a/` or `b/`: C-quoted when it holds
/// unusual bytes, followed by a tab when it holds a space.
fn patch_path(name: &[u8]) -> String {
    let name = name.strip_suffix(b"\t").unwrap_or(name);
    let Some(quoted) = name
        .strip_prefix(b"\"")
        .and_then(|name| name.strip_suffix(b"\""))
    else {
        return String::from_utf8_lossy(name).into_owned();
    };
    let mut bytes = Vec::with_capacity(quoted.len());
    let mut rest = quoted.iter().copied();
    while let Some(byte) = rest.next() {
        if byte != b'\\' {
            bytes.push(byte);
            continue;
        }
        match rest.next() {
            Some(b'n') => bytes.push(b'\n'),
            Some(b't') => bytes.push(b'\t'),
            Some(digit @ b'0'..=b'7') => {
                let mut value = digit - b'0';
                for _ in 0..2 {
                    if let Some(next) = rest.next() {
                        value = value.wrapping_mul(8).wrapping_add(next.wrapping_sub(b'0'));
                    }
                }
                bytes.push(value);
            }
            Some(other) => bytes.push(other),
            None => {}
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The old and new line counts of `@@ -a[,b] +c[,d] @@`; a missing count is 1.
fn hunk_lengths(header: &[u8]) -> (u64, u64) {
    let header = String::from_utf8_lossy(header);
    let mut sides = header.split(' ').skip(1).take(2);
    let mut length = || {
        let side = sides.next().and_then(|side| side.get(1..)).unwrap_or("");
        side.split_once(',')
            .map_or(Ok(1), |(_, count)| count.parse())
            .unwrap_or(0)
    };
    (length(), length())
}

/// An untracked file reads as one addition, unless it is empty or binary (a
/// NUL in Git's first 8,000 bytes), when there is nothing to step to.
fn untracked_changes(path: &Path) -> u64 {
    use std::io::Read;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => return 1,
        Ok(metadata) if metadata.is_file() => {}
        _ => return 0,
    }
    let mut head = Vec::with_capacity(8_000);
    let read = std::fs::File::open(path).and_then(|file| file.take(8_000).read_to_end(&mut head));
    u64::from(read.is_ok() && !head.is_empty() && !head.contains(&0))
}

pub fn task_diff(
    issue: &str,
    path: Option<&str>,
    base: &str,
    head: Option<&str>,
    draft: Option<&str>,
) -> OpsResult<TaskDiffSnapshot> {
    let (checkout, base) = comparison_context(issue, base)?;
    if head.is_some() && draft.is_some() {
        return Err(task_error(
            "A commit comparison cannot include a working-tree draft",
        ));
    }
    let head = head
        .map(|head| resolve_file_base(&checkout.worktree, &base, head))
        .transpose()?;
    let workspace = TaskComparison {
        checkout: TaskWorkspace::from(&checkout),
        base_commit: &base,
        head_commit: head.as_deref(),
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
    let mut args = comparison_diff_args(workspace);
    // Both sides of a rename are needed for Git to preserve its identity.
    if let Some(old) = &old_path {
        args.push(old);
    }
    if let Some(path) = &relative {
        args.push(path);
    }
    let mut patch = git_output_bytes(workspace.checkout.worktree, &args)?;
    let untracked = if workspace.head_commit.is_none() {
        untracked_paths(workspace.checkout.worktree)?
    } else {
        Vec::new()
    };
    let include_untracked = untracked
        .into_iter()
        .filter(|candidate| relative.as_ref().is_none_or(|path| path == candidate));
    for path in include_untracked {
        let output = crate::observe::git(
            workspace.checkout.worktree,
            &[
                &["diff", "--no-index"][..],
                &PINNED_PATCH_FLAGS,
                &["--", "/dev/null", &path],
            ]
            .concat(),
        )
        .map_err(|error| task_error(format!("failed to diff untracked file {path}: {error}")))?;
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

pub fn task_workspace_context(task: &Task) -> OpsResult<String> {
    if !task.worktree.as_ref().is_some_and(|path| path.is_dir()) {
        return Ok("This Task has no checkout here: none was made, or it was retired when the Task closed. Read its changes from its pull request and Git history.".into());
    }
    if !crate::git::commit_exists(task.worktree()?, &task.base_commit)? {
        return Ok(format!(
            "The recorded Task comparison base {} is unavailable in {}. The saved checkout and its edits remain the working copy. Inspect Git history and the actual files, recover the comparison base through managed sync, and do not treat the missing diff as an empty change.",
            task.base_commit,
            task.worktree()?.display()
        ));
    }
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
        && crate::prompt::count_tokens(&diff.patch) < crate::prompt::change::PATCH_INLINE_TOKENS;
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

/// Largest binary file whose bytes a read carries, base64, in `data`.
pub const MAX_BINARY_DATA_BYTES: usize = 16 * 1024 * 1024;

/// One file of a Task: the working tree, or the version at `at` (`parent`,
/// `head` or a resolved commit SHA), which is read-only and has no recoveries.
pub fn task_file(
    issue: &str,
    path: &str,
    at: Option<&str>,
    inspect_recovery: bool,
) -> OpsResult<TaskFileSnapshot> {
    if let Some(at) = at {
        let (checkout, commit) = comparison_context(issue, at)?;
        return committed_file_snapshot(TaskWorkspace::from(&checkout), &commit, path);
    }
    let checkout = file_context(&file_store()?, issue)?;
    let workspace = TaskWorkspace::from(&checkout);
    let mut file = file_snapshot(workspace, path)?;
    if inspect_recovery {
        file.recoveries = file_save::recoveries(workspace, &file.path)?;
    }
    Ok(file)
}

fn missing_snapshot(workspace: TaskWorkspace<'_>, path: String) -> TaskFileSnapshot {
    TaskFileSnapshot {
        recoveries: Vec::new(),
        issue_identifier: workspace.issue_identifier.to_string(),
        task_id: workspace.task_id.to_string(),
        path,
        content: None,
        data: None,
        state: TaskFileState::Missing,
        revision: None,
        read_only_reason: None,
        size_bytes: 0,
    }
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
    let mut snapshot = missing_snapshot(workspace, relative.clone());
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
    let read = |limit| {
        crate::observe::file_bytes(&absolute, limit)
            .map_err(|error| task_error(format!("cannot read Task file {relative:?}: {error}")))
    };
    let (size, bytes) = read(MAX_FILE_BYTES + 1)?;
    snapshot.size_bytes = size;
    read_contents(&mut snapshot, bytes, || Ok(read(MAX_BINARY_DATA_BYTES)?.1))?;
    Ok(snapshot)
}

pub(super) fn committed_file_snapshot(
    workspace: TaskWorkspace<'_>,
    commit: &str,
    path: &str,
) -> OpsResult<TaskFileSnapshot> {
    let relative = validate_task_relative_path(path)?;
    let mut snapshot = missing_snapshot(workspace, relative.clone());
    snapshot.read_only_reason = Some("A committed version is read-only.".into());
    // `<mode> <type> <object> <size>\t<path>`; nothing when the commit has no such path.
    let listing = git_output_bytes(
        workspace.worktree,
        &["ls-tree", "-l", "-z", commit, "--", &relative],
    )?;
    let Some(entry) = listing
        .split(|byte| *byte == 0)
        .find(|entry| !entry.is_empty())
    else {
        return Ok(snapshot);
    };
    let header =
        String::from_utf8_lossy(entry.split(|byte| *byte == b'\t').next().unwrap_or(entry))
            .into_owned();
    let fields = header.split_whitespace().collect::<Vec<_>>();
    let [_, "blob", object, size] = fields[..] else {
        return Err(task_error(format!(
            "Task file {relative:?} is not a file at {commit}"
        )));
    };
    snapshot.size_bytes = size
        .parse()
        .map_err(|_| task_error(format!("Git listed no size for {relative:?}")))?;
    let bytes = git_blob_prefix(workspace.worktree, object, MAX_FILE_BYTES + 1)?;
    read_contents(&mut snapshot, bytes, || {
        git_output_bytes(workspace.worktree, &["cat-file", "blob", object])
    })?;
    Ok(snapshot)
}

/// Classifies a file from its first bytes (at most `MAX_FILE_BYTES + 1`).
/// `whole` reads the rest of a binary file small enough to carry in `data`.
fn read_contents(
    snapshot: &mut TaskFileSnapshot,
    bytes: Vec<u8>,
    whole: impl FnOnce() -> OpsResult<Vec<u8>>,
) -> OpsResult<()> {
    if bytes.contains(&0) {
        snapshot.state = TaskFileState::Binary;
        if snapshot.size_bytes <= MAX_BINARY_DATA_BYTES as u64 {
            let bytes = if bytes.len() as u64 >= snapshot.size_bytes {
                bytes
            } else {
                whole()?
            };
            snapshot.data = Some(base64::engine::general_purpose::STANDARD.encode(bytes));
        }
    } else if bytes.len() > MAX_FILE_BYTES {
        snapshot.state = TaskFileState::Truncated;
    } else if let Ok(content) = String::from_utf8(bytes) {
        snapshot.revision = Some(hex::encode(Sha256::digest(content.as_bytes())));
        snapshot.content = Some(content);
        snapshot.state = TaskFileState::Text;
    } else {
        snapshot.state = TaskFileState::UnsupportedEncoding;
    }
    Ok(())
}

/// The first `limit` bytes of a blob, without reading a large one whole.
fn git_blob_prefix(worktree: &Path, object: &str, limit: usize) -> OpsResult<Vec<u8>> {
    use std::io::Read as _;
    let mut child = Command::new("git")
        .current_dir(worktree)
        .args(["cat-file", "blob", object])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .expect("stdout is piped")
        .take(limit as u64)
        .read_to_end(&mut bytes)?;
    // A blob longer than the limit leaves git blocked on a full pipe.
    let _ = child.kill();
    child.wait()?;
    Ok(bytes)
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
                "A revision must be parent, head, or a resolved commit SHA",
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
    let mut args = vec![
        "diff",
        "--name-status",
        "--no-ext-diff",
        "--find-renames",
        "-z",
        workspace.base_commit,
    ];
    args.extend(workspace.head_commit);
    args.push("--");
    let output = git_output_bytes(workspace.checkout.worktree, &args)?;
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
        head_commit: workspace.head_commit.map(str::to_owned),
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
                changes: 0,
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

pub fn git_output(worktree: &Path, args: &[&str]) -> OpsResult<String> {
    Ok(String::from_utf8_lossy(&git_output_bytes(worktree, args)?).into_owned())
}

pub(super) fn git_output_bytes(worktree: &Path, args: &[&str]) -> OpsResult<Vec<u8>> {
    let output = crate::observe::git(worktree, args)
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
