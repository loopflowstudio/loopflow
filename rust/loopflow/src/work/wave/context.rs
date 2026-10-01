//! Durable Wave resolution and authored context.
//!
//! Resolution: explicit `--wave` (the caller passes it) > `LF_WAVE_ID` from a
//! managed Work process. Wave names resolve only inside the canonical
//! repository; the UUID remains durable identity across locator changes.
//!
//! Registry resolution uses the canonical repository. Authored Wave files are
//! gathered by the prompt engine from the executing checkout.

use crate::id::WaveId;
use crate::work::wave::{Wave, WaveLocator};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The durable Wave attributed to this process.
pub const WAVE_ID_ENV: &str = "LF_WAVE_ID";

/// Resolve this process's ambient Wave name through the durable Wave id.
pub fn resolve_ambient_wave_name() -> Option<String> {
    let repo = crate::repo::find_repo_root().ok();
    resolve_managed_wave_sync(repo.as_deref(), None)
        .ok()
        .map(|wave| wave.slug().to_string())
}

/// The Exec attribution decision for the current process: the wave name to
/// attribute an Exec to (if any), plus a classified failure to record when a
/// supplied managed identity failed validation.
///
/// - valid UUID or registered repository-local name → `wave: Some(name)`, `failure: None`
/// - no managed identity (`NoContext`) → `wave: None`, `failure: None`
///   (worktree inference stays a legitimate fallback for this case alone)
/// - stale UUID / registry read failure → `wave: None`, `failure: Some(...)`
///   naming the stale source and the safe explicit recovery (`--wave <name>`)
///
/// Attribution is non-fatal: a stale identity is never silently re-attributed to
/// a wave inferred from the worktree. The command journal records `None` and the failure so
/// the stale source stays visible and actionable; see W2-239.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecAttribution {
    pub wave: Option<String>,
    pub failure: Option<String>,
}

/// The Exec attribution decision for the current process environment inside
/// `repo`. One classification shared by every trace/Exec attribution site
/// ([`crate::journal::ensure_run_context`] and the `lf` Exec wrapper).
pub fn exec_attribution(repo: Option<&Path>) -> ExecAttribution {
    match resolve_managed_wave_sync(repo, None) {
        Ok(wave) => ExecAttribution {
            wave: Some(wave.slug().to_string()),
            failure: None,
        },
        Err(WaveResolveError::NoContext) => ExecAttribution {
            wave: None,
            failure: None,
        },
        Err(error) => ExecAttribution {
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
    let repo = crate::repo::find_repo_root().ok();
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
         run `lf wave list` to list known waves, or pass --wave <known-name>"
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
                let locator = WaveLocator::discover(repo, wave.slug())
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
            let locator = WaveLocator::discover(repo, wave.slug())
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

/// The canonical repository used to resolve Wave identity: the main checkout when
/// `repo_root` is a worktree root, `repo_root` itself otherwise (see
/// [`repo_origin`] for the guard).
pub fn wave_origin(repo_root: &Path) -> PathBuf {
    repo_origin(repo_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `exec_attribution` keeps the classified failure instead of swallowing it.
    /// Absent context is `(None, None)` — worktree inference stays a legitimate
    /// fallback for it alone. A hand-set name resolves through the same scoped
    /// registry as every interactive command; an unregistered name is stale context,
    /// never self-authenticating identity.
    #[test]
    fn run_attribution_classifies_absent_context_and_hand_set_names() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let previous = std::env::var(WAVE_ID_ENV).ok();
        let fixture = loopflow_test_support::TestRepo::new();
        let repo = fixture.path().to_path_buf();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let store = crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                ledger.home().join("loopflow.db"),
            ))
            .await
            .unwrap();
            let locator = WaveLocator::discover(&repo, "product").unwrap();
            let wave = Wave::new(
                WaveId::new(),
                "product".to_string(),
                locator.repo().to_string(),
            );
            store.create_wave(&wave).await.unwrap();
        });

        std::env::remove_var(WAVE_ID_ENV);
        let absent = exec_attribution(Some(&repo));
        assert_eq!(absent.wave, None);
        assert_eq!(absent.failure, None);

        std::env::set_var(WAVE_ID_ENV, "product");
        let named = exec_attribution(Some(&repo));
        assert_eq!(named.wave.as_deref(), Some("product"));
        assert_eq!(named.failure, None);

        std::env::set_var(WAVE_ID_ENV, "ghost");
        let unregistered = exec_attribution(Some(&repo));
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
}
