---
requires: diff vs main | scratch/ analysis | both
produces: wave/<wave>/ (updated or deleted), scratch/ (folded files removed)
---
Single owner of `wave/<wave>/`. Creates, updates, and deletes wave state.

## Goal

`wave/<wave>/` is planning scaffolding — it tracks what's left to build, not what's been built. This step is the only writer. Whether you're creating a wave from analysis, cleaning up after a build, or reconciling both at once:

- Shipped items are deleted, not marked as complete
- Context that upcoming items need is folded into those items before deletion
- New work from `scratch/` is folded into wave items or becomes new sprint files
- When nothing remains, the wave directory is deleted

## Bias: fold, don't drop

`scratch/` is cleared on land. Anything left there is lost. Anything folded into `wave/` survives. **Dropping content is a worse failure mode than duplicating it.**

Every scratch file with planned work or future-relevant analysis must be folded into wave:
- Proposals, open questions, analysis of upcoming work → fold into existing sprint items or create new ones
- Reviews with forward-looking recommendations → fold into relevant sprint items
- Questions about future work → fold into sprint items or wave README risks/strategy
- If content overlaps with what's already in wave, merge it — don't skip it

Design docs for already-shipped work and other purely historical content can be left for git history. The test: does this content inform future work? If yes, fold it. If it only describes what was already built, let it go.

## Workflow

1. Read the diff (if any) to understand what was actually built.
2. Read `wave/<wave>/` — README and item files — to understand current state.
3. Read `scratch/` — every file, completely.
4. Delete shipped items. Before deleting, fold context that remaining items need into those items.
5. Fold scratch content into `wave/<wave>/`. Merge into existing sprint items where there's a clear match. Create new sprint items for content that doesn't fit existing ones. Skip only purely historical content (shipped design docs with nothing forward-looking).
6. If destination files already exist, merge/dedupe — but keep both sides' content. When in doubt, include it.
7. Remove scratch files after their content has been folded into wave.
8. If `wave/<wave>/MEMORY.md` exists, fold useful observations into remaining items and trim.
9. If the wave directory has no remaining work items, delete the entire `wave/<wave>/` directory.

## Creating a new wave

When `scratch/` contains analysis or a proposal and no wave exists yet, create one:

1. Write `wave/<wave>/README.md` with vision, goals, and risks. The README is the anchor — when items change and plans shift, this stays. **No roadmap tables or phase lists** — the item files are the roadmap.
2. Write numbered item files (`01-name.md`, `02-name.md`, ...) for each phase or work item. **Create every item file**, even sketches (title + finish line + one paragraph) — `ingest` needs them to exist.

### Vision first

Before writing items, capture what you're building clearly enough that someone could explain it in a conversation:

- **What is this?** One paragraph. What it does, who it's for, why it exists.
- **Core components.** The pieces that make up the system. What each does, why it's separate.
- **Invariants.** Rules that always hold regardless of sequencing.
- **Differentiators.** What makes this different from the obvious approach. Why those decisions.

### Sequencing principles

- **Frontload the risk.** Start with the thing you need to try to see if it works. Don't pre-build infrastructure before you've proven the core idea.
- **Sequence by learning, not dependencies.** What are you most uncertain about? Build that first.
- **Defer abstractions.** Build the concrete thing, then extract the pattern.
- **Encode uncertainty.** Each item should state what you expect to learn and what might change.

## The files are the roadmap

The numbered item files in `wave/<wave>/` *are* the queue. Don't maintain a separate roadmap table, phase status section, or retrospective in `README.md` — it's redundant and drifts. A reader can see what's left by listing the directory. The README should contain vision, goals, and risks.

If the README has a roadmap table or phase status section, remove it.

## What counts as "shipped"

An item is shipped when the code is on main (or will be when this branch merges). Don't keep items around to admire — if the work is done, delete the file.

## Preserving context

Shipped items often contain history that upcoming items build on — decisions made, alternatives rejected, patterns established. That context belongs in the remaining wave items as free text, not as standalone shipped-item files.

If there are no remaining items, the context doesn't need a home. Git has the history.

## Output

Updated `wave/<wave>/` plus cleanup of `scratch/` files whose content has been folded in.

If the wave is fully shipped and scratch is empty: delete `wave/<wave>/`. Commit message: `wave: complete <wave>`.

**"No changes needed" is only valid when scratch/ is empty.** If scratch has files, something must move into wave.
