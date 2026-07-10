//! `lf radio` — broadcast on the agent bus.
//!
//! The bus is a table in the shared store, so publishing is an INSERT and
//! nothing else: no endpoint, no HTTP, no served wave. `lf radio` works with
//! zero loopflow processes running, and two detached hands hear each other
//! with no mind awake between them.
//!
//! # Targeting
//! - default: the invoking context's channel — `LFD_CHANNEL` (set by dispatch),
//!   else `LFD_WAVE_ID`, else the worktree name, which IS the channel name.
//! - `--channel <name>`: any name on the bus. Whoever is tuned in hears it.
//! - `--parent`: the parent wave's channel, walked through the registry.
//!
//! No wave context anywhere — or no registry store on this machine — means
//! there is no bus to publish on: the broadcast drops with exit 0 and one
//! stderr note. That is correct pubsub, and it is what makes the speech
//! vocabulary safe in every prompt unconditionally.
//!
//! # Attribution
//! Byline is testimony, channel is evidence. With no server in the path,
//! client-submitted attribution is the only kind possible: the client derives
//! its byline from the same ambient identity it resolves for routing, and
//! `--from` overrides it. A forged byline is not prevented — it is visible, as
//! a mismatch between the byline and the channel the row arrived on.

use std::io::Read;

use anyhow::{anyhow, Result};

use crate::engine::wave_context::{resolve_ambient_channel, AmbientWaveRef};
use crate::lf::commands::chat::CliContext;
use crate::lfdb::SharedStore;
use crate::wave::channel::family_head;
use crate::wave::runtime::wave_channel_name;

pub fn run(
    text_args: &[String],
    channel: Option<&str>,
    parent: bool,
    from_label: Option<&str>,
) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let context = CliContext::detect().await;
        run_with_context(&context, text_args, channel, parent, from_label).await
    })
}

pub(crate) async fn run_with_context(
    context: &CliContext,
    text_args: &[String],
    channel: Option<&str>,
    parent: bool,
    from_label: Option<&str>,
) -> Result<()> {
    let Some(store) = context.store.as_ref() else {
        eprintln!("no registry store here; broadcast dropped");
        return Ok(());
    };
    let own = ambient_channel(context, store).await;
    let Some(channel) = target_channel(store, channel, parent, own.as_deref()).await? else {
        eprintln!("no wave here; broadcast dropped");
        return Ok(());
    };
    // Testimony: what the client says it is. `--from` is the machine-speech
    // label (`--from ci`); bare, a speaker names its own channel.
    let byline = from_label
        .or(own.as_deref())
        .unwrap_or("cli")
        .trim()
        .to_string();
    let text = message_text(text_args, std::io::stdin())?;

    store
        .publish_bus(channel.clone(), byline.clone(), text)
        .await?;
    println!("broadcast on '{channel}' as [{byline}]");
    Ok(())
}

/// The invoking context's channel name: the shared ambient rule, with the
/// id arm resolved through the store to a wave name.
pub(crate) async fn ambient_channel(context: &CliContext, store: &SharedStore) -> Option<String> {
    match resolve_ambient_channel(
        context.env_channel.as_deref(),
        context.env_wave_id.as_deref(),
        context.repo.as_deref(),
    )? {
        AmbientWaveRef::Id(id) => {
            let wave = store.get_wave(&id.parse().ok()?).await.ok().flatten()?;
            Some(wave_channel_name(wave.name()))
        }
        AmbientWaveRef::Name(name) => Some(name),
    }
}

/// Where the broadcast lands: an explicit channel, the parent wave's channel,
/// or the caller's own. `None` when nothing resolves — the drop.
async fn target_channel(
    store: &SharedStore,
    channel: Option<&str>,
    parent: bool,
    own: Option<&str>,
) -> Result<Option<String>> {
    if let Some(channel) = channel {
        return Ok(Some(channel.to_string()));
    }
    if !parent {
        return Ok(own.map(str::to_string));
    }
    let own = own.ok_or_else(|| {
        anyhow!(
            "cannot resolve the invoking wave for --parent: no LFD_CHANNEL or \
             LFD_WAVE_ID in env and no registered wave matches this worktree"
        )
    })?;
    let head = family_head(own);
    let row = store
        .get_wave_by_name(head)
        .await?
        .ok_or_else(|| anyhow!("--parent: the registry has no wave named '{head}'"))?;
    let parent_id = row.parent_wave_id().ok_or_else(|| {
        anyhow!(
            "wave '{head}' has no parent — it is a root wave; the human \
             fall-through arrives with Decisions"
        )
    })?;
    let parent = store.get_wave(parent_id).await?.ok_or_else(|| {
        anyhow!("wave '{head}' names parent {parent_id}, but no such wave exists")
    })?;
    Ok(Some(wave_channel_name(parent.name())))
}

/// Message text from the args (joined) or stdin (heredoc-friendly).
fn message_text(args: &[String], mut stdin: impl Read) -> Result<String> {
    let joined = args.join(" ").trim().to_string();
    if !joined.is_empty() {
        return Ok(joined);
    }
    let mut buffer = String::new();
    stdin.read_to_string(&mut buffer)?;
    let text = buffer.trim().to_string();
    if text.is_empty() {
        anyhow::bail!("no message text: pass TEXT or pipe it on stdin");
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;

    use crate::lfdb::{open_store, StorageConfig};

    async fn temp_store(dir: &Path) -> SharedStore {
        Arc::new(
            open_store(&StorageConfig::sqlite(dir.join("lfd.db")))
                .await
                .expect("open sqlite store"),
        )
    }

    fn context(store: Option<SharedStore>, channel: Option<&str>) -> CliContext {
        CliContext {
            store,
            repo: None,
            env_wave_id: None,
            env_channel: channel.map(str::to_string),
        }
    }

    /// The whole publish path with no server anywhere: the row is on the bus,
    /// bylined with the caller's own channel.
    #[tokio::test]
    async fn publishing_needs_no_server() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let context = context(Some(store.clone()), Some("ship.148e"));

        run_with_context(&context, &["landed".into(), "PR".into()], None, false, None)
            .await
            .expect("publish");

        let rows = store.read_bus_after(0).await.expect("bus rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].channel, "ship.148e");
        assert_eq!(rows[0].byline, "ship.148e");
        assert_eq!(rows[0].text, "landed PR");
    }

    /// Byline is testimony, channel is evidence: `--from ci` on a hand's
    /// channel writes both, and the mismatch is in the record.
    #[tokio::test]
    async fn a_forged_byline_is_visible_beside_the_arrival_channel() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let context = context(Some(store.clone()), Some("ship"));

        run_with_context(
            &context,
            &["all".into(), "green".into()],
            Some("ship.148e"),
            false,
            Some("ci"),
        )
        .await
        .expect("publish");

        let rows = store.read_bus_after(0).await.expect("bus rows");
        assert_eq!(rows[0].byline, "ci", "the client's testimony, verbatim");
        assert_eq!(rows[0].channel, "ship.148e", "where it actually arrived");
    }

    /// Two detached hands exchange messages with no served wave: one publishes,
    /// the other reads it off the table.
    #[tokio::test]
    async fn two_hands_converse_with_no_served_wave() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;

        let first = context(Some(store.clone()), Some("ship.a"));
        run_with_context(&first, &["over to you".into()], Some("ship.b"), false, None)
            .await
            .expect("publish");

        let heard = store.read_bus_after(0).await.expect("bus rows");
        assert_eq!(heard[0].channel, "ship.b");
        assert_eq!(heard[0].byline, "ship.a");

        let second = context(Some(store.clone()), Some("ship.b"));
        run_with_context(&second, &["heard you".into()], Some("ship.a"), false, None)
            .await
            .expect("publish");
        let heard = store.read_bus_after(heard[0].id).await.expect("bus rows");
        assert_eq!(heard[0].channel, "ship.a");
        assert_eq!(heard[0].byline, "ship.b");
    }

    /// Publish-to-no-subscriber: no wave context anywhere drops with exit 0.
    #[tokio::test]
    async fn no_wave_context_drops_the_broadcast() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let context = context(Some(store.clone()), None);

        run_with_context(&context, &["nobody".into()], None, false, None)
            .await
            .expect("dropped broadcast exits 0");
        assert!(store.read_bus_after(0).await.expect("bus rows").is_empty());
    }

    #[test]
    fn message_text_prefers_args_then_stdin_then_errors() {
        let text = message_text(&["hello".into(), "world".into()], std::io::empty()).unwrap();
        assert_eq!(text, "hello world");
        let text = message_text(&[], std::io::Cursor::new("from stdin\n")).unwrap();
        assert_eq!(text, "from stdin");
        assert!(message_text(&[], std::io::empty()).is_err());
    }
}
