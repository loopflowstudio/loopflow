## Try it!

```bash
cargo test -p loopflow trigger_listeners_area_derived
cargo test -p loopflow block_signal_fires_chord_tend
cargo test -p loopflow area_derived_coalesces_member_completions
cargo test -p loopflow activation_log_includes_source_wave_name
cargo test -p loopflow webhook_merge_fires_area_derived_triggers
cargo test -p loopflow managed_mode_not_polled
grep -n 'cron:' wave/redesign/redesign.yaml
swift test --package-path swift

orig_home="$HOME"; tmp_home=$(mktemp -d); \
  HOME="$tmp_home" RUSTUP_HOME="$orig_home/.rustup" CARGO_HOME="$orig_home/.cargo" \
  cargo test -p loopflow
```

What you should see:
- area-derived `wave` triggers wake the redesign chord when a member wave completes
- persistent queue blocks emit `block` activations for the chord
- multiple member completions coalesce into one serialized activation with member names preserved in the reason
- managed member waves are skipped by loop/cron pollers
- redesign is configured with a daily cron heartbeat plus `wave` + `block` triggers

## Intent

Make the redesign chord-wave coordinate through normal wave primitives instead of manual tending. This change teaches the existing trigger/runtime model how to treat `area` as chord membership, adds persistent block escalation as a first-class signal, and marks member waves as `managed` so the chord owns their rhythm.

## Assumptions

- Chord membership is encoded by `area` entries shaped like `wave/<member-name>/` in the same repo.
- Only persistent queue blocks (`scratch_dirty`, `rebase_conflict`, `promotion_failed`) are worth escalating; transient queue states should stay noisy-but-local.
- Merged PR webhooks are the reliable completion point for merged member work; completion should not depend on `lf ops` behavior.
- Internal wave YAML can move to list-form `triggers:` without a compatibility shim.

## Key decisions

- Reused `Signal::Wave` for area-derived listeners instead of creating a chord-only signal.
- Added `Signal::Block` and a dedicated event listener rather than overloading `ci_failure`.
- Routed area-derived activations through existing pending-activation coalescing so debounce falls out of current queue semantics.
- Added `mode: managed` as a semantic wave mode while keeping existing execution machinery intact.
- Updated Concerto's shared models so `block` and `managed` render cleanly in the app.

## Not included

- Cross-repo area-derived triggers.
- Automatic unblock / self-healing behavior for blocked member waves.
- New activation-log UI beyond the model updates needed for `block` and `managed`.
- Backwards compatibility for singular `triggers:` config.
