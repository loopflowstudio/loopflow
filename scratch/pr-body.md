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
