# Chord triggers review

## What was implemented

Added chord-ready trigger plumbing to lfd so a wave can react to its member waves through ordinary wave config:
- `Signal::Block` now escalates persistent queue blocks to listener waves.
- `Signal::Wave` can be area-derived when `source` is omitted, so chord membership comes from `area: [wave/<name>/]` instead of per-member trigger wiring.
- `WaveMode::Managed` marks member waves that should be driven by a parent chord rather than loop/cron pollers.
- Wave YAML now accepts `triggers:` as a list, and the redesign/member wave configs were updated to use `cron`, `managed`, `wave`, and `block` together.
- Webhook merge handling now fires area-derived completion listeners so merged member work wakes the chord automatically.
- Swift models/UI were updated so Concerto understands `managed` mode and the new `block` signal label.

## Key choices

- **Area-derived membership over explicit member lists.** A sourceless `wave` or `block` trigger resolves membership from the listener wave's `area`, keeping chord membership in one place.
- **Reuse existing activation/coalescing paths.** Area-derived completions still flow through normal trigger rows and activation queues, so debounce comes from existing coalescing instead of new chord-specific timers.
- **Persistent blocks only.** Only `scratch_dirty`, `rebase_conflict`, and `promotion_failed` emit `WaveBlocked`; transient queue states stay local to reconciliation.
- **Webhook merge is the completion source of truth for merged PRs.** That avoids new CLI↔daemon coupling and lines the trigger up with actual merge completion.
- **`managed` is semantic, not a new execution engine.** Loop/cron pollers skip managed waves, but everything else uses existing wave/run machinery.

## How it fits together

Queue reconciliation now emits `Event::WaveBlocked` when a persistent block is recorded. The new block trigger listener subscribes to that event hub stream, resolves listener waves by area membership, and activates them through the same activation helper used by completion triggers.

Wave completion and webhook merge handling both call `trigger_listeners_on_completion`, which now supports two modes: explicit `source_wave_id` matching and area-derived matching for sourceless `wave` triggers. Activation reasons are stored in activation logs so coalesced chord runs still show which member waves completed or blocked.

## Risks and bottlenecks

- Area-derived membership depends on the `wave/<name>/` convention inside `area`; mis-typed area entries silently mean "not a member."
- Area-derived matching is repo-local. Cross-repo chord membership still needs explicit trigger wiring or future design work.
- Reviewer note: a full `cargo test -p loopflow` run in a developer shell can pick up `~/.lf/config.yaml`; using an isolated `HOME` avoids that local-environment bleed and matches CI-style expectations.

## What's not included

- Cross-repo area-derived trigger resolution.
- Automatic recovery/self-healing for blocked member waves.
- New UI for activation-log detail beyond the existing signal/mode model updates.
- Backwards-compatibility shims for singular `triggers:` YAML.

## Validation

### Done-when checks

```bash
cargo test -p loopflow trigger_listeners_area_derived
cargo test -p loopflow block_signal_fires_chord_tend
cargo test -p loopflow area_derived_coalesces_member_completions
cargo test -p loopflow activation_log_includes_source_wave_name
cargo test -p loopflow webhook_merge_fires_area_derived_triggers
cargo test -p loopflow managed_mode_not_polled
grep -n 'cron:' wave/redesign/redesign.yaml
```

All passed locally.

### Broader validation

```bash
cargo fmt --check
cargo clippy -p loopflow -- -D warnings
orig_home="$HOME"; tmp_home=$(mktemp -d); \
  HOME="$tmp_home" RUSTUP_HOME="$orig_home/.rustup" CARGO_HOME="$orig_home/.cargo" \
  cargo test -p loopflow
swift test --package-path swift
```

- `cargo fmt --check` ✅
- `cargo clippy -p loopflow -- -D warnings` ✅
- `cargo test -p loopflow` ✅ with isolated `HOME` (local `~/.lf/config.yaml` otherwise affects three pre-existing config tests)
- `swift test --package-path swift` ✅
- `xcodegen generate && xcodebuild test -project LoopflowSwift.xcodeproj -scheme Concerto -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO` ⚠️ hit a local UITest runner bootstrap crash (`ConcertoUITests-Runner ... Early unexpected exit, operation never finished bootstrapping`). Package/unit tests passed; no assertion-level app/UI failure was reported before the runner died.
