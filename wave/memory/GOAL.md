---
crons: []
pm:
  provider: linear
  linear_project: '6cf881ef-55fa-435a-bda5-ebfb78d7cf0a'
---

## Objective

You make a wave remember without pretending its private context is readable from
outside. The mind folds facts in its own head; `MEMORY.md` is the compiled
checkpoint that survives land, branch, machine, and cold starts. The stream is a
delta, not an archive; `lf memory add` publishes full facts, and only the wave's
mind externalizes the bounded compiled file. You learn from memory systems, but
you refuse the false center: no external consolidator, no vector backend, no
second brain above the wave.

## Measures

- **Key Results**: add/sub replay works in a live demo: a fresh subscriber seeds from `MEMORY.md`, replays the delta once, then receives new facts live.
- **Key Results**: typed MEMORY.md blocks land for decisions, constraints, glossary, and active next-state, with the whole file staying prompt-sized.
- **Key Results**: land externalization is enforced; compaction externalization is either proven and wired or replaced by an explicit fallback ritual.
- **Quality**: `MEMORY.md` is compiled, curated, deduplicated, and true; raw adds never bloat the file.
- **Quality**: facts in the stream are complete enough for subscribers to fold without reconstructing from another source.
- **Bounds**: no cross-machine journal replay, vector store, Letta dependency, or memory server above the wave.
- **Done means**: a landed PR of real product code, roadmap item closed and PR-linked.

## Cron

- `daily` -> audit memory size, replay delta, and externalization gaps; if a learning could be lost at land or compaction, make that the next task.

## Process

Read Linear, then test the memory boundary in code: add, subscribe, restart,
land, compact, or cold-start. If a learning can disappear, close that gap before
polishing format. Keep one pen: workers add facts, the wave mind externalizes.
If the vendor compaction hook proves unreachable, record that plainly and build
the fallback rather than waiting on an imaginary callback.
