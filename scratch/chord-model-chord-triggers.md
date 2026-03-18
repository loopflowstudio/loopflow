# Chord-Wave Triggers

## Problem

The redesign chord-wave (`wave/redesign/`) runs `tend` only when a human types the command. It has no way to react when member waves land work, stall, or block. A chord-wave that doesn't notice its member waves isn't coordinating — it's auditing on demand.

Three trigger signals are missing: member-wave completion, block escalation, and daily cron. The existing trigger system handles `repo`, `wave`, and `ci_failure`. This design extends it to cover chord-wave semantics without introducing chord-specific runtime concepts.

## Approach

### Area-derived membership for wave triggers

The core insight: **`Signal::Wave` becomes area-aware**. Today a wave trigger requires an explicit `source` field naming a single source wave. When `source` is omitted, the trigger derives sources from the listening wave's `area` entries matching `wave/<name>/`.

For the redesign chord-wave with `area: [wave/chord-model/, wave/signals/, ...]`, a single `signal: wave` trigger (no `source`) automatically fires on completion of any of those four waves. When area entries change, membership changes, trigger routing changes — zero sync.

This preserves the existing explicit-source behavior. A wave trigger with `source: infra` still listens to exactly one wave. Omitting `source` opts into area-derived behavior. The two modes use the same `Signal::Wave` enum value, same activation path, same coalescing.

### New `Signal::Block` for escalation

A new signal type (`Block = 4`) fires when a member wave enters a blocked state (`QueueBlock` with reason `RebaseConflict`, `ScratchDirty`, or `PromotionFailed`). Area-derived the same way — a `signal: block` trigger on a chord-wave fires for blocks in any member wave.

The queue reconciler (`reconcile_wave_queue`) already detects blocks. After recording a `QueueBlock`, it emits an `Event::WaveBlocked` through the event hub. A new `spawn_block_handler` listener (same pattern as `spawn_ci_failure_handler`) picks up these events and dispatches activations to chord-waves with matching area-derived membership.

Not all blocks escalate. `MissingPr` and `WaveRunning` are transient — they resolve in the next reconciliation cycle. Only persistent blocks (`RebaseConflict`, `ScratchDirty`, `PromotionFailed`) fire the signal.

### Plural triggers in wave YAML

`WaveConfig.triggers` changes from `Option<TriggerDef>` to `Option<Vec<TriggerDef>>`. Multiple triggers per wave are already supported in the runtime store — this change aligns the config format.

```yaml
# wave/redesign/redesign.yaml
flow: tend
mode: cron
cron: "0 9 * * *"
area:
  - wave/chord-model/
  - wave/clear-the-deck/
  - wave/agent-embedding/
  - wave/signals/
direction:
  - care
  - clarity
triggers:
  - signal: wave
    flow: tend
  - signal: block
    flow: tend
```

### `mode: managed` for member waves

A new wave mode. A managed wave doesn't have its own heartbeat — its chord-wave dispatches it. The cron poller ignores it, the loop ticker ignores it, but trigger activations (from the chord-wave's tend decisions) work normally.

`manual` means "a human runs this." `managed` means "my chord runs this." The difference matters for display (`lfq list` shows managed waves as part of their chord's rhythm, not idle), stall detection (a managed wave that hasn't run means its chord isn't tending, not that nobody remembered to type a command), and future beatgrids (managed waves are the natural children in beat subdivision).

Member waves (`chord-model`, `signals`, etc.) become `mode: managed`. The redesign chord-wave stays `mode: cron` — it's the heartbeat source.

```yaml
# wave/chord-model/chord-model.yaml
flow: ship-wave
mode: managed
# ...
```

### Cron

Set `mode: cron` and `cron: "0 9 * * *"` on the chord-wave. The cron poller already evaluates expressions and fires the wave's primary flow. Cron waves already receive event-driven triggers (e.g. `ci_failure` fires regardless of mode), so `mode: cron` is the base rhythm and wave/block triggers layer on top. No new code needed — just configure the redesign wave.

### Webhook merge fires completion

When a member wave's PR merges, lfd's webhook handler already processes the event via `QueueTrigger::WebhookMerged` in `reconcile_wave_queue`. This path gains a call to `trigger_listeners_on_completion` for the merged wave, which fires area-derived wave triggers on chord-waves.

No new API endpoint. No CLI coupling. `lf ops` stays freeform — it doesn't call lfd. The webhook merge path is more accurate (fires after actual merge, not merge-queue submission) and more reliable (doesn't depend on CLI behavior).

### Debounce via coalescing

Area-derived wave triggers use a single trigger record per chord-wave (one `Signal::Wave` trigger with no `source_wave_id`). All member-wave completions activate the same trigger_id. The existing `enqueue_pending_activation` coalesces activations for the same `(wave_id, trigger_id)` pair — so four member waves completing in quick succession produce one tend run, not four.

The activation reason accumulates the source wave names: `"wave completion: chord-model, signals"` — visible in run history.

### Activation reasons in run history

Every `ActivationLog` already stores a `reason` string. Area-derived triggers include the signal type and source wave name in the reason:

- `"member wave chord-model completed"`
- `"member wave signals blocked: rebase_conflict"`
- `"cron: 0 9 * * *"`

`lfq logs` and Concerto surface these through existing activation log queries.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Explicit trigger per member wave | Verbose YAML, must sync triggers with area | Violates "membership is area"; becomes bookkeeping |
| New `Signal::Members` type | Clean but introduces a signal that only chord-waves use | Unnecessary specialization — `Signal::Wave` without `source` achieves the same thing |
| Time-based debounce window | Configurable delay before dispatching | Adds complexity; existing coalescing already batches same-trigger activations |
| Block escalation via `Signal::CiFailure` | Reuse existing signal | Blocks aren't CI failures; conflating them confuses activation logs and trigger routing |

## Key decisions

**Area-derived, not explicit.** A chord-wave's trigger doesn't name its members — it discovers them from `area`. This is the "membership is area" principle applied to triggers. If area changes, triggers follow.

**One trigger record, many sources.** A sourceless `Signal::Wave` trigger produces one row in the trigger table. All member completions coalesce through that row. This turns the existing coalescing mechanism into chord-wave debounce for free.

**Persistent blocks only.** `MissingPr` and `WaveRunning` are queue states that self-resolve. Escalating them would flood tend with noise. Only `RebaseConflict`, `ScratchDirty`, and `PromotionFailed` fire `Signal::Block`.

**Webhook merge fires completion.** When a member wave's PR merges, lfd's existing webhook handler calls `trigger_listeners_on_completion`. No new endpoint, no CLI-to-daemon coupling. `lf ops` stays freeform.

**Managed mode for member waves.** `mode: managed` means "my chord runs this." Distinguishes from `manual` (human-initiated) for display, stall detection, and future beatgrids. Cron/loop pollers ignore managed waves, same as manual — the difference is semantic, not mechanical (yet).

**Breaking YAML change for `triggers`.** Singular → list. No backwards compatibility shim. Any existing wave configs with `triggers:` as a single object get a migration note in the PR.

## Scope

**In scope:**
- `Signal::Block` variant (value 4) in trigger types
- `mode: managed` variant in wave modes
- Area-derived source resolution in `trigger_listeners_on_completion`
- `spawn_block_handler` listener for block escalation
- `WaveConfig.triggers` → `Vec<TriggerDef>`
- `trigger_listeners_on_completion` called from webhook merge handler
- Redesign wave config update (mode: cron + triggers)
- Member wave config updates (mode: managed)
- Activation reason formatting for area-derived triggers

**Out of scope:**
- Letta memory integration (chord-model/03)
- Wave mutation API (chord-model/06)
- Cross-repo triggers (`source_repo` for area-derived triggers)
- Block self-healing (signals wave)
- Concerto UI for activation logs

## Done when

```bash
# Redesign chord-wave tends after member wave completes
cargo test -p loopflow trigger_listeners_area_derived

# Block escalation fires tend
cargo test -p loopflow block_signal_fires_chord_tend

# Cron tend runs daily (config-only, existing cron tests cover machinery)
grep 'cron:' wave/redesign/redesign.yaml

# Debounce: multiple completions coalesce
cargo test -p loopflow area_derived_coalesces_member_completions

# Activation reasons visible in logs
cargo test -p loopflow activation_log_includes_source_wave_name

# Webhook merge fires area-derived triggers
cargo test -p loopflow webhook_merge_fires_area_derived_triggers

# Managed mode ignored by cron/loop pollers
cargo test -p loopflow managed_mode_not_polled
```

Advances chord-model wave goals:
> "The redesign chord-wave can run `tend` against its own member waves through ordinary wave configs and APIs"
> "Wave mutation stays waves-only: direction, area, flow, triggers, work items, and lifecycle all mutate through one model"

Addresses chord-model risk:
> "DAG and trigger work could leak chord-specific special cases back into the runtime" — mitigated by making area-derived triggers a general capability of `Signal::Wave`, not a chord-only concept.
