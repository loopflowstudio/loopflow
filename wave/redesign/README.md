# Redesign

The agent layer is commoditizing. Claude Code, Codex, and OpenCode are all good enough — and getting better faster than we can compete. Loopflow's future is the layer above: orchestrating agents, not being one.

Three things differentiate loopflow:

1. **The wave/flow data model** — composable area, direction, flow, triggers
2. **Chord-waves** — waves whose area is other waves, tending the system and accumulating judgment
3. **Music as organizing metaphor** — not a skin, but a way of thinking about creative technical work

Everything below the orchestration layer is someone else's problem. Everything above it is ours.

## How this project works

Build and tend. Two voices in counterpoint.

A wave builds — code, tests, PRs. A chord-wave tends — `scan-waves`, `assess`, then routes to tune or silence. Build rounds route to play or silence. Same flow engine, same data model. The difference is area: files vs `wave/`.

This wave is the first chord-wave. Its first job is building itself. Its second job is tending the waves that build everything else. The recursive case — a chord-wave that tends its own construction — is the real test.

All four member waves use existing `build` / `ship-wave` flows until the chord-model wave produces `tend`. Then this wave starts using what it built.

During bootstrap, these waves register in `manual` mode. Wiring the structure together should not immediately start build/tend loops.

```
wave: redesign (chord-wave)
│
│  area: wave/chord-model/, wave/clear-the-deck/,
│        wave/agent-embedding/, wave/signals/
│  flow: tend
│
├── wave: chord-model           6 items — tend flow, Letta, mutations
├── wave: clear-the-deck        2 items — deploy collapse and sandbox pruning
├── wave: agent-embedding       6 items — Concerto as conductor
└── wave: signals               5 items — block taxonomy, cascade, memory
```

## Phasing

### Phase 1: Bootstrap

Build enough to run the first tend cycle. Uses existing flows.

- **chord-model/02** — live tend-cycle validation (boot lfd, register redesign, run tend for real)
- **signals/01** — block taxonomy (types, data model, API) in parallel with item 02
- **chord-model/03** — Letta integration once live tend output exists to remember

Current sequencing matters:
- `chord-model/02` owns the operational gap between "the tend machinery parses" and "the redesign chord actually runs in lfd"
- `signals/01` should start alongside item 02, not after it — the block queue work is already waiting on concrete block types
- `agent-embedding/01` can keep building isolated Swift scaffolding, but backend/auth/pm work that escapes `swift/` should move into the owning wave instead of broadening its scope silently
- `clear-the-deck` stays intentionally quiet until the shared `lfd/` + `python/loopflow/` area settles

First tend cycle runs. The chord-wave observes its own bootstrap commits. Letta records its first memories. The recursive loop is live.

### Phase 2: Build and Tend in Counterpoint

Waves produce real PRs. The chord-wave tends them. Build and tend alternate — waves create, chord-wave observes, chord-wave proposes, waves adjust.

**clear-the-deck** runs fast — two remaining cuts, each a PR-sized decision.

**agent-embedding** starts with block queue view (01) — the chord-wave needs somewhere to surface blocks. Terminal embedding (02) and portfolio (03) follow.

**chord-model** continues with triggers (04), area model (05), wave mutation (06). Each informed by what tend reveals.

**signals** builds cascade (02), stall detection (03), quality signals (04) as real blocks emerge from the other waves' work.

### Phase 3: The Chord-Wave Earns Its Keep

Enough tend cycles to answer the open questions with evidence.

- chord-model/07 — DAG enforcement and default chord-wave
- signals/05 — signal memory (patterns from resolutions)
- agent-embedding/05 — calibration view (trajectory review UX)

The default chord-wave absorbs the existing five waves (foundation, trust, context, concerto, scale) and restructures them through tend cycles. Not manual reshuffling — the chord-wave proposes, the human reviews.

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

Loopflow is an open source tool for a single producer. It's designed to drive production — one person or team building something real, with agents doing the work and chord-waves tending the system.

Not a platform. Not a service. Not selling seats or tiers. The business model is: loopflow makes one producer dramatically more effective, and that producer ships products that make money. Loopflow is the tool, not the product.

## Existing waves

The five existing waves (foundation, trust, context, concerto, scale) keep running as-is. They aren't manually reshuffled into the new structure. When the default chord-wave is ready (chord-model/07), it absorbs them and proposes restructuring through tend cycles.

Some existing work items are already covered by the new waves:
- scale/04 (chord UI) -> agent-embedding/01 + 03
- scale/05 (cross-repo UI) -> agent-embedding/03
- trust/04-05 (sandbox) -> clear-the-deck/02
- foundation/01 (code cleanup) -> clear-the-deck energy

These overlaps resolve naturally when the default chord-wave runs its first tend cycle and proposes consolidation.

## What this proves

If this chord-wave works — coordinates four parallel streams, remembers across runs, surfaces what matters, reshapes waves based on accumulated judgment — then chord-waves work.

The second chord-wave is whatever the default chord-wave proposes when it absorbs the existing waves. The third is Cadenza (../cadenza).

The recursive test is the hardest test. A wave that can build itself, tend itself, and remember what it learned is a wave that can do anything.

## Research (reference)

### VSM (Moskov/Scaffold)

Stafford Beer's Viable System Model applied to coding agents. Seven agent roles (S1-S5), persistent Letta memory, signal propagation, algedonic channel.

Useful as conceptual framework. We take the meta-layer concept (chord-waves) and signal propagation (blocks). We don't take fixed governance layers, mechanical trust scoring, rigid autonomy tiers, or per-agent persistence at every level.

### Letta

Stateful agent platform (formerly MemGPT). Layered memory: core, recall, archival. Self-hosted via Docker, REST API, Python SDK. Apache-2.0.

Key tension: Letta wants to be the agent runtime; we want it as a memory service. Decision: Letta at chord-wave level, files at wave level. Their own benchmarks show file-based memory scores well — Letta earns its keep where cross-cutting qualitative judgment matters.

### Daytona

AI agent sandbox infrastructure. Docker containers, sub-90ms creation. AGPL-3.0. SDKs in Python, TypeScript, Go, Rust. First-class git API. Self-hosted via Docker Compose.

Positioned for dev tool builders. Loopflow would use it under the hood, not expose it to end users.

### OpenCode

Go core with TUI. Desktop apps via Tauri/Electron. SolidJS UI. Client/server architecture — we could build a Swift client against their protocol if it stabilizes. Watch and wait.
