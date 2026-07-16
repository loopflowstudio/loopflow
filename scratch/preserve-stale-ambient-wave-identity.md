# W2-239 — Preserve stale ambient Wave identity failures in trace and run attribution

Infrastructure task `W2-239` (Linear `c098765f-d10a-42e8-95fb-1e92dc98c20d`, project
`developer-efficiency`). Discovered after Product W2-151 (the shared ambient-wave
resolver, PRs #915/#979) merged.

## The bug

`engine::wave_context::resolve_run_wave_name()` is

```rust
pub fn resolve_run_wave_name() -> Option<String> {
    resolve_managed_wave_name_sync(None).ok()   // <- swallows the classified error
}
```

`.ok()` collapses **every** `WaveResolveError` — `NoContext`, `StaleIdentity`,
`Registry` — to a single `None`. The trace/run attribution path
(`bin/lf.rs::with_runtime` and `journal::ensure_run_context`) reads only this
`Option`, so a run whose `LF_WAVE_ID=<uuid>` names a Wave this machine's registry
has no row for is attributed to **no wave**, indistinguishable from a bare command
with no managed identity at all. The classified `StaleIdentity` — the signal that a
durable identity *was supplied and failed validation* — is erased.

The PM arm already does the right thing (`ops/mod.rs:494`):

```rust
match resolve_managed_wave_name_sync(explicit) {
    Ok(name) => Ok(Some(name)),
    Err(WaveResolveError::NoContext) => Ok(None),   // bare command: all-waves fallback
    Err(other) => Err(other.into()),                // stale/registry: loud, classified
}
```

Trace/run attribution is non-fatal (a run must not crash because its wave identity
is stale), so "loud" here means **propagate the classified failure into the record
and warn**, not abort the command.

## Contract (from the task's "Done when")

1. Trace/run attribution **propagates** classified stale identity and registry
   failures (does not swallow them to `None`).
2. Worktree inference is used **only** when no managed identity was supplied
   (`NoContext`); **never** after a supplied identity failed validation. The
   resolver invariant holds — "repository location cannot identify a Wave" — so
   this fix introduces **no** worktree inference; it only ensures the stale case
   is not silently re-attributed. `None` is the honest wave for both absent and
   stale; the difference is that stale carries a recorded failure.
3. Error text names the stale source and the safe explicit recovery
   (`--wave <name>`).
4. Tests cover stale UUID, stale name (a hand-set name is durable, never stale),
   absent identity, valid UUID/name, and explicit override.
5. Existing traces are not rewritten; wire DTOs stay honest (`wave` is `None` when
   there is no valid wave name — never a UUID or an invented name).

## Design

### `engine/wave_context.rs`

Add the run-attribution decision as a first-class value so both attribution sites
share one classification and it is unit-testable:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunAttribution {
    /// The wave name to attribute the run to, or `None` when there is no valid
    /// managed identity (absent context, or a supplied identity that failed).
    pub wave: Option<String>,
    /// A classified failure to record when a supplied identity failed
    /// validation (`StaleIdentity` / `Registry` / `UnknownExplicit`). `None` for
    /// a valid name and for absent context (absent context is not a failure —
    /// worktree inference stays a legitimate fallback for it alone).
    pub failure: Option<String>,
}

pub fn run_attribution() -> RunAttribution {
    match resolve_managed_wave_name_sync(None) {
        Ok(name) => RunAttribution { wave: Some(name), failure: None },
        Err(WaveResolveError::NoContext) => RunAttribution { wave: None, failure: None },
        Err(error) => RunAttribution {
            wave: None,
            failure: Some(recovery_text(&error)),
        },
    }
}
```

`recovery_text` returns the error's `Display` (which for `StaleIdentity` already
names the id and `pass --wave <name>`) and appends the recovery hint for
`Registry` (whose `Display` doesn't carry one). `resolve_run_wave_name()` stays as
the thin `run_attribution().wave`-only wrapper so non-attribution callers
(`lf home`) keep their current behavior — out of this task's scope.

### `journal::ensure_run_context`

Resolve through `run_attribution()` instead of `resolve_run_wave_name()`. On a
classified failure, set `RunContext.wave = None` (honest — no valid wave to name)
and `debug!`-log that the run was **not** inferred from the worktree. No warn here
(the producer-facing warn lives in `with_runtime`, once per command).

### `bin/lf.rs::with_runtime`

Resolve once via `run_attribution()`. The `Run/Started` event carries
`wave_name = attribution.wave` and `error = attribution.failure`. When
`attribution.failure` is `Some`, `warn!` once naming the failure and the recovery.
`wave` is `None` for both absent and stale; the `error` field is what makes a stale
run distinguishable from an absent-context run in the durable ledger — honest, in
the existing wire shape (no schema/DTO change, no backfill).

Run status is derived from the **terminal** event (`runs.rs:856`), so an `error` on
the `Started` row never marks the run as errored — the run still completes
non-fatal, exactly as before.

## What is explicitly NOT in scope

- No worktree-name inference for `NoContext` (would violate "repository location
  cannot identify a Wave" and risk misattribution from task-worktree basenames).
  `NoContext` stays `None`, as today.
- `lf home` routing/probe keep using the `Option` wrapper (unchanged). Home is not
  trace/run attribution; surfacing stale there is a sibling concern.
- No `run_events` schema change, no DTO mirror change. The stale signal rides the
  existing `error` column; `wave` stays `None`.

## Done when

- [x] `run_attribution` classifies NoContext / StaleIdentity / Registry.
- [x] `ensure_run_context` + `with_runtime` propagate the classified failure; wave
      is `None` on stale, never inferred from the worktree.
- [x] Tests: stale UUID (ledger), stale name + absent + valid name
      (`run_attribution`), valid UUID (existing ledger test), explicit override
      (existing `wave_resolution_tests` matrix).
- [x] `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test -p loopflow`.
