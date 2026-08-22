//! Durable Wave context: every `lf` run born inside a Wave inherits curated
//! `MEMORY.md` files. Conversation enters a run only when an operation selects
//! the relevant Turn explicitly.
//!
//! Resolution: explicit `--wave` (the caller passes it) > `LF_WAVE_ID` from a
//! managed Work process. Human names resolve only inside the canonical
//! repository; the UUID remains durable identity across locator changes.
//!
//! Wave state (journal, endpoint pointer, MEMORY.md) lives under the ORIGIN
//! repo — a worktree resolves its main repo first.

use crate::id::WaveId;
use crate::wave::server::endpoint_path;
use crate::wave::{Wave, WaveLocator};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The durable Wave attributed to this process.
pub const WAVE_ID_ENV: &str = "LF_WAVE_ID";

/// Resolve this process's ambient Wave name through the durable Wave id.
pub fn resolve_ambient_wave_name() -> Option<String> {
    let repo = crate::engine::repo::find_repo_root().ok();
    resolve_managed_wave_sync(repo.as_deref(), None)
        .ok()
        .map(|wave| wave.name().to_string())
}

/// The run-attribution decision for the current process: the wave name to
/// attribute a run to (if any), plus a classified failure to record when a
/// supplied managed identity failed validation.
///
/// - valid UUID or registered repository-local name → `wave: Some(name)`, `failure: None`
/// - no managed identity (`NoContext`) → `wave: None`, `failure: None`
///   (worktree inference stays a legitimate fallback for this case alone)
/// - stale UUID / registry read failure → `wave: None`, `failure: Some(...)`
///   naming the stale source and the safe explicit recovery (`--wave <name>`)
///
/// Attribution is non-fatal: a stale identity is never silently re-attributed to
/// a wave inferred from the worktree. The run records `None` and the failure so
/// the stale source stays visible and actionable; see W2-239.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunAttribution {
    pub wave: Option<String>,
    pub failure: Option<String>,
}

/// The run-attribution decision for the current process environment inside
/// `repo`. One classification shared by every trace/run attribution site
/// ([`crate::journal::ensure_run_context`] and the `lf` run wrapper).
pub fn run_attribution(repo: Option<&Path>) -> RunAttribution {
    match resolve_managed_wave_sync(repo, None) {
        Ok(wave) => RunAttribution {
            wave: Some(wave.name().to_string()),
            failure: None,
        },
        Err(WaveResolveError::NoContext) => RunAttribution {
            wave: None,
            failure: None,
        },
        Err(error) => RunAttribution {
            wave: None,
            failure: Some(attribution_failure_text(&error)),
        },
    }
}

/// The text recorded for a supplied identity that failed validation. The
/// `StaleIdentity` and `UnknownExplicit` `Display` already name the source and
/// the `--wave <name>` recovery; `Registry` does not, so the recovery hint is
/// appended.
fn attribution_failure_text(error: &WaveResolveError) -> String {
    match error {
        WaveResolveError::Registry(_) => format!("{error}; pass --wave <name> to recover"),
        _ => error.to_string(),
    }
}

/// Resolve a top-level `--wave` to the durable Wave row used by prompt,
/// journal, and child-process attribution. Surfaces the same
/// [`WaveResolveError`] classification as the shared resolver: an empty name is
/// [`WaveResolveError::EmptyExplicit`], an unknown name is
/// [`WaveResolveError::UnknownExplicit`].
pub fn resolve_explicit_wave(name: &str) -> anyhow::Result<Wave> {
    let repo = crate::engine::repo::find_repo_root().ok();
    resolve_managed_wave_sync(repo.as_deref(), Some(name)).map_err(anyhow::Error::from)
}

/// Why an ambient Wave could not be resolved. The cases a caller must be able
/// to tell apart: no context to resolve from, a context that named a Wave this
/// machine's registry has never seen (stale), an explicit `--wave` that was
/// empty, and an explicit `--wave` naming a Wave the registry has no row for.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WaveResolveError {
    /// No `--wave` and no `LF_WAVE_ID`: nothing to resolve from.
    #[error("no wave in context; pass --wave <name>")]
    NoContext,
    /// `LF_WAVE_ID` names a Wave (id or name) this machine's registry has no
    /// row for. The env outlived the registry it points into.
    #[error(
        "ambient wave '{0}' (LF_WAVE_ID) is not in this machine's registry; \
         the context is stale — pass --wave <name>"
    )]
    StaleIdentity(String),
    /// `--wave` was given but empty/whitespace after normalization.
    #[error("--wave requires a non-empty wave name")]
    EmptyExplicit,
    /// `--wave` named a wave this machine's registry has no row for.
    #[error(
        "wave '{0}' is not registered on this machine; \
         run `lf ls` to list known waves, or pass --wave <known-name>"
    )]
    UnknownExplicit(String),
    /// More than one repository owns the requested slug and the caller
    /// supplied no repository context.
    #[error("wave '{slug}' is ambiguous; it belongs to: {repositories}")]
    AmbiguousWave { slug: String, repositories: String },
    /// A durable UUID resolved, but not inside the repository invoking the
    /// command.
    #[error("wave {wave_id} belongs to {actual}, not invoking repository {expected}")]
    RepositoryMismatch {
        wave_id: WaveId,
        expected: String,
        actual: String,
    },
    /// The registry read itself failed (I/O, not a miss).
    #[error("failed to read wave registry: {0}")]
    Registry(String),
}

async fn resolve_slug(
    store: &crate::store::Store,
    repo: Option<&Path>,
    slug: &str,
) -> Result<Wave, WaveResolveError> {
    if let Some(repo) = repo {
        let locator = WaveLocator::discover(repo, slug)
            .map_err(|error| WaveResolveError::Registry(error.to_string()))?;
        return store
            .get_wave_at(&locator)
            .await
            .map_err(|error| WaveResolveError::Registry(error.to_string()))?
            .ok_or_else(|| WaveResolveError::UnknownExplicit(slug.to_string()));
    }

    let waves = store
        .find_waves_by_slug(slug)
        .await
        .map_err(|error| WaveResolveError::Registry(error.to_string()))?;
    match waves.as_slice() {
        [wave] => Ok(wave.clone()),
        [] => Err(WaveResolveError::UnknownExplicit(slug.to_string())),
        _ => Err(WaveResolveError::AmbiguousWave {
            slug: slug.to_string(),
            repositories: waves
                .iter()
                .map(|wave| wave.repo())
                .collect::<Vec<_>>()
                .join(", "),
        }),
    }
}

/// Resolve the durable Wave row selected by explicit or ambient context.
pub async fn resolve_managed_wave(
    store: Option<&crate::store::Store>,
    repo: Option<&Path>,
    explicit: Option<&str>,
    env_wave_id: Option<&str>,
) -> Result<Wave, WaveResolveError> {
    if let Some(raw) = explicit {
        if let Ok(id) = raw.trim().parse::<WaveId>() {
            let store = store.ok_or_else(|| {
                WaveResolveError::Registry("no wave registry on this machine".to_string())
            })?;
            let wave = store
                .get_wave(&id)
                .await
                .map_err(|error| WaveResolveError::Registry(error.to_string()))?
                .ok_or_else(|| WaveResolveError::UnknownExplicit(raw.to_string()))?;
            if wave.is_retired() {
                return Ok(wave);
            }
            if let Some(repo) = repo {
                let locator = WaveLocator::discover(repo, wave.name())
                    .map_err(|error| WaveResolveError::Registry(error.to_string()))?;
                let scoped = store
                    .get_wave_at(&locator)
                    .await
                    .map_err(|error| WaveResolveError::Registry(error.to_string()))?;
                if scoped.as_ref().map(Wave::id) != Some(&id) {
                    return Err(WaveResolveError::RepositoryMismatch {
                        wave_id: id,
                        expected: locator.repo().to_string(),
                        actual: wave.repo().to_string(),
                    });
                }
            }
            return Ok(wave);
        }
        let slug =
            crate::ops::util::normalize_wave_name(raw).ok_or(WaveResolveError::EmptyExplicit)?;
        let store = store.ok_or_else(|| {
            WaveResolveError::Registry("no wave registry on this machine".to_string())
        })?;
        return resolve_slug(store, repo, &slug).await;
    }

    let raw = env_wave_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(WaveResolveError::NoContext)?;
    let store = store.ok_or_else(|| WaveResolveError::StaleIdentity(raw.to_string()))?;
    if let Ok(id) = raw.parse::<WaveId>() {
        let wave = store
            .get_wave(&id)
            .await
            .map_err(|error| WaveResolveError::Registry(error.to_string()))?
            .ok_or_else(|| WaveResolveError::StaleIdentity(raw.to_string()))?;
        if wave.is_retired() {
            return Ok(wave);
        }
        if let Some(repo) = repo {
            let locator = WaveLocator::discover(repo, wave.name())
                .map_err(|error| WaveResolveError::Registry(error.to_string()))?;
            let scoped = store
                .get_wave_at(&locator)
                .await
                .map_err(|error| WaveResolveError::Registry(error.to_string()))?;
            if let Some(scoped) = scoped {
                if scoped.id() == &id {
                    return Ok(scoped);
                }
            }
            return Err(WaveResolveError::RepositoryMismatch {
                wave_id: id,
                expected: locator.repo().to_string(),
                actual: wave.repo().to_string(),
            });
        }
        return Ok(wave);
    }

    let slug = crate::ops::util::normalize_wave_name(raw).ok_or(WaveResolveError::NoContext)?;
    resolve_slug(store, repo, &slug)
        .await
        .map_err(|error| match error {
            WaveResolveError::UnknownExplicit(_) => {
                WaveResolveError::StaleIdentity(raw.to_string())
            }
            other => other,
        })
}

/// Resolve a durable Wave row from synchronous command and context assembly.
/// The caller supplies its repository scope; explicit names win over the
/// ambient `LF_WAVE_ID`. The scratch thread keeps this safe inside an existing
/// async runtime without creating a name-only resolution API.
pub fn resolve_managed_wave_sync(
    repo: Option<&Path>,
    explicit: Option<&str>,
) -> Result<Wave, WaveResolveError> {
    let repo = repo.map(Path::to_path_buf);
    let explicit = explicit.map(str::to_string);
    let env_wave_id = std::env::var(WAVE_ID_ENV).ok();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime always builds");
        runtime.block_on(async move {
            let store = crate::store::open_existing_store().await;
            resolve_managed_wave(
                store.as_ref(),
                repo.as_deref(),
                explicit.as_deref(),
                env_wave_id.as_deref(),
            )
            .await
        })
    })
    .join()
    .unwrap_or_else(|_| {
        Err(WaveResolveError::Registry(
            "resolver thread panicked".to_string(),
        ))
    })
}

/// Resolve a linked worktree to its canonical checkout once per process.
fn repo_origin(repo_root: &Path) -> PathBuf {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, PathBuf>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut cache = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache
        .entry(repo_root.to_path_buf())
        .or_insert_with(|| query_repo_origin(repo_root))
        .clone()
}

/// The single git call behind [`repo_origin`]: toplevel and common dir in
/// one `rev-parse`. A directory that is not itself a working-tree root
/// (fixture trees, plain directories) is its own origin — it must not walk
/// up into an enclosing checkout.
fn query_repo_origin(repo_root: &Path) -> PathBuf {
    let not_a_root = || repo_root.to_path_buf();
    let Ok(output) = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args([
            "rev-parse",
            "--path-format=absolute",
            "--show-toplevel",
            "--git-common-dir",
        ])
        .output()
    else {
        return not_a_root();
    };
    if !output.status.success() {
        return not_a_root();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines().map(str::trim);
    let toplevel = PathBuf::from(lines.next().unwrap_or_default());
    let common_dir = PathBuf::from(lines.next().unwrap_or_default());
    let toplevel = toplevel.canonicalize().unwrap_or(toplevel);
    let root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    if toplevel != root {
        return not_a_root();
    }
    common_dir
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| repo_root.to_path_buf())
}

/// The origin repo a wave's state lives under: the main checkout when
/// `repo_root` is a worktree root, `repo_root` itself otherwise (see
/// [`repo_origin`] for the guard).
pub fn wave_origin(repo_root: &Path) -> PathBuf {
    repo_origin(repo_root)
}

/// The Wave's prompt memory, read directly from applicable `MEMORY.md` files.
pub fn gather_wave_memory(repo_root: &Path, wave: &str) -> Option<String> {
    let origin = wave_origin(repo_root);
    let chain = memory_wave_chain(&origin, wave).unwrap_or_else(|| vec![wave.to_string()]);
    gather_memory_chain(&origin, &chain)
}

/// Resolve lexical memory scope through the registry.
fn memory_wave_chain(origin: &Path, wave: &str) -> Option<Vec<String>> {
    let origin = origin.to_path_buf();
    let wave = wave.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(async {
            let store = crate::store::open_existing_store().await?;
            memory_wave_chain_from_store(&store, &origin, &wave).await
        })
    })
    .join()
    .ok()
    .flatten()
}

async fn memory_wave_chain_from_store(
    store: &crate::store::Store,
    origin: &Path,
    wave: &str,
) -> Option<Vec<String>> {
    let locator = WaveLocator::discover(origin, wave).ok()?;
    let mut current = store.get_wave_at(&locator).await.ok().flatten()?;
    let mut seen = HashSet::new();
    let mut chain = Vec::new();
    loop {
        if !seen.insert(current.id().clone()) {
            tracing::warn!(
                wave,
                "cycle in parent_wave_id; using the acyclic memory prefix"
            );
            break;
        }
        chain.push(current.name().to_string());
        let Some(parent) = current.parent_wave_id() else {
            break;
        };
        current = match store.get_wave(parent).await.ok().flatten() {
            Some(parent) => parent,
            None => {
                tracing::warn!(wave, parent = %parent, "missing parent wave in memory scope");
                break;
            }
        };
    }
    chain.reverse();
    Some(chain)
}

/// Render each wave's memory oldest-ancestor first. A lone wave reads as its
/// own memory, unheadered; an inherited chain labels who owns what.
fn gather_memory_chain(origin: &Path, chain: &[String]) -> Option<String> {
    let leaf = chain.last()?;
    let scoped = chain
        .iter()
        .filter_map(|wave| {
            let base = crate::wave::memory::Memory::for_wave(origin, wave).read();
            let memory = render_wave_memory(&base)?;
            if chain.len() == 1 {
                return Some(memory);
            }
            let ownership = if wave == leaf {
                "owned by"
            } else {
                "inherited from"
            };
            Some(format!("## Memory {ownership} {wave}\n\n{memory}"))
        })
        .collect::<Vec<_>>();
    (!scoped.is_empty()).then(|| scoped.join("\n\n"))
}

/// The `wave/<name>/.wave-endpoint` discovery pointer's contents, trimmed.
/// Missing or empty pointer → `None`. `lf chat` uses this to find the Wave
/// conversation server; the Wave server owns writes.
pub fn read_endpoint_pointer(origin: &Path, wave: &str) -> Option<String> {
    let addr = std::fs::read_to_string(endpoint_path(origin, wave)).ok()?;
    let addr = addr.trim();
    (!addr.is_empty()).then(|| addr.to_string())
}

fn render_wave_memory(base: &str) -> Option<String> {
    let base = base.trim();
    (!base.is_empty()).then(|| base.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `run_attribution` keeps the classified failure instead of swallowing it.
    /// Absent context is `(None, None)` — worktree inference stays a legitimate
    /// fallback for it alone. A hand-set name resolves through the same scoped
    /// registry as every human command; an unregistered name is stale context,
    /// never self-authenticating identity.
    #[test]
    fn run_attribution_classifies_absent_context_and_hand_set_names() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let previous = std::env::var(WAVE_ID_ENV).ok();
        let repo = crate::engine::repo::find_repo_root().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let store = crate::store::open_store(&crate::store::StorageConfig::sqlite(
                ledger.home().join("loopflow.db"),
            ))
            .await
            .unwrap();
            crate::wave::registry::ensure_wave_row(&store, &repo, "product")
                .await
                .unwrap();
        });

        std::env::remove_var(WAVE_ID_ENV);
        let absent = run_attribution(Some(&repo));
        assert_eq!(absent.wave, None);
        assert_eq!(absent.failure, None);

        std::env::set_var(WAVE_ID_ENV, "product");
        let named = run_attribution(Some(&repo));
        assert_eq!(named.wave.as_deref(), Some("product"));
        assert_eq!(named.failure, None);

        std::env::set_var(WAVE_ID_ENV, "ghost");
        let unregistered = run_attribution(Some(&repo));
        assert_eq!(unregistered.wave, None);
        assert!(unregistered
            .failure
            .is_some_and(|failure| failure.contains("context is stale")));

        match previous {
            Some(value) => std::env::set_var(WAVE_ID_ENV, value),
            None => std::env::remove_var(WAVE_ID_ENV),
        }
    }

    #[test]
    fn wave_origin_of_a_worktree_is_the_main_checkout() {
        let repo = loopflow_test_support::TestRepo::new();
        let worktree = repo.create_named_worktree("origin-check");
        let origin = wave_origin(&worktree);
        assert_eq!(
            origin.canonicalize().unwrap(),
            repo.path().canonicalize().unwrap()
        );

        // A plain directory is its own origin.
        let tmp = tempfile::tempdir().expect("tempdir");
        assert_eq!(wave_origin(tmp.path()), tmp.path());
    }

    #[test]
    fn render_wave_memory_uses_only_the_file() {
        assert_eq!(
            render_wave_memory("# Memory\n\ncompiled base\n").as_deref(),
            Some("# Memory\n\ncompiled base")
        );
        assert!(render_wave_memory("  \n").is_none());
    }

    #[test]
    fn gather_wave_memory_reads_the_file_without_a_server() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/goals")).unwrap();
        std::fs::write(
            tmp.path().join("wave/goals/MEMORY.md"),
            "# Goals\n\ncompiled\n",
        )
        .unwrap();

        let memory = gather_wave_memory(tmp.path(), "goals").expect("memory");
        assert_eq!(memory, "# Goals\n\ncompiled");
    }

    #[tokio::test]
    async fn child_memory_walks_parent_scope() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = crate::store::open_store(&crate::store::StorageConfig::sqlite(
            tmp.path().join("loopflow.db"),
        ))
        .await
        .unwrap();
        let parent = crate::wave::Wave::new(
            WaveId::new(),
            "platform".into(),
            tmp.path().display().to_string(),
        );
        let child = crate::wave::Wave::new(
            WaveId::new(),
            "release".into(),
            tmp.path().display().to_string(),
        )
        .with_parent(parent.id().clone());
        store.create_wave(&parent).await.unwrap();
        store.create_wave(&child).await.unwrap();
        std::fs::create_dir_all(tmp.path().join("wave/platform")).unwrap();
        std::fs::create_dir_all(tmp.path().join("wave/release")).unwrap();
        std::fs::write(
            tmp.path().join("wave/platform/MEMORY.md"),
            "Parent constraint.",
        )
        .unwrap();
        std::fs::write(tmp.path().join("wave/release/MEMORY.md"), "Child decision.").unwrap();

        let chain = memory_wave_chain_from_store(&store, tmp.path(), "release")
            .await
            .expect("scope resolves");
        assert_eq!(chain, ["platform", "release"]);
        let memory = gather_memory_chain(tmp.path(), &chain).expect("memory renders");
        assert!(memory.contains("## Memory inherited from platform\n\nParent constraint."));
        assert!(memory.contains("## Memory owned by release\n\nChild decision."));
        assert!(
            memory.find("Parent constraint.").unwrap() < memory.find("Child decision.").unwrap()
        );
    }
}
