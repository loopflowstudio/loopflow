//! A Session moves to another Machine: its branch through the remote, its
//! provider transcript inside the move, its identity unchanged. `send` runs
//! where it lives and completes it there, naming where it went; `receive`
//! checks out the branch where it arrives and admits the same Session, so the
//! same provider conversation resumes there.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::{provider_conversation, session_by_id, session_not_found};
use crate::durable::{MachineId, RepositoryId};
use crate::harness::HarnessKind;
use crate::id::{AgentSessionId, LfSessionId, WaveId};
use crate::provider_auth::Provider;
use crate::repository::CanonicalRepo;
use crate::session::{LfSession, PrimaryScope, TitleSource};
use crate::store::sqlite::SqliteStore;
use crate::store::SharedStore;
use crate::transcript::{Located, Source};

/// One Session on its way from one Machine to another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Move {
    pub from_machine_id: MachineId,
    pub session: MovedSession,
    pub checkout: MovedCheckout,
    /// Absent before the conversation's first turn: nothing to resume yet.
    pub history: Option<MovedHistory>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MovedSession {
    pub id: LfSessionId,
    pub title: String,
    pub title_source: TitleSource,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// The Wave or repository whose ongoing conversation this is.
    pub primary: Option<MovedPrimary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum MovedPrimary {
    Repository,
    Wave { wave_id: WaveId },
}

/// Where the conversation's code is: a pushed commit on a branch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MovedCheckout {
    pub repository_id: RepositoryId,
    pub branch: String,
    pub commit: String,
}

/// The provider's own transcript, file by file. Claude's paths are relative to
/// its project directory, which is named for the checkout; Codex's to its home.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MovedHistory {
    pub agent_session_id: AgentSessionId,
    pub files: Vec<MovedFile>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct MovedFile {
    pub path: String,
    /// Base64 of the file's bytes.
    pub content: String,
}

/// Commit and push the Session's branch, stop its agent, read its transcript
/// and complete it here as moved to `to`. Repeating it after a failed receive
/// reads the same Session again.
pub(crate) async fn send(store: &SharedStore, id: &str, to: &MachineId) -> Result<Move> {
    let session = session_by_id(store, id)
        .await?
        .ok_or_else(|| session_not_found(id))?;
    let checkout = checkpoint(&store.sqlite, &session, to)?;
    crate::harness::agent_process::stop_session_agent(&store.sqlite, &session.id)?;
    let history = history(&store.sqlite, &session)?;
    let primary = match store.sqlite.session_primary_scope(&session.id)?.as_deref() {
        Some("repository") => Some(MovedPrimary::Repository),
        Some("wave") => Some(MovedPrimary::Wave {
            wave_id: session
                .wave_id
                .clone()
                .ok_or_else(|| anyhow!("Wave Session {} names no Wave", session.id))?,
        }),
        _ => None,
    };
    store.sqlite.complete_moved_session(&session.id, to)?;
    Ok(Move {
        from_machine_id: store.sqlite.local_machine()?.id,
        session: MovedSession {
            id: session.id.clone(),
            title: session.title.clone(),
            title_source: session.title_source,
            provider: session.provider.clone(),
            model: session.model.clone(),
            primary,
        },
        checkout,
        history,
    })
}

/// Bring the Session's code to the remote: commit what is uncommitted and push
/// its branch. The default branch is never committed to or pushed; it moves
/// only when clean.
fn checkpoint(store: &SqliteStore, session: &LfSession, to: &MachineId) -> Result<MovedCheckout> {
    let name = &session.title;
    let root = crate::git::worktree_root(&session.cwd).map_err(|_| {
        anyhow!(
            "Session {name} runs outside a repository ({}); only a conversation in a Git checkout moves",
            session.cwd.display()
        )
    })?;
    if let Some(state) = crate::git::intervention_state(&root)? {
        bail!("{name}'s worktree is mid-{state}; finish or abort the {state}, then move it");
    }
    let branch = crate::git::current_branch(&root)?.ok_or_else(|| {
        anyhow!("{name}'s worktree has no branch checked out; check one out, then move it")
    })?;
    let clean = crate::git::is_clean(&root)?;
    if branch == crate::git::get_default_branch(&root)? {
        anyhow::ensure!(
            clean,
            "{name}'s checkout has uncommitted work on {branch}; move it to a branch first"
        );
    } else {
        if !clean {
            crate::git::stage_all(&root, &|_| {})?;
            crate::git::commit(&root, &format!("Move {name} to {to}"), &|_| {})?;
        }
        crate::git::push_with_upstream(&root, "origin", &branch, &|_| {})
            .with_context(|| format!("push {branch} before moving {name}"))?;
    }
    let canonical = CanonicalRepo::discover(&root)?;
    Ok(MovedCheckout {
        repository_id: store
            .repository_id(&canonical.to_string())?
            .ok_or_else(|| {
                anyhow!("repository identity unavailable; run `lf repo identity` in {canonical}")
            })?,
        branch,
        commit: crate::git::rev_parse(&root, "HEAD")?,
    })
}

fn harness(provider: Provider) -> HarnessKind {
    match provider {
        Provider::Claude => HarnessKind::Claude,
        _ => HarnessKind::Codex,
    }
}

fn history(store: &SqliteStore, session: &LfSession) -> Result<Option<MovedHistory>> {
    let Some(key) = session.artifact_key.as_deref() else {
        return Ok(None);
    };
    let Some(reference) = store.input_provider_session(key)? else {
        return Ok(None);
    };
    let provider: Provider = session
        .provider
        .as_deref()
        .unwrap_or_default()
        .parse()
        .map_err(|_| {
            anyhow!(
                "Session {} runs on {}, whose conversations do not move between Machines",
                session.id,
                session.provider.as_deref().unwrap_or("an unknown provider")
            )
        })?;
    let thread = reference.agent_session;
    let located = crate::transcript::locate(
        store,
        harness(provider),
        &thread,
        reference.account_id.as_ref(),
        &session.cwd,
    )?;
    let (path, home) = match located {
        Located::Found {
            source: Source::File(path),
            home: Some(home),
        } => (path, home),
        Located::Found { .. } => bail!("{provider} conversation {thread} has no transcript file"),
        Located::Gone(gap) => bail!(
            "the {provider} transcript of Session {} ({thread}) is unavailable ({gap:?}); nothing moved",
            session.id
        ),
    };
    let base = match provider {
        Provider::Claude => path
            .parent()
            .expect("a Claude transcript is in its project directory")
            .to_path_buf(),
        _ => home,
    };
    let mut files = vec![read_file(&base, &path)?];
    let sibling = path.with_extension("");
    if provider == Provider::Claude && sibling.is_dir() {
        read_tree(&base, &sibling, &mut files)?;
    }
    Ok(Some(MovedHistory {
        agent_session_id: thread,
        files,
    }))
}

fn read_tree(base: &Path, dir: &Path, files: &mut Vec<MovedFile>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            read_tree(base, &path, files)?;
        } else {
            files.push(read_file(base, &path)?);
        }
    }
    Ok(())
}

fn read_file(base: &Path, path: &Path) -> Result<MovedFile> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(MovedFile {
        path: path
            .strip_prefix(base)
            .expect("a transcript file is under its base")
            .to_string_lossy()
            .into_owned(),
        content: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

/// Check out the moved branch in a worktree of this repository, admit the
/// same Session there and place its transcript where its provider resumes it.
/// `repo` is this Machine's checkout of the moved repository.
pub(crate) async fn receive(store: &SharedStore, repo: &Path, moved: Move) -> Result<LfSession> {
    let main = crate::git::worktrees::main_repo_root(repo)?;
    let cwd = check_out(&main, &moved.checkout)?;
    let canonical = CanonicalRepo::discover(&cwd)?;
    let session = admit(store, &moved.session, &cwd, &canonical).await?;
    let (Some(history), Some(provider)) = (&moved.history, moved.session.provider.as_deref())
    else {
        return Ok(session);
    };
    let provider: Provider = provider
        .parse()
        .map_err(|_| anyhow!("unknown provider {provider}"))?;
    let home = crate::transcript::conversation_home(
        &store.sqlite,
        provider,
        &history.agent_session_id,
        None,
    )?
    .unwrap_or_else(|| crate::provider_account::activation::native_home(provider, None));
    let base = match provider {
        Provider::Claude => home.join("projects").join(claude_project(&cwd)),
        _ => home,
    };
    for file in &history.files {
        write_file(&base, file)?;
    }
    provider_conversation::select(
        store,
        &session,
        provider,
        history.agent_session_id.as_str(),
        None,
    )
    .await
}

/// Claude names a project directory for its absolute path, every character
/// other than a letter or digit replaced by `-`.
fn claude_project(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

fn write_file(base: &Path, file: &MovedFile) -> Result<()> {
    let relative = Path::new(&file.path);
    anyhow::ensure!(
        relative
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_))),
        "moved transcript path {} leaves its directory",
        file.path
    );
    let path = base.join(relative);
    fs::create_dir_all(path.parent().expect("a joined path has a parent"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&file.content)
        .with_context(|| format!("decode moved transcript {}", file.path))?;
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))
}

/// The worktree holding `checkout.branch` at `checkout.commit` or later:
/// the main checkout for the default branch, else the branch's worktree,
/// created from origin when there is none.
fn check_out(main: &Path, checkout: &MovedCheckout) -> Result<PathBuf> {
    let branch = &checkout.branch;
    crate::git::fetch(main, "origin", branch)?;
    let existing = crate::git::worktrees::list_porcelain(main)?
        .into_iter()
        .find_map(|(path, at)| (at.as_deref() == Some(branch)).then_some(path));
    let path = match existing {
        Some(path) => path,
        None => {
            let leaf = branch
                .rsplit('/')
                .next()
                .unwrap_or(branch)
                .replace('.', "-");
            let segment = crate::git::worktrees::WorktreeSegment::parse(&leaf)?;
            let plan = crate::git::worktrees::plan_branch_placement(main, segment, Some(branch))?;
            crate::git::worktrees::create_from_placement_plan(main, &plan)?.path
        }
    };
    if !crate::git::is_ancestor(&path, &checkout.commit, "HEAD")? {
        anyhow::ensure!(
            crate::git::is_clean(&path)?
                && crate::git::is_ancestor(&path, "HEAD", &checkout.commit)?,
            "{} has work on {branch} that {} lacks; reconcile it there, then move again",
            path.display(),
            checkout.commit
        );
        crate::git::git_stdout(&path, &["merge", "--ff-only", &checkout.commit])?;
    }
    Ok(path)
}

/// The moved Session here, open in `cwd`: the row it left, reopened, or a new
/// row with its identity.
async fn admit(
    store: &SharedStore,
    moved: &MovedSession,
    cwd: &Path,
    repo: &CanonicalRepo,
) -> Result<LfSession> {
    if store.session(&moved.id).await?.is_some() {
        store.sqlite.reopen_moved_session(&moved.id, cwd)?;
        return store
            .session(&moved.id)
            .await?
            .ok_or_else(|| session_not_found(&moved.id));
    }
    let session = LfSession {
        captured: None,
        id: moved.id.clone(),
        artifact_key: Some(crate::session_record::new_artifact_key()),
        caller_artifact_key: None,
        input_published: false,
        cwd: cwd.to_path_buf(),
        skill: None,
        provider: moved.provider.clone(),
        model: moved.model.clone(),
        node: None,
        iterations: None,
        task_id: None,
        wave_id: match &moved.primary {
            Some(MovedPrimary::Wave { wave_id }) => Some(wave_id.clone()),
            _ => None,
        },
        graph_lf_process_id: None,
        work_source: None,
        bound_at: None,
        interactive: true,
        repo: Some(repo.to_string()),
        title: moved.title.clone(),
        title_source: moved.title_source,
        request: None,
        ready_summary: None,
        completed_at: None,
        created_at: crate::store::rows::now_unix(),
    };
    let scope = match &moved.primary {
        None => return Ok(store.create_session(session).await?),
        Some(MovedPrimary::Repository) => PrimaryScope::Repository(repo.clone()),
        Some(MovedPrimary::Wave { wave_id }) => PrimaryScope::Wave(wave_id.clone()),
    };
    let caller = crate::journal::current_lf_process_id();
    let admitted = store
        .sqlite
        .ensure_primary_session(&scope, None, session, caller.as_ref())?;
    anyhow::ensure!(
        admitted.id == moved.id,
        "this Machine already runs its own Session {} for that scope; complete it, then move again",
        admitted.id
    );
    Ok(admitted)
}

#[cfg(test)]
mod tests {
    use super::{claude_project, Move};
    use std::path::Path;

    #[test]
    fn a_move_round_trips_and_requires_every_field() {
        let json = include_str!("../../../../../tests/fixtures/dto/move.json");
        let moved: Move = serde_json::from_str(json).unwrap();
        assert_eq!(
            serde_json::from_str::<Move>(&serde_json::to_string(&moved).unwrap()).unwrap(),
            moved
        );
        let mut missing: serde_json::Value = serde_json::from_str(json).unwrap();
        missing.as_object_mut().unwrap().remove("checkout");
        assert!(serde_json::from_value::<Move>(missing).is_err());
    }

    #[test]
    fn claude_names_a_project_for_its_path() {
        assert_eq!(
            claude_project(Path::new("/Users/jack/src/loopflow.loo-628")),
            "-Users-jack-src-loopflow-loo-628"
        );
    }
}
