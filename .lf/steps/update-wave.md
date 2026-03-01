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

1. Read the diff (if any) to understand what was built on this branch.
2. Read `wave/<wave>/` — README and item files — to understand current state.
3. Verify each sprint against the actual codebase. Check `git log main`, read relevant files, run relevant commands. For each sprint: is the finish line crossed? If yes, treat it as shipped. For unshipped sprints: are file paths, function signatures, data structures, and technical approach still accurate given what's on main? Update sprint content to reflect the codebase as it actually is — not as it was when the sprint was written.
4. Read `scratch/` — every file.
5. Delete shipped items. Before deleting, fold context that remaining items need into those items.
6. Fold scratch content into `wave/<wave>/`. Merge into existing sprint items where there's a clear match. Create new sprint items for content that doesn't fit existing ones. Skip only purely historical content (shipped design docs with nothing forward-looking).
7. If destination files already exist, merge/dedupe — but keep both sides' content. When in doubt, include it.
8. Remove scratch files after their content has been folded into wave.
9. If `wave/<wave>/MEMORY.md` exists, fold useful observations into remaining items and trim.
10. If the wave directory has no remaining work items, delete the entire `wave/<wave>/` directory.

## Creating a new wave

When `scratch/` contains analysis or a proposal and no wave exists yet, create one:

1. Write `wave/<wave>/README.md` — the anchor that survives when plans change. Vision, strategy, goals, risks, metrics. **No roadmap tables or phase lists** — the sprint files are the roadmap.
2. Write numbered sprint files (`01-name.md`, `02-name.md`, ...) — the roadmap. **Create every sprint file**, even sketches (title + finish line + one paragraph) — `ingest` needs them to exist.

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
- Retrospectives — context for remaining sprints gets folded into those sprints

If the README has any of these, delete them.

### Sprints

Roadmaps are made up of sprints. Each sprint is a numbered file (`01-*.md`, `02-*.md`, ...) with a clear finish line — a concrete deliverable you're racing to reach. Not a phase, not a layer, not a bucket of tasks.

**Every sprint must open with a bold finish line.** What's true when this sprint is done that isn't true now? Make it specific enough that you know when you've crossed it.

```markdown
# 01: Audit Breakdown

**Finish line:** `lf implement` shows separate token rows for scratch, wave, and docs.
```

### Sequencing principles

- **Frontload the risk.** Start with the thing you need to try to see if it works. Don't pre-build infrastructure before you've proven the core idea.
- **Sequence by learning, not dependencies.** What are you most uncertain about? Build that first.
- **Defer abstractions.** Build the concrete thing, then extract the pattern.
- **Encode uncertainty.** Each sprint should state what you expect to learn and what might change.

## What counts as "shipped"

A sprint is shipped when the code is on main (or will be when this branch merges) and its finish line has been crossed. Don't keep sprints around to admire — if the work is done, delete the file.

## Preserving context

Shipped sprints often contain history that upcoming sprints build on — decisions made, alternatives rejected, patterns established. That context belongs in the remaining sprint files as free text, not as standalone shipped files.

If there are no remaining sprints, the context doesn't need a home. Git has the history.

## Output

Updated `wave/<wave>/` plus cleanup of `scratch/` files whose content has been folded in.

If the wave is fully shipped and scratch is empty: delete `wave/<wave>/`. Commit message: `wave: complete <wave>`.

**"No changes needed" is only valid when scratch/ is empty.** If scratch has files, something must move into wave.
