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

This redesign is itself parallel work across multiple threads. Not a
waterfall — not "finish the cuts, then design chords, then build them."
The cuts are front-loaded because they're low-risk and reduce noise, not
because they're prerequisites. Chord design starts now, alongside
everything else.

The redesign is the first chord. Each section below is a wave. They run
concurrently, loosely coordinated, with the dependency being
risk-ordering: clear the low-risk decisions first so we have fewer
distractions when answering the hard questions.

```
chord: redesign
├── wave: clear-the-deck        (auth, deployment, sandbox — decisions, not design)
├── wave: agent-embedding       (Claude Code, Codex, OpenCode in Concerto)
├── wave: chord-model           (the big open design problem)
├── wave: signals               (block cascade, signal taxonomy)
├── wave: lfdhub-consolidation  (bring hub ideas into lfd)
└── wave: cadenza               (validation — build a real app, find the gaps)
```

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

First-class embeddings of Claude Code, Codex, and eventually OpenCode
in Concerto. Not trying to out-polish their chat experience — trying
to be the thing that makes running three of them at once coherent.

Concerto's job:
- Portfolio view (multi-repo, multi-wave status at a glance)
- Worktree/PR lifecycle management
- Wave configuration and monitoring
- Block queue — what's stuck, what needs you
- Terminal embedding for actual coding sessions (Ghostty)

The agent runs in a real terminal. Concerto provides everything outside
the coding session. When OpenCode ships a desktop app or native
components, evaluate adopting them for the chat view. Until then,
Ghostty terminal embedding.

## Build chords

This is the central design problem. Everything else in this doc is
either clearing the way for it or a facet of it.

A chord is a wave whose area is other waves. Same infrastructure, same
flows, same triggers — but its work product is coordination, not code.

- A **wave** has area over files. Produces code, tests, PRs.
- A **chord** has area over its member waves. Produces wave
  configuration, health assessment, coordination decisions.

No mixing. A chord coordinates. A wave produces code. If a wave needs
to coordinate sub-efforts, split it: coordination becomes a chord,
work becomes sub-waves.

### What a chord does

- **Detect** when member waves conflict (same files, competing PRs).
  Resequence or regroup.
- **Review** wave run history. Pause underperforming waves. Adjust
  resource allocation.
- **Audit** quality across wave outputs. Flag drift.
- **Scan** for ecosystem changes. Propose new waves or modifications.
- **Create/split/combine/prune** waves in response to what it observes.

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

### Gardening as a chord flow

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

Build steps create. Garden steps tend. Both are needed.

### Open design questions

These are the hard questions. The cuts and embeddings can proceed
without answers here, but chord development can't.

1. **Data model.** What's the schema for a chord in lfd? Waves are
   repo-local yaml. Chords are server-side — what state do they hold?
2. **Chord runs.** Same flow engine as waves? Different? What triggers
   a chord run?
3. **Area over waves.** What does a chord actually see — wave configs,
   run logs, PR outcomes, all three?
4. **DAG evaluation.** How is acyclicity enforced? What happens when a
   chord's member is another chord?
5. **Minimal chord.** What's the simplest thing that proves the concept?
   One chord, two waves, one tend flow?
6. **Signal taxonomy.** Beyond ci_failure — merge conflict, quality
   gate, stall detection, resource exhaustion? How does a chord detect
   "shallow work"?
7. **Letta integration.** Thin wrapper (Letta stores memories, loopflow
   reads them) or deep (Letta manages agent lifecycle)? The "agents are
   ephemeral" philosophy suggests thin. Letta at chord level only —
   waves stay file-based.

## lfdhub → lfd

<!-- TODO: Jack to fill in what lfdhub ideas are in scope -->

Bring [lfdhub concepts] into lfd core. [Scope TBD.]

## Validate through Cadenza

Cadenza (music learning iPhone app) is the forcing function. Every pain
point in Cadenza development is a loopflow bug.

Not scoped here — but it's why this redesign needs to produce a system
that's low-maintenance and adaptive. The open design questions above
get answered empirically through building Cadenza, not speculatively
through more design docs.

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
