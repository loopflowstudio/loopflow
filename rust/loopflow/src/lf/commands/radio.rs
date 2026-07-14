//! `lf radio pub` — broadcast on the agent bus.
//!
//! The bus is a table in the shared store, so publishing is an INSERT and
//! nothing else: no endpoint, no HTTP, no served wave. `lf radio pub` works with
//! zero loopflow processes running, and two Task Sessions hear each other
//! with no mind awake between them.
//!
//! # Targeting
//! - default: the invoking context's channel — `LF_CHANNEL` (set by dispatch),
//!   else `LF_WAVE_ID`.
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

use anyhow::{anyhow, Result};

use crate::engine::wave_context::{resolve_ambient_channel, AmbientChannelRef};
use crate::lf::commands::chat::{parent_wave, CliContext};
use crate::lf::commands::util::message_text;
use crate::wave::Wave;
use crate::store::SharedStore;
use crate::wave::channel::family_head;
use crate::wave::runtime::wave_channel_name;

pub fn run_pub(
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
    let ambient = ambient_wave(context, store).await;
    let own = ambient.as_ref().map(AmbientWave::channel);
    let Some(channel) = target_channel(store, channel, parent, ambient.as_ref()).await? else {
        eprintln!("no wave here; broadcast dropped");
        return Ok(());
    };
    // Testimony: what the client says it is. `--from` is the machine-speech
    // label (`--from ci`); bare, a speaker names its own channel.
    let byline = from_label.or(own).unwrap_or("cli").trim().to_string();
    let text = message_text(text_args, std::io::stdin())?;

    store
        .publish_bus(channel.clone(), byline.clone(), text)
        .await?;
    println!("broadcast on '{channel}' as [{byline}]");
    Ok(())
}

/// What the caller is: its channel, plus the wave row behind it when the
/// registry knows one. The row is what `--parent` walks — never the channel
/// name, which is sanitized (`web/ui` mints `web-ui`) and would not find its
/// own registry row by name.
#[derive(Debug)]
pub(crate) struct AmbientWave {
    channel: String,
    row: Option<Wave>,
}

impl AmbientWave {
    fn channel(&self) -> &str {
        &self.channel
    }
}

/// The invoking context: the shared ambient rule (`LF_CHANNEL`, else
/// `LF_WAVE_ID`), with the Wave row resolved when the registry has it.
pub(crate) async fn ambient_wave(context: &CliContext, store: &SharedStore) -> Option<AmbientWave> {
    match resolve_ambient_channel(
        context.env_channel.as_deref(),
        context.env_wave_id.as_deref(),
    )? {
        AmbientChannelRef::WaveId(id) => {
            let row = store.get_wave(&id.parse().ok()?).await.ok().flatten()?;
            Some(AmbientWave {
                channel: wave_channel_name(row.name()),
                row: Some(row),
            })
        }
        AmbientChannelRef::Channel(name) => {
            let row = store
                .get_wave_by_name(family_head(&name))
                .await
                .ok()
                .flatten();
            Some(AmbientWave { channel: name, row })
        }
    }
}

/// The invoking context's channel name — what a subscriber tunes in to by
/// default.
pub(crate) async fn ambient_channel(context: &CliContext, store: &SharedStore) -> Option<String> {
    Some(ambient_wave(context, store).await?.channel)
}

/// Where the broadcast lands: an explicit channel, the parent wave's channel,
/// or the caller's own. `None` when nothing resolves — the drop.
async fn target_channel(
    store: &SharedStore,
    channel: Option<&str>,
    parent: bool,
    ambient: Option<&AmbientWave>,
) -> Result<Option<String>> {
    if let Some(channel) = channel {
        return Ok(Some(channel.to_string()));
    }
    if !parent {
        return Ok(ambient.map(|ambient| ambient.channel.clone()));
    }
    let own = ambient
        .and_then(|ambient| ambient.row.as_ref())
        .ok_or_else(|| {
            anyhow!(
                "cannot resolve the invoking wave for --parent: no LF_CHANNEL or \
                 LF_WAVE_ID in env"
            )
        })?;
    let parent = parent_wave(store, own).await?;
    Ok(Some(wave_channel_name(parent.name())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lf::commands::fixtures::{make_wave, temp_store};

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

    /// Two Task Sessions exchange messages with no served Wave: one publishes,
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

    /// `--parent` walks the registry row, not the channel name. A wave whose
    /// name sanitizes (`web/ui` → channel `web-ui`) would never find itself by
    /// channel name, so escalation resolves through `LF_WAVE_ID`'s row.
    #[tokio::test]
    async fn parent_escalation_walks_the_wave_row_of_a_sanitized_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let parent = make_wave("goals", tmp.path(), None);
        store.create_wave(&parent).await.expect("seed parent");
        let child = make_wave("web/ui", tmp.path(), Some(parent.id()));
        store.create_wave(&child).await.expect("seed child");

        let context = CliContext {
            store: Some(store.clone()),
            repo: None,
            env_wave_id: Some(child.id().as_str().to_string()),
            env_channel: None,
        };
        run_with_context(&context, &["blocked".into()], None, true, None)
            .await
            .expect("escalate");

        let rows = store.read_bus_after(0).await.expect("bus rows");
        assert_eq!(rows[0].channel, "goals", "it landed on the parent");
        assert_eq!(rows[0].byline, "web-ui", "bylined with its own channel");
    }

    /// A root wave has nowhere to escalate, and says so.
    #[tokio::test]
    async fn parent_of_a_root_wave_is_a_clear_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let root = make_wave("goals", tmp.path(), None);
        store.create_wave(&root).await.expect("seed root");

        let context = CliContext {
            store: Some(store),
            repo: None,
            env_wave_id: Some(root.id().as_str().to_string()),
            env_channel: None,
        };
        let err = run_with_context(&context, &["blocked".into()], None, true, None)
            .await
            .expect_err("root has no parent");
        assert!(
            err.to_string().contains("wave 'goals' has no parent"),
            "{err}"
        );
    }
}
