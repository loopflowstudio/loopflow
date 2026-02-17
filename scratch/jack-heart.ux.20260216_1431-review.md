# Built-in Wave Schemas — Gate Review

## What was implemented

- Added wave schema discovery from two sources:
  - built-ins embedded from `builtins/waves/*.yaml`
  - repo-local schemas from `wave/<name>/<name>.yaml`
- Added `GET /v0/wave/schemas` returning schema metadata (`schema_ref`, source, defaults, stimulus, active wave mapping).
- Extended `POST /v0/waves` with optional `schema` input (`scan`, `builtin://scan`, `file://...`) and schema-defaulted creation.
- Added wave provenance persistence on wave rows (`schema_ref`, `schema_name`) with migration `006_wave_schema_provenance`.
- Added Swift `WaveSchema` model + service fetch (`listWaveSchemas`) and Concerto UI entry points (schema quick-start and “instantiate all”).
- Added/updated tests for schema resolution and schema stimulus validation.

## Key choices

- **Schema refs as canonical identity**: active schema mapping uses stored `schema_ref` first, then fallback name matching for legacy rows.
- **Resolution policy**: unqualified names prefer local schemas; explicit refs select exact sources.
- **Conflict behavior**: duplicate local schema names are surfaced as conflicts, not silently accepted.
- **Polish changes in this gate pass**:
  - canonicalized explicit `file://...` lookups so non-canonical paths resolve reliably
  - validated cron schema stimuli require a non-empty cron expression
  - propagated server error messages in Swift `createWave` for clearer user feedback
  - refreshed schema state after wave create/delete events so sidebar instantiation status stays accurate

## How it fits together

Rust now discovers and resolves `WaveSchema` definitions, applies them during wave creation, and stores provenance in the wave record. The schemas endpoint merges discovered schemas with active-wave state from storage. Swift consumes that endpoint into `RepoState.waveSchemas`, and the sidebar uses it to render one-click schema instantiation and batch actions.

## Risks and bottlenecks

- Local schema scanning is filesystem-based on each list/resolve request; large `wave/` directories may increase latency.
- `schema_ref` for local files is absolute; moving repos can leave stale provenance on existing rows (expected, but worth noting).
- Batch instantiation currently creates waves sequentially; large batches are correct but not maximally fast.
- API coverage is mostly unit-level around resolution logic; end-to-end HTTP behavior still relies on broader integration coverage.

## What's not included

- Owner/source filtering UX for safer selective batch instantiation.
- Schema editor/versioning/migration workflows.
- Remote schema registries/marketplace.
- Auto-instantiation/recommendation logic.
