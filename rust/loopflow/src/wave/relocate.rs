use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use fs2::FileExt;
use serde::{Deserialize, Serialize};

use crate::durable::WorkRef;
use crate::id::WaveId;
use crate::repository::CanonicalRepo;
use crate::store::{Store, WaveLocatorUpdate};
use crate::wave::server::{live_endpoint, ENDPOINT_FILE};
use crate::wave::wire::RESIDENT_TOKEN_FILE;
use crate::wave::{Wave, WaveLocator};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaveRelocationReceipt {
    pub wave_id: String,
    pub from_repo: String,
    pub from_name: String,
    pub to_repo: String,
    pub to_name: String,
    pub waves_moved: usize,
}

#[derive(Debug)]
pub(crate) struct WaveLocatorLock {
    _file: File,
}

impl WaveLocatorLock {
    pub(crate) fn acquire(repo: &Path, slug: &str) -> Result<Self> {
        let path = repo
            .join(".lf/tmp/locks/waves")
            .join(format!("{slug}.lock"));
        let parent = path
            .parent()
            .ok_or_else(|| anyhow!("Wave lock has no parent: {}", path.display()))?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create Wave lock directory {}", parent.display()))?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .with_context(|| format!("open Wave locator lock {}", path.display()))?;
        file.try_lock_exclusive().map_err(|error| {
            anyhow!(
                "Wave locator {}/{} is active or being relocated; stop it first (--force cannot break the relocation fence): {error}",
                repo.display(),
                slug
            )
        })?;
        Ok(Self { _file: file })
    }
}

#[derive(Debug, Clone)]
struct PlannedWaveMove {
    wave: Wave,
    target: WaveLocator,
    retire_collision: Option<WaveId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RelocationPath {
    wave_id: WaveId,
    from_repo: String,
    from_name: String,
    to_repo: String,
    to_name: String,
}

impl RelocationPath {
    fn from_planned(planned: &PlannedWaveMove) -> Self {
        Self {
            wave_id: planned.wave.id().clone(),
            from_repo: planned.wave.repo().to_string(),
            from_name: planned.wave.name().to_string(),
            to_repo: planned.target.repo().to_string(),
            to_name: planned.target.slug().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RelocationRecovery {
    receipt: WaveRelocationReceipt,
    paths: Vec<RelocationPath>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TreeEntry {
    Directory,
    File(Vec<u8>),
    Symlink(PathBuf),
}

pub async fn relocate_wave(
    store: &Store,
    wave_id: &WaveId,
    invoking_repo: &Path,
    target_repo: Option<&Path>,
    target_name: Option<&str>,
) -> Result<WaveRelocationReceipt> {
    let original_wave = store
        .get_wave(wave_id)
        .await?
        .ok_or_else(|| anyhow!("Wave {wave_id} is not registered"))?;
    if let Some(retired_at) = original_wave.retired_at() {
        return Err(anyhow!(
            "Wave {wave_id} retired at {retired_at}; it was superseded by {}",
            original_wave
                .superseded_by_wave_id()
                .map_or("-", WaveId::as_str)
        ));
    }
    let invoking_repo = CanonicalRepo::discover(invoking_repo)?;
    let mut wave = original_wave.clone();
    if CanonicalRepo::discover(Path::new(wave.repo())).is_ok_and(|repo| repo == invoking_repo) {
        let locator = WaveLocator::new(invoking_repo.clone(), wave.name())?;
        let scoped = store
            .get_wave_at(&locator)
            .await?
            .ok_or_else(|| anyhow!("Wave {} disappeared during repository repair", wave.id()))?;
        if scoped.id() != wave.id() {
            return Err(anyhow!(
                "repository repair resolved {}/{} to Wave {}, not {}",
                locator.repo(),
                locator.slug(),
                scoped.id(),
                wave.id()
            ));
        }
        wave = scoped;
    }
    WaveLocator::new(invoking_repo.clone(), wave.name())?;
    let target_repo = match target_repo {
        Some(repo) => CanonicalRepo::discover(repo)?,
        None => CanonicalRepo::discover(Path::new(wave.repo())).map_err(|error| {
            anyhow!(
                "Wave {} has stale repository {}; pass --repo <existing-checkout>: {error}",
                wave.id(),
                wave.repo()
            )
        })?,
    };
    let target = WaveLocator::new(target_repo, target_name.unwrap_or(wave.name()))?;
    let source_repo = CanonicalRepo::discover(Path::new(wave.repo())).ok();
    match source_repo.as_ref() {
        Some(source) if source != &invoking_repo => {
            return Err(anyhow!(
                "Wave {} belongs to {}; invoke relocation from that repository",
                wave.id(),
                wave.repo()
            ));
        }
        None if target.repo() != &invoking_repo => {
            return Err(anyhow!(
                "Wave {} has stale repository {}; invoke relocation from its explicit target {}",
                wave.id(),
                wave.repo(),
                target.repo()
            ));
        }
        _ => {}
    }
    if wave.repo() == target.repo().to_string() && wave.name() == target.slug() {
        if let Some(receipt) = recover_committed_relocation(store, &wave, &invoking_repo).await? {
            return Ok(receipt);
        }
        if original_wave.repo() != wave.repo() || original_wave.name() != wave.name() {
            return Ok(WaveRelocationReceipt {
                wave_id: wave.id().to_string(),
                from_repo: original_wave.repo().to_string(),
                from_name: original_wave.name().to_string(),
                to_repo: wave.repo().to_string(),
                to_name: wave.name().to_string(),
                waves_moved: 1,
            });
        }
        return Err(anyhow!("relocation must change --repo or --name"));
    }
    if wave.repo() != target.repo().to_string() && wave.parent_wave_id().is_some() {
        return Err(anyhow!(
            "repository relocation must start from a root Wave; {} has parent {}",
            wave.id(),
            wave.parent_wave_id().expect("parent checked as present")
        ));
    }
    ensure_repository_team_compatible(&wave, &target)?;

    let mut moves = plan_moves(store, wave, target).await?;
    preflight(store, &mut moves).await?;
    let recovery = relocation_recovery(&moves);
    let _locks = acquire_locks(&recovery.paths)?;
    ensure_no_shadow_relocation_receipts(&moves)?;
    ensure_no_live_endpoints(&recovery.paths).await?;
    write_recovery(&recovery)?;

    for planned in &moves {
        stage_wave_paths(planned)?;
    }

    let updates = moves
        .iter()
        .map(|planned| WaveLocatorUpdate {
            wave_id: planned.wave.id().clone(),
            expected_repo: planned.wave.repo().to_string(),
            expected_slug: planned.wave.name().to_string(),
            target: planned.target.clone(),
            retire_collision: planned.retire_collision.clone(),
        })
        .collect();
    store.relocate_waves(updates).await?;

    for planned in &moves {
        remove_old_paths(&RelocationPath::from_planned(planned)).with_context(|| {
            format!(
                "Wave {} relocation committed at {}/{} but cleanup of {}/{} failed",
                planned.wave.id(),
                planned.target.repo(),
                planned.target.slug(),
                planned.wave.repo(),
                planned.wave.name()
            )
        })?;
    }
    remove_recovery(&recovery)?;

    let root = moves
        .first()
        .expect("relocation always contains the requested Wave");
    Ok(WaveRelocationReceipt {
        wave_id: root.wave.id().to_string(),
        from_repo: root.wave.repo().to_string(),
        from_name: root.wave.name().to_string(),
        to_repo: root.target.repo().to_string(),
        to_name: root.target.slug().to_string(),
        waves_moved: moves.len(),
    })
}

fn ensure_repository_team_compatible(wave: &Wave, target: &WaveLocator) -> Result<()> {
    let Ok(source) = CanonicalRepo::discover(Path::new(wave.repo())) else {
        return Ok(());
    };
    if &source == target.repo() {
        return Ok(());
    }
    let source_team = crate::ops::pm::repository_team_for_snapshot_validation(source.as_path())
        .map_err(|error| anyhow!(error.to_string()))?;
    let target_team =
        crate::ops::pm::repository_team_for_snapshot_validation(target.repo().as_path())
            .map_err(|error| anyhow!(error.to_string()))?;
    if let (Some(source_team), Some(target_team)) = (source_team, target_team) {
        if source_team != target_team {
            return Err(anyhow!(
                "cannot rehome Wave {} from repository Team {} to {}; run `lf pm reteam` explicitly before relocating",
                wave.id(),
                source_team,
                target_team
            ));
        }
    }
    Ok(())
}

async fn plan_moves(
    store: &Store,
    root: Wave,
    target: WaveLocator,
) -> Result<Vec<PlannedWaveMove>> {
    let rehome = root.repo() != target.repo().to_string();
    let rename = root.name() != target.slug();
    let mut moves = vec![PlannedWaveMove {
        wave: root.clone(),
        target,
        retire_collision: None,
    }];
    if !rehome && !rename {
        return Ok(moves);
    }

    let mut pending = VecDeque::from([root.id().clone()]);
    while let Some(parent) = pending.pop_front() {
        for child in store.list_child_waves(&parent).await? {
            pending.push_back(child.id().clone());
            let target_slug =
                relocated_descendant_slug(root.name(), moves[0].target.slug(), child.name());
            if !rehome && target_slug.is_none() {
                continue;
            }
            moves.push(PlannedWaveMove {
                target: WaveLocator::new(
                    moves[0].target.repo().clone(),
                    target_slug.as_deref().unwrap_or(child.name()),
                )?,
                wave: child,
                retire_collision: None,
            });
        }
    }
    Ok(moves)
}

fn relocated_descendant_slug(root: &str, target: &str, candidate: &str) -> Option<String> {
    let relative = Path::new(candidate).strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        return None;
    }
    Path::new(target)
        .join(relative)
        .to_str()
        .map(str::to_string)
}

async fn preflight(store: &Store, moves: &mut [PlannedWaveMove]) -> Result<()> {
    let moved_ids = moves
        .iter()
        .map(|planned| planned.wave.id().to_string())
        .collect::<BTreeSet<_>>();
    let registered = store.list_waves(None).await?;
    for planned in moves {
        ensure_move_paths_do_not_overlap(planned)?;
        if let Some(existing) = store.get_wave_at(&planned.target).await? {
            if existing.id() != planned.wave.id() {
                let blockers = store.wave_retirement_blockers(existing.id()).await?;
                let mut blockers = blockers;
                if recovery_path_at(planned.target.repo().as_path(), existing.id()).exists() {
                    blockers.push("relocation receipt".to_string());
                }
                if !blockers.is_empty() {
                    return Err(anyhow!(
                        "cannot relocate established Wave {} over destination Wave {} at {}/{}: {}",
                        planned.wave.id(),
                        existing.id(),
                        planned.target.repo(),
                        planned.target.slug(),
                        blockers.join(", ")
                    ));
                }
                planned.retire_collision = Some(existing.id().clone());
            }
        }
        let source_repo = CanonicalRepo::discover(Path::new(planned.wave.repo())).ok();
        for other in &registered {
            if moved_ids.contains(&other.id().to_string()) {
                continue;
            }
            let Ok(other_repo) = CanonicalRepo::discover(Path::new(other.repo())) else {
                continue;
            };
            let inside_source = source_repo.as_ref().is_some_and(|repo| {
                repo == &other_repo && is_strict_descendant(other.name(), planned.wave.name())
            });
            let inside_target = &other_repo == planned.target.repo()
                && is_strict_descendant(other.name(), planned.target.slug());
            if inside_source || inside_target {
                return Err(anyhow!(
                    "cannot relocate {}/{} because it contains registered Wave {}/{} ({})",
                    planned.wave.repo(),
                    planned.wave.name(),
                    other.repo(),
                    other.name(),
                    other.id()
                ));
            }
        }
        ensure_wave_stopped(store, planned.wave.id()).await?;
    }
    Ok(())
}

fn ensure_no_shadow_relocation_receipts(moves: &[PlannedWaveMove]) -> Result<()> {
    for planned in moves {
        let Some(shadow) = &planned.retire_collision else {
            continue;
        };
        if recovery_path_at(planned.target.repo().as_path(), shadow).exists() {
            return Err(anyhow!(
                "cannot retire destination Wave {shadow}: relocation receipt"
            ));
        }
    }
    Ok(())
}

fn ensure_move_paths_do_not_overlap(planned: &PlannedWaveMove) -> Result<()> {
    let source_repo = Path::new(planned.wave.repo());
    let target_repo = planned.target.repo().as_path();
    for (source, target) in [
        (
            authored_path(source_repo, planned.wave.name()),
            authored_path(target_repo, planned.target.slug()),
        ),
        (
            journal_path(source_repo, planned.wave.name()),
            journal_path(target_repo, planned.target.slug()),
        ),
    ] {
        if source != target && (source.starts_with(&target) || target.starts_with(&source)) {
            return Err(anyhow!(
                "Wave relocation paths overlap; choose a sibling locator: {} -> {}",
                source.display(),
                target.display()
            ));
        }
    }
    Ok(())
}

fn is_strict_descendant(candidate: &str, parent: &str) -> bool {
    candidate != parent && Path::new(candidate).starts_with(parent)
}

async fn ensure_wave_stopped(store: &Store, wave_id: &WaveId) -> Result<()> {
    let mut work = vec![WorkRef::Wave(wave_id.clone())];
    work.extend(
        store
            .list_projects(Some(wave_id))
            .await?
            .into_iter()
            .map(|project| WorkRef::Project(project.id)),
    );
    work.extend(
        store
            .list_tasks(Some(wave_id))
            .await?
            .into_iter()
            .map(|task| WorkRef::Task(task.id)),
    );
    for work in work {
        if let Some(run) = store.current_run(&work).await? {
            return Err(anyhow!(
                "cannot relocate Wave {wave_id} while Run {} owns {} {}",
                run.id,
                work.kind(),
                work.id()
            ));
        }
    }
    Ok(())
}

fn acquire_locks(paths: &[RelocationPath]) -> Result<Vec<WaveLocatorLock>> {
    let mut locators = BTreeSet::new();
    for path in paths {
        let source_repo = CanonicalRepo::discover(Path::new(&path.from_repo))
            .map(|repo| repo.as_path().to_path_buf())
            .unwrap_or_else(|_| PathBuf::from(&path.from_repo));
        if source_repo.is_dir() {
            locators.insert((source_repo, path.from_name.clone()));
        }
        locators.insert((PathBuf::from(&path.to_repo), path.to_name.clone()));
    }
    locators
        .into_iter()
        .map(|(repo, slug)| WaveLocatorLock::acquire(&repo, &slug))
        .collect()
}

async fn ensure_no_live_endpoints(paths: &[RelocationPath]) -> Result<()> {
    let mut locators = BTreeSet::new();
    for path in paths {
        locators.insert((PathBuf::from(&path.from_repo), path.from_name.as_str()));
        locators.insert((PathBuf::from(&path.to_repo), path.to_name.as_str()));
    }
    for (repo, slug) in locators {
        if let Some(endpoint) = live_endpoint(&repo, slug).await {
            return Err(anyhow!(
                "cannot relocate live Wave at {}/{} ({endpoint})",
                repo.display(),
                slug
            ));
        }
    }
    Ok(())
}

fn authored_path(repo: &Path, slug: &str) -> PathBuf {
    repo.join("wave").join(slug)
}

fn journal_path(repo: &Path, slug: &str) -> PathBuf {
    repo.join(".lf/journal/waves").join(slug)
}

fn stage_wave_paths(planned: &PlannedWaveMove) -> Result<()> {
    let source_repo = Path::new(planned.wave.repo());
    let target_repo = planned.target.repo().as_path();
    let stale_source = !source_repo.is_dir();
    stage_tree(
        &authored_path(source_repo, planned.wave.name()),
        &authored_path(target_repo, planned.target.slug()),
        true,
        true,
        stale_source,
    )?;
    stage_tree(
        &journal_path(source_repo, planned.wave.name()),
        &journal_path(target_repo, planned.target.slug()),
        false,
        false,
        stale_source,
    )
}

fn stage_tree(
    source: &Path,
    target: &Path,
    required: bool,
    skip_boot_files: bool,
    stale_source: bool,
) -> Result<()> {
    if source == target {
        return Ok(());
    }
    if !source.exists() {
        if stale_source && (target.exists() || !required) {
            return Ok(());
        }
        if !required && !target.exists() {
            return Ok(());
        }
        return Err(anyhow!(
            "relocation source is missing and the target cannot be trusted: {} -> {}",
            source.display(),
            target.display()
        ));
    }
    if target.exists() {
        if tree_contents(source, skip_boot_files)? == tree_contents(target, skip_boot_files)? {
            remove_boot_files(target)?;
            return Ok(());
        }
        return Err(anyhow!(
            "relocation target diverges from source: {} -> {}",
            source.display(),
            target.display()
        ));
    }

    let parent = target
        .parent()
        .ok_or_else(|| anyhow!("relocation target has no parent: {}", target.display()))?;
    std::fs::create_dir_all(parent)?;
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("wave");
    let temporary = parent.join(format!(
        ".{name}.lf-relocate-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let staged = (|| {
        copy_tree(source, &temporary, skip_boot_files)?;
        if tree_contents(source, skip_boot_files)? != tree_contents(&temporary, skip_boot_files)? {
            return Err(anyhow!(
                "staged relocation did not verify: {}",
                temporary.display()
            ));
        }
        std::fs::rename(&temporary, target).with_context(|| {
            format!(
                "publish staged Wave path {} -> {}",
                temporary.display(),
                target.display()
            )
        })?;
        Ok(())
    })();
    if staged.is_err() {
        let _ = std::fs::remove_dir_all(&temporary);
    }
    staged
}

fn copy_tree(source: &Path, target: &Path, skip_boot_files: bool) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if skip_boot_files
            && name
                .to_str()
                .is_some_and(|name| matches!(name, ENDPOINT_FILE | RESIDENT_TOKEN_FILE))
        {
            continue;
        }
        let source_path = entry.path();
        let target_path = target.join(&name);
        let metadata = std::fs::symlink_metadata(&source_path)?;
        if metadata.is_dir() {
            copy_tree(&source_path, &target_path, skip_boot_files)?;
        } else if metadata.file_type().is_symlink() {
            copy_symlink(&source_path, &target_path)?;
        } else {
            std::fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn copy_symlink(source: &Path, target: &Path) -> Result<()> {
    std::os::unix::fs::symlink(std::fs::read_link(source)?, target)?;
    Ok(())
}

#[cfg(not(unix))]
fn copy_symlink(source: &Path, _target: &Path) -> Result<()> {
    Err(anyhow!(
        "cannot preserve symlink during relocation: {}",
        source.display()
    ))
}

fn tree_contents(root: &Path, skip_boot_files: bool) -> Result<BTreeMap<PathBuf, TreeEntry>> {
    fn visit(
        root: &Path,
        current: &Path,
        skip_boot_files: bool,
        entries: &mut BTreeMap<PathBuf, TreeEntry>,
    ) -> Result<()> {
        for entry in std::fs::read_dir(current)? {
            let entry = entry?;
            let name = entry.file_name();
            if skip_boot_files
                && name
                    .to_str()
                    .is_some_and(|name| matches!(name, ENDPOINT_FILE | RESIDENT_TOKEN_FILE))
            {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(root)?.to_path_buf();
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.is_dir() {
                entries.insert(relative, TreeEntry::Directory);
                visit(root, &path, skip_boot_files, entries)?;
            } else if metadata.file_type().is_symlink() {
                entries.insert(relative, TreeEntry::Symlink(std::fs::read_link(path)?));
            } else {
                entries.insert(relative, TreeEntry::File(std::fs::read(path)?));
            }
        }
        Ok(())
    }

    let mut entries = BTreeMap::new();
    visit(root, root, skip_boot_files, &mut entries)?;
    Ok(entries)
}

fn remove_boot_files(path: &Path) -> Result<()> {
    for name in [ENDPOINT_FILE, RESIDENT_TOKEN_FILE] {
        match std::fs::remove_file(path.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn remove_old_paths(path: &RelocationPath) -> Result<()> {
    let source_repo = Path::new(&path.from_repo);
    let target_repo = Path::new(&path.to_repo);
    for (source, target, skip_boot_files) in [
        (
            authored_path(source_repo, &path.from_name),
            authored_path(target_repo, &path.to_name),
            true,
        ),
        (
            journal_path(source_repo, &path.from_name),
            journal_path(target_repo, &path.to_name),
            false,
        ),
    ] {
        if source != target && source.exists() {
            if !target.exists()
                || tree_contents(&source, skip_boot_files)?
                    != tree_contents(&target, skip_boot_files)?
            {
                return Err(anyhow!(
                    "old Wave path changed after staging; preserving both copies: {} -> {}",
                    source.display(),
                    target.display()
                ));
            }
            std::fs::remove_dir_all(&source)
                .with_context(|| format!("remove old Wave path {}", source.display()))?;
        }
    }
    Ok(())
}

fn relocation_recovery(moves: &[PlannedWaveMove]) -> RelocationRecovery {
    let root = moves
        .first()
        .expect("relocation always contains the requested Wave");
    RelocationRecovery {
        receipt: WaveRelocationReceipt {
            wave_id: root.wave.id().to_string(),
            from_repo: root.wave.repo().to_string(),
            from_name: root.wave.name().to_string(),
            to_repo: root.target.repo().to_string(),
            to_name: root.target.slug().to_string(),
            waves_moved: moves.len(),
        },
        paths: moves.iter().map(RelocationPath::from_planned).collect(),
    }
}

fn recovery_path(recovery: &RelocationRecovery) -> PathBuf {
    Path::new(&recovery.receipt.to_repo)
        .join(".lf/tmp/wave-relocations")
        .join(format!("{}.json", recovery.receipt.wave_id))
}

fn recovery_path_at(repo: &Path, wave_id: &WaveId) -> PathBuf {
    repo.join(".lf/tmp/wave-relocations")
        .join(format!("{wave_id}.json"))
}

fn recovery_path_for_wave(wave: &Wave) -> PathBuf {
    Path::new(wave.repo())
        .join(".lf/tmp/wave-relocations")
        .join(format!("{}.json", wave.id()))
}

fn write_recovery(recovery: &RelocationRecovery) -> Result<()> {
    let path = recovery_path(recovery);
    if path.exists() {
        let existing: RelocationRecovery = serde_json::from_slice(&std::fs::read(&path)?)?;
        if existing == *recovery {
            return Ok(());
        }
        return Err(anyhow!(
            "Wave {} has a different unfinished relocation at {}",
            recovery.receipt.wave_id,
            path.display()
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("relocation recovery path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("relocation"),
        uuid::Uuid::new_v4().simple()
    ));
    let bytes = serde_json::to_vec_pretty(recovery)?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    std::io::Write::write_all(&mut file, &bytes)?;
    file.sync_all()?;
    std::fs::rename(&temporary, &path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn remove_recovery(recovery: &RelocationRecovery) -> Result<()> {
    let path = recovery_path(recovery);
    match std::fs::remove_file(&path) {
        Ok(()) => {
            let parent = path.parent().ok_or_else(|| {
                anyhow!("relocation recovery path has no parent: {}", path.display())
            })?;
            File::open(parent)?.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn recover_committed_relocation(
    store: &Store,
    wave: &Wave,
    invoking_repo: &CanonicalRepo,
) -> Result<Option<WaveRelocationReceipt>> {
    let path = recovery_path_for_wave(wave);
    if !path.exists() {
        return Ok(None);
    }
    if Path::new(wave.repo()) != invoking_repo.as_path() {
        return Err(anyhow!(
            "unfinished relocation for Wave {} must be recovered from {}",
            wave.id(),
            wave.repo()
        ));
    }
    let recovery: RelocationRecovery = serde_json::from_slice(&std::fs::read(&path)?)
        .with_context(|| format!("read relocation recovery {}", path.display()))?;
    if recovery.receipt.wave_id != wave.id().to_string() {
        return Err(anyhow!(
            "relocation recovery {} belongs to Wave {}, not {}",
            path.display(),
            recovery.receipt.wave_id,
            wave.id()
        ));
    }
    for move_path in &recovery.paths {
        let current = store.get_wave(&move_path.wave_id).await?.ok_or_else(|| {
            anyhow!(
                "Wave {} disappeared during relocation recovery",
                move_path.wave_id
            )
        })?;
        if current.repo() != move_path.to_repo || current.name() != move_path.to_name {
            return Err(anyhow!(
                "relocation recovery cannot clean up before Wave {} reaches {}/{}",
                current.id(),
                move_path.to_repo,
                move_path.to_name
            ));
        }
        ensure_wave_stopped(store, current.id()).await?;
    }
    let _locks = acquire_locks(&recovery.paths)?;
    ensure_no_live_endpoints(&recovery.paths).await?;
    for move_path in &recovery.paths {
        remove_old_paths(move_path)?;
    }
    remove_recovery(&recovery)?;
    Ok(Some(recovery.receipt))
}

#[cfg(test)]
mod tests {
    use super::{
        recovery_path, relocate_wave, relocation_recovery, remove_old_paths, stage_tree,
        write_recovery, PlannedWaveMove, RelocationPath,
    };
    use crate::id::WaveId;
    use crate::store::{open_store, StorageConfig, WaveLocatorUpdate};
    use crate::wave::{Wave, WaveLocator};

    #[test]
    fn committed_cleanup_preserves_a_source_that_changed_after_staging() {
        let tmp = tempfile::tempdir().unwrap();
        let source_repo = tmp.path().join("source");
        let target_repo = tmp.path().join("target");
        std::fs::create_dir_all(source_repo.join("wave/infrastructure")).unwrap();
        std::fs::create_dir_all(target_repo.join("wave/platform")).unwrap();
        std::fs::write(
            source_repo.join("wave/infrastructure/GOAL.md"),
            "changed after staging\n",
        )
        .unwrap();
        std::fs::write(
            target_repo.join("wave/platform/GOAL.md"),
            "staged content\n",
        )
        .unwrap();
        let path = RelocationPath {
            wave_id: WaveId::new(),
            from_repo: source_repo.display().to_string(),
            from_name: "infrastructure".to_string(),
            to_repo: target_repo.display().to_string(),
            to_name: "platform".to_string(),
        };

        let error = remove_old_paths(&path).unwrap_err();

        assert!(error.to_string().contains("preserving both copies"));
        assert!(source_repo.join("wave/infrastructure/GOAL.md").is_file());
        assert!(target_repo.join("wave/platform/GOAL.md").is_file());
    }

    #[test]
    fn absent_source_journal_never_adopts_an_unrelated_target() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source/journal");
        let target = tmp.path().join("target/journal");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("events.jsonl"), "unrelated\n").unwrap();

        let error = stage_tree(&source, &target, false, false, false).unwrap_err();

        assert!(error.to_string().contains("target cannot be trusted"));
        assert_eq!(
            std::fs::read_to_string(target.join("events.jsonl")).unwrap(),
            "unrelated\n"
        );
    }

    #[tokio::test]
    async fn retry_after_commit_finishes_filesystem_cleanup() {
        let tmp = tempfile::tempdir().unwrap();
        let source_repo = tmp.path().join("source");
        let target_repo = tmp.path().join("target");
        std::fs::create_dir_all(source_repo.join("wave/infrastructure")).unwrap();
        std::fs::create_dir_all(&target_repo).unwrap();
        std::fs::write(
            source_repo.join("wave/infrastructure/GOAL.md"),
            "# infrastructure\n",
        )
        .unwrap();
        let store = open_store(&StorageConfig::sqlite(tmp.path().join("registry.db")))
            .await
            .unwrap();
        let wave = Wave::new(
            WaveId::new(),
            "infrastructure".to_string(),
            std::fs::canonicalize(&source_repo)
                .unwrap()
                .display()
                .to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let target = WaveLocator::discover(&target_repo, "platform").unwrap();
        let planned = PlannedWaveMove {
            wave: wave.clone(),
            target: target.clone(),
            retire_collision: None,
        };
        super::stage_wave_paths(&planned).unwrap();
        let recovery = relocation_recovery(std::slice::from_ref(&planned));
        write_recovery(&recovery).unwrap();
        store
            .relocate_waves(vec![WaveLocatorUpdate {
                wave_id: wave.id().clone(),
                expected_repo: wave.repo().to_string(),
                expected_slug: wave.name().to_string(),
                target,
                retire_collision: None,
            }])
            .await
            .unwrap();
        assert!(source_repo.join("wave/infrastructure").is_dir());
        assert!(recovery_path(&recovery).is_file());

        let receipt = relocate_wave(&store, wave.id(), &target_repo, None, None)
            .await
            .unwrap();

        assert_eq!(receipt.from_name, "infrastructure");
        assert_eq!(receipt.to_name, "platform");
        assert!(!source_repo.join("wave/infrastructure").exists());
        assert!(target_repo.join("wave/platform/GOAL.md").is_file());
        assert!(!recovery_path(&recovery).exists());
    }
}
