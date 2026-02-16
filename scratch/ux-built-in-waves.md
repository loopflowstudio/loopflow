---
status: design
---

# Built-in Waves

## Problem

Loopflow ships built-in steps and flows, but users still have to manually assemble waves from scratch. A user who wants "scan my dependencies daily" has to: create a wave, pick the `scan` flow, set area to `.`, then add a cron stimulus. That's four steps for something that should be one click.

Built-in waves close this gap. They're pre-configured wave definitions — flow, area, and stimulus bundled together — that ship with the binary. Users browse them in Concerto and activate with one click. `lfd` only runs activated waves.

The first built-in wave is `scan` (daily CVE/dependency/upstream checks). The infrastructure supports any number of future built-in waves and repo-local wave definitions in `.lf/waves/`.

## Approach

### Rust: embed wave definitions + new API endpoint

**1. Embed wave YAML in binary via `build.rs`**

Add a fifth `generate_map` call in `build.rs` to scan `builtins/waves/` and produce `BUILTIN_WAVES`. Add `get_builtin_wave()` and `builtin_wave_names()` to `builtins.rs`, following the exact pattern of steps/flows/directions.

**2. Wave definition type**

New struct `WaveDefinition` in `engine/builtins.rs` (or a dedicated `wave_def.rs`):

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct WaveDefinition {
    pub name: String,
    pub flow: String,
    pub area: Vec<String>,
    pub stimulus: Option<StimulusDef>,
    pub direction: Option<Vec<String>>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct StimulusDef {
    pub kind: String,       // "cron", "watch", "loop"
    pub cron: Option<String>,
}
```

Parse each YAML string from `BUILTIN_WAVES` into `WaveDefinition` on demand. Also scan `.lf/waves/*.yaml` in the repo for repo-local definitions.

**3. `GET /v0/waves/available` endpoint**

Returns all wave definitions (built-in + repo-local), cross-referenced with active waves to show activation status.

```rust
#[derive(Serialize)]
struct AvailableWaveDto {
    name: String,
    flow: String,
    area: Vec<String>,
    stimulus: Option<StimulusDefDto>,
    direction: Vec<String>,
    description: Option<String>,
    source: String,              // "builtin" or "repo"
    active_wave_id: Option<String>, // non-null if already activated
}
```

Logic:
1. Collect all `WaveDefinition`s from builtins + `.lf/waves/*.yaml`
2. Load active waves from the store
3. Match by name — if an active wave has the same name as a definition, it's "activated"
4. Return the merged list

**4. `POST /v0/waves/available/:name/activate` endpoint**

Creates a real wave + stimulus from a definition. Reuses existing `create_wave` + `add_stimulus` logic internally.

Request: `{ "repo": "/path/to/repo" }` (the only thing the definition doesn't know).

Response: standard `WaveDto` of the newly created wave.

Activation flow:
1. Look up `WaveDefinition` by name (builtin first, then repo-local)
2. Check no active wave with that name exists in this repo
3. Create wave via store (name, flow, area, direction from definition)
4. If definition has stimulus, create stimulus via store
5. Emit `WaveCreated` event
6. Return enriched `WaveDto`

**5. Deactivation**

No special endpoint needed — deleting the wave deactivates it. The existing `DELETE /v0/waves/:wave_id` handles this. The `GET /waves/available` response will show `active_wave_id: null` after deletion.

### Swift: available waves section in sidebar

**1. Add `listAvailableWaves` to `WaveServiceProtocol`**

```swift
func listAvailableWaves(repo: URL) async throws -> [AvailableWave]
```

Model:
```swift
struct AvailableWave: Sendable, Identifiable {
    let name: String
    let flow: String
    let area: [String]
    let direction: [String]
    let description: String?
    let source: Source  // .builtin, .repo
    let activeWaveId: String?

    var id: String { name }
    var isActive: Bool { activeWaveId != nil }

    enum Source: String { case builtin, repo }
}
```

**2. RepoState integration**

- New property: `availableWaves: [AvailableWave]`
- Load on `refreshFlowsAsync()` (piggyback on existing startup call)
- Refresh after wave create/delete (activation status changes)

**3. Sidebar UI: "Available" section**

Add a third section to `WaveSidebar` below Idle (above On Disk):

```
Active     (2)
  engbot ● running
  fixbot ● waiting

Idle       (1)
  design-wave

Available  (1)
  scan  [Activate]

On Disk    (2)
  feature-xyz
```

Each available wave row shows:
- Name (e.g., "scan")
- Flow name as subtitle
- Source badge: "Built-in" or "Local"
- Activate button (or "Active" indicator if already activated)

Clicking Activate calls `POST /waves/available/:name/activate`, gets back a wave, inserts it into the store, and selects it. Uses the same optimistic pattern as wave creation.

If the wave is already active, the row shows a muted "Active" label and clicking selects the existing wave.

**4. No separate view or modal**

Available waves live in the sidebar, not a modal or separate screen. This keeps discovery natural — you see what's available right where you see what's running. As the number of available waves grows, the section could become collapsible, but with 1-3 waves it should stay visible.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Wave template modal | Separate discovery flow, more UI surface | Overkill for <10 available waves. A sidebar section is simpler and keeps everything in one place. |
| Auto-activate all built-in waves on first run | Zero-click setup, but presumptuous | Users should opt in. A cron job scanning every day shouldn't start without consent. |
| `POST /v0/waves` with `from_template: "scan"` | Reuse existing endpoint | Conflates creation with template instantiation. The available endpoint makes the catalog browsable. Separate activate endpoint keeps responsibilities clean. |
| Store definitions in the database | Queryable, standard CRUD | Unnecessary persistence. Definitions are static — they ship with the binary. Database stores only activated instances. |
| Repo-local definitions in `.lf/waves/` only | No binary embedding needed | Built-in waves should work in any repo without setup. Repo-local is the extension point, not the primary source. |

## Key decisions

1. **Match by name, not a separate `template_id`.** A wave named "scan" is the activated form of the "scan" definition. Simple, no foreign keys, no mapping table. Tradeoff: users can't have two waves both named "scan" — but that's already enforced by the unique(name, repo) constraint.

2. **`build.rs` handles wave embedding.** Same `generate_map` pattern as steps/flows/directions. Adding a new built-in wave is: drop a YAML file in `builtins/waves/`, rebuild. No manual registration.

3. **Available section in sidebar, not a separate view.** Discovery should be ambient. When you have zero waves, the available section is the first thing you see. When you have many waves, it's a quiet section you can ignore.

4. **Activation creates a real wave + stimulus atomically.** After activation, the wave is indistinguishable from one created manually. No special "built-in wave" status in the database. This keeps the execution engine simple.

5. **Deactivation = deletion.** No pause/disable state for the template relationship. Delete the wave and re-activate later if needed. The definition is always there.

## Scope

**In scope:**
- `build.rs` wave embedding (`BUILTIN_WAVES` HashMap)
- `WaveDefinition` struct + YAML parsing
- `GET /v0/waves/available` endpoint
- `POST /v0/waves/available/:name/activate` endpoint
- `AvailableWave` Swift model
- `listAvailableWaves()` service method
- Sidebar "Available" section with activate button
- Repo-local `.lf/waves/*.yaml` scanning

**Out of scope:**
- Wave definition editor (create/edit `.lf/waves/` files)
- Wave marketplace or remote definition registry
- Auto-activation or recommended waves
- Wave definition versioning or migration
- Description/documentation rendering for definitions (future: when we have more than `scan`)

## Implementation order

1. **Rust: embed + parse** — `build.rs` wave map, `WaveDefinition` struct, `get_builtin_wave()` / `builtin_wave_names()`
2. **Rust: API** — `GET /waves/available`, `POST /waves/available/:name/activate`
3. **Swift: model + service** — `AvailableWave`, `listAvailableWaves()`, `activateAvailableWave()`
4. **Swift: UI** — Sidebar "Available" section, activate button, state management
5. **Tests** — Rust: definition parsing, available endpoint. Swift: `WaveStore` integration.

## Done when

- `GET /v0/waves/available` returns `scan` (and any `.lf/waves/*.yaml` definitions)
- Concerto sidebar shows "Available" section with `scan` wave
- Clicking "Activate" on `scan` creates a wave + cron stimulus in the database
- The activated wave appears in the Active/Idle section immediately
- Deleting the wave makes it reappear in the Available section
- `cargo test` and `swift test` pass with new tests covering definition parsing and activation
