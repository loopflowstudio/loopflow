# Self-heal PM snapshots when a referenced Project disappears

## Problem

`lf status <wave>` errors entirely when a non-terminal Session references a
Project that the local PM snapshot omits. The Mac app gets a non-zero exit and
no JSON — it loses the wave info, runs, attention, and home runtime along with
the projects. The Product Wave surface proof
(`scripts/prove_product_wave_surface.sh`) fails because `lf status product`
errors on a stale `product-performance` reference.

Root cause: `WaveDetailSnapshot.projects` is `Vec<ProjectDetailSnapshot>`, not
`Evidence<ProjectDetailSnapshot>`. The DTO already wraps `runs` and `attention`
in `Evidence` — "we looked and found nothing" vs "we could not look" — but
projects are all-or-nothing. A missing Project kills the entire status read.

A secondary cause: `lf status` never refreshes a stale snapshot. `lf pm show`
auto-refreshes via a bounded TTL policy (`PmRefresh::Auto`), but `lf status`
reads the cache directly. A stale snapshot that omitted a Project stays stale
until a human runs `lf pm sync`.

## The demo

```bash
lf status product --json | jq '.projects.state'
# "ok" — projects render from a fresh snapshot

# Simulate a stale snapshot: remove a Project from the stored payload,
# then read status. The wave still renders — projects carry an unavailable
# state with the recovery reason, not a silent zero-project result.
lf status product --json | jq '.projects'
# {"state":"unavailable","reason":"Project Session ps_... references Project X, absent from the current PM snapshot; run `lf pm sync --wave product`. If the Project remains absent, settle the stale Session with `lf project abandon X ...`"}

# After `lf pm sync --wave product`, the next `lf status product` renders
# the Project, KRs, and Task rows — no Wave restart needed.
scripts/prove_product_wave_surface.sh
# PASS — real Product data rendered through the production registry path.
```

## Approach

Three changes, each independently testable:

### 1. Wrap `WaveDetailSnapshot.projects` in `Evidence<ProjectDetailSnapshot>`

Change `projects: Vec<ProjectDetailSnapshot>` to
`projects: Evidence<ProjectDetailSnapshot>`. This mirrors `runs` and
`attention` in the same DTO and `WaveRoadmap.projects` in the roadmap DTO. The
Mac app can then render wave info with projects marked unavailable, instead of
losing everything on a command error.

`lf status` wraps the `snapshot_projects` result:
- `Ok(details)` → `Evidence::complete(details)`
- `Err(err)` → `Evidence::Unavailable { reason: err.to_string() }`

The error message already carries recovery commands (`lf pm sync --wave`, `lf
project abandon`/`lf task abandon`) from the uncommitted `session_project_index`
work.

### 2. Add bounded auto-reconciliation before erroring

Before wrapping a missing-Project error in `Evidence::Unavailable`, `lf status`
tries one bounded refresh when the snapshot is stale (>1h):

1. Pre-check: do any non-terminal Sessions reference a Project not in
   `planning.projects`?
2. If yes, is the snapshot stale (> `PM_SOFT_STALE_SECS`)?
3. If stale, call `try_timed_refresh` (5s timeout, same mechanism as `lf pm
   show`). Best-effort: a failed refresh falls through to the stale planning.
4. Re-read planning from the store.
5. Call `snapshot_projects` with the refreshed planning.

This is "bounded targeted reconciliation": one refresh attempt, one wave,
5-second ceiling. It does not loop. If the Project is genuinely gone from
Linear, the refresh succeeds but the Project remains absent — the error
becomes `Evidence::Unavailable` with recovery instructions.

`lf roadmap`'s `wave_roadmap_projects` gets the same pre-check + reconcile
before its existing `Evidence::Unavailable` fallback.

### 3. Keep the uncommitted session-handling fix

The working-tree changes already handle the session-vs-snapshot mismatch:
- Terminal Sessions for an absent Project → skipped (history, not current work)
- Non-terminal Sessions for an absent Project → error with recovery commands

These stay as-is. The `Evidence` wrapping and auto-reconciliation layer on top.

## De-risking

| Question | Finding | Impact on design |
|----------|---------|-----------------|
| Does `Evidence<T>` already exist in the status DTO? | Yes — `runs: Evidence<SkillRunEntry>` and `attention: Evidence<AttentionItem>` in `WaveDetailSnapshot`. The Swift `WorkEvidence` decoder already handles `ok`/`unavailable`. | Adding `Evidence<ProjectDetailSnapshot>` reuses the existing wire pattern; no new serialization logic. |
| Does `lf roadmap` already wrap projects in `Evidence`? | Yes — `WaveRoadmap.projects: Evidence<RoadmapProject>`. The Swift `RoadmapView` already switches on `.available`/`.unavailable`. | The status path follows the same pattern; `WaveDetailPane` gets the same switch. |
| Is there a bounded refresh mechanism we can reuse? | Yes — `try_timed_refresh` in `ops/pm.rs` (5s timeout, used by `lf pm show`'s `PmRefresh::Auto`). Currently private to `ops/pm.rs`. | Expose a `pub(crate)` reconciliation entry point; no new timeout or staleness logic. |
| Will the DTO change break existing consumers? | `WaveDetailSnapshot.projects` is read as an array in ~10 Swift files and the Rust fixture test. The DTO fixture (`tests/fixtures/dto/wave_detail.json`) has `projects: [...]`. | One mechanical migration: `projects` becomes `{state:"ok", items:[...], truncated:false}`. Update fixture, Swift model, `WaveDetailPane`, and tests. No backwards-compat shim. |
| Does `prove_product_wave_surface.sh` need updating? | It checks `if not projects: FAIL` (Python). With `Evidence`, `projects` is a dict, not a list. | Update the script to check `projects["state"] == "ok"` and `projects["items"]`. |
| Can the product wave render without auto-reconciliation? | All `product-performance` sessions are terminal (abandoned/completed). The uncommitted fix skips them. The remaining projects (mac-surface-ux, loopflow-api, auditability) are in the snapshot. | Auto-reconciliation is not needed for the product wave today, but it's the safety net for future stale snapshots with live sessions. |
| Does `lf status` stay fast? | Auto-reconciliation only triggers when a non-terminal Session references a missing Project AND the snapshot is stale. Fresh snapshots and healthy waves never touch the network. | No latency impact on the common path. The stale+missing path adds at most 5s. |
| Does `snapshot_projects` need a new error type? | No — the pre-check (set membership on `planning.projects` vs non-terminal session project ids) detects the missing-Project condition without parsing error strings. | `snapshot_projects` stays unchanged; the reconciliation logic lives in the caller. |

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep `Vec`, error on missing Project (current uncommitted behavior) | Simpler — no DTO change. | Mac app loses all wave info on error. Can't distinguish "stale" from "empty" — criterion 1 requires the distinction, and the status DTO is the only signal the Mac app has. |
| Auto-sync on every `lf status` when stale | Always-fresh snapshots. | Adds network latency to every stale read, even when the snapshot is healthy. The task says "triggers ... when a referenced Project is missing," not "on every read." |
| Change `snapshot_projects` to return a typed missing-Project error | Clean error handling. | More invasive than a pre-check. `snapshot_projects` would need a new error variant, and every caller would need to match on it. The pre-check is a simple set-membership test. |
| Don't auto-reconcile, just expose `Evidence::Unavailable` | Simpler — no network logic in `lf status`. | Criterion 2 says "triggers bounded targeted reconciliation OR exposes one actionable recovery path." The `Evidence` wrapper satisfies the OR, but the auto-reconciliation is the "self-heal" the task title promises, and it's the difference between the surface proof passing automatically vs requiring a manual `lf pm sync`. |

## Key decisions

1. **`Evidence` wrapping is the primary fix.** It lets every consumer
   (status, roadmap, Mac app) distinguish "unavailable" from "empty" without
   parsing error messages or losing wave-level info. The DTO change is
   mechanical: the `Evidence` pattern already exists in the same struct.

2. **Auto-reconciliation is bounded and targeted.** One refresh attempt, one
   wave, 5-second ceiling. It triggers only when a non-terminal Session
   references a missing Project and the snapshot is stale. A fresh snapshot
   with a missing Project means the Project is genuinely gone — no retry, just
   the unavailable state with recovery commands.

3. **The pre-check avoids error-string matching.** Before calling
   `snapshot_projects`, check if any non-terminal Session's project id is
   absent from `planning.projects`. This is a `HashSet` lookup, not parsing
   `anyhow::Error` strings.

4. **No DTO backwards-compat shim.** Change `projects` to `Evidence`, update
   the fixture, Swift model, and all consumers in one commit. The
   `AGENTS.md` rule: "Don't maintain backwards compatibility unless explicitly
   required."

5. **Terminal Sessions for absent Projects stay skipped.** An abandoned
   Session referencing a renamed Project is history, not current work. The
   uncommitted `session_project_index` fix handles this correctly.

## Scope

- In scope:
  - Change `WaveDetailSnapshot.projects` to `Evidence<ProjectDetailSnapshot>`
  - Wrap `snapshot_projects` result in `Evidence` in `lf status`
  - Add bounded auto-reconciliation (pre-check + `try_timed_refresh`) to `lf
    status` and `wave_roadmap_projects`
  - Expose `pub(crate)` reconciliation entry point in `ops/pm.rs`
  - Update Swift `WaveDetailSnapshot` model, `WaveDetailPane` rendering, and
    Swift tests
  - Update `tests/fixtures/dto/wave_detail.json` fixture
  - Update `scripts/prove_product_wave_surface.sh` Python parser
  - Deterministic Rust tests: missing Project → `Evidence::Unavailable`;
    stale + missing → auto-reconcile → `Evidence::Ok`; fresh + missing →
    `Evidence::Unavailable` with recovery; terminal sessions skipped
  - Keep the uncommitted `session_project_index` / `find_project_index` changes

- Out of scope:
  - Changing `lf pm show` behavior (already auto-refreshes)
  - Adding `Evidence` to other `Vec` fields in `WaveDetailSnapshot`
  - Mac app UI redesign for the unavailable state (reuse the roadmap pattern)
  - Renaming or deleting the `product-performance` Project in Linear

## Done when

1. `cargo test -p loopflow --lib waves::` passes — all existing tests plus new
   tests for `Evidence` wrapping and auto-reconciliation.
2. `swift test --package-path swift` passes — updated DTO fixture and model
   tests.
3. `scripts/prove_product_wave_surface.sh` passes against the installed `lf`
   (after `lf sync-skills` or `cargo install --path rust/loopflow`):
   `lf status product --json` returns `projects.state == "ok"` with the
   wave's referenced Projects.
4. `scripts/prove_wave_surface_states.sh` still passes (five states render
   distinctly).
5. A deterministic test removes one Project from a snapshot, calls
   `snapshot_projects`, and proves: (a) the result is
   `Evidence::Unavailable` with recovery commands; (b) after a bounded
   refresh restores the Project, the result is `Evidence::Ok` with KRs,
   Tasks, and runtime — no Wave restart.

## Measure

| Metric | Baseline | Target |
|--------|----------|--------|
| `lf status product` exit code | 1 (error) | 0 (renders) |
| `prove_product_wave_surface.sh` | FAIL | PASS |
| `WaveDetailSnapshot.projects` type | `Vec` (all-or-nothing) | `Evidence` (distinguishable) |
| Auto-reconciliation trigger rate | N/A | Only when non-terminal Session references missing Project + stale snapshot |
