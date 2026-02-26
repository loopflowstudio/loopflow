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
- ~~Same wave step works across all surfaces (headless, session, TUI, mobile)~~ **Verified** — Phase 03 replaced `run_mode` string matching with `Surface` enum; each surface gets tailored instructions via `Surface::instructions()`

## Risks

- **Memory quality.** Agents write varying-quality observations. Wrong-but-plausible observations persist until consolidation catches them. Mitigate: consolidation step reviews memory for accuracy against current code.
- **Memory bloat.** Without pressure, `MEMORY.md` grows until it crowds out other context. Mitigate: memory trims before summaries/docs in prompt assembly, and ops maintenance distills durable items into canonical docs.
- **Cross-wave leakage.** Shared memory between listening waves could propagate bad observations. Mitigate: memory is wave-private by default; sharing is explicit.
- **Orphaned skill files on crash.** If lfd crashes mid-session, injected `.claude/commands/` files linger. Harmless (overwritten on next injection) but messy. Tracked, not urgent.
- **Skill injection gap.** Global `~/.lf/steps/` and external skills (superpowers, rams) are not injected yet — only builtins and repo-local. Acceptable for now; revisit when external skill sources mature.
- **Prompt/storage model split.** Prompt assembly uses `Surface` enum; execution and storage still persist `run_mode` strings. Two mental models coexist. Acceptable while the prompt-side abstraction stabilizes, but should converge before adding surface-aware execution behavior.

## Metrics

- ~~Agent in run N can read observations written by agent in run N-1~~ **Verified** — Phase 02 wires `MEMORY.md` into prompt assembly via `DocumentSource::WaveMemory`
- Memory stays under budget without manual intervention (trimming drops memory before summaries/docs; ops steps distill into canonical docs)
- ~~Cold start (new wave, no memory) is indistinguishable from current behavior~~ **Verified** — absent `MEMORY.md` shows `(no memory yet)` marker, no errors
- ~~Skill injection works across all three launch paths (CLI, session, wave executor)~~ **Verified** — Phase 01 wired injection into CLI, wave executor, and sessions

## Phases

| # | Phase | What it unlocks | Status |
|---|-------|----------------|--------|
| 01 | Skill injection | Agents see loopflow steps/directions as native commands | shipped |
| 02 | Wave memory | Agents read/write durable observations scoped to their wave | shipped |
| 03 | Surface-adaptive prompts | Same step adapts to headless, CLI, desktop, mobile | shipped |
| 04 | Memory lifecycle | Aging, inheritance on split-wave, read-failure logging | |

### Phase 01 retrospective

Shipped simpler than designed. The `collect_injectable_skills()` abstraction from the original design collapsed into `inject_skills()` which does collection and writing in one pass. Track-and-remove cleanup works. One gate fix was needed: sessions had to thread `repo_root` separately from `cwd` to discover repo-local steps when the working directory differs from the repo root.

Scope narrower than planned: only builtins and repo-local steps/directions are injected. Global `~/.lf/steps/` and external skill sources (superpowers, rams) are not yet included. This is fine — the pattern is established and extending it is straightforward.

### Phase 02 retrospective

Shipped simpler than designed — again. The original design specified a `memory/` directory with topic files (`SUMMARY.md`, `codebase.md`, `patterns.md`, `preferences.md`). This collapsed into a single `wave/<wave>/MEMORY.md` file. A single file is easier to reason about, easier for agents to write, and creates natural size pressure — one file can't grow invisibly the way a directory of topic files can.

Memory distillation landed in `update-wave` and `add-to-wave` ops prompts rather than extending the `consolidate` step. This is lighter-weight: ops steps that already touch wave docs now also prune memory, rather than adding a separate memory-consolidation pass.

Trimming order (memory drops before summaries/docs) is the right default. Task context matters more than accumulated observations — an agent that forgets past patterns but can see the current diff will do better than one that remembers everything but can't see what it's working on.

Two known gaps remain:
1. **Persistence across execution surfaces.** Unverified whether any headless/session path could lose `MEMORY.md` edits due to detached workspaces. Open question tracked in `scratch/questions.md`.
2. **Read-failure visibility.** `MEMORY.md` read errors soft-fail silently. Acceptable for now but should get logging before memory becomes load-bearing.

### Phase 03 retrospective

Shipped simpler than designed — the pattern holds. The `Surface` enum (`Headless`, `Cli`, `ConcertoMac`, `ConcertoIphone`) with a single `instructions()` method replaced scattered `run_mode == "auto"` / `run_mode == "interactive"` string checks across prompt assembly. One match arm per surface, one source of truth for behavioral text.

Surface naming diverged from the original plan. The plan said "headless, session, TUI, mobile" but building revealed that "session" and "TUI" were the same concept (CLI terminal), and "mobile" needed to be specific to a platform. The actual surfaces — headless, CLI, Concerto macOS, Concerto iPhone — map to real rendering environments, not abstract interaction modes.

`#[serde(other)]` on `Headless` gives safe degradation: unknown surface strings from session configs parse without error and default to headless (autonomous, log-oriented). This is the right default — if we don't know the surface, behave conservatively.

One deliberate gap: prompt assembly now uses `surface`, but execution/storage still persists `run_mode`. This model split is acceptable for now — the two serve different purposes (behavioral instruction vs. execution metadata). Migrating storage is a separate decision, not a debt.

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
