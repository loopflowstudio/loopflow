---
status: implemented
---

# Wave Schemas

## Summary

Wave schemas provide one-click wave creation from predefined defaults (flow, area, direction, and optional stimulus). Users can now instantiate built-in and repo-local schemas directly in Concerto and through `POST /v0/waves`.

## Problem

Before this work, users had to manually create each wave and wire up flow/area/stimulus fields. Common setups like daily dependency scans took multiple manual steps.

## Current implementation

### Schema model

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct WaveSchema {
    pub name: String,
    pub flow: String,
    pub area: Vec<String>,
    pub stimulus: Option<StimulusDef>,
    pub direction: Option<Vec<String>>,
    pub owner: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StimulusDef {
    pub kind: String, // "cron", "watch", "loop"
    pub cron: Option<String>,
}
```

`owner` is preserved for future filtering, but not used in v1 behavior.

### Schema sources

- **Built-in**: `builtins/waves/*.yaml` embedded via `build.rs` (`BUILTIN_WAVES`)
- **Local**: `wave/<name>/<name>.yaml` discovered from the repo at request time

### API surface

- **`GET /v0/wave/schemas`**
  - Returns built-in and local schemas
  - Includes `schema_ref`, `source`, defaults, and `active_wave_id`
  - Maps active state by stored wave provenance (`schema_ref`) with fallback name matching for legacy rows

- **`POST /v0/waves`** (extended)
  - Adds optional `schema` input
  - Supports:
    - unqualified names (`scan`)
    - explicit built-in refs (`builtin://scan`)
    - explicit local refs (`file:///.../wave/<name>/<name>.yaml`)
  - Applies schema defaults for missing request fields
  - Creates schema stimulus when provided
  - Stores wave provenance (`schema_ref`, `schema_name`)

### Resolution and conflict behavior

- Unqualified schema names prefer **local** over built-in
- Explicit refs always select exact targets
- Ambiguous resolution returns **`409 Conflict`**
- Missing schema returns **`404`**
- Explicit `file://...` lookups are canonicalized for reliable matching

### Persistence

Wave rows now store:

- `schema_ref: Option<String>`
- `schema_name: Option<String>`

Migration: `006_wave_schema_provenance`.

### Swift + Concerto integration

- Added `WaveSchema` model in LoopflowCore
- Added `listWaveSchemas(repo:)` in `WaveServiceProtocol` and `LocalWaveService`
- Added `RepoState.waveSchemas`
- Refreshes schema state after create/delete so instantiation status stays accurate
- Added sidebar quick-start and **Instantiate All** entry points

## Quality and validation updates

- Cron schema stimuli now require non-empty cron expressions
- Swift `createWave` propagates server error messages for clearer feedback
- Added/updated tests around schema resolution and stimulus validation

## Known risks

- Local schema discovery is filesystem-based per request; large `wave/` trees may increase latency
- Local provenance uses absolute `file://` refs; moving repos can leave stale historical refs
- Batch instantiation is sequential (correct, but not maximally fast)
- Most coverage for this feature is unit-level; more HTTP-level integration coverage would reduce regression risk

## Not included

- Owner/source filtering in UI
- Schema editor/versioning/migrations
- Remote schema registry/marketplace
- Auto-instantiation or recommendations
