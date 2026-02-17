---
status: design
---

# Wave Schemas

## Problem

Loopflow ships built-in steps and flows, but users still have to manually assemble waves from scratch. A user who wants "scan my dependencies daily" has to: create a wave, pick the `scan` flow, set area to `.`, then add a cron stimulus. That's four steps for something that should be one click.

Wave schemas close this gap. They're pre-configured wave definitions — flow, area, stimulus, and direction bundled together. Users browse schemas in Concerto and instantiate them with one click. The first built-in schema is `scan` (daily CVE/dependency/upstream checks).

## Approach

### Core type: WaveSchema

A schema that produces waves. Same type across Rust and Swift.

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
    pub kind: String,       // "cron", "watch", "loop"
    pub cron: Option<String>,
}
```

`owner` is preserved in the format for future use (filtering by owner, batch instantiation per-user). V1 ignores it — all schemas are shown and instantiable regardless of owner.

### Two sources

| Source | Path | How |
|--------|------|-----|
| Built-in | `builtins/waves/*.yaml` | Embedded in binary via `build.rs` `generate_map` |
| Wave directory | `wave/<name>/<name>.yaml` | Scanned from repo at request time |

Built-in schemas work in any repo without setup. Wave directory schemas are the repo-local extension point — co-located with wave plans.

Schema inputs support unqualified and explicit-ref forms:
- `scan` (unqualified, default resolution rules)
- `builtin://scan`
- `file:///abs/path/to/repo/wave/scan/scan.yaml`

### Rust: embed + parse

**1. Embed built-in schemas via `build.rs`**

Add a fifth `generate_map` call to scan `builtins/waves/` and produce `BUILTIN_WAVES`. Add `get_builtin_wave()` and `builtin_wave_names()` to `builtins.rs`, following the exact pattern of steps/flows/directions.

**2. Scan wave directory schemas**

At request time, scan `wave/*/` for `<name>.yaml` files matching the directory name. Parse into `WaveSchema`. Invalid YAML is skipped with a warning.

### Rust: API

**`GET /v0/wave/schemas`**

Returns all schemas (built-in + wave directory), cross-referenced with active waves to show instantiation status.

```rust
#[derive(Serialize)]
struct WaveSchemaDto {
    name: String,
    schema_ref: String,          // "builtin://scan" or absolute "file://..." URI
    flow: String,
    area: Vec<String>,
    stimulus: Option<StimulusDefDto>,
    direction: Vec<String>,
    owner: Option<String>,
    description: Option<String>,
    source: String,              // "builtin" or "local"
    active_wave_id: Option<String>, // non-null if already instantiated
}
```

Logic:
1. Collect all `WaveSchema`s from builtins + `wave/<name>/<name>.yaml`
2. Load active waves from the store
3. Match instantiated state by stored schema provenance (`schema_ref`) on wave rows
4. For older waves without provenance, fallback to name match with local-over-builtin precedence
5. Return the merged list

**`POST /v0/waves` (extended)**

The existing wave creation endpoint gains an optional `schema` field:

```rust
pub struct CreateWaveRequest {
    repo: String,
    name: Option<String>,
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
    schema: Option<String>,      // new: schema name or explicit schema_ref URI
}
```

When `schema` is present:
1. Resolve schema input:
   - **Unqualified (`scan`)**: local wins over builtin
   - **Explicit ref (`builtin://scan`, `file://...`)**: exact match
2. Ensure exactly one matching schema candidate; return `409 Conflict` when ambiguous
3. Return `404` when no matching schema is found
4. Fill in flow, area, direction from the schema (request fields override if also provided)
5. Create wave via store
6. If schema has stimulus, create stimulus via store
7. Persist schema provenance on the created wave (`schema_ref`)
8. Emit `WaveCreated` event
9. Return enriched `WaveDto`

No separate instantiation endpoint. The schema just provides defaults for wave creation.

### Collision and dedupe policy

- **Local overrides builtin for unqualified lookups.**
  - If both local `scan` and builtin `scan` exist, `schema: "scan"` resolves to local.
- **Error on ambiguous matches.**
  - Multiple local schemas matching one input (or any other ambiguous resolution) returns `409 Conflict` with candidate schema refs.
- **Use explicit refs to dedupe.**
  - `schema: "builtin://scan"` always picks builtin.
  - `schema: "file:///abs/path/to/repo/wave/scan/scan.yaml"` always picks that local schema.
- **Validation:**
  - Duplicate local schema names are an error (include file paths in response/logs).
  - Duplicate builtin names fail at build time.

### Schema provenance on waves

Persist provenance on instantiated waves so active-state mapping is exact and collision-safe.

Proposed fields on wave rows:
- `schema_ref: Option<String>` — absolute schema ref URI (`builtin://scan` or `file://...`)
- `schema_name: Option<String>` — human-friendly name (`scan`) for display/filtering

Behavior:
- Instantiated waves always store `schema_ref` and `schema_name`.
- Manually created waves leave these fields as `None`.
- `GET /v0/wave/schemas` uses `schema_ref` for `active_wave_id` mapping.
- If the repo moves, stale absolute file refs remain valid provenance for existing waves; future schema lookups use current scan results.

**Deinstantiation**

No special endpoint — deleting the wave deinstantiates it. The existing `DELETE /v0/waves/:wave_id` handles this. `GET /v0/wave/schemas` will show `active_wave_id: null` after deletion.

### Swift: model + service

```swift
struct WaveSchema: Sendable, Identifiable {
    let schemaRef: String
    let name: String
    let flow: String
    let area: [String]
    let direction: [String]
    let owner: String?
    let description: String?
    let source: Source  // .builtin, .local
    let activeWaveId: String?

    var id: String { schemaRef }
    var isInstantiated: Bool { activeWaveId != nil }

    enum Source: String, Sendable { case builtin, local }
}
```

Add to `WaveServiceProtocol`:

```swift
func listWaveSchemas(repo: URL) async throws -> [WaveSchema]
```

**RepoState integration:**
- New property: `waveSchemas: [WaveSchema]`
- Load on startup (piggyback on existing `refreshFlowsAsync()`)
- Refresh after wave create/delete (instantiation status changes)

### Concerto UI

Sidebar UI details deferred to a separate design pass. The key interaction:

- **Empty state**: show available schemas as quick-start options
- **"+" button**: offer "New wave" + available schemas in a popover
- **Batch instantiation**: one button to instantiate all uninstantiated schemas and start them

"Instantiate all" is the primary UX goal — open Concerto, click one button, all your schemas become running waves.

For safety, batch instantiation includes a confirm sheet showing:
- Schemas that will be instantiated
- Which schemas add `cron`, `watch`, or `loop` stimuli
- Whether waves will start immediately

Filters (source/owner) are a planned follow-up so "instantiate all" naturally means "instantiate all visible."

## Key decisions

1. **WaveSchema, not WaveDefinition or AvailableWave.** One name for one concept, consistent across the stack.

2. **`/v0/wave/schemas` namespace.** Singular `wave` separates schema meta-resources from the `/v0/waves` instance CRUD. Avoids ambiguity where `schemas` could look like a wave ID.

3. **Extend `POST /v0/waves` instead of a separate instantiate endpoint.** The schema provides defaults for wave creation. No new write route needed.

4. **Unqualified convenience + absolute refs.** `schema: "scan"` is a convenience lookup (local > builtin). Exact selection uses `builtin://...` or absolute `file://...` refs.

5. **`build.rs` handles embedding.** Same `generate_map` pattern as steps/flows/directions. Adding a new built-in schema: drop a YAML in `builtins/waves/`, rebuild.

6. **Instantiation creates a real wave + stimulus.** After instantiation, the wave is indistinguishable from one created manually. No special status in the database. The execution engine stays untouched.

7. **Deinstantiation = deletion.** No pause/disable state. Delete the wave and re-instantiate later. The schema is always there.

8. **Schema provenance is stored on waves.** Instantiated waves persist `schema_ref` (+ `schema_name`) so active mapping is exact, collision-safe, and debuggable.
9. **Owner field preserved but unused in v1.** The YAML format supports `owner` for future filtering/batch-per-user. V1 shows all schemas regardless.
10. **`/v0/wave/schemas` stays singular by design.** Schema meta-resources live under `wave`; instantiated resource CRUD remains under `/v0/waves`.

## Scope

**In scope:**
- `build.rs` wave embedding (`BUILTIN_WAVES` HashMap)
- `WaveSchema` struct + YAML parsing
- `GET /v0/wave/schemas` endpoint
- `POST /v0/waves` extended with `schema` field
- Wave provenance fields on wave rows (`schema_ref`, `schema_name`)
- `WaveSchema` Swift model
- `listWaveSchemas()` service method
- Wave directory scanning (`wave/<name>/<name>.yaml`)
- Batch instantiation (instantiate all uninstantiated schemas)
- Batch confirmation UI that previews created waves/stimuli/start behavior
- Explicit schema refs (`builtin://...`, `file://...`)

**Out of scope:**
- Sidebar UI details (separate design pass)
- Owner-based filtering (follow-up to make batch safer/targeted)
- Wave schema editor
- Schema marketplace or remote registry
- Auto-instantiation or recommended schemas
- Schema versioning or migration

## Implementation order

1. **Rust: embed + parse** — `build.rs` wave map, `WaveSchema` struct, `get_builtin_wave()` / `builtin_wave_names()`, wave directory scanning
2. **Rust: API** — `GET /v0/wave/schemas`, extend `POST /v0/waves` with `schema` field
3. **Swift: model + service** — `WaveSchema`, `listWaveSchemas()`, `createWave(schema:)`
4. **Swift: UI** — batch instantiation button, confirmation sheet, creation flow integration
5. **Tests** — Rust: schema parsing, schemas endpoint, schema-based creation. Swift: store integration.

## Done when

- `GET /v0/wave/schemas` returns `scan` (and any `wave/<name>/<name>.yaml` schemas)
- `POST /v0/waves` with `schema: "scan"` creates a wave + cron stimulus
- `POST /v0/waves` supports `schema: "builtin://scan"` and absolute `schema: "file://..."` for explicit selection
- Local-vs-builtin collisions resolve as specified (local default; explicit refs; ambiguous => `409`)
- Concerto can instantiate all uninstantiated schemas with one action
- Concerto shows a confirmation preview before batch instantiation
- The instantiated wave appears in Active/Idle immediately
- Deleting the wave makes the schema show as uninstantiated again
- `cargo test` and `swift test` pass with new tests
