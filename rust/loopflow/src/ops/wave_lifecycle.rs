//! Adding, removing and renaming Waves: one lifecycle for the CLI and Desktop.
//!
//! The planning store changes for every checkout at once; the authored
//! `.lf/wave/<name>/` files change on the repository's main checkout and are
//! committed there, so later launches from any worktree read them after rebase.

pub(crate) mod recovery;

use std::path::{Path, PathBuf};

use recovery::{Identity, Operation, ProviderChange};

use serde::{Deserialize, Serialize};

use crate::id::WaveId;
use crate::ops::{OpsError, OpsResult};
use crate::store::Store;
use crate::work::project::Project;
use crate::work::wave::{Wave, WaveLocator};

fn lifecycle_error(message: impl ToString) -> OpsError {
    OpsError::Message(message.to_string())
}

/// What happened to the authored files on the main checkout.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum AuthoredFiles {
    Committed { commit: String },
    Unchanged,
    Uncommitted { reason: String },
}

/// `lf wave add --json`: the receipt Desktop and the CLI both show. `summary`
/// holds what is left to do after the add, one sentence per line; empty when
/// nothing is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveAdded {
    pub id: WaveId,
    pub name: String,
    pub created: bool,
    pub project: Project,
    pub main_checkout: PathBuf,
    pub files: AuthoredFiles,
    pub summary: Vec<String>,
}

/// Register a Wave, seed its authored files on the main checkout, ensure its
/// first Project and commit the files. Re-adding a live Wave changes nothing.
pub async fn add(repo: &Path, name: &str, objective: Option<&str>) -> OpsResult<WaveAdded> {
    let main = crate::repository::CanonicalRepo::discover(repo)
        .map_err(lifecycle_error)?
        .as_path()
        .to_path_buf();
    let _lifecycle = recovery::lock(&main)?;
    let locator = WaveLocator::discover(&main, name).map_err(lifecycle_error)?;
    let store = super::planning_sync::planning_store().await?;
    // A clone's authored Waves join the store with the first explicit write here.
    store
        .sqlite
        .register_authored_waves(&main.to_string_lossy())
        .map_err(lifecycle_error)?;
    let created = store
        .get_wave_at(&locator)
        .await
        .map_err(lifecycle_error)?
        .is_none();
    let wave = crate::work::wave::ensure_wave_row(&store, &main, locator.slug())
        .await
        .map_err(lifecycle_error)?;
    if created {
        if let Some(objective) = objective.map(str::trim).filter(|text| !text.is_empty()) {
            write_objective(&main, wave.slug(), objective)?;
        }
    }
    let project = ensure_first_project(&store, &wave).await?;
    let files = commit_authored(
        &main,
        &[authored_path(wave.slug())],
        &format!("lf wave add: {}", wave.slug()),
    );
    let mut added = WaveAdded {
        id: wave.id().clone(),
        name: wave.slug().to_string(),
        created,
        project,
        main_checkout: main,
        files,
        summary: Vec::new(),
    };
    added.summary = uncommitted_summary(&added.name, &added.files);
    Ok(added)
}

/// Report authored files the main checkout could not commit.
fn uncommitted_summary(slug: &str, files: &AuthoredFiles) -> Vec<String> {
    match files {
        AuthoredFiles::Uncommitted { reason } => {
            vec![format!(".lf/wave/{slug} is not committed: {reason}.")]
        }
        _ => Vec::new(),
    }
}

pub fn render_added(added: &WaveAdded) -> String {
    let headline = if added.created {
        format!("Added Wave {}", added.name)
    } else {
        format!("{} already exists", added.name)
    };
    let headline = match &added.files {
        AuthoredFiles::Committed { commit } => format!(
            "{headline}; committed .lf/wave/{} on {} ({})",
            added.name,
            added.main_checkout.display(),
            &commit[..commit.len().min(10)]
        ),
        _ => headline,
    };
    std::iter::once(headline)
        .chain(added.summary.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n")
}

async fn ensure_first_project(store: &Store, wave: &crate::work::wave::Wave) -> OpsResult<Project> {
    store
        .sqlite
        .record_project_activation(wave.id(), crate::journal::current_lf_process_id().as_ref())
        .map_err(lifecycle_error)?;
    let acquisition = super::planning_sync::lock_wave_planning(wave).await?;
    super::project::import_binding(store, wave, &acquisition)?;
    let id = store
        .sqlite
        .ensure_project(wave.id(), wave.slug())
        .map_err(lifecycle_error)?;
    store
        .sqlite
        .project(&id)
        .map_err(lifecycle_error)?
        .ok_or_else(|| lifecycle_error("selected Project is missing"))
}

/// Register the authored Wave directories of `repo`'s main checkout that the
/// planning store does not know yet. Desktop lists only what the store knows, so
/// this runs when a repository joins the portfolio and before `lf wave connect
/// --all`; a directory of a removed Wave stays removed.
pub async fn register_authored(repo: &Path) -> OpsResult<()> {
    let main = crate::repository::CanonicalRepo::discover(repo).map_err(lifecycle_error)?;
    let store = super::planning_sync::planning_store().await?;
    store
        .sqlite
        .register_authored_waves(&main.to_string())
        .map_err(lifecycle_error)
}

/// The live Wave `name` addresses: Desktop names a Wave by its id, which survives
/// a concurrent rename; the CLI by its address in this repository.
fn resolve_wave(store: &Store, main: &Path, name: &str) -> OpsResult<Option<Wave>> {
    match WaveId::parse(name) {
        Ok(id) => store.sqlite.get_wave(&id).map_err(lifecycle_error),
        Err(_) => {
            let locator = WaveLocator::discover(main, name).map_err(lifecycle_error)?;
            store.sqlite.get_wave_at(&locator).map_err(lifecycle_error)
        }
    }
}

/// Capture the live subtree before changing its addresses or retiring it.
/// Retries use the saved identities, since live-child reads exclude retired Waves.
fn wave_subtree(store: &Store, root: Wave) -> OpsResult<Vec<Wave>> {
    let mut waves = vec![root];
    let mut next = 0;
    while next < waves.len() {
        if !waves[next].is_retired() {
            waves.extend(
                store
                    .sqlite
                    .list_child_waves(waves[next].id())
                    .map_err(lifecycle_error)?,
            );
        }
        next += 1;
    }
    Ok(waves)
}

pub(crate) fn authored_path(slug: &str) -> String {
    format!(".lf/wave/{slug}")
}

/// Replace the seeded objective, leaving any GOAL.md a person already wrote.
fn write_objective(main: &Path, slug: &str, objective: &str) -> OpsResult<()> {
    let path = main.join(authored_path(slug)).join("GOAL.md");
    let seeded = format!("## Objective\n\n{slug}\n");
    if std::fs::read_to_string(&path).ok().as_deref() == Some(seeded.as_str()) {
        std::fs::write(&path, format!("## Objective\n\n{objective}\n"))
            .map_err(|error| lifecycle_error(format!("{}: {error}", path.display())))?;
    }
    Ok(())
}

/// Commit the files add/connect just wrote on main, leaving unrelated changes
/// untouched. Removal/rename commit their retained images through `Operation`.
/// A failed commit keeps the files and reports why.
pub(crate) fn commit_authored(main: &Path, paths: &[String], message: &str) -> AuthoredFiles {
    let mut status = vec!["status", "--porcelain", "--"];
    status.extend(paths.iter().map(String::as_str));
    match crate::git::git_stdout(main, &status) {
        Ok(changes) if changes.trim().is_empty() => return AuthoredFiles::Unchanged,
        Ok(_) => {}
        Err(error) => {
            return AuthoredFiles::Uncommitted {
                reason: error.to_string(),
            }
        }
    }
    if crate::git::git_stdout(main, &["symbolic-ref", "-q", "HEAD"]).is_err() {
        return AuthoredFiles::Uncommitted {
            reason: format!("{} is not on a branch", main.display()),
        };
    }
    match crate::ops::commit_selected(main, paths, Some(message))
        .map_err(lifecycle_error)
        .and_then(|()| {
            crate::git::git_stdout(main, &["rev-parse", "HEAD"]).map_err(lifecycle_error)
        }) {
        Ok(commit) => AuthoredFiles::Committed {
            commit: commit.trim().to_string(),
        },
        // A concurrent lifecycle command may have committed the same files.
        Err(_) if crate::git::git_stdout(main, &status).is_ok_and(|s| s.trim().is_empty()) => {
            AuthoredFiles::Unchanged
        }
        Err(error) => AuthoredFiles::Uncommitted {
            reason: error.to_string(),
        },
    }
}

// ── Remove ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemovalTask {
    pub id: String,
    pub identifier: String,
    pub title: String,
    pub started: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitiativeArchive {
    pub wave: String,
    pub initiative: String,
    /// Why Linear still has it live; `None` once archived.
    pub error: Option<String>,
}

/// The preview Desktop and the CLI render before removal, and the receipt after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveRemoval {
    pub id: WaveId,
    pub name: String,
    pub preview: bool,
    pub already_removed: bool,
    /// Where open Tasks go: the parent Wave, or the repository when `None`.
    pub destination: Option<String>,
    /// The Wave and every child Wave that retires with it.
    pub waves: Vec<String>,
    pub moved: Vec<RemovalTask>,
    pub deleted: Vec<RemovalTask>,
    pub completed_tasks: usize,
    pub chapters: usize,
    pub sessions: usize,
    pub initiatives: Vec<InitiativeArchive>,
    pub has_files: bool,
    /// Files under `.lf/wave/<name>` with edits nobody committed on the main
    /// checkout; removal deletes them with the directory, and Git keeps no copy.
    pub uncommitted: Vec<String>,
    /// `None` in a preview.
    pub files: Option<AuthoredFiles>,
    /// Prose that still names the Wave, as `path:line`; never rewritten.
    pub mentions: Vec<String>,
    pub summary: Vec<String>,
}

/// Retire a Wave and its child Waves. Open Tasks move to the parent Wave (or the
/// repository); `delete_backlog` deletes the unstarted ones instead. Each step is
/// idempotent, so re-running finishes an interrupted removal. Synchronous because
/// the Task moves and deletes block on their own runtime.
pub fn remove(
    repo: &Path,
    name: &str,
    preview: bool,
    delete_backlog: bool,
) -> OpsResult<WaveRemoval> {
    let runtime = tokio::runtime::Runtime::new().map_err(lifecycle_error)?;
    let main = crate::repository::CanonicalRepo::discover(repo).map_err(lifecycle_error)?;
    let repo_key = main.to_string();
    let main = main.as_path().to_path_buf();
    let store = runtime.block_on(super::planning_sync::planning_store())?;
    let _lifecycle = recovery::lock(&main)?;
    let operations = store
        .sqlite
        .wave_operations(&repo_key, "remove")
        .map_err(lifecycle_error)?;
    let pending = operations
        .iter()
        .find(|operation| !operation.complete() && operation.matches(name, None));
    // An unfinished request retains its identity even when the address is reused.
    let wave = if let Some(operation) = pending {
        store
            .sqlite
            .get_wave(&operation.waves[0].id)
            .map_err(lifecycle_error)?
    } else {
        resolve_wave(&store, &main, name)?
    };
    let wave = match wave {
        Some(wave) => wave,
        None => {
            let locator = WaveLocator::discover(&main, name).map_err(lifecycle_error)?;
            store
                .sqlite
                .retired_wave_at(&locator)
                .map_err(lifecycle_error)?
                .ok_or_else(|| lifecycle_error(format!("no Wave {name} here")))?
        }
    };
    let saved = pending.or_else(|| {
        operations
            .iter()
            .find(|operation| &operation.waves[0].id == wave.id())
    });
    let already_removed = wave.is_retired();
    let owns_address =
        resolve_wave(&store, &main, wave.slug())?.is_none_or(|current| current.id() == wave.id());
    let subtree = if let Some(operation) = saved {
        operation
            .waves
            .iter()
            .map(|identity| {
                store
                    .sqlite
                    .get_wave(&identity.id)
                    .map_err(lifecycle_error)?
                    .ok_or_else(|| lifecycle_error(format!("Wave {} is missing", identity.id)))
            })
            .collect::<OpsResult<Vec<_>>>()?
    } else {
        wave_subtree(&store, wave.clone())?
    };
    let parent_id = saved.map_or_else(
        || wave.parent_wave_id().cloned(),
        |operation| operation.destination.clone(),
    );
    let destination = match &parent_id {
        Some(parent) => store
            .sqlite
            .get_wave(parent)
            .map_err(lifecycle_error)?
            .filter(|parent| !parent.is_retired())
            .map(|parent| parent.slug().to_string()),
        None => None,
    };
    let delete_backlog = saved.map_or(delete_backlog, |operation| operation.delete_backlog);
    let ids: Vec<WaveId> = subtree.iter().map(|wave| wave.id().clone()).collect();
    let mut moved = Vec::new();
    let mut deleted = Vec::new();
    let mut completed_tasks = 0;
    let mut chapters = 0;
    for id in &ids {
        chapters += store
            .sqlite
            .list_projects(Some(id))
            .map_err(lifecycle_error)?
            .len();
        for task in store.sqlite.list_tasks(Some(id)).map_err(lifecycle_error)? {
            let state = store.sqlite.task_state(&task.id).map_err(lifecycle_error)?;
            if state.is_terminal() {
                completed_tasks += 1;
                continue;
            }
            let started = state != crate::durable::TaskState::Unstarted;
            let line = RemovalTask {
                id: task.id.to_string(),
                identifier: task.identifier,
                title: task.title,
                started,
            };
            if delete_backlog && !started {
                deleted.push(line);
            } else {
                moved.push(line);
            }
        }
    }
    let sessions = store
        .sqlite
        .wave_session_count(&ids)
        .map_err(lifecycle_error)?;
    let slugs: Vec<String> = saved.map_or_else(
        || subtree.iter().map(|wave| wave.slug().to_string()).collect(),
        |operation| {
            operation
                .waves
                .iter()
                .map(|wave| wave.slug.clone())
                .collect()
        },
    );
    let directory = main.join(authored_path(wave.slug()));
    let has_files = owns_address && directory.exists();
    let initiatives = if let Some(operation) = saved {
        operation
            .providers
            .iter()
            .map(|provider| InitiativeArchive {
                wave: provider.wave.clone(),
                initiative: provider.initiative.clone(),
                error: None,
            })
            .collect()
    } else {
        slugs
            .iter()
            .filter(|_| owns_address)
            .filter_map(|slug| {
                super::linear::read_initiative(&main, slug).map(|initiative| InitiativeArchive {
                    wave: slug.clone(),
                    initiative,
                    error: None,
                })
            })
            .collect()
    };
    let mut receipt = WaveRemoval {
        id: wave.id().clone(),
        name: saved.map_or_else(
            || wave.slug().to_string(),
            |operation| operation.from.clone(),
        ),
        preview,
        already_removed,
        destination,
        waves: slugs,
        moved,
        deleted,
        completed_tasks,
        chapters,
        sessions,
        initiatives,
        has_files,
        uncommitted: if has_files {
            uncommitted_edits(&main, &authored_path(wave.slug()))
        } else {
            Vec::new()
        },
        files: None,
        mentions: prose_mentions(&main, wave.slug()),
        summary: Vec::new(),
    };
    if preview {
        receipt.summary = removal_summary(&receipt);
        return Ok(receipt);
    }

    let mut operation = if let Some(operation) = saved {
        operation.clone()
    } else {
        let owners = store
            .sqlite
            .list_waves(Some(&repo_key))
            .map_err(lifecycle_error)?
            .iter()
            .map(Identity::from)
            .collect::<Vec<_>>();
        let changes = if has_files {
            recovery::prepare_files(&main, &owners, |copy| {
                remove_authored(copy, &receipt.name, receipt.destination.as_deref())
            })?
        } else {
            Vec::new()
        };
        let operation = Operation {
            id: uuid::Uuid::new_v4().to_string(),
            repo: repo_key.clone(),
            kind: "remove".into(),
            from: receipt.name.clone(),
            to: String::new(),
            target_repo: repo_key.clone(),
            waves: subtree.iter().map(Identity::from).collect(),
            destination: parent_id,
            delete_backlog,
            providers: receipt
                .initiatives
                .iter()
                .map(|archive| ProviderChange {
                    wave: archive.wave.clone(),
                    initiative: archive.initiative.clone(),
                    title: None,
                    done: false,
                })
                .collect(),
            changes,
            planning_done: false,
            applied: false,
            files_done: false,
        };
        operation.save(&store)?;
        operation
    };

    // Tasks first: nothing retires until every open Task has its disposition.
    let total = receipt.moved.len() + receipt.deleted.len();
    let mut done = 0;
    for task in &receipt.deleted {
        super::task::task_delete(&main, &task.id)
            .map_err(|error| interrupted(&receipt.name, done, total, task, error))?;
        done += 1;
    }
    for task in &receipt.moved {
        super::task::task_move_membership(&main, &task.id, receipt.destination.as_deref())
            .map_err(|error| interrupted(&receipt.name, done, total, task, error))?;
        done += 1;
    }
    if subtree.iter().any(|wave| !wave.is_retired()) {
        store
            .sqlite
            .retire_waves(&ids, "removed")
            .map_err(lifecycle_error)?;
        runtime.block_on(super::planning_sync::sync_after_save(&store, &repo_key));
    }

    operation.planning_done = true;
    operation.save(&store)?;

    for (index, archive) in receipt.initiatives.iter_mut().enumerate() {
        if operation.providers[index].done {
            continue;
        }
        match runtime.block_on(super::linear::archive_initiative(
            &main,
            &archive.initiative,
        )) {
            Ok(()) => {
                operation.providers[index].done = true;
                operation.save(&store)?;
            }
            Err(error) => archive.error = Some(error.to_string()),
        }
    }
    receipt.files = Some(
        if operation.providers.iter().any(|provider| !provider.done) {
            AuthoredFiles::Uncommitted {
                reason: "kept until the Linear Initiative is archived".into(),
            }
        } else {
            operation.finish_files(&store, &main)
        },
    );
    receipt.summary = removal_summary(&receipt);
    Ok(receipt)
}

fn interrupted(
    wave: &str,
    done: usize,
    total: usize,
    task: &RemovalTask,
    error: OpsError,
) -> OpsError {
    lifecycle_error(format!(
        "{done} of {total} open Tasks moved or deleted; {} could not: {error}. \
         {wave} is not removed yet; run `lf wave remove {wave}` again to continue.",
        task.identifier
    ))
}

/// Paths under `path` whose edits are not committed on `main`.
fn uncommitted_edits(main: &Path, path: &str) -> Vec<String> {
    crate::git::git_stdout(main, &["status", "--porcelain", "--", path])
        .map(|status| {
            status
                .lines()
                .filter_map(|line| line.get(3..))
                .map(|path| path.trim_matches('"').to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Prepare removal on the private authored copy, folding into the parent first.
fn remove_authored(main: &Path, slug: &str, parent: Option<&str>) -> OpsResult<()> {
    let directory = main.join(authored_path(slug));
    if let Some(parent) = parent {
        let parent_dir = main.join(authored_path(parent));
        fold_memory(&directory, slug, &parent_dir.join("MEMORY.md"))?;
        fold_config(
            &directory.join("config.yaml"),
            &parent_dir.join("config.yaml"),
        )?;
        rewrite_references(main, slug, parent)?;
    }
    std::fs::remove_dir_all(&directory)
        .map_err(|error| lifecycle_error(format!("{}: {error}", directory.display())))
}

/// Append each removed Wave's curated memory (the Wave and its children) to the parent's.
fn fold_memory(directory: &Path, slug: &str, parent_memory: &Path) -> OpsResult<()> {
    let date = time::OffsetDateTime::now_utc().date();
    let mut folded = String::new();
    let memories = directories(directory, None)
        .into_iter()
        .map(|dir| dir.join("MEMORY.md"))
        .filter(|memory| memory.is_file());
    for memory in memories {
        let relative = memory
            .parent()
            .and_then(|dir| dir.strip_prefix(directory).ok())
            .map(|rest| rest.to_string_lossy().to_string())
            .filter(|rest| !rest.is_empty());
        let name = relative.map_or_else(|| slug.to_string(), |rest| format!("{slug}/{rest}"));
        let body = std::fs::read_to_string(&memory).unwrap_or_default();
        let body = body
            .trim()
            .strip_prefix(&format!("# {name} wave memory"))
            .unwrap_or(body.trim())
            .trim();
        if !body.is_empty() {
            folded.push_str(&format!("\n## From {name} (removed {date})\n\n{body}\n"));
        }
    }
    if folded.is_empty() {
        return Ok(());
    }
    let mut existing = std::fs::read_to_string(parent_memory).unwrap_or_default();
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&folded);
    std::fs::write(parent_memory, existing)
        .map_err(|error| lifecycle_error(format!("{}: {error}", parent_memory.display())))
}

/// `root` and every directory beneath it except `skip`'s subtree, sorted.
fn directories(root: &Path, skip: Option<&Path>) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        if Some(dir.as_path()) == skip {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            pending.extend(
                entries
                    .flatten()
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir()),
            );
        }
        found.push(dir);
    }
    found.sort();
    found
}

/// Copy config keys the parent lacks; never its identity or provider binding.
fn fold_config(config: &Path, parent_config: &Path) -> OpsResult<()> {
    use serde_yaml_ng::{Mapping, Value};
    let read = |path: &Path| -> OpsResult<Mapping> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(serde_yaml_ng::from_str::<Option<Mapping>>(&text)
                .map_err(|error| lifecycle_error(format!("{}: {error}", path.display())))?
                .unwrap_or_default()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Mapping::new()),
            Err(error) => Err(lifecycle_error(format!("{}: {error}", path.display()))),
        }
    };
    let child = read(config)?;
    let mut parent = read(parent_config)?;
    let mut changed = false;
    for (key, value) in child {
        if matches!(key.as_str(), Some("id" | "linear")) || parent.contains_key(&key) {
            continue;
        }
        parent.insert(key, value);
        changed = true;
    }
    if changed {
        let text = serde_yaml_ng::to_string(&Value::Mapping(parent)).map_err(lifecycle_error)?;
        std::fs::write(parent_config, text)
            .map_err(|error| lifecycle_error(format!("{}: {error}", parent_config.display())))?;
    }
    Ok(())
}

/// Every authored Wave document on main outside `slug`'s own directory.
fn wave_documents(main: &Path, slug: &str) -> Vec<PathBuf> {
    let root = main.join(".lf/wave");
    directories(&root, Some(&root.join(slug)))
        .into_iter()
        .flat_map(|dir| [dir.join("GOAL.md"), dir.join("MEMORY.md")])
        .filter(|document| document.is_file())
        .collect()
}

fn path_reference(slug: &str) -> regex::Regex {
    regex::Regex::new(&format!(
        r"\.lf/wave/{}(?P<end>/|[^A-Za-z0-9_/-]|$)",
        regex::escape(slug)
    ))
    .expect("escaped Wave path is a valid pattern")
}

/// Rewrite `.lf/wave/<slug>` paths in other Waves' documents to `replacement`.
fn rewrite_references(main: &Path, slug: &str, replacement: &str) -> OpsResult<()> {
    let pattern = path_reference(slug);
    for document in wave_documents(main, slug) {
        let Ok(text) = std::fs::read_to_string(&document) else {
            continue;
        };
        let rewritten = pattern.replace_all(&text, format!(".lf/wave/{replacement}$end").as_str());
        if rewritten != text {
            std::fs::write(&document, rewritten.as_ref())
                .map_err(|error| lifecycle_error(format!("{}: {error}", document.display())))?;
        }
    }
    Ok(())
}

/// Lines in other Waves' documents naming the Wave as a bare word or path.
fn prose_mentions(main: &Path, slug: &str) -> Vec<String> {
    let leaf = slug.rsplit('/').next().unwrap_or(slug);
    let word = regex::Regex::new(&format!(r"\b{}\b", regex::escape(leaf)))
        .expect("escaped Wave name is a valid pattern");
    let mut mentions = Vec::new();
    for document in wave_documents(main, slug) {
        let Ok(text) = std::fs::read_to_string(&document) else {
            continue;
        };
        let relative = document
            .strip_prefix(main)
            .unwrap_or(&document)
            .to_string_lossy()
            .to_string();
        for (index, line) in text.lines().enumerate() {
            if word.is_match(line) {
                mentions.push(format!("{relative}:{}", index + 1));
            }
        }
    }
    mentions
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The sentences both the CLI and Desktop show; Desktop renders them verbatim.
fn removal_summary(removal: &WaveRemoval) -> Vec<String> {
    let name = &removal.name;
    if removal.already_removed {
        let mut lines = vec![format!("{name} was already removed.")];
        lines.extend(leftovers(removal));
        return lines;
    }
    let mut lines = Vec::new();
    let children = removal.waves.len() - 1;
    if children > 0 {
        lines.push(format!(
            "{} ({}) {} with it.",
            count(children, "child Wave", "child Waves"),
            removal.waves[1..].join(", "),
            if removal.preview {
                "will be removed"
            } else {
                "were removed"
            }
        ));
    }
    let open = removal.moved.len() + removal.deleted.len();
    if open == 0 && removal.completed_tasks == 0 && removal.sessions == 0 {
        lines.push(format!("{name} has no Tasks or Sessions."));
    }
    let destination = removal
        .destination
        .as_deref()
        .map_or_else(|| "the repository".to_string(), |parent| parent.to_string());
    if !removal.moved.is_empty() {
        let started = removal.moved.iter().filter(|task| task.started).count();
        let verb = if removal.preview { "move" } else { "moved" };
        let mut line = format!(
            "{} {verb} to {destination}",
            count(removal.moved.len(), "open Task", "open Tasks")
        );
        if started > 0 {
            let verb = if started == 1 { "keeps" } else { "keep" };
            line.push_str(&format!(
                " ({started} started {verb} its worktree, PR and running Graph)"
            ));
        }
        line.push('.');
        lines.push(line);
    }
    if !removal.deleted.is_empty() {
        lines.push(format!(
            "{} {}: {}.",
            count(removal.deleted.len(), "unstarted Task", "unstarted Tasks"),
            if removal.preview {
                "will be deleted"
            } else {
                "deleted"
            },
            removal
                .deleted
                .iter()
                .map(|task| task.identifier.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let mut kept = Vec::new();
    if removal.completed_tasks > 0 {
        kept.push(count(
            removal.completed_tasks,
            "completed Task",
            "completed Tasks",
        ));
    }
    if removal.chapters > 0 {
        kept.push(count(removal.chapters, "chapter", "chapters"));
    }
    if !kept.is_empty() {
        let verb = if removal.completed_tasks + removal.chapters == 1 {
            "stays"
        } else {
            "stay"
        };
        lines.push(format!(
            "{} {verb} readable in history.",
            kept.join(" and ")
        ));
    }
    if removal.sessions > 0 {
        lines.push(match removal.sessions {
            1 => "The Wave conversation stays in history.".to_string(),
            n => format!("{n} Wave conversations stay in history."),
        });
    }
    if removal.has_files {
        let pending = matches!(removal.files, Some(AuthoredFiles::Uncommitted { .. }));
        lines.push(match &removal.destination {
            Some(parent) if removal.preview => format!(
                ".lf/wave/{name} folds its memory into {parent} and is deleted on the main checkout."
            ),
            None if removal.preview && removal.uncommitted.is_empty() => {
                format!(".lf/wave/{name} is deleted on the main checkout; Git history keeps it.")
            }
            None if removal.preview => format!(
                ".lf/wave/{name} is deleted on the main checkout; Git history keeps its committed files."
            ),
            _ if pending => format!("Authored changes for .lf/wave/{name} remain pending on the main checkout."),
            _ => format!(".lf/wave/{name} removed on the main checkout."),
        });
        if !removal.uncommitted.is_empty() && !pending {
            lines.push(format!(
                "Uncommitted edits in .lf/wave/{name} {} deleted with it: {}.",
                if removal.preview { "are" } else { "were" },
                removal.uncommitted.join(", ")
            ));
        }
    }
    if removal.preview {
        for archive in &removal.initiatives {
            lines.push(format!(
                "Linear Initiative {} for {} will be archived.",
                archive.initiative, archive.wave
            ));
        }
    }
    lines.extend(leftovers(removal));
    lines
}

pub fn render_removed(removal: &WaveRemoval) -> String {
    let headline = (!removal.preview && !removal.already_removed)
        .then(|| format!("Removed Wave {}.", removal.name));
    headline
        .into_iter()
        .chain(removal.summary.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n")
}

fn leftovers(removal: &WaveRemoval) -> Vec<String> {
    let mut lines = Vec::new();
    for archive in &removal.initiatives {
        if let Some(error) = &archive.error {
            lines.push(format!(
                "Linear Initiative {} is still live: {error}. Run `lf wave remove {}` again to retry.",
                archive.initiative, removal.name
            ));
        }
    }
    if let Some(files) = &removal.files {
        lines.extend(uncommitted_summary(&removal.name, files));
    }
    if !removal.mentions.is_empty() {
        lines.push(format!(
            "Still mentioned in: {}.",
            removal.mentions.join(", ")
        ));
    }
    lines
}

// ── Rename ─────────────────────────────────────────────────────────────

/// `lf wave rename --json`: the receipt Desktop and the CLI both show. `summary`
/// holds what is left to do after the rename, one sentence per line; empty when
/// nothing is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveRenamed {
    pub id: WaveId,
    pub from: String,
    pub to: String,
    pub renamed: bool,
    pub files: AuthoredFiles,
    /// The connected Initiative's new title, or why Linear kept the old one.
    pub initiative: Option<InitiativeRename>,
    pub mentions: Vec<String>,
    pub summary: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitiativeRename {
    pub initiative: String,
    pub title: String,
    pub error: Option<String>,
}

/// Give a Wave a new address (and optionally another repository). Identity,
/// Projects, Tasks and Sessions are untouched; `.lf/wave/<name>` moves on main
/// with path references in other Waves' documents.
pub async fn rename(
    repo: &Path,
    wave: &str,
    new_name: Option<&str>,
    target_repo: Option<&Path>,
    title: Option<&str>,
) -> OpsResult<WaveRenamed> {
    let store = super::planning_sync::planning_store().await?;
    let main = crate::repository::CanonicalRepo::discover(repo)
        .map_err(lifecycle_error)?
        .as_path()
        .to_path_buf();
    let target = new_name
        .map(|name| {
            crate::ops::normalize_wave_name(name)
                .ok_or_else(|| lifecycle_error(format!("invalid Wave name {name:?}")))
        })
        .transpose()?;
    let _lifecycle = recovery::lock(&main)?;
    let repo_key = main.to_string_lossy().to_string();
    let target_checkout = crate::repository::CanonicalRepo::discover(target_repo.unwrap_or(&main))
        .map_err(lifecycle_error)?
        .to_string();
    let operations = store
        .sqlite
        .wave_operations(&repo_key, "rename")
        .map_err(lifecycle_error)?;
    let matching = |operation: &&Operation| {
        operation.matches(wave, target.as_deref()) && operation.target_repo == target_checkout
    };
    let pending = operations
        .iter()
        .filter(matching)
        .find(|operation| !operation.complete());
    let resolved = if let Some(operation) = pending {
        store
            .sqlite
            .get_wave(&operation.waves[0].id)
            .map_err(lifecycle_error)?
    } else {
        resolve_wave(&store, &main, wave)?
    };
    let saved = pending.or_else(|| {
        operations.iter().filter(matching).find(|operation| {
            resolved.as_ref().is_none_or(|current| {
                current.id() == &operation.waves[0].id
                    && current.slug() == operation.to
                    && operation.from != operation.to
            })
        })
    });
    let mut operation = if let Some(operation) = saved {
        operation.clone()
    } else {
        let wave = resolved
            .as_ref()
            .ok_or_else(|| lifecycle_error(format!("no Wave {wave} here")))?;
        if wave.is_retired() {
            return Err(lifecycle_error(format!("Wave {} is retired", wave.id())));
        }
        let from = wave.slug().to_string();
        let to = target.unwrap_or_else(|| from.clone());
        let moves = to != from || repo_key != target_checkout;
        if let Some(existing) = resolve_wave(&store, Path::new(&target_checkout), &to)? {
            if existing.id() != wave.id() {
                return Err(lifecycle_error(format!(
                    "Wave {to} already exists; remove or rename it first"
                )));
            }
        }
        if to != from && repo_key == target_checkout && main.join(authored_path(&to)).exists() {
            return Err(lifecycle_error(format!(
                ".lf/wave/{to} already exists in {}; move or delete it first",
                main.display()
            )));
        }
        let subtree = wave_subtree(&store, wave.clone())?;
        let mut owners = store
            .sqlite
            .list_waves(Some(&repo_key))
            .map_err(lifecycle_error)?
            .iter()
            .map(Identity::from)
            .collect::<Vec<_>>();
        owners.extend(subtree.iter().map(|child| Identity {
            id: child.id().clone(),
            slug: format!("{to}{}", &child.slug()[from.len()..]),
        }));
        let changes = if moves && repo_key == target_checkout {
            recovery::prepare_files(&main, &owners, |copy| {
                let source = copy.join(authored_path(&from));
                if source.exists() {
                    let destination = copy.join(authored_path(&to));
                    std::fs::create_dir_all(
                        destination.parent().expect("Wave directory has a parent"),
                    )?;
                    std::fs::rename(source, destination)?;
                }
                rewrite_references(copy, &from, &to)?;
                Ok(())
            })?
        } else {
            Vec::new()
        };
        let providers = super::linear::read_initiative(&main, &from)
            .filter(|_| moves || title.is_some())
            .map(|initiative| ProviderChange {
                wave: from.clone(),
                initiative,
                title: Some(title.map_or_else(|| super::linear::title_case(&to), str::to_string)),
                done: false,
            })
            .into_iter()
            .collect();
        let operation = Operation {
            id: uuid::Uuid::new_v4().to_string(),
            repo: repo_key.clone(),
            kind: "rename".into(),
            from,
            to,
            target_repo: target_checkout.clone(),
            waves: subtree.iter().map(Identity::from).collect(),
            destination: None,
            delete_backlog: false,
            providers,
            changes,
            planning_done: false,
            applied: false,
            files_done: !moves,
        };
        operation.save(&store)?;
        operation
    };
    let renamed_now = !operation.planning_done
        && (operation.from != operation.to || operation.repo != operation.target_repo);
    if !operation.planning_done {
        let current = store
            .sqlite
            .get_wave(&operation.waves[0].id)
            .map_err(lifecycle_error)?
            .ok_or_else(|| lifecycle_error("original Wave is missing"))?;
        if current.slug() != operation.to || current.repo() != operation.target_repo {
            if current.slug() != operation.from || current.repo() != operation.repo {
                return Err(lifecycle_error(
                    "original Wave moved again; this rename remains pending",
                ));
            }
            crate::work::wave::relocate::relocate_wave(
                &store,
                current.id(),
                &main,
                Some(Path::new(&operation.target_repo)),
                Some(&operation.to),
            )
            .await
            .map_err(lifecycle_error)?;
        }
        operation.planning_done = true;
        operation.save(&store)?;
    }
    let files = operation.finish_files(&store, &main);
    let initiative = if let Some(provider) = operation.providers.first() {
        let mut result = InitiativeRename {
            initiative: provider.initiative.clone(),
            title: provider.title.clone().expect("rename retains its title"),
            error: None,
        };
        if !provider.done {
            match super::linear::rename_initiative(
                Path::new(&operation.target_repo),
                &result.initiative,
                &result.title,
            )
            .await
            {
                Ok(()) => {
                    operation.providers[0].done = true;
                    operation.save(&store)?;
                }
                Err(error) => result.error = Some(error.to_string()),
            }
        }
        Some(result)
    } else {
        None
    };
    let mut renamed = WaveRenamed {
        id: operation.waves[0].id.clone(),
        mentions: prose_mentions(&main, &operation.from),
        from: operation.from,
        to: operation.to,
        renamed: renamed_now,
        files,
        initiative,
        summary: Vec::new(),
    };
    renamed.summary = rename_summary(&renamed);
    Ok(renamed)
}

/// What the rename left for a person or agent to finish.
fn rename_summary(renamed: &WaveRenamed) -> Vec<String> {
    let mut lines = uncommitted_summary(&renamed.to, &renamed.files);
    if let Some(InitiativeRename {
        initiative,
        error: Some(error),
        ..
    }) = &renamed.initiative
    {
        lines.push(format!(
            "Linear Initiative {initiative} kept its title: {error}. Repeat the original `lf wave rename` command to retry."
        ));
    }
    if !renamed.mentions.is_empty() {
        lines.push(format!(
            "Still mentioned in: {}.",
            renamed.mentions.join(", ")
        ));
    }
    lines
}

pub fn render_renamed(renamed: &WaveRenamed) -> String {
    let headline = if renamed.renamed {
        format!("Renamed Wave {} → {}", renamed.from, renamed.to)
    } else {
        format!("{} is already {}", renamed.from, renamed.to)
    };
    std::iter::once(headline)
        .chain(renamed.summary.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n")
}
