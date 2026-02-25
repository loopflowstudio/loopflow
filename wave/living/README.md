# Living

Software that reacts to its environment, itself, and its collaborative humans. Make waves self-sustaining — they remember, adapt, and rewrite themselves to fit.

## Vision

The unit of life is the wave, not the agent. Agents are ephemeral and stateless. Waves persist, remember, adapt. Context management is the memory system — curated by the wave, not accumulated by the agent.

A living wave:
- **Configures** every agent it spawns with the right tools and context
- **Remembers** what worked and what didn't across runs
- **Adapts** its presentation to the surface it's running on
- **Metabolizes** — consolidates, ages, rewrites its own memory

### Not here

- Agent memory / persistent agent identity (agents are stateless by design)
- Conversation history storage (observations, not transcripts)
- Structured memory databases (plain markdown, filesystem is the mechanism)

## Goals

- Every wave-spawned agent starts with the wave's accumulated knowledge
- Agents write durable observations back to wave memory without special tools
- Memory consolidates naturally through existing steps (update-wave, add-to-wave, review)
- The system prompt is the only injection point — no new APIs for memory
- Waves that split inherit relevant memory; waves that listen can share observations
- Same wave step works across all surfaces (headless, session, TUI, mobile)

## Risks

- **Memory quality.** Agents write varying-quality observations. Wrong-but-plausible observations persist until consolidation catches them. Mitigate: consolidation step reviews memory for accuracy against current code.
- **Memory bloat.** Without pressure, `MEMORY.md` grows until it crowds out other context. Mitigate: memory trims before summaries/docs in prompt assembly, and ops maintenance distills durable items into canonical docs.
- **Cross-wave leakage.** Shared memory between listening waves could propagate bad observations. Mitigate: memory is wave-private by default; sharing is explicit.
- **Orphaned skill files on crash.** If lfd crashes mid-session, injected `.claude/commands/` files linger. Harmless (overwritten on next injection) but messy. Tracked, not urgent.
- **Skill injection gap.** Global `~/.lf/steps/` and external skills (superpowers, rams) are not injected yet — only builtins and repo-local. Acceptable for now; revisit when external skill sources mature.

## Metrics

- Agent in run N can read observations written by agent in run N-1
- Memory stays under budget without manual intervention (consolidation keeps it trim)
- Cold start (new wave, no memory) is indistinguishable from current behavior
- ~~Skill injection works across all three launch paths (CLI, session, wave executor)~~ **Verified** — Phase 01 wired injection into CLI, wave executor, and sessions

## Phases

| # | Phase | What it unlocks | Status |
|---|-------|----------------|--------|
| 01 | Skill injection | Agents see loopflow steps/directions as native commands | shipped |
| 02 | Wave memory | Agents read/write durable observations scoped to their wave | next |
| 03 | Surface-adaptive prompts | Same step adapts to headless, session, TUI, mobile | |
| 04 | Memory lifecycle | Consolidation, aging, inheritance on split-wave | |

### Phase 01 retrospective

Shipped simpler than designed. The `collect_injectable_skills()` abstraction from the original design collapsed into `inject_skills()` which does collection and writing in one pass. Track-and-remove cleanup works. One gate fix was needed: sessions had to thread `repo_root` separately from `cwd` to discover repo-local steps when the working directory differs from the repo root.

Scope narrower than planned: only builtins and repo-local steps/directions are injected. Global `~/.lf/steps/` and external skill sources (superpowers, rams) are not yet included. This is fine — the pattern is established and extending it is straightforward.

## Architecture

```
wave/<name>/
  <name>.yaml          ← flow, area, direction, stimulus
  README.md            ← vision and phases
  NN-phase.md          ← planning docs
  MEMORY.md            ← durable wave observations (curated over time)
```

Memory enters prompt assembly through `DocumentSource::WaveMemory` and is surfaced in the `<lf:wave>` block. When budget is tight, memory trims before summaries/docs, creating pressure toward concise, high-value observations.

No new tools. Agents read memory because it's in the prompt. Agents write memory because the system prompt tells them to and they have file access. The filesystem is the mechanism; the prompt is the interface.
