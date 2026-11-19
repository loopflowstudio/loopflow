//! Launch-owned authority for provider context callbacks.
use std::path::Path;

/// The [`SteerAdapter`] of a launch whose hooks report to the Machine server.
pub const PROVIDER_CONTEXT_ENV: &str = "LF_PROVIDER_CONTEXT";
/// The [`SteerAdapter`] of a bare launch, whose hook claims its steers from
/// the store itself and reports to no server.
pub const BARE_STEER_ENV: &str = "LF_BARE_STEER";
/// The provider process's seen marker; see [`loopflow_storage::steer_markers::latest_marker`].
pub const STEER_SEEN_ENV: &str = "LF_STEER_SEEN";

/// The Session's own marker and, when it belongs to exactly one Task, that
/// Task's; see [`loopflow_storage::steer_markers::latest_marker`].
pub(crate) const STEER_SESSION_ENV: &str = "LF_STEER_SESSION";
pub(crate) const STEER_TASK_ENV: &str = "LF_STEER_TASK";

pub use loopflow_storage::steer::*;

/// Steers claimed for one conversation and rendered for its provider.
#[derive(Debug)]
pub struct ClaimedSteers {
    pub steers: Vec<loopflow_model::id::SteerId>,
    pub text: String,
}

/// Claim the Session's unread steers for the conversation `thread` and render
/// them. The caller prints `text`, then calls [`delivered`]; a caller that
/// dies between the two leaves a claim that lapses, so a steer can arrive
/// twice and is never dropped. `boundary` says whether this callback may
/// select the Session's native conversation.
pub fn claim(
    store: &loopflow_storage::sqlite::SqliteStore,
    repo: &Path,
    adapter: &SteerAdapter,
    thread: &str,
    boundary: NativeBoundary<'_>,
) -> crate::store::StoreResult<Option<ClaimedSteers>> {
    if !store.has_undelivered_steers(&adapter.session)? {
        return Ok(None);
    }
    store.with_provider_context(adapter, thread, boundary, || {
        let (steers, remaining) = store.claim_session_steers(adapter, thread)?;
        if steers.is_empty() {
            return Ok(None);
        }
        // A render that fails leaves its claim to lapse, like a hook that died.
        let text = render(repo, &adapter.session, &steers, remaining)
            .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
        let steers = steers.into_iter().map(|steer| steer.id).collect();
        Ok(Some(ClaimedSteers { steers, text }))
    })
}

/// The conversation took the claimed steers.
pub fn delivered(
    store: &loopflow_storage::sqlite::SqliteStore,
    session: &str,
    claimed: &ClaimedSteers,
) {
    if let Err(error) = store.deliver_session_steers(session, &claimed.steers) {
        tracing::warn!(session, %error, "printed steers not recorded; they will arrive again");
    }
}

fn excerpt(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Each steer's attribution, a pointer to its complete source saved under the
/// checkout, and at most 400 bytes of its text.
fn render(
    repo: &Path,
    session: &str,
    steers: &[Steer],
    remaining: usize,
) -> anyhow::Result<String> {
    let mut text = String::from("<lf:steers>\n");
    for steer in steers {
        let path = crate::prompt::write_prompt_log(
            repo,
            &serde_json::to_string_pretty(steer)?,
            "steer",
            None,
        )?;
        let path = path.strip_prefix(repo).unwrap_or(&path);
        // A Task update is a steer lf wrote about what it saw, not direction.
        let update = steer.source.starts_with("update:");
        // A notice is news about a Task this conversation conducts.
        let notice = steer.source.starts_with("notice:");
        let mut item = if notice {
            format!(
                "Task notice {}. News about work this conversation conducts; following it up is yours. Read the Task's current status before acting. Complete source: {}\n",
                steer.id,
                serde_json::to_string(path)?
            )
        } else if update {
            format!(
                "Task update {}. Report this recorded progress briefly. This is observation, not a request or permission to launch work, advance review, publish or merge. Read current Task status before acting. Complete source: {}\n",
                steer.id,
                serde_json::to_string(path)?
            )
        } else {
            format!(
                "Steer {}. Source {} revision {}; author {}. Complete source: {}\n",
                steer.id,
                serde_json::to_string(excerpt(&steer.source, 96))?,
                serde_json::to_string(excerpt(&steer.revision, 64))?,
                serde_json::to_string(excerpt(&steer.author.to_string(), 96))?,
                serde_json::to_string(path)?
            )
        };
        let mut body = excerpt(&steer.text, if update || notice { 600 } else { 400 });
        while !body.is_empty()
            && item.len() + crate::prompt::render_reference(body).len() + 32 > 1024
        {
            body = excerpt(body, body.len() - 1);
        }
        item.push_str(&crate::prompt::render_reference(body));
        if body.len() != steer.text.len() {
            item.push_str(" [excerpt; full source above]");
        }
        item.push('\n');
        anyhow::ensure!(
            item.len() <= 1024,
            "Steer attribution and source pointer exceed the item budget"
        );
        text.push_str(&item);
    }
    text.push_str(&format!(
        "{remaining} pending. Inspect: lf session steers {session} --json\n</lf:steers>\n"
    ));
    Ok(text)
}

/// Only confirmed live, managed conversations can be woken.
pub fn idle_conversations(
    store: &loopflow_storage::sqlite::SqliteStore,
    session: Option<&str>,
) -> crate::store::StoreResult<Vec<IdleConversation>> {
    Ok(store
        .idle_conversation_candidates(session)?
        .into_iter()
        .filter_map(|candidate| {
            candidate
                .identity
                .is_some_and(|(pid, start)| {
                    crate::process::os_process::process_identity_evidence(pid, start)
                        == crate::store::ProcessIdentityEvidence::Live
                })
                .then_some(candidate.conversation)
        })
        .collect())
}

/// Publication is local; Session claims and receipts never enter planning sync.
pub async fn save_steer(
    store: &crate::store::SharedStore,
    wave: &str,
    author: &str,
    message: &str,
) -> anyhow::Result<crate::steer::Steer> {
    let wave = crate::work::wave::context::resolve_in(store, &std::env::current_dir()?, Some(wave))
        .await?;
    Ok(store.sqlite.save_wave_steer(wave.id(), author, message)?)
}
