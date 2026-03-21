# 04: Actionable Roadmap

## Problem

The roadmap pane shows items but you can't do anything with them. Priority isn't editable, content is collapsed so you can't tell what things are, and there's no way to launch work on an item. You look at the roadmap, then leave Concerto to actually start working.

The workspace should be the place where you see what's next, reprioritize, and kick off a run — all without leaving.

## Approach

Three changes to the roadmap pane, plus one new operation in Rust.

### 1. Targeted ingest (`lf ops ingest --item <filename>`)

Current `lf ops ingest` auto-picks the highest-priority item. The play button needs to ingest a *specific* item.

**Rust change:** Add `item: Option<String>` to `IngestOptions`. When set, copy that file (by filename, e.g. `03-calibration-view.md`) instead of auto-picking. Validation: file must exist in the wave directory and parse as a valid wave item. The rest of the ingest path (copy to `scratch/<wave>-<slug>.md`, delete original) stays the same.

**CLI surface:** `lf ops ingest --item 03-calibration-view.md` (or `--item calibration-view` matching by slug).

**lfd API:** The existing run-wave endpoint already accepts a flow parameter. Concerto calls targeted ingest as a pre-step, then runs the build flow. Alternatively, a new `ingest` endpoint that takes wave ID + item filename, returns the scratch path.

### 2. Play button on roadmap cards

Each non-shipped roadmap card gets a play button. Tap it to:

1. Call `lf ops ingest --item <filename>` (through lfd)
2. Run the build flow for the wave

The button is disabled when the wave is already running. Shows a spinner during ingest. If ingest fails (item gone, wave dir missing), show the error inline on the card.

**Implementation:** Add `ingestAndBuild(item:)` to `RoadmapPaneView`. This calls a new method on `RepoState` (or `LocalWaveService`) that chains ingest + run. The roadmap card's play button calls this.

### 3. Priority picker

Each card shows a priority picker — segmented control or dropdown with Urgent / High / Medium / Low. Current priority derived from filename prefix (`1-` through `4-`).

Changing priority renames the file on disk:
- `03-calibration-view.md` → `2-calibration-view.md` (High → uses bucket prefix, not legacy `0N-`)
- Uses `FileManager.moveItem` in the wave directory

After rename, re-parse wave content to refresh the list. Items reorder by new priority.

**Edge case:** Two items could collide if they have the same slug in different buckets. Unlikely (slugs are unique per wave), but validate before rename.

### 4. Inline summary

Show the first 2-3 lines of item content by default, without requiring expand. The `WaveContentParser.extractContent` already grabs up to 20 lines — just display the first few unconditionally.

- First 3 lines of content shown in secondary text below the title
- "Show more" expands to the full 20-line preview (same as current expand behavior)
- Shipped items still show no content (already the case)

## Key decisions

**Targeted ingest via Rust, not Swift file copy.** Keeps ingest logic in one place. The slug derivation, scratch path conventions, and file deletion all stay in `ops/ingest.rs`. Concerto calls through lfd rather than reimplementing.

**Priority as rename, not metadata.** The filename prefix *is* the priority. No new metadata format, no sidecar files. Git sees a rename, which is fine — wave items are meant to be reshuffled.

**Bucket prefixes for priority changes.** When you change priority, the new filename uses bucket-style prefix (`1-`, `2-`, `3-`, `4-`), not legacy two-digit prefix (`01-`, `02-`). This matches the direction of the codebase.

**Deferred: multiplexer interaction improvements.** Pane type picker, markdown file browser, unified diff viewer, directional focus, and layout presets are real improvements but they're about workspace interaction, not workspace content. They move to a follow-up.

## Scope

- **In scope:** Targeted ingest (`--item` flag), play button on roadmap cards, priority picker, inline summary, tests for targeted ingest
- **Out of scope:** Pane type picker, markdown browser, diff viewer, directional focus, layout presets (follow-up), drag-to-reorder (four discrete levels don't need drag)

## Done when

- `lf ops ingest --item <filename>` copies a specific item to scratch/
- Roadmap card play button ingests and launches the build flow
- Priority picker renames files and reorders the list
- First 2-3 lines of content visible without expanding
- `cargo test -p loopflow` passes with targeted ingest tests
- `swift test --package-path swift` passes
- `xcodebuild test` passes for Concerto UI
