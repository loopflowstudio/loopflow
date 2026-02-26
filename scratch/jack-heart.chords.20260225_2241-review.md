# Design Review: Chords Phases 03–03.5 (Signal Simplification + Parallel Execution)

## What was implemented

**Phase 03: Listen Authoring.** Listen stimuli are now fully functional — wave config YAML supports `kind: listen` with `source` and `source_repo` fields. Source resolution is eager and FK-backed. On source wave completion, listener waves receive activations through the existing pending-activation queue with coalescing.

**Phase 03.5: Signal Simplification.** Collapsed two separate type hierarchies (`WaveRunKind`/`SidecarKind` and `StimulusKind`) into a single `Signal` enum. CI failure remediation is now a normal activation (`Signal::CiFailure` with `flow: "ci-fix"`) rather than a dedicated sidecar executor path. Added `flow: Option<String>` on `Stimulus` so any stimulus can override the wave's default flow at activation time.

**Parallel execution foundation.** Non-serialized waves spawn per-run worktrees (`-run-{hash}` suffix) so concurrent activations don't stomp each other. Added pre-step `fetch+rebase` and post-step `commit+push` git sync at step boundaries. Serialized waves continue using the activation queue for sequential dispatch.

**Build flow update.** Added `lint` step to the `build` flow between `compress` and `gate`.

## Key choices

| Decision | Why | Alternative rejected |
|----------|-----|---------------------|
| `Signal` enum replaces both `StimulusKind` and `WaveRunKind` | One concept, one type. CI-fix is just a stimulus with a different signal and flow override. | Keep separate enums — more type safety but duplicated machinery |
| `wave.serialized: bool` controls queue vs parallel | Simple binary choice covers the two modes. Most waves will use default (parallel). | Per-stimulus concurrency limits — over-engineers current needs |
| Eager listen source resolution | Fail fast at wave creation. Better UX than discovering a broken reference at trigger time. | Lazy resolution — tolerates missing sources but hides config errors |
| CI-fix skips PR creation/advancement | CI-fix pushes to the existing branch; creating a separate PR would be confusing. | Always create PRs — simpler code but wrong UX |
| Per-run worktrees with `-run-{hash}` suffix | Clean separation for parallel runs. Worktree is cleaned up after run completes. | Shared worktree with locks — simpler but blocks concurrency |
| Pre/post step git sync | Origin is source of truth for wave branches. Concurrent writers need coordination at step boundaries. | Background sync — eventually consistent but risks lost commits |
| Keep `kind` in JSON API, use `Signal` internally | Python client and wave config YAML use `kind`. Coordinated rename deferred (noted in 035-signal-simplification.md). | Rename API field to `signal` — cleaner but requires Python client + Concerto update in lockstep |

## How it fits together

```
Trigger (watch/cron/loop/listen/ci_failure)
  │
  ├── wave.serialized == true ──→ enqueue_pending_activation → dispatch_wave_if_ready
  │                                 (queue + coalesce)           (create_wave_run_with_id)
  │
  └── wave.serialized == false ─→ spawn_immediate_activation
                                    (create_parallel_wave_run → per-run worktree)

Wave Executor (per run):
  pre_step_sync(fetch + rebase) → run_step → post_step_sync(commit + push) → advance
```

The `Stimulus.flow` field allows any trigger to override the wave's default flow. CI failure uses this to route to `ci-fix` flow. Listen stimuli can use it to route to a custom flow for inter-wave handoffs.

## Risks and bottlenecks

- **Pre-step rebase failures.** If concurrent pushes create conflicts, the rebase aborts and the step is skipped with a warning. This is correct behavior but may surprise users — the run continues on stale state. Post-step sync is stricter: push failure → fetch+rebase retry → hard fail.
- **Run worktree accumulation.** Cleanup happens on run completion/failure, but if the daemon crashes mid-run, orphaned worktrees remain until the next janitor sweep.
- **CI-fix flow name coupling.** `CI_FIX_FLOW = "ci-fix"` is a constant, and the SQL query `snapshot_flow <> 'ci-fix'` in `ListStackRuns` hardcodes the same string. A rename requires updating both.
- **Listen fan-out is unbounded.** N listeners on the same source all trigger simultaneously. No scheduler-aware throttling for fan-out.
- **Migration 017 is destructive.** Drops `kind` column from `stimuli` and `run_kind`/`ci_fix_kind` from `wave_runs`. No rollback path. Acceptable for current deployment stage.

## What's not included

- **Source-run context injection.** Listener runs don't receive source-run context (PR title, changed files, diff). Designed but deferred to post-Phase 03.
- **Cycle detection.** Chained listen graphs (A→B→C→A) are not detected. Current scale makes this unlikely.
- **Multi-source listen.** A stimulus can only listen to one source wave. Sufficient for current use cases.
- **Git sync hardening (Phase 03.6).** The `target_branch` field on `WaveRun` and `PendingActivation` is in unstaged WIP — not part of this review scope.
- **External API rename.** JSON API still serializes stimulus signal as `kind` for Python client compatibility. Internal function renamed to `signal_str`; API field rename deferred for coordinated rollout.

## Gate polish (this pass)

| Fix | File | What changed |
|-----|------|-------------|
| Function rename | `dto.rs:284` | `stimulus_kind_str(kind: Signal)` → `signal_str(signal: Signal)` — parameter and function name now match the `Signal` type |
| Stale reference | `wave/chords/README.md:62` | `StimulusKind::Listen` → `Signal::Listen` in Phase 01 retrospective |

**Noted but not changed (coordinated API change):**
- `StimulusDto.kind` field name — stays `kind` to match Python client and wave config YAML. The internal function is correctly named `signal_str`; the field value it produces is used in a `"kind"` JSON key. Rename requires Python client, Concerto, and wave config schema update in lockstep.
- `#[serde(alias = "kind")]` on `AddStimulusRequest` — pragmatic transition shim; request accepts both `signal` and `kind`.

**Stale but out of scope:**
- `proto/loopflow/control/v1/control.proto` still uses `StimulusKind`. Proto is vestigial (HTTP JSON API is primary). Update when proto becomes active.
- `.lf/summary.md` references old type names. Auto-generated; will self-correct on next regeneration.

## Test coverage

- `execute_starts_listen_wave_on_completion` — listener receives a run when source completes
- `listen_trigger_queues_when_listener_running` — serialized listener queues when busy
- `listen_trigger_queues_when_scheduler_full` — parallel listener falls back to queue
- `resolve_ci_failure_stimulus_reuses_existing` — CI failure uses existing stimulus
- `resolve_ci_failure_stimulus_creates_default_ci_fix_flow` — auto-creates stimulus with ci-fix flow
- `handle_ci_failure_event_enqueues_push_activation_for_serialized_wave` — serialized CI failure queues
- `resolve_wave_id_in_repo_matches_repo_scope` — source resolution respects repo scope
- `listen_stimulus_schema_requires_source` / `parses_source_and_source_repo` — schema validation
- `is_ephemeral_worktree_path_detects_run_suffix` — cleanup identifies run worktrees
- `ci_failure_signal_storage_value_is_stable` — storage value pinned for compatibility
- `listen_signal_storage_value_is_stable` — storage value pinned for compatibility

## Validation

```
cargo fmt --check           ✅
cargo clippy -- -D warnings ✅
cargo test --all            ✅ (587 tests across 20 test targets)
uv run pytest python/tests/ ✅ (67 tests pass)
```
