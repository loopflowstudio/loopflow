---
requires: diff vs main | scratch/ analysis | both
produces: wave/<wave>/ (updated or deleted), scratch/ (folded files removed)
---
Single owner of `wave/<wave>/`. Creates, updates, and deletes wave state.

## Goal

`wave/<wave>/` is planning scaffolding — it tracks what's left to build, not what's been built. This step is the only writer. Whether you're creating a wave from analysis, cleaning up after a build, or reconciling both at once:

- Shipped items are deleted, not marked as complete
- Context that upcoming items need is folded into those items before deletion
- New work from `scratch/` is folded into wave items or becomes new item files
- When nothing remains and the wave is standalone, the wave directory is deleted
- When nothing remains and the wave is a chord member, the wave persists — see "Silence" below

## Bias: fold, don't drop

`scratch/` is cleared on land. Anything left there is lost. Anything folded into `wave/` survives. **Dropping content is a worse failure mode than duplicating it.**

Every scratch file with planned work or future-relevant analysis must be folded into wave:
- Proposals, open questions, analysis of upcoming work → fold into existing items or create new ones
- Reviews with forward-looking recommendations → fold into relevant items
- Questions about future work → fold into items or wave README risks/strategy
- If content overlaps with what's already in wave, merge it — don't skip it

Design docs for already-shipped work and other purely historical content can be left for git history. The test: does this content inform future work? If yes, fold it. If it only describes what was already built, let it go.

## Workflow

1. Read the diff (if any) to understand what was built on this branch.
2. Read `wave/<wave>/` — README and item files — to understand current state.
3. Verify each item against the actual codebase. Check `git log main`, read relevant files, run relevant commands. For each item, ask three questions:
   - **Shipped?** Is the finish line crossed? If yes, treat it as shipped.
   - **Accurate?** Are file paths, function signatures, data structures, and technical approach still accurate given what's on main? Update item content to reflect the codebase as it actually is — not as it was when the item was written.
   - **Coherent?** Is this item still worth building? The codebase evolves — other waves ship code, the user's understanding deepens, the problem shifts. An item can become stale without being shipped: the 80% case got solved a different way, the design assumed a structure that no longer exists, or the remaining value is marginal. See "Coherence" below.
4. Read `scratch/` — every file, completely.
5. Delete shipped items. Before deleting, fold context that remaining items need into those items.
6. Fold scratch content into `wave/<wave>/`. Merge into existing items where there's a clear match. Create new items for content that doesn't fit existing ones. Skip only purely historical content (shipped design docs with nothing forward-looking).
7. If destination files already exist, merge/dedupe — but keep both sides' content. When in doubt, include it.
8. **Trim scratch docs for shipped work.** Don't delete them — `lf ops land` handles that. But strip implementation details that are now in the code. Keep only:
   - **Validation procedures** — "Done when" checks, commands to run, expected output
   - **Measurement instructions** — benchmarks, before/after comparisons, how to reproduce results
   - **Try-it recipes** — quick ways for a reviewer to exercise the change
   If a scratch doc has none of these, delete it. The goal: a reviewer landing on this branch can find how to evaluate the work without reconstructing it from the diff.
9. If `wave/<wave>/MEMORY.md` exists, fold useful observations into remaining items and trim.
10. If the wave directory has no remaining work items:
    - **Standalone wave** (not referenced by any chord-wave's area): delete the entire `wave/<wave>/` directory.
    - **Chord member** (referenced by another wave's area): keep the directory. The README survives as the wave's identity and sensor. The wave is now **silent** — alive, watching its area, but not proposing work. See "Silence" below.

## Creating a new wave

When `scratch/` contains analysis or a proposal and no wave exists yet, create one:

1. Write `wave/<wave>/README.md` — the anchor that survives when plans change. Vision, strategy, goals, risks, metrics. **No roadmap tables or phase lists** — the item files are the roadmap.
2. Write priority item files (`1-fix-broken-build.md`, `2-next-step.md`, `3-big-rock.md`, `4-speculative-bet.md`) — the roadmap. **Create every item file**, even sketches (title + finish line + one paragraph) — `ingest` needs them to exist.

### README.md

The README anchors the wave's identity. Concerto parses specific sections for the UI, so the structure matters.

**Required sections, in order:**

- **H1 + `## Vision`.** What this is, who it's for, why it exists. Scope boundaries go here as natural qualifiers — "Not transcription, not dictation."
- **`## Strategy`.** Why this approach and not the alternatives. Invariants, architecture, decisions, open questions. Sub-sections are free. Each wave reads differently.
- **`## Goals`.** What success looks like.
- **`## Risks`.** What could go wrong.
- **`## Metrics`.** Numeric measurements — percentages, counts, durations, rates. Not qualitative indicators or behavioral descriptions. If you can't put a number on it, it's a goal, not a metric.

Additional free sections are welcome — data models, tech direction, guardrails. The wave's own voice lives here.

**README.md must not contain:**

- Status indicators (shipped / in-progress / planned)
- Retrospectives — context for remaining items gets folded into those items

If the README has any of these, delete them.

### Items

Roadmaps are made up of items. Each item is a priority-prefixed file (`1-*.md`, `2-*.md`, `3-*.md`, `4-*.md`) with a clear finish line — a concrete deliverable you're racing to reach. Not a phase, not a layer, not a bucket of tasks.

**The filename prefix carries priority meaning:**

- `1-*` — Urgent: the codebase is broken or blocked; fix this before forward progress
- `2-*` — High: the clear next step
- `3-*` — Medium: a committed later bet; "when, not if"
- `4-*` — Low: speculative work

**Every item must open with a bold finish line.** What's true when this item is done that isn't true now? Make it specific enough that you know when you've crossed it.

```markdown
# Audit Breakdown

**Finish line:** `lf implement` shows separate token rows for scratch, wave, and docs.
```

### Bucket principles

- **Use the smallest bucket that is honest.** Don't inflate work into `p0` just to force it to the front.
- **Frontload the risk.** The most uncertain concrete deliverable usually belongs in `p1`, not buried behind fake staging.
- **Keep buckets semantic, not numeric theater.** Don't recreate `01/02/03` inside a bucket. Within-bucket ordering is intentionally loose.
- **Encode uncertainty.** Each item should state what you expect to learn and what might change.

## Coherence

Items go stale. The codebase moves, other waves ship code that changes the
landscape, the user's understanding evolves. When update-wave detects incoherent
items, it reorganizes them — this is internal housekeeping, not new work.

An item is incoherent when:
- **The finish line moved.** The goal was achieved by a different path. The item
  describes work that's no longer needed as specified.
- **The design diverged.** The codebase evolved in a direction incompatible with
  the item's approach. Building it as written would fight the current architecture.
- **The value diminished.** The 80% case is solved. What remains is marginal
  improvement that doesn't justify the cost.
- **Items overlap.** Multiple items now describe aspects of the same work, or
  items in different waves cover the same ground.

When incoherence is found:
1. Delete items that are fully obsolete (the work happened differently).
2. Rewrite items whose goal is still valid but whose approach is stale.
   Update the finish line, the technical approach, and the rationale to
   reflect the codebase as it actually is.
3. Merge items that have converged into the same work.
4. If remaining items no longer form a compelling roadmap, rewrite the
   set with a coherent forward-looking vision. What's the most valuable
   thing this wave could do *now*, given everything that's changed?

This reorganization is a single beat — it doesn't require human review. It's
the wave maintaining its own coherence, the way a musician adjusts tuning
between movements. The result should be a wave whose items, if any survive,
describe genuinely compelling work against the current state of the world.

If no items survive coherence review and the wave is a chord member, it
becomes silent. That's fine — see below.

## Silence

A silent wave isn't necessarily empty. It may have no items, or it may have
had items that didn't survive coherence review. Either way, it's a chord member
that owns an area of the problem space — watching, sensing, but not building.

A silent wave:
- Keeps its README (vision, strategy, goals, risks, metrics)
- Has no roadmap item files
- Is a valid, healthy state — not a failure or stall
- Signals to the human: "this area is covered, add items here if you want work done"
- Signals to the chord: "nothing compelling to build right now"

Silence is the most important note a wave can play. Shipping mediocre work to
avoid being empty trains the user to ignore the wave. A wave that stays quiet
until it has something genuinely compelling earns trust that compounds.

**When to stay silent vs close out:**
- If the wave is a chord member → stay silent (default)
- If the wave is standalone with no remaining purpose → delete
- If the wave's area is still active and evolving → stay silent
- If the human explicitly closes the wave → delete

The chord's tend flow (specifically assess and play-chord) can propose waking
a silent wave by adding items, or closing it entirely. The human reviews these
proposals in review-chord.

## What counts as "shipped"

An item is shipped when the code is on main (or will be when this branch merges) and its finish line has been crossed. Don't keep items around to admire — if the work is done, delete the file.

## Preserving context

Shipped items often contain history that upcoming items build on — decisions made, alternatives rejected, patterns established. That context belongs in the remaining item files as free text, not as standalone shipped files.

If there are no remaining items, the context doesn't need a home. Git has the history.

## Output

Updated `wave/<wave>/` plus cleanup of `scratch/` files whose content has been folded in.

If the wave is fully shipped and scratch is empty: delete `wave/<wave>/`. Commit message: `wave: complete <wave>`.

**"No changes needed" is only valid when scratch/ is empty.** If scratch has files, something must move into wave.
