# Living Waves

A wave that learns from its own runs and rewrites itself to fit.

---

## What a wave is today

A wave is a named scope: flow + area + direction + stimulus. It spawns agents, each agent runs a step, the step produces artifacts (code, docs, PRs). Between runs, the wave persists as config and planning docs in `wave/<name>/`. But every agent starts fresh — no awareness of prior runs, no accumulation.

The summary mechanism (`ensure_summary_fresh`) is the closest thing to memory: it hashes the area files, runs a summarize agent when stale, and injects the result into future prompts. But summaries describe the *codebase*, not the *wave's experience*.

## What a living wave adds

A living wave remembers what it has learned. Not conversation logs — observations. Patterns the codebase follows. Preferences the user has expressed. Things that failed. Things that worked.

Three properties:

**Recall.** Every agent in the wave starts with accumulated observations from prior runs. The wave curates what each agent sees — not raw history, but distilled knowledge.

**Observation.** Agents write back to wave memory when they learn something durable. "This test suite needs `--no-sandbox` on CI." "The user prefers commits under 200 lines." "The auth module uses refresh tokens, not session cookies."

**Metabolism.** Memory doesn't just grow. It consolidates, ages, and gets rewritten. Stale observations drop. Redundant ones merge. The wave maintains its own clarity.

## Memory structure

```
wave/<name>/
  <name>.yaml        ← config (exists today)
  README.md          ← vision (exists today)
  01-phase.md        ← planning (exists today)
  memory/
    SUMMARY.md       ← one-page overview, regenerated periodically
    codebase.md      ← what the code looks like, how it's organized
    patterns.md      ← what works, what doesn't, conventions learned
    preferences.md   ← user preferences, project norms observed
```

Memory files are plain markdown. Agents read and write them directly — no special tools, no structured format. The system prompt tells agents where memory lives and when to write.

`SUMMARY.md` is special: it's the only file that always fits in context. When the full memory directory is too large for the token budget, `SUMMARY.md` is included and the rest is dropped. Think of it as the wave's working memory vs long-term storage.

## How memory flows through context

Memory enters through `gather_context()` as another document source, alongside step content, directions, diff, area docs, and repo docs.

Priority when budget is tight:

1. Step (the task — what to do)
2. Direction (the perspective — how to think)
3. Diff (what's changed on this branch)
4. Wave memory (what the wave knows)
5. Summary (area summary — codebase overview)
6. Area docs (architecture context)
7. Repo docs (reference)

When memory exceeds its budget allocation, `SUMMARY.md` survives and the topic files are trimmed or dropped. This creates natural pressure: if memory is too large, agents see the summary but not the details. The `consolidate` step can respond to this by compressing topic files.

## How agents interact with memory

### Reading

Automatic. The wave executor includes memory files in the prompt. Agents don't request memory — it's already there. Same as how directions and area docs work today.

### Writing

Prompt-driven. The system prompt tells the agent:

```
This wave has persistent memory at wave/{name}/memory/.
Read existing memory files at the start of your work.
Write observations that should persist for future agents:
- Codebase patterns and conventions you discover
- User preferences you observe
- Things that worked or failed
- Corrections to existing memory

Don't write session-specific notes. Only durable observations.
Update existing files rather than creating new ones when possible.
```

No special tool. The agent has file access. Writing to `wave/<name>/memory/patterns.md` is the same as writing to any other file. The instruction shapes behavior; the filesystem is the mechanism.

### Why no tool?

A memory tool would be an abstraction over `write_file`. It would need a schema, a key-value model, maybe tags. That's building a database. The filesystem already works: agents can read, write, create, organize. The system prompt provides the structure. If a future agent reorganizes memory files, that's fine — it's self-meta-programming.

## Metabolism: memory lifecycle

### Growth

First agent to observe something writes it. Second agent adds. Files grow naturally.

### Consolidation

The existing `consolidate` step (already part of the `ship` flow) is the right place. After implementation work, consolidation merges duplicates, removes stale observations, and updates `SUMMARY.md`. This isn't a special mechanism — it's just a step that reads memory and rewrites it.

Consolidation prompt addition:

```
Review wave/{name}/memory/ for:
- Duplicate or redundant observations (merge them)
- Stale information contradicted by current code (remove it)
- SUMMARY.md accuracy (regenerate if needed)
- Overall size (compress if memory exceeds ~2000 tokens)
```

### Aging

Observations that haven't been referenced or updated in N runs could be flagged for review. Not automatic deletion — agents during consolidation decide what's still relevant.

### Inheritance

When `split-wave` creates children, memory can optionally copy from parent. Not automatic — the split step decides what's relevant to each child wave.

## What memory is NOT

**Not conversation history.** Waves don't store transcripts. Each run produces observations, not logs.

**Not agent identity.** Agents are stateless. The wave provides context; the agent doesn't "remember" its prior self.

**Not a database.** No schemas, no queries, no structured records. Plain markdown files that agents read and write.

**Not append-only.** Memory gets rewritten, consolidated, corrected. An observation from run 3 might be wrong by run 7. The agent that discovers this corrects the file.

## Relationship to existing mechanisms

| Mechanism | Scope | Lifecycle | Content |
|-----------|-------|-----------|---------|
| **Summary** (exists) | Wave × area | Regenerated when area files change | Codebase structure overview |
| **Wave docs** (exists) | Wave | Persists in git | Vision, phases, planning |
| **Wave memory** (new) | Wave | Grows/consolidates across runs | Learned observations |
| **scratch/** | Branch | Deleted on merge | Design docs, review notes |

Summaries describe what the code *is*. Memory describes what the wave has *learned*. Wave docs describe what the wave *intends*. These are complementary, not competing.

## What changes

`DocumentSource` gets a new variant: `WaveMemory`. The gathering pipeline reads `wave/<name>/memory/*.md` when assembling context for a wave-scoped run. The trimming pipeline allocates budget between memory and other sources.

The system prompt for wave-scoped agents gets a memory section: where memory lives, when to write, what kinds of observations belong.

The `consolidate` step gets memory awareness: review, compress, regenerate `SUMMARY.md`.

`split-wave` gets an option for memory inheritance.

## What doesn't change

No new tools. No new APIs. No structured memory format. No agent identity. No conversation persistence.

The wave executor, prompt assembly, and context trimming already handle multiple document sources with budget allocation. Memory is one more source.

## Open questions

- **Memory vs summary overlap.** The summary mechanism already generates codebase descriptions. Should wave memory replace it, extend it, or stay parallel? Leaning: parallel — summaries are automated and area-scoped; memory is agent-written and wave-scoped.

- **Cross-wave memory.** Should waves that `listen` to each other share memory? Or is memory always wave-private? A chord might benefit from shared observations across its constituent waves.

- **Memory in CLI.** When running `lf implement -w engbot`, should the CLI agent see engbot's memory? Leaning yes — if you're working in a wave's context, you should see what it knows.

- **Memory quality.** Agents will write varying-quality observations. Consolidation handles staleness, but what about wrong observations that seem plausible? This might need a human-in-the-loop review step for critical waves.

- **Bootstrap.** A new wave has no memory. Should there be a `seed-memory` step that reads the codebase and bootstraps initial observations? Or is the summary mechanism already sufficient for cold starts?
