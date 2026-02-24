# Design Creates Waves

## Problem

Wave creation is a two-step tax: `lf design` produces `scratch/wave-proposal.md`, then the user runs `lf add-to-wave` to materialize it into `wave/`. This intermediate step loses context (the design conversation understood the vision, the promotion step just moves files) and creates friction (users forget to run it, or the proposal format doesn't match what `add-to-wave` expects).

Meanwhile, waves that grow too large have no organic way to decompose. You either started with the right granularity or you're stuck refactoring manually.

Both problems share a root cause: wave creation and wave structure are disconnected from the design conversation where the thinking actually happens.

## Approach

**Design writes waves directly.** When the user chooses "wave plan" in Phase 4, `design.md` creates the wave directory structure — README, YAML schema, numbered roadmap items — in a single step. No intermediate proposal file. The conversation context maps naturally to wave content:

- Dream phase → Vision
- Detail phase → Goals, Risks, Metrics
- Fork phase → Roadmap items + YAML config

**Split-wave decomposes overgrown waves.** A new interactive step reads a wave's README and roadmap, identifies natural boundaries, and creates child waves. The parent becomes a coordination wave whose roadmap references children instead of listing work directly.

### What changes

1. **`design.md`** — Phase 4 "wave plan" path rewrites its output section. Instead of writing `scratch/wave-proposal.md` and telling the user to run `add-to-wave`, it creates `wave/<name>/` directly with README.md, `<name>.yaml`, and `01-*.md` roadmap files. The "implement" path is unchanged.

2. **New `split-wave.md`** — Interactive step in `steps/interactive/`. Reads a wave, proposes decomposition, creates child waves, rewires the parent.

3. **`design.md` frontmatter** — `produces:` field updated to include `wave/<name>/`.

### What doesn't change

- Phase 1-3 of design (Dream, Detail, Size-check) — unchanged
- `add-to-wave` — still useful for promoting plan flow artifacts
- Wave content model (Vision/Goals/Risks/Metrics + numbered roadmap) — this is the target format
- `wave-plan.md` step — still useful for non-interactive wave planning from analysis
- Wave schema discovery in Rust — `load_local_wave_schemas` already scans `wave/<dir>/<dir>.yaml`; design just creates files in that expected location
- `ingest.md`, `update-wave.md` — consume wave content, don't care how it was created

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep proposal intermediate, improve `add-to-wave` | Lower blast radius | Doesn't solve the real problem — context loss between steps. The design session knows the vision; `add-to-wave` just moves files. |
| New `create-wave` step after design | More composable | Extra step that does less than design could do directly. The conversation already has all the content; delegating to another agent loses it. |
| `split-wave` as automated (non-interactive) | Faster in flows | Wave decomposition is a strategic decision. Getting the boundaries wrong creates orphaned child waves nobody maintains. Worth the human pause. |
| Parent-child wave relationship in Rust data model | Richer querying | Chord/voice tree already exists in `WaveData.parent_id`. No new data model needed — split-wave creates independent waves and rewires the parent's roadmap files to reference them. The relationship is in content, not schema. |

## Key decisions

**Design creates the YAML schema file.** The wave schema (`<name>.yaml`) holds flow, area, direction, stimulus. Design infers these from conversation context:
- `flow`: default `ship-wave` for new waves
- `area`: inferred from which files/directories the conversation referenced
- `direction`: inferred from the persona/perspective discussed
- `stimulus`: ask the user, or default to none (manual trigger)

This works because `load_local_wave_schemas` in `wave_schemas.rs` already discovers YAML files at `wave/<dir>/<dir>.yaml`. Design just creates files where the engine expects them.

**The first roadmap item becomes `scratch/<branch>.md`.** After creating the wave, design also writes the first item as a design doc for immediate implementation. This preserves the current `design → implement` flow while also creating the wave. The user can run `lf implement` right after design, or start the wave with `lf flow ship-wave`.

**Split-wave is interactive, not automated.** Wave boundaries affect team structure, agent allocation, and project direction. Getting them wrong is expensive — orphaned child waves, duplicated scope, lost context. The agent analyzes and proposes; the human decides.

**Split-wave creates independent waves, not nested data.** Child waves are full `wave/<child>/` directories with their own README, YAML, and roadmap. The parent wave's roadmap items get rewritten to reference children (e.g., "See `wave/<child>/`"). No new Rust types or database schema — the relationship lives in content.

**Wave README follows "Not here" pattern from `wavemodel`.** The "Not here" subsection under Vision (used in `wave/wavemodel/README.md` and other waves) is the established convention for scope boundaries. Design follows this pattern. Per the wave README's own risk note: "Section placement varies across waves" — so the parser should match the four canonical sections and treat everything else as supplementary.

## Scope

**In scope:**
- Rewrite Phase 4 "wave plan" output in `design.md` to create `wave/<name>/` directly
- New `split-wave.md` interactive step
- Update `design.md` frontmatter (`produces:` field)

**Out of scope:**
- Changes to Rust engine, wave executor, or schema discovery (already handles the file layout)
- Changes to `add-to-wave` (still useful for plan flows)
- Changes to `wave-plan.md` (still useful for non-interactive wave creation from analysis)
- Concerto UI for wave creation (depends on agentapi, called out in wave README "Not here")
- Database schema changes for parent-child relationships (content-level references are sufficient)
- Automated wave schema validation (convention first, as stated in wave README)

## Done when

1. `lf design` on a new branch, choosing "wave plan" in Phase 4, creates:
   - `wave/<name>/README.md` with Vision, Goals, Risks, Metrics sections
   - `wave/<name>/<name>.yaml` with flow, area, direction, stimulus
   - `wave/<name>/01-*.md` (etc.) roadmap items
   - `scratch/<branch>.md` design doc for the first item
2. `lf design` choosing "implement" still works exactly as before
3. `split-wave.md` exists as a valid interactive step
4. Verification: after `lf design` creates a wave, `load_local_wave_schemas` discovers the YAML — confirm by checking `lfq list` shows the new wave schema
