# Review: Audit Breakdown

Split the merged "docs" line in the token audit header into separate rows for scratch, wave, and docs.

## What was implemented

- Added `DocumentSource::Scratch` variant so scratch/ docs are tagged distinctly from repo root docs
- Replaced the single `doc_count` field with per-source `source_counts: HashMap<DocumentSource, usize>` tracking
- Split the single "docs" output row into three: `scratch`, `wave` (includes summaries), `docs` (repo root only)
- Zero-token rows are now hidden instead of showing "0" — the header only shows what's actually loaded
- Renamed the "wave" metadata row to "scope" to avoid collision with the new "wave" token row
- Updated step prose (design, ingest, split-wave, update-wave) to use "sprint" terminology with finish lines

## Key choices

**Per-source HashMap over dedicated fields.** `source_counts` mirrors `source_tokens` — same access pattern, same drop logic. Avoids adding `scratch_count`, `wave_count`, `doc_count` as separate struct fields.

**Summary tokens folded into wave row.** Summaries are wave-derived context. Showing them as a separate row would add noise without aiding debugging. The wave count includes summary files.

**Drop order preserved via gather order.** Scratch docs gather first, then wave, then repo root. `pop()` removes from the end, so repo docs drop first, then wave, then scratch. No explicit sort needed — the gather order encodes priority.

**Zero-row hiding.** Previously, step/direction/system/diff always appeared even at 0 tokens. Now all rows hide at zero. Cleaner output, especially for steps that don't use every source.

## How it fits together

`gather_scratch_docs()` now tags with `DocumentSource::Scratch` instead of `DocumentSource::RepoDoc`. The `gather_context` match arm routes Scratch into the same `docs` vec as RepoDoc and Wave. In `trim_context_with_breakdown`, docs are counted per-source via `add_source_count`. Output renders three separate rows from the per-source token and count maps.

## Risks and bottlenecks

**Low risk.** The change is additive — a new enum variant, a new HashMap field, and display logic. The prompt XML structure is unchanged (scratch docs still render inside `<lf:docs>`). No behavioral change to what agents see.

**Session API serialization.** `source_key()` in `types.rs` maps `Scratch` → `"scratch"`. Clients consuming the context snapshot will see a new key. This is forward-compatible — unknown keys are ignored by existing clients.

## What's not included

- No `#[non_exhaustive]` added to `DocumentSource` (pre-existing gap, not part of this sprint's scope)
- No pluralization fix for "1 files" (pre-existing across all rows, not a regression)
- Parity tests don't exist at the expected path — no updates needed
