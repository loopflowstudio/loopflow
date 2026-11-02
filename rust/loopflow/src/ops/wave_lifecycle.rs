//! Adding, removing and renaming Waves: one lifecycle for the CLI and Desktop.
//!
//! The planning store changes for every checkout at once; the authored
//! `.lf/wave/<name>/` files change on the repository's main checkout and are
//! committed there, so later launches from any worktree read them after rebase.

use std::path::{Path, PathBuf};

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

#[derive(Debug, Clone, Serialize)]
pub struct WaveAdded {
    pub id: WaveId,
    pub name: String,
    pub created: bool,
    pub project: Project,
    pub main_checkout: PathBuf,
    pub files: AuthoredFiles,
}

/// Register a Wave, seed its authored files on the main checkout, ensure its
/// first Project and commit the files. Re-adding a live Wave changes nothing.
pub async fn add(repo: &Path, name: &str, objective: Option<&str>) -> OpsResult<WaveAdded> {
    let main = crate::repository::CanonicalRepo::discover(repo)
        .map_err(lifecycle_error)?
        .as_path()
        .to_path_buf();
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
    Ok(WaveAdded {
        id: wave.id().clone(),
        name: wave.slug().to_string(),
        created,
        project,
        main_checkout: main,
        files,
    })
}

pub fn render_added(added: &WaveAdded) -> String {
    let headline = if added.created {
        format!("Added Wave {}", added.name)
    } else {
        format!("{} already exists", added.name)
    };
    match &added.files {
        AuthoredFiles::Committed { commit } => format!(
            "{headline}; committed .lf/wave/{} on {} ({})",
            added.name,
            added.main_checkout.display(),
            &commit[..commit.len().min(10)]
        ),
        AuthoredFiles::Unchanged => headline,
        AuthoredFiles::Uncommitted { reason } => format!(
            "{headline}; .lf/wave/{} is uncommitted on {}: {reason}",
            added.name,
            added.main_checkout.display()
        ),
    }
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

fn authored_path(slug: &str) -> String {
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

/// Commit exactly `paths` on the main checkout, leaving every other change a
/// person has there untouched. A checkout that cannot take the commit keeps
/// the files as they are and says why.
fn commit_authored(main: &Path, paths: &[String], message: &str) -> AuthoredFiles {
    // A pathspec must match: keep paths on disk or in the index.
    let paths: Vec<String> = paths
        .iter()
        .filter(|path| {
            main.join(path).exists()
                || crate::git::git_stdout(main, &["ls-files", "--", path.as_str()])
                    .is_ok_and(|tracked| !tracked.trim().is_empty())
        })
        .cloned()
        .collect();
    if paths.is_empty() {
        return AuthoredFiles::Unchanged;
    }
    let paths = paths.as_slice();
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
    /// `None` in a preview.
    pub files: Option<AuthoredFiles>,
    /// Prose that still names the Wave, as `path:line`; never rewritten.
    pub mentions: Vec<String>,
    pub summary: Vec<String>,
}

/// Retire a Wave and its child Waves. Open Tasks move to the parent Wave (or the
/// repository); `delete_backlog` deletes the unstarted ones instead. Each step is
/// idempotent, so re-running finishes an interrupted removal.
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
    // A removed Wave's address still resolves, so a repeat run reports it.
    let wave = match resolve_wave(&store, &main, name)? {
        Some(wave) => wave,
        None => {
            let locator = WaveLocator::discover(&main, name).map_err(lifecycle_error)?;
            store
                .sqlite
                .retired_wave_at(&repo_key, locator.slug())
                .map_err(lifecycle_error)?
                .ok_or_else(|| lifecycle_error(format!("no Wave {name} here")))?
        }
    };
    let already_removed = wave.is_retired();
    // A new Wave may hold a removed Wave's address; its files and Initiative are not ours.
    let owns_address = !already_removed || resolve_wave(&store, &main, wave.slug())?.is_none();
    let mut subtree = vec![wave.clone()];
    let mut next = 0;
    while !already_removed && next < subtree.len() {
        let children = store
            .sqlite
            .list_child_waves(subtree[next].id())
            .map_err(lifecycle_error)?;
        subtree.extend(children);
        next += 1;
    }
    let destination = match wave.parent_wave_id() {
        Some(parent) => store
            .sqlite
            .get_wave(parent)
            .map_err(lifecycle_error)?
            .filter(|parent| !parent.is_retired())
            .map(|parent| parent.slug().to_string()),
        None => None,
    };
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
    let slugs: Vec<String> = subtree.iter().map(|wave| wave.slug().to_string()).collect();
    let directory = main.join(authored_path(wave.slug()));
    let has_files = owns_address && directory.exists();
    let initiatives = slugs
        .iter()
        .filter(|_| owns_address)
        .filter_map(|slug| {
            super::linear::read_initiative(&main, slug).map(|initiative| InitiativeArchive {
                wave: slug.clone(),
                initiative,
                error: None,
            })
        })
        .collect();
    let mut receipt = WaveRemoval {
        id: wave.id().clone(),
        name: wave.slug().to_string(),
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
        files: None,
        mentions: prose_mentions(&main, wave.slug()),
        summary: Vec::new(),
    };
    if preview {
        receipt.summary = removal_summary(&receipt);
        return Ok(receipt);
    }

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
    if !already_removed {
        store
            .sqlite
            .retire_waves(&ids, "removed")
            .map_err(lifecycle_error)?;
        runtime.block_on(super::planning_sync::sync_after_save(&store, &repo_key));
    }

    for archive in &mut receipt.initiatives {
        if let Err(error) = runtime.block_on(super::linear::archive_initiative(
            &main,
            &archive.initiative,
        )) {
            archive.error = Some(error.to_string());
        }
    }
    let archived = receipt
        .initiatives
        .iter()
        .all(|archive| archive.error.is_none());
    receipt.files = Some(if !has_files {
        AuthoredFiles::Unchanged
    } else if !archived {
        // The Initiative id lives in these files; keep them until a retry archives it.
        AuthoredFiles::Uncommitted {
            reason: "kept until the Linear Initiative is archived".into(),
        }
    } else {
        let mut paths = remove_authored(&main, &receipt.name, receipt.destination.as_deref())?;
        paths.push(authored_path(&receipt.name));
        commit_authored(&main, &paths, &format!("lf wave remove: {}", receipt.name))
    });
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

/// Delete the removed Wave's directory on main, first folding its memory and the
/// config keys its parent lacks into the parent. Returns other changed paths.
fn remove_authored(main: &Path, slug: &str, parent: Option<&str>) -> OpsResult<Vec<String>> {
    let directory = main.join(authored_path(slug));
    let mut changed = Vec::new();
    if let Some(parent) = parent {
        let parent_dir = main.join(authored_path(parent));
        fold_memory(&directory, slug, &parent_dir.join("MEMORY.md"))?;
        fold_config(
            &directory.join("config.yaml"),
            &parent_dir.join("config.yaml"),
        )?;
        changed.push(authored_path(parent));
        changed.extend(rewrite_references(main, slug, parent)?);
    }
    std::fs::remove_dir_all(&directory)
        .map_err(|error| lifecycle_error(format!("{}: {error}", directory.display())))?;
    Ok(changed)
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
fn rewrite_references(main: &Path, slug: &str, replacement: &str) -> OpsResult<Vec<String>> {
    let pattern = path_reference(slug);
    let mut changed = Vec::new();
    for document in wave_documents(main, slug) {
        let Ok(text) = std::fs::read_to_string(&document) else {
            continue;
        };
        let rewritten = pattern.replace_all(&text, format!(".lf/wave/{replacement}$end").as_str());
        if rewritten != text {
            std::fs::write(&document, rewritten.as_ref())
                .map_err(|error| lifecycle_error(format!("{}: {error}", document.display())))?;
            if let Ok(relative) = document.strip_prefix(main) {
                changed.push(relative.to_string_lossy().to_string());
            }
        }
    }
    Ok(changed)
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
        lines.push(match &removal.destination {
            Some(parent) if removal.preview => format!(
                ".lf/wave/{name} folds its memory into {parent} and is deleted on the main checkout."
            ),
            None if removal.preview => {
                format!(".lf/wave/{name} is deleted on the main checkout; Git history keeps it.")
            }
            _ => format!(".lf/wave/{name} removed on the main checkout."),
        });
    }
    for archive in &removal.initiatives {
        if removal.preview {
            lines.push(format!(
                "Linear Initiative {} for {} will be archived.",
                archive.initiative, archive.wave
            ));
        }
    }
    lines.extend(leftovers(removal));
    lines
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
    if let Some(AuthoredFiles::Uncommitted { reason }) = &removal.files {
        lines.push(format!(
            ".lf/wave/{} is not committed: {reason}.",
            removal.name
        ));
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
    let wave = match resolve_wave(&store, &main, wave)? {
        Some(wave) => wave,
        None => {
            // Already renamed: the old address is free and the new one is live.
            if let Some(target) = &target {
                if let Some(existing) = resolve_wave(&store, &main, target)? {
                    return Ok(WaveRenamed {
                        id: existing.id().clone(),
                        from: wave.to_string(),
                        to: existing.slug().to_string(),
                        renamed: false,
                        files: AuthoredFiles::Unchanged,
                        initiative: None,
                        mentions: Vec::new(),
                        summary: Vec::new(),
                    });
                }
            }
            return Err(lifecycle_error(format!("no Wave {wave} here")));
        }
    };
    let from = wave.slug().to_string();
    let to = target.clone().unwrap_or_else(|| from.clone());
    let moves = to != from || target_repo.is_some();
    // Refuse before changing the store: the directory move would fail after it.
    if to != from
        && target_repo.is_none()
        && main.join(authored_path(&from)).exists()
        && main.join(authored_path(&to)).exists()
    {
        return Err(lifecycle_error(format!(
            ".lf/wave/{to} already exists in {}; move or delete it first",
            main.display()
        )));
    }
    let relocation = if moves {
        Some(
            crate::work::wave::relocate::relocate_wave(
                &store,
                wave.id(),
                &main,
                target_repo,
                Some(&to),
            )
            .await
            .map_err(lifecycle_error)?,
        )
    } else {
        None
    };
    let same_repo = relocation
        .as_ref()
        .is_none_or(|receipt| receipt.from_repo == receipt.to_repo);
    let source = main.join(authored_path(&from));
    let files = if !moves || !source.exists() {
        AuthoredFiles::Unchanged
    } else if !same_repo {
        AuthoredFiles::Uncommitted {
            reason: format!(
                "move .lf/wave/{from} into the target repository; files do not cross repositories"
            ),
        }
    } else {
        let destination = main.join(authored_path(&to));
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| lifecycle_error(format!("{}: {error}", parent.display())))?;
        }
        std::fs::rename(&source, &destination)
            .map_err(|error| lifecycle_error(format!("{}: {error}", source.display())))?;
        let mut paths = vec![authored_path(&from), authored_path(&to)];
        paths.extend(rewrite_references(&main, &from, &to)?);
        commit_authored(&main, &paths, &format!("lf wave rename: {from} → {to}"))
    };
    let checkout = relocation
        .as_ref()
        .map_or_else(|| main.clone(), |receipt| PathBuf::from(&receipt.to_repo));
    let initiative = match super::linear::read_initiative(&checkout, &to) {
        Some(initiative) if moves || title.is_some() => {
            let title = title.map_or_else(|| super::linear::title_case(&to), str::to_string);
            let error = super::linear::rename(
                &checkout,
                &super::linear::LinearRenameOptions {
                    wave: Some(to.clone()),
                    title: title.clone(),
                },
                &crate::ops::NullProgress,
            )
            .await
            .err()
            .map(|error| error.to_string());
            Some(InitiativeRename {
                initiative,
                title,
                error,
            })
        }
        _ => None,
    };
    let mut renamed = WaveRenamed {
        id: wave.id().clone(),
        mentions: if moves {
            prose_mentions(&main, &from)
        } else {
            Vec::new()
        },
        from,
        to,
        renamed: moves,
        files,
        initiative,
        summary: Vec::new(),
    };
    renamed.summary = rename_summary(&renamed);
    Ok(renamed)
}

/// What the rename left for a person or agent to finish.
fn rename_summary(renamed: &WaveRenamed) -> Vec<String> {
    let mut lines = Vec::new();
    if let AuthoredFiles::Uncommitted { reason } = &renamed.files {
        lines.push(format!(
            ".lf/wave/{} is not committed: {reason}.",
            renamed.to
        ));
    }
    if let Some(InitiativeRename {
        initiative,
        error: Some(error),
        ..
    }) = &renamed.initiative
    {
        lines.push(format!(
            "Linear Initiative {initiative} kept its title: {error}. Run `lf wave rename {} --title <title>` to retry.",
            renamed.to
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
