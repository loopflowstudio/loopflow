# Algedonic Signals: Error → Repair → Escalate

## The model

Every chord is a viable system (VSM). Each chord maintains all five systems for its members:

- **S1** — member waves doing the work
- **S2** — coordination between members (triggers, activation coalescing)
- **S3** — control and assessment (tend flow, resource allocation)
- **S4** — intelligence (watching for changes, CI status, environment shifts)
- **S5** — identity and policy (what this chord is responsible for, what it allows autonomously, what it escalates)

The algedonic channel bypasses the normal hierarchy. A pain signal from S1 goes straight to S5 of the nearest chord. If that chord can't handle it, it escalates to its parent's S5. The root chord's S5 is the most consequential — it holds the identity and boundary of the whole system, and its algedonic signals go to the human because there's no parent above it.

## The loop

The general pattern for any step failure:

```
step fails
  → was this already a repair attempt?
    → yes: create algedonic signal → route to parent chord
    → no: classify error → launch headless repair in same branch/worktree
      → repair succeeds: continue flow
      → repair fails: create algedonic signal → route to parent chord
  → chord receives algedonic signal
    → applies its S5 policy: can I handle this?
      → yes: handle (re-run, reassign, adjust config)
      → no: escalate to parent chord
      → no parent (root): surface to human via attention queue
        → human opens interactive session with full context
```

Error classification drives repair strategy:

| Error class | Detection | Repair flow |
|-------------|-----------|-------------|
| CI failure | GitHub webhook | `ci-fix` |
| Agent crash / non-zero exit | Exit code | `debug` with error log |
| Step contract violation | Post-step output check | Re-run with format guidance |
| Branch router ambiguous | No valid path matched | Re-run with stricter prompt |
| Rebase conflict | Git exit code | Auto-resolve attempt |
| Unknown | Catch-all | `debug` with error context |

## CI failure vertical slice

First end-to-end path. Proves the atoms work.

### What exists today

- GitHub webhook receives check_run failure → emits `Event::CiFailure`
- `ci_failure_handler` listens, creates activation, launches `ci-fix` flow
- `ci-fix` flow runs in the failing branch's worktree
- `AttentionItem` type exists with `kind: Algedonic`
- Concerto has `AttentionQueueView`

### What's missing

1. **Post-run failure hook.** Generic, not CI-specific. After any run completes with `Failed`, the executor checks: was this a repair attempt? If yes, escalate. If no, attempt repair.

2. **Repair lineage.** `WaveRun.repair_of: Option<LfdId>` — explicit link to the run being repaired. The executor checks this to know whether failure means "attempt repair" or "escalate."

3. **Error classifier.** Examines the failed run and returns a repair strategy (flow name + context). CI failures return `ci-fix`. Agent crashes return `debug`. Unknown returns `debug` with error log.

4. **Algedonic signal creation.** On repair failure, create `AttentionItem(kind: Algedonic)` with:
   - `wave_id`: the wave that failed
   - `run_id`: the failed repair run
   - `chord_id`: target chord (parent of the failing wave, or root)
   - `context`: JSON with original error, repair attempt logs, branch state
   - Emits `Event::AttentionCreated`

5. **Auto-resolve on success.** When the original problem is fixed (CI passes, step succeeds on retry), resolve any pending algedonic items for that wave + branch.

6. **Interactive fallback.** Clicking an algedonic item in Concerto launches interactive `debug` in the failing worktree with full error context.

### Sequence

```
step fails (any step, any wave)
  → executor post-run hook
  → run.repair_of is None → first failure
    → classify_error(&run) → "ci-fix" / "debug" / etc.
    → launch repair run in same branch/worktree
      → repair_run.repair_of = Some(failed_run.id)
  → repair run fails
    → executor post-run hook
    → run.repair_of is Some → repair attempt failed
    → create AttentionItem(kind: Algedonic, chord_id: parent_chord)
    → emit Event::AttentionCreated
    → Concerto shows in attention queue
    → human clicks → interactive debug with context
```

### Retry limits

- First failure: launch repair
- Repair fails: algedonic signal, stop
- Repair succeeds but same problem recurs: repair again
- 3 repair attempts on same (wave, branch): algedonic signal, stop retrying

## Atoms that change

### WaveRun

```rust
pub struct WaveRun {
    // ... existing fields ...
    pub repair_of: Option<LfdId>,  // links to the run this is repairing
}
```

### AttentionItem

Add `chord_id: Option<LfdId>` — the chord this signal is addressed to. `None` means root (goes to human). When nested chords exist, this enables routing.

### Executor post-run hook

```rust
// After any run completes:
match (run.status, run.repair_of.as_ref()) {
    (Failed, Some(_)) => {
        // Repair attempt failed — escalate
        create_algedonic_signal(store, event_hub, &run, parent_chord_id).await;
    }
    (Failed, None) => {
        // First failure — attempt repair
        let repair = classify_error(&run);
        launch_repair_run(store, executor, &run, repair).await;
    }
    _ => {} // success or other status
}
```

## Done when

```bash
# Any step failure triggers repair attempt
cargo test -p loopflow step_failure_launches_repair_run

# Failed repair creates algedonic signal
cargo test -p loopflow repair_failure_creates_algedonic_signal

# CI-specific: webhook → ci-fix → escalation path
cargo test -p loopflow ci_failure_repair_escalation_e2e

# Success resolves algedonic signals
cargo test -p loopflow repair_success_resolves_algedonic_signal

# Retry limit respected
cargo test -p loopflow repair_respects_max_attempts
```
