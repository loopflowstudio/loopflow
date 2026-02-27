---
requires: diff vs main | scratch/ analysis | both
produces: wave/<wave>/ (updated or deleted), scratch/ (promoted files removed)
---
Single owner of `wave/<wave>/`. Creates, updates, and deletes wave state.

## Goal

`wave/<wave>/` is planning scaffolding — it tracks what's left to build, not what's been built. This step is the only writer. Whether you're creating a wave from analysis, cleaning up after a build, or reconciling both at once:

- Shipped items are deleted, not marked as complete
- Context that upcoming items need is folded into those items before deletion
- New work from `scratch/` is promoted as numbered item files
- When nothing remains, the wave directory is deleted

## Workflow

1. Read the diff (if any) to understand what was actually built.
2. Read `wave/<wave>/` — README and item files — to understand current state.
3. Read `scratch/` for analysis, proposals, or unfinished artifacts.
4. Delete shipped items. Before deleting, fold context that remaining items need into those items.
5. Promote actionable work from `scratch/` into `wave/<wave>/` as numbered item files.
6. If destination files already exist, merge/dedupe content intentionally.
7. Remove scratch files that were promoted.
8. If `wave/<wave>/MEMORY.md` exists, fold useful observations into remaining items and trim.
9. If the wave directory has no remaining work items, delete the entire `wave/<wave>/` directory.

## Creating a new wave

When `scratch/` contains analysis or a proposal and no wave exists yet, create one:

1. Write `wave/<wave>/README.md` — the anchor that survives when plans change.
2. Write numbered item files (`01-name.md`, `02-name.md`, ...) for each phase or work item.

### README.md

The README anchors the wave's identity. Concerto parses specific sections for the UI, so the structure matters.

**Required sections, in order:**

- **H1 + `## Vision`.** What this is, who it's for, why it exists. Scope boundaries go here as natural qualifiers — "Not transcription, not dictation."
- **`## Strategy`.** Why this approach and not the alternatives. Invariants, architecture, decisions, open questions. Sub-sections are free. Each wave reads differently.
- **`## Goals`.** What success looks like.
- **`## Risks`.** What could go wrong.
- **`## Metrics`.** How we know it works.

Additional free sections are welcome — data models, tech direction, guardrails. The wave's own voice lives here.

**README.md must not contain:**

- Roadmap tables or phase lists — denormalized; the item files are the roadmap
- Status indicators (shipped / in-progress / planned)
- Retrospectives — context for remaining items gets folded into those items

If the README has any of these, delete them.

### Sequencing principles

- **Frontload the risk.** Start with the thing you need to try to see if it works. Don't pre-build infrastructure before you've proven the core idea.
- **Sequence by learning, not dependencies.** What are you most uncertain about? Build that first.
- **Defer abstractions.** Build the concrete thing, then extract the pattern.
- **Encode uncertainty.** Each item should state what you expect to learn and what might change.

## What counts as "shipped"

An item is shipped when the code is on main (or will be when this branch merges). Don't keep items around to admire — if the work is done, delete the file.

## Preserving context

Shipped items often contain history that upcoming items build on — decisions made, alternatives rejected, patterns established. That context belongs in the remaining wave items as free text, not as standalone shipped-item files.

If there are no remaining items, the context doesn't need a home. Git has the history.

## Output

Updated `wave/<wave>/` (fewer files, not more) plus cleanup of promoted `scratch/` files.

If the wave is fully shipped: delete `wave/<wave>/`. Commit message: `wave: complete <wave>`.

If no wave changes are needed: `wave: reviewed, no changes needed`.
