# Root

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

Member waves use existing `build` / `build-or-silent` flows. The garden flow lives here in root; member waves build.

```
wave: root (chord-wave)
│
│  area: wave/desktop/, wave/mobile/, wave/workflows/
│  flow: garden
│
├── wave: desktop      Concerto macOS — embedded terminal build driver + native chat UX
├── wave: mobile       iOS read-only view of remote lfd — waves, roadmap, attention
└── wave: workflows    Engine — lfd, model, pm, gstack, flows, runboard, governance UX
```

## Phasing

### Phase 1: Bootstrap

Run the first real garden cycle. Uses existing flows.

- **workflows** (live garden validation, Letta integration) — the operational gap between "garden machinery parses" and "the root chord actually runs in lfd"
- **desktop** (embedded terminal build driver) — the conductor's daily surface, so running a garden cycle feels first-class instead of ceremonial

First garden cycle runs. The chord-wave observes its own commits. Letta records its first memories. The recursive loop is live.

### Phase 2: Build and Garden in Counterpoint

Waves produce real PRs. The chord-wave gardens them. Build and garden alternate — waves create, chord-wave observes, chord-wave proposes, waves adjust.

**desktop** drives daily build work — embedded terminal for builds, native chat UX as second priority. Workspace multiplexer, worktree lifecycle, typed auth UI.

**workflows** extends the engine: wave mutation, Letta memory, VSM flows, PM sync, governance UX (calibration view, portfolio view), runboard, gstack workstyle. Stall detection, self-healing polish, and signal memory all live here.

**mobile** ships the remote read surface — no build work, just the conductor's read channel away from the laptop. Folds in the lfd/model deps needed to make that self-contained.

### Phase 3: The Chord-Wave Earns Its Keep

Enough garden cycles to answer the open questions with evidence.

- workflows (DAG enforcement, default chord-wave, Letta patterns for repeated algedonic signals)
- workflows (calibration view — trajectory review UX)
- desktop (window-composition polish once the embedded-terminal build flow is proven)

The default chord-wave proposes further restructuring through garden cycles. Not manual reshuffling — the chord-wave proposes, the human reviews.

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
