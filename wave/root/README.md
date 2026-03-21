# Redesign

Orchestrating agents, not being one.

The agent layer is commoditizing. Claude Code, Codex, and OpenCode are all good enough — and getting better faster than we can compete. Loopflow's future is the layer above.

Three things differentiate loopflow:

1. **The wave/flow data model** — composable area, direction, flow, triggers
2. **Chord-waves** — waves whose area is other waves, gardening the system and accumulating judgment
3. **Music as organizing metaphor** — not a skin, but a way of thinking about creative technical work

Everything below the orchestration layer is someone else's problem. Everything above it is ours.

## How this project works

Build and garden. Two voices in counterpoint.

A wave builds — code, tests, PRs. A chord-wave gardens — `scan-waves`, `assess`, then routes to tune or silence. Build rounds route to play or silence. Same flow engine, same data model. The difference is area: files vs `wave/`.

This wave is the first chord-wave. Its first job is building itself. Its second job is gardening the waves that build everything else. The recursive case — a chord-wave that gardens its own construction — is the real test.

Both member waves use existing `build` / `build-or-silent` flows until the chord-model wave produces `garden`. Then this wave starts using what it built.

During bootstrap, these waves register in `manual` mode. Wiring the structure together should not immediately start build/garden loops.

```
wave: redesign (chord-wave)
│
│  area: wave/chord-model/,
│        wave/agent-embedding/
│  flow: garden
│
├── wave: chord-model           13 items — runtime config, governance, Letta, mutations, APIs
└── wave: agent-embedding       5 items — Concerto as conductor
```

## Phasing

### Phase 1: Bootstrap

Build enough to run the first garden cycle. Uses existing flows.

- **chord-model/02** — live garden-cycle validation (boot lfd, register redesign, run garden for real)
- **chord-model/03** — Letta integration once live garden output exists to remember

Current sequencing matters:
- `chord-model/02` owns the operational gap between "the garden machinery parses" and "the redesign chord actually runs in lfd"
- stall detection, algedonic polish, and memory-backed pattern work stay inside `chord-model` after the live proof exists
- `agent-embedding/01` can keep tightening queue and workspace UX inside `swift/`, but backend/auth/pm work that escapes `swift/` should move into the owning wave instead of broadening its scope silently

First garden cycle runs. The chord-wave observes its own bootstrap commits. Letta records its first memories. The recursive loop is live.

### Phase 2: Build and Garden in Counterpoint

Waves produce real PRs. The chord-wave gardens them. Build and garden alternate — waves create, chord-wave observes, chord-wave proposes, waves adjust.

**agent-embedding** now turns the old detail panel into a workspace. The remaining roadmap starts with interactive checkpoints in the queue (01), closes the last lifecycle gaps (02), then expands into portfolio (03) and calibration (04).

**chord-model** continues with Letta (04), area model (05), wave mutation (06), and later API expansion (08). Stall detection, self-healing polish, and signal memory now live here instead of in a separate `signals` lane.

### Phase 3: The Chord-Wave Earns Its Keep

Enough garden cycles to answer the open questions with evidence.

- chord-model/07 — DAG enforcement and default chord-wave
- chord-model/04 — memory starts carrying repeated algedonic and calibration patterns
- agent-embedding/04 — calibration view (trajectory review UX)
- agent-embedding/05 — window-composition polish once the workspace usage patterns are proven

The default chord-wave absorbs the existing five waves (foundation, trust, context, concerto, scale) and restructures them through garden cycles. Not manual reshuffling — the chord-wave proposes, the human reviews.

## What stays

- **Wave/Flow/Direction/Area data model** — the core
- **`lf` CLI** — step/flow execution, worktree ops, prompt assembly
- **`lfd` daemon** — wave orchestration, triggers, run management
- **`lfq` Python CLI + API** — wave management and querying
- **Concerto** — refocused as conductor, not chat client
- **Prompt assembly engine** — genuine IP
- **Three agent harnesses** — Claude, Codex, OpenCode
- **Music metaphor** — chord, concerto, cadenza, wave

## What goes

- **Studio auth service + tier gating** — auth moves into lfd
- **Custom sandbox** — pause, evaluate Daytona
- **Concerto as chat app** — it's a conductor
- **Waitlist/growth infrastructure** — ship product, not marketing
- **Per-dimension deployment config** — collapse into blessed configs
- **lfdhub as separate concept** — absorbed into wave model
- **Separate chord data model** — chords are waves, no separate CRUD

## Distribution

Loopflow is an open source tool for a single producer. It's designed to drive production — one person or team building something real, with agents doing the work and chord-waves gardening the system.

Not a platform. Not a service. Not selling seats or tiers. The business model is: loopflow makes one producer dramatically more effective, and that producer ships products that make money. Loopflow is the tool, not the product.

## Existing waves

The five existing waves (foundation, trust, context, concerto, scale) keep running as-is. They aren't manually reshuffled into the new structure. When the default chord-wave is ready (chord-model/07), it absorbs them and proposes restructuring through garden cycles.

Some existing work items are already covered by the new waves:
- scale/04 (chord UI) -> agent-embedding/03 + 04
- scale/05 (cross-repo UI) -> agent-embedding/03

These overlaps resolve naturally when the default chord-wave runs its first garden cycle and proposes consolidation.

## What this proves

If this chord-wave works — coordinates parallel streams, remembers across runs, surfaces what matters, reshapes waves based on accumulated judgment — then chord-waves work.

The second chord-wave is whatever the default chord-wave proposes when it absorbs the existing waves. The third is Cadenza (../cadenza).

The recursive test is the hardest test. A wave that can build itself, garden itself, and remember what it learned is a wave that can do anything.

## Research (reference)

### VSM (Moskov/Scaffold)

Stafford Beer's Viable System Model applied to coding agents. Seven agent roles (S1-S5), persistent Letta memory, signal propagation, algedonic channel.

Useful as conceptual framework. We take the meta-layer concept (chord-waves) and algedonic escalation. We don't take fixed governance layers, mechanical trust scoring, rigid autonomy tiers, or a separate block system parallel to attention items.

### Letta

Stateful agent platform (formerly MemGPT). Layered memory: core, recall, archival. Self-hosted via Docker, REST API, Python SDK. Apache-2.0.

Key tension: Letta wants to be the agent runtime; we want it as a memory service. Decision: Letta at chord-wave level, files at wave level. Their own benchmarks show file-based memory scores well — Letta earns its keep where cross-cutting qualitative judgment matters.

### Daytona

AI agent sandbox infrastructure. Docker containers, sub-90ms creation. AGPL-3.0. SDKs in Python, TypeScript, Go, Rust. First-class git API. Self-hosted via Docker Compose.

Positioned for dev tool builders. Loopflow would use it under the hood, not expose it to end users.

### OpenCode

Go core with TUI. Desktop apps via Tauri/Electron. SolidJS UI. Client/server architecture — we could build a Swift client against their protocol if it stabilizes. Watch and wait.
