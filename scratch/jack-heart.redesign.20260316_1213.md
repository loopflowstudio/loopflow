# Redesign

The agent layer is commoditizing. Claude Code, Codex, and OpenCode are
all good enough — and getting better faster than we can compete.
Loopflow's future is the layer above: orchestrating agents, not being
one.

Three things differentiate loopflow:

1. **The wave/flow data model** — composable area, direction, flow,
   triggers
2. **Chords** — the meta-layer that coordinates waves, tends the system,
   and accumulates judgment
3. **Music as organizing metaphor** — not a skin, but a way of thinking
   about creative technical work

Everything below the orchestration layer is someone else's problem.
Everything above it is ours.

## How this project works

Build and tend. Two voices in counterpoint.

A wave builds — code, tests, PRs. A chord tends — scan, assess,
propose, apply. Same flow engine, same infrastructure. The difference
is area: files vs waves.

The redesign is the first chord. Its first job is building itself.
Its second job is tending the waves that build everything else. The
recursive case — a chord that tends its own construction — is the
real test.

All four waves use existing `build` / `ship-wave` flows until the
chord-model wave produces `tend`. Then the chord starts using what
it built.

```
chord: redesign
│
│  build: creates chord machinery, signals, Letta integration
│  tend:  observes its own waves, surfaces blocks, remembers
│
├── wave: clear-the-deck        (auth, deployment, sandbox — cuts)
├── wave: agent-embedding       (Concerto as conductor)
├── wave: chord-model           (tend flow, Letta, mutations)
└── wave: signals               (block taxonomy, cascade, memory)
```

See `scratch/roadmap.md` for the full wave roadmap with phasing,
work items, and ordering.

## Clear the deck

Low-risk cuts that reduce the surface area draining attention. These are
decisions, not design problems.

**Auth consolidation.** Delete the studio auth service. Move WorkOS
OAuth into lfd — PKCE, JWT issuance, user identity. Three modes:

```
Solo:   local token (auto-generated, current behavior)
Team:   WorkOS OAuth built into lfd
CI:     static token via LFD_AUTH_TOKEN
```

**Deployment collapse.** Stop letting users modulate auth × storage ×
isolation × agent independently. Three blessed configs:

```
Solo:   local lfd + local agents + file-based state
Team:   shared lfd + auth + postgres + container isolation
CI:     headless lfd + single-run mode + no persistence
```

**Stop competing on chat.** Claude Code, Cursor, Windsurf, OpenCode —
all ahead on polish, all iterating faster. Concerto embeds their
terminals and builds the UX *around* coding sessions. The conductor
view, not the chat view.

**Sandbox.** Pause custom sandbox work. Evaluate Daytona as the
container isolation layer.

## Embed agents

Concerto becomes the conductor, not the chat client. The agent runs in
a real terminal. Concerto provides everything outside the coding
session.

Concerto's job:
- Block queue — what's stuck, what needs you
- Terminal embedding for coding sessions (Ghostty)
- Portfolio view (multi-repo, multi-wave status at a glance)
- Worktree/PR lifecycle management
- Wave configuration and monitoring
- Calibration view for tend flow trajectory review
- Window composition — native Swift alternative to tmux, mixing
  terminals with native diff viewers, chat views, wave editors

When OpenCode ships a desktop app or native components, evaluate
adopting them for the chat view. Until then, Ghostty terminal
embedding.

## Build and tend

This is the central design. Everything else in this doc is either
clearing the way for it or a facet of it.

A chord is a wave whose area is other waves. Same infrastructure, same
flows, same triggers — but its work product is coordination, not code.

- A **wave** has area over files. Produces code, tests, PRs.
- A **chord** has area over its member waves. Produces wave
  configuration, health assessment, coordination decisions.

No mixing. A chord coordinates. A wave produces code. If a wave needs
to coordinate sub-efforts, split it: coordination becomes a chord,
work becomes sub-waves.

### The tend flow

Build creates. Tend maintains. Counterpoint — two voices moving
independently but in harmony.

```yaml
# "tend" flow
- scan-waves    # read member wave state, run history, PR outcomes
- assess        # compare against directions, find drift
- propose       # suggest changes (new waves, config, pruning)
- apply         # make the changes (or flag for human review)
```

Convention over configuration. Tolerant readers — wave state files
should be readable even when they drift from ideal format. Agents fix
what they find, not fail on it.

### Human intervention points

Three kinds, spaced between agent work, each a different attention:

**Build: design review** (forward-looking, single wave). Is this the
right thing to build? The prompt shows the design, alternatives, risks.
Verdict: go / rethink / scope down.

**Build: code review** (backward-looking, single wave). Is what we
built good enough? The diff in context of design intent, not just "does
this compile." Verdict: ship / iterate / reject.

**Tend: calibration** (meta, cross-cutting, panoramic). The
highest-leverage human moment. The chord presents:
- Are we making real, measurable progress?
- Are we lost in details that don't matter, or skipping details that do?
- Do agents have the tools to evaluate they're creating polished,
  reliable user experiences?
- Is the human still connected to what's being produced, or drifting?
- Proposed wave mutations with rationale.

The human approves mutations, writes trajectory notes (which become
Letta core memories), or overrides. Ingest is dumb because the human
shows up at design review. The human interactions are spaced between
scoped agent work — each one provides both forward-looking design
intent and backward-looking integration and polish.

### Wave mutation levers

When the chord (or human at calibration) needs to change how a wave
operates:

- **Direction** — shift what a wave optimizes for (add `care` if
  shipping sloppy, `simplicity` if over-engineering)
- **Area** — tighten scope if producing shallow work, widen if missing
  the point
- **Flow** — change the process (inject research step if building
  without understanding, remove gates if they're ceremony)
- **Work items** — re-prioritize, rewrite stale items, delete non-issues
- **Agent** — shift model (opus for research, haiku for cleanup)
- **Step agents** — different models for different steps in the flow
- **Triggers** — change frequency, add/remove trigger sources
- **Lifecycle** — pause, resume, split, combine, or prune a wave

### Signals as blocks

The default state is *running*. When something blocks progress, the
system tries to unblock itself first.

```
Block occurs (CI failure, merge conflict, quality gate, stall)
  → Can the wave unblock itself? (ci-fix, rebase, retry)
    → yes: keep running, log it
    → no: block propagates to chord
      → Can the chord unblock it? (resequence, pause conflicting wave)
        → yes: keep running, log it
        → no: block surfaces to human in Concerto
```

The Concerto UX is fundamentally a queue of blocks — "here's what's
stuck and what you need to decide." Not a notification feed. A machine
waiting for you.

Beyond mechanical blocks (CI, merge conflicts), the chord detects
qualitative signals:

- **Shallow work** — PRs landing but quality is thin relative to intent
- **Stall** — wave running but not producing meaningful progress
- **Capability gap** — wave shipping code without validating user
  experience (no integration tests, no screenshots, no end-to-end)
- **Human-system drift** — approvals getting mechanical, no course
  corrections, the human losing the thread of what's being produced

These surface at calibration, not as interrupts.

### Depth over speed

The trap: systems that emphasize scale and speed end up with jagged
polish. Some parts pixelated summaries, others handcrafted. The
inconsistency makes the whole product hard to trust.

The block queue isn't just "what's stuck." It's also "what needs a
human eye before it compounds." A wave producing shallow work should
surface that as a block: "this wave is moving but the output is thin —
go deeper or ship it?"

**It is better to go deep on fewer things than to leave unknown
unknowns accumulating across a wide surface.**

### Letta memory

Thin integration. Letta is a memory service, not an agent runtime.
Waves stay ephemeral with file-based state. The chord is the only
thing with persistent qualitative memory — the architectural boundary
that makes chords more than fancy cron jobs.

```
chord tend cycle starts
  → load from Letta:
      core:     design principles, key decisions, current priorities
      recall:   recent wave activity, conflict resolutions, human decisions
      archival: full redesign context, abandoned approaches, research
  → run tend flow with memories in prompt context
  → write to Letta:
      what was observed, what was decided, what was proposed
chord tend cycle ends
```

Block resolutions feed into Letta. The chord accumulates judgment —
"last time we saw this pattern (stall after three PRs on the same
item), narrowing scope and adding a research step worked." Patterns
emerge from repeated resolutions and get applied to future similar
situations.

### VSM relationship

Two levels of VSM influence:

**Absorbed into the DNA.** Every chord, by default, asks VSM-level
questions. The tend flow's steps map naturally to S2–S5 concerns:

| Tend step | VSM concern | What it asks |
|-----------|-------------|-------------|
| scan-waves | S2 (Coordination) | What's happening across waves? Information flow, trigger state, shared files |
| assess | S3 (Optimization) | Are resources well-allocated? Are waves in conflict? Is work balanced? |
| assess | S4 (Intelligence) | Is the environment changing? Are we building the right things? What's emerging? |
| (directions) | S5 (Identity) | What are we building and why? Does current work serve the mission? |
| (block queue) | Algedonic | Urgent signals that bypass normal flow and go straight to the human |

This isn't configuration — it's what chords *are*. Any chord with a
tend flow is asking S2–S5 questions about its member waves. Not in a
strict hierarchy, but as facets of the same gardening attention.

**Expressible as a chord configuration.** For users who want the full
Moskov system — explicit S2 through S5 agents with distinct roles,
formal escalation paths — they can build that as a chord structure.
A five-member chord where each member has a specific S-level focus.
Or a five-step flow where each step embodies a level. Or nested
chords where each level is its own chord tending the level below.

The system must be expressive enough to represent VSM directly. If
you can't build Moskov's architecture as a chord configuration, the
chord model isn't general enough. This is a design constraint, not a
feature request — it's the litmus test for chord expressiveness.

### Two chords

**Redesign chord.** The first chord. Coordinates four waves of this
redesign. Recursive — builds its own tools, then tends its own
construction. All open design questions get answered empirically
through building this chord, not speculatively through more design docs.

**Default chord.** Every project gets one. After the redesign chord
proves tend works, the default chord absorbs the existing five waves
(foundation, trust, context, concerto, scale) and restructures them
through tend cycles — not manual reshuffling. The chord proposes, the
human reviews.

Chords can contain chords (DAG, acyclicity enforced). The default chord
at the top, project chords inside, waves at the leaves.

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
- **lfdhub as separate concept** — absorbed into chord model

## Distribution

Loopflow is an open source tool for a single producer. It's designed
to drive production — one person or team building something real, with
agents doing the work and chords tending the system.

Not a platform. Not a service. Not selling seats or tiers. The
business model is: loopflow makes one producer dramatically more
effective, and that producer ships products that make money. Loopflow
is the tool, not the product.

## Research (reference)

### VSM (Moskov/Scaffold)

Stafford Beer's Viable System Model applied to coding agents. Seven
agent roles (S1–S5), persistent Letta memory, signal propagation,
algedonic channel.

Useful as conceptual framework. We take the meta-layer concept (chords)
and signal propagation (blocks). We don't take fixed governance layers,
mechanical trust scoring, rigid autonomy tiers, or per-agent persistence
at every level.

### Letta

Stateful agent platform (formerly MemGPT). Layered memory: core, recall,
archival. Self-hosted via Docker, REST API, Python SDK. Apache-2.0.

Key tension: Letta wants to be the agent runtime; we want it as a memory
service. Decision: Letta at chord level, files at wave level. Their own
benchmarks show file-based memory scores well — Letta earns its keep
where cross-cutting qualitative judgment matters.

### Daytona

AI agent sandbox infrastructure. Docker containers, sub-90ms creation.
AGPL-3.0. SDKs in Python, TypeScript, Go, Rust. First-class git API.
Self-hosted via Docker Compose.

Positioned for dev tool builders. Loopflow would use it under the hood,
not expose it to end users.

### OpenCode

Go core with TUI. Desktop apps via Tauri/Electron. SolidJS UI.
Client/server architecture — we could build a Swift client against their
protocol if it stabilizes. Watch and wait.
