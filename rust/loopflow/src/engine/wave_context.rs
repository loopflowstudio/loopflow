//! Durable Wave context: every `lf` run born inside a Wave inherits curated
//! `MEMORY.md` files. Conversation enters a run only when an operation selects
//! the relevant Turn explicitly.
//!
//! Resolution: explicit `--wave` (the caller passes it) > `LF_WAVE_ID` from a
//! managed session. Repository location cannot identify a
//! Wave: every Wave and Project operates from the same canonical checkout.
//!
//! Wave state (journal, endpoint pointer, MEMORY.md) lives under the ORIGIN
//! repo — a worktree resolves its main repo first.

use crate::id::WaveId;
use crate::wave::server::endpoint_path;
use crate::wave::Wave;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// The durable Wave attributed to this process.
pub const WAVE_ID_ENV: &str = "LF_WAVE_ID";

/// Resolve the Wave owned by a managed process.
///
/// Humans choose a Wave explicitly. Managed Wave, Project, and Task processes
/// inherit `LF_WAVE_ID` from their launcher.
pub fn resolve_ambient_wave(env_wave_id: Option<&str>) -> Option<String> {
    env_wave_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Resolve this process's ambient Wave name through the durable Wave id.
pub fn resolve_ambient_wave_name() -> Option<String> {
    let env_wave_id = std::env::var(WAVE_ID_ENV).ok()?;
    wave_name_for_id(&env_wave_id)
}

/// The run-attribution decision for the current process: the wave name to
/// attribute a run to (if any), plus a classified failure to record when a
/// supplied managed identity failed validation.
///
/// - valid UUID or hand-set name → `wave: Some(name)`, `failure: None`
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

/// The run-attribution decision for the current process environment. One
/// classification shared by every trace/run attribution site
/// ([`crate::journal::ensure_run_context`] and the `lf` run wrapper).
pub fn run_attribution() -> RunAttribution {
    match resolve_managed_wave_name_sync(None) {
        Ok(name) => RunAttribution {
            wave: Some(name),
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

/// The Wave a managed run is attributed to, or `None` when there is no valid
/// managed identity. Thin wrapper over [`run_attribution`] for non-attribution
/// callers (`lf home`); trace/run attribution uses [`run_attribution`] directly
/// so a stale identity is propagated, not swallowed.
pub fn resolve_run_wave_name() -> Option<String> {
    run_attribution().wave
}

/// Resolve a top-level `--wave` to the durable Wave row used by prompt,
/// journal, and child-process attribution. Surfaces the same
/// [`WaveResolveError`] classification as the shared resolver: an empty name is
/// [`WaveResolveError::EmptyExplicit`], an unknown name is
/// [`WaveResolveError::UnknownExplicit`].
pub fn resolve_explicit_wave(name: &str) -> anyhow::Result<Wave> {
    let name = crate::ops::util::normalize_wave_name(name)
        .ok_or_else(|| anyhow::anyhow!("{}", WaveResolveError::EmptyExplicit))?;
    let lookup_name = name.clone();
    let lookup = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime always builds");
        runtime.block_on(async move {
            let store = crate::store::open_existing_store()
                .await
                .ok_or_else(|| anyhow::anyhow!("no wave registry on this machine"))?;
            store
                .get_wave_by_name(&lookup_name)
                .await
                .map_err(|error| anyhow::anyhow!("failed to read wave registry: {error}"))
        })
    })
    .join()
    .map_err(|_| anyhow::anyhow!("failed to resolve explicit wave '{name}'"))??;
    lookup.ok_or_else(|| anyhow::anyhow!("{}", WaveResolveError::UnknownExplicit(name)))
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
    /// The registry read itself failed (I/O, not a miss).
    #[error("failed to read wave registry: {0}")]
    Registry(String),
}

/// THE resolver for the ambient Wave's durable name. One rule, shared by every
/// consumer that acts on "the wave I am inside":
///
/// 1. explicit `--wave` always wins — normalized and validated against the
///    registry. An unknown name is [`WaveResolveError::UnknownExplicit`]; an
///    empty one is [`WaveResolveError::EmptyExplicit`]. No store →
///    [`WaveResolveError::Registry`] (a machine with no registry has no valid
///    wave names). Creation flows that accept unregistered names bypass this
///    resolver.
/// 2. else `LF_WAVE_ID` as a durable registry **UUID** → mapped to its Wave's
///    name through the store. A UUID the registry has no row for is
///    [`WaveResolveError::StaleIdentity`], never silently re-read as a name.
/// 3. else `LF_WAVE_ID` as a hand-set **name** (intentional fallback) → used
///    directly (no membership check — PM keys files by name, status reports
///    no row).
/// 4. else [`WaveResolveError::NoContext`].
///
/// The env is only a pointer used to find the durable Wave; identity is the
/// registry row, never the string in the environment.
pub async fn resolve_managed_wave_name(
    store: Option<&crate::store::Store>,
    explicit: Option<&str>,
    env_wave_id: Option<&str>,
) -> Result<String, WaveResolveError> {
    if let Some(raw) = explicit {
        let name =
            crate::ops::util::normalize_wave_name(raw).ok_or(WaveResolveError::EmptyExplicit)?;
        let store = store.ok_or_else(|| {
            WaveResolveError::Registry("no wave registry on this machine".to_string())
        })?;
        return match store.get_wave_by_name(&name).await {
            Ok(Some(_)) => Ok(name),
            Ok(None) => Err(WaveResolveError::UnknownExplicit(name)),
            Err(error) => Err(WaveResolveError::Registry(error.to_string())),
        };
    }
    let raw = env_wave_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(WaveResolveError::NoContext)?;
    if let Ok(id) = raw.parse::<WaveId>() {
        let store = store.ok_or_else(|| WaveResolveError::StaleIdentity(raw.to_string()))?;
        return match store.get_wave(&id).await {
            Ok(Some(row)) => Ok(row.name().to_string()),
            Ok(None) => Err(WaveResolveError::StaleIdentity(raw.to_string())),
            Err(error) => Err(WaveResolveError::Registry(error.to_string())),
        };
    }
    // A hand-set name: use it directly. No registry membership required — the
    // name is the durable key file and PM surfaces already use.
    crate::ops::util::normalize_wave_name(raw).ok_or(WaveResolveError::NoContext)
}

/// [`resolve_managed_wave_name`] for sync, store-free call sites (`lf pm …`).
/// Reads `LF_WAVE_ID` from the env. The explicit arm validates against the
/// registry (opening a store on a scratch thread — the same idiom as the UUID
/// arm); the hand-set-name arm touches no store. Context assembly is sync and
/// sometimes already inside a runtime.
pub fn resolve_managed_wave_name_sync(explicit: Option<&str>) -> Result<String, WaveResolveError> {
    if let Some(raw) = explicit {
        let name =
            crate::ops::util::normalize_wave_name(raw).ok_or(WaveResolveError::EmptyExplicit)?;
        let name = name.to_string();
        return std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("current-thread runtime always builds");
            runtime.block_on(async move {
                let store = crate::store::open_existing_store().await;
                resolve_managed_wave_name(store.as_ref(), Some(&name), None).await
            })
        })
        .join()
        .unwrap_or_else(|_| {
            Err(WaveResolveError::Registry(
                "resolver thread panicked".to_string(),
            ))
        });
    }
    let env_wave_id = std::env::var(WAVE_ID_ENV).ok();
    let raw = env_wave_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(WaveResolveError::NoContext)?;
    // Fast path: a hand-set name needs no registry.
    if raw.parse::<WaveId>().is_err() {
        return crate::ops::util::normalize_wave_name(raw).ok_or(WaveResolveError::NoContext);
    }
    let raw = raw.to_string();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime always builds");
        runtime.block_on(async move {
            let store = crate::store::open_existing_store().await;
            resolve_managed_wave_name(store.as_ref(), None, Some(&raw)).await
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

/// Map an ambient `LF_WAVE_ID` value to its durable Wave name through the shared
/// [`resolve_managed_wave_name`] rule: a UUID maps through the store, a hand-set
/// name is used directly. The store API is async and context assembly is sync
/// (sometimes already inside a runtime — flow skills), so it runs on a scratch
/// thread. Any resolve error (`StaleIdentity`, registry I/O) → `None`, and
/// resolution falls back to the worktree.
fn wave_name_for_id(id: &str) -> Option<String> {
    let id = id.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(async {
            let store = crate::store::open_existing_store().await;
            resolve_managed_wave_name(store.as_ref(), None, Some(&id))
                .await
                .ok()
        })
    })
    .join()
    .ok()
    .flatten()
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
    let chain = memory_wave_chain(wave).unwrap_or_else(|| vec![wave.to_string()]);
    gather_memory_chain(&origin, &chain)
}

/// Resolve lexical memory scope through the registry.
fn memory_wave_chain(wave: &str) -> Option<Vec<String>> {
    let wave = wave.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().ok()?;
        rt.block_on(async {
            let store = crate::store::open_existing_store().await?;
            memory_wave_chain_from_store(&store, &wave).await
        })
    })
    .join()
    .ok()
    .flatten()
}

async fn memory_wave_chain_from_store(
    store: &crate::store::Store,
    wave: &str,
) -> Option<Vec<String>> {
    let mut current = store.get_wave_by_name(wave).await.ok().flatten()?;
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

    /// The one rule the ambient call sites share: managed identity is explicit
    /// in the process environment.
    #[test]
    fn ambient_rule_env_id_wins_and_is_trimmed() {
        assert_eq!(
            resolve_ambient_wave(Some(" wave-1 ")),
            Some("wave-1".to_string())
        );
        assert_eq!(resolve_ambient_wave(Some("  ")), None);
        assert_eq!(resolve_ambient_wave(None), None);
    }

    /// Trace attribution and `lf home` read the ambient wave through
    /// [`resolve_run_wave_name`]. A hand-set `LF_WAVE_ID=<name>` (not a UUID)
    /// now resolves to that name — the fix — while an absent env is `None` so
    /// the caller falls back to the worktree. The name arm touches no store.
    #[test]
    fn run_wave_name_resolves_a_hand_set_name_and_none_without_context() {
        let _lock = crate::journal::test_env_lock();
        let previous = std::env::var(WAVE_ID_ENV).ok();

        std::env::set_var(WAVE_ID_ENV, "product");
        assert_eq!(resolve_run_wave_name(), Some("product".to_string()));

        std::env::remove_var(WAVE_ID_ENV);
        assert_eq!(resolve_run_wave_name(), None);

        match previous {
            Some(value) => std::env::set_var(WAVE_ID_ENV, value),
            None => std::env::remove_var(WAVE_ID_ENV),
        }
    }

    /// `run_attribution` keeps the classified failure instead of swallowing it.
    /// Absent context is `(None, None)` — worktree inference stays a legitimate
    /// fallback for it alone. A hand-set name, even one no registry row backs, is
    /// durable and never stale: it attributes to itself with no failure (a
    /// "stale name" is not a state the resolver produces — only UUIDs go stale).
    /// The name arm touches no store.
    #[test]
    fn run_attribution_classifies_absent_context_and_hand_set_names() {
        let _lock = crate::journal::test_env_lock();
        let previous = std::env::var(WAVE_ID_ENV).ok();

        std::env::remove_var(WAVE_ID_ENV);
        let absent = run_attribution();
        assert_eq!(absent.wave, None);
        assert_eq!(absent.failure, None);

        std::env::set_var(WAVE_ID_ENV, "product");
        let named = run_attribution();
        assert_eq!(named.wave.as_deref(), Some("product"));
        assert_eq!(named.failure, None);

        std::env::set_var(WAVE_ID_ENV, "ghost");
        let unregistered = run_attribution();
        assert_eq!(unregistered.wave.as_deref(), Some("ghost"));
        assert_eq!(unregistered.failure, None);

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

        let chain = memory_wave_chain_from_store(&store, "release")
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
