# 04: Prompt Pipeline

## Problem

`engine/prompt.rs` still treats document origin as free-form strings and gathers context through scattered boolean gates. That makes prompt assembly brittle (typos become behavior), hard to extend (new source requires touching many branches), and risky to refactor in a 2400+ line file.

Who benefits:
- Contributors touching prompt assembly get compile-time guarantees instead of string matching.
- Reviewers get a single gather pipeline and a single formatting entry point.
- Users benefit from safer prompt evolution without parity regressions.

Why now: stage 04 is the remaining simplification item in the Rust wave, and it is intentionally independent of store work.

## Approach

Replace ad-hoc prompt assembly with a typed, declarative pipeline in one focused refactor:

1. **Type document origin**
   - Introduce `DocumentSource` enum:
     `Step | Direction | Wave | Area | Diff | Clipboard | RepoDoc | Summary`.
   - Change `Document.category: String` to `Document.source: DocumentSource`.
   - Derive `Debug, Clone, PartialEq, Eq, Hash` for map/set use.

2. **Make gathering spec-driven**
   - Replace `GatherContextOpts` booleans with `GatherSpec { sources, repo_root, area, wave, ... }`.
   - Implement `gather_documents(spec: &GatherSpec) -> Vec<Document>` as ordered dispatch over explicit sources.
   - Keep source-specific logic in focused helper functions (`gather_wave_docs`, `gather_area_docs`, etc.).
   - Preserve output ordering exactly to keep parity stable.

3. **Unify formatting path**
   - Add `PromptFormatMode` enum (context/task/full).
   - Consolidate existing formatters into `format_prompt(mode, ...)`.
   - Remove duplicate branching across old formatter functions.

4. **Move breakdown accounting to enum keys**
   - Update `ContextBreakdown` to key by `DocumentSource`.
   - Replace string literal category checks with enum matches.

5. **Delete replaced paths in the same PR**
   - Remove old string category literals and boolean-gated gathering branches.
   - No compatibility shim layer.

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Keep `String` categories but centralize constants | Lower migration cost | Still stringly-typed; typos remain runtime bugs |
| Add enum but keep old gather booleans | Safer incremental rollout | Leaves duplicated control plane and complexity in place |
| Split `prompt.rs` first, then refactor behavior | Better file ergonomics | Higher parity risk from structural + behavioral change together |

## Key decisions

- **Decision: one decisive refactor, not staged compatibility.**
  - Follows wave principle: **"Each stage deletes what it replaces — no deferred cleanup."**
  - We will remove legacy paths immediately after migrating callsites.

- **Decision: keep stage boundary strict.**
  - Follows wave principle: **"Stage 04 is independent — it touches `engine/prompt.rs`, not the store layer."**
  - No store/executor changes in this work.

- **Decision: parity is a hard gate.**
  - Follows wave north star: **"Prompt assembly uses typed document sources."**
  - Any refactor that changes output ordering/format is rejected.

- **Wild success target:** adding a new source becomes one enum variant + one gatherer + one formatter match arm, with no string hunting.
- **Wild failure to avoid:** enum exists but old booleans/literals survive, creating two competing pipelines.

## Scope

- In scope:
  - `DocumentSource` and `Document` field migration
  - `GatherSpec` + `gather_documents(spec)` dispatch
  - Consolidated formatter entry point with mode enum
  - `ContextBreakdown` migration from string keys to enum keys
  - Prompt parity test updates/fixes required by refactor

- Out of scope:
  - `lfd/executor.rs` decomposition
  - Store/backends and SQL catalog work
  - Product behavior changes to waves/flows/scheduler
  - Non-parity prompt content redesign

## Done when

- `Document` no longer has string category fields.
- Prompt assembly contains no source string literals (`"wave"`, `"docs"`, `"area"`, etc.) for behavior routing.
- `gather_documents(&GatherSpec)` is the single gathering entry point.
- Formatting runs through one mode-based entry point.
- Verification passes:

```bash
cargo test -p loopflow golden_prompt
uv run pytest tests/parity/test_prompt_parity.py
cargo test -p loopflow prompt
```
