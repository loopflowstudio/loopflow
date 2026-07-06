---
primary_flow: ship-roadmap
mode: manual
workers: 0
metrics:
- No learning is lost between sessions — a new agent seeds from MEMORY.md and replays the stream since; nothing an earlier agent knew silently disappears
- MEMORY.md is compiled, never accreted — only externalization writes it; a raw `add` never bloats the file
- Memory survives the two loss moments — context compaction and land both force externalization; a long session and a shipped branch both keep what they learned
- The stream carries full facts, not summaries — a subscriber can fold exactly what it receives, with no round-trip to reconstruct meaning
- MEMORY.md stays context-sized and legible — typed blocks under budget, bounded not archived; the whole compiled memory always fits in a prompt
- The fold is the mind's job and it's done well — externalized blocks stay curated, deduplicated, and true; no external consolidator, no vector store, no Letta dependency
pm:
  provider: linear
  linear_project: 6cf881ef-55fa-435a-bda5-ebfb78d7cf0a
---

Run one loop iteration for the Memory wave.

You own how a wave *remembers*. Not the machinery around the code (Systems) or
the shape of the code (Architecture) — the model by which agents accumulate,
consolidate, and carry forward what they learn. The premise: an agent's working
memory lives in its own context and can't be read from outside; the only
externalized, inspectable, branch-carried form is `MEMORY.md`. Everything this
wave builds serves that asymmetry.

The model in one breath: `lf memory add` publishes an immutable fact to an
append stream; running agents subscribe (`lf sub`) and fold each fact into their
own heads; `MEMORY.md` is a *checkpoint of a mind's compiled state*, written only
when a mind externalizes via `lf memory update`. The stream replays within a
server's life (journal snapshot+tail); `MEMORY.md` is the only thing that crosses
land, branch, and machine boundaries. Externalization is forced at the two moments
an in-head fold would otherwise vanish: context compaction and land. Memory is
structured as typed blocks (decisions / constraints / roster / glossary), bounded
to context size — no archive, no retrieval. Learn from Letta; depend on nothing.

Read the roadmap, judge the state of memory against the metrics, and pick the next
useful move: close a durability gap where a learning can still be lost, tighten the
stream so subscribers fold full facts, give `MEMORY.md` its block structure, or wire
a forced-externalization moment into the land or compaction ritual. Dispatch the
appropriate flow against it. The proof is always a demo: two panes, an `add`, a
subscriber that receives it, a boundary crossed with memory intact.

The hardest unknown is the compaction hook — whether loopflow can act before the
vendor CLI compacts its own context. Treat it as research, not a given: if the hook
proves unreachable, lean on land-externalization plus mind-initiated updates rather
than blocking on it.

If no safe move remains, record the blocker instead of inventing work.
