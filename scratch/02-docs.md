# 02: Workflow Docs & Wave Guide

## Problem

The docs teach components bottom-up (Step, Flow, Direction, Area, Stimulus) but never show users what to *do*. Wave authoring — the most powerful part of loopflow — is completely undocumented. The `wave/` directory structure, README format, numbered items, YAML config, auto-loop lifecycle, and the ingest → kickoff → build → update-wave cycle exist only as tribal knowledge in the prompt files.

The docs also don't reflect loopflow's natural progression: start with the CLI to see what it does, graduate to waves when you're ready to scale, go remote when you want agents working while you're away.

**Who benefits:** Every user. New users who don't know where to start. Experienced users who want to author waves but have to reverse-engineer the structure from examples.

**Why now:** Sprint 01 fixed the front door (init). This sprint makes the hallway navigable. Sprint 03 (accuracy) verifies everything is correct — it needs these pages to exist first.

**Wave goals advanced:**
- "New users understand within 5 minutes which mode fits them"
- "A developer can author a wave from scratch using only the docs"
- "Setup entry points (lf init, lfd install, Concerto) have clear ownership and hand off cleanly"

## Approach

Three deliverables, each a file change:

### 1. Rewrite `docs/getting-started.md` — Progressive Journey

The current getting-started has good bones (install, routing table, quick fix, feature workflow, waves intro). Restructure as a natural progression instead of a flat list of features.

The audience is developers who found the repo on GitHub. `lf` is the entry point. Concerto appears when it's relevant — not as a parallel path, but as something you graduate to.

**Structure:**

```
Install
  → curl + lf init (same for everyone)

Try It (keep above the fold — proof before commitment)
  → Demo repo: see a bug, copy error, lf debug -c, fixed
  → Or: lf design, describe your idea, watch it ship

Build Features
  → lf design → implement → gate (the manual workflow)
  → lf build (named flow — same steps, automated handoffs)
  → Steps chain: design writes scratch/, implement reads it
  → Customize: add your own steps in .lf/steps/

Scale with Waves
  → "Ready to automate? Waves run your workflows continuously."
  → lf steps are the manual building blocks. Waves automate them.
  → Concerto (macOS) is the native wave experience — create waves,
     monitor progress, review PRs. Requires lfd.
  → lfq is the CLI equivalent — same lfd backend, terminal interface.
  → You can draft wave content with lf design (local) or by hand,
     then Concerto/lfq picks it up and runs it.
  → "Learn more → Wave Authoring"

Go Remote
  → lfd install on server — your agents work while you sleep
  → Docker mode for isolation
  → Concerto mobile connects to remote lfd
  → Brief, forward-looking. Auth model is evolving — keep it light
    on specifics, emphasize what's possible.

CLI Power User (sidebar/footnote)
  → tmux plugin: status bar, keybindings, layouts
  → Install via TPM: set -g @plugin 'loopflowstudio/loopflow.tmux'
  → Full prompt management, agent pipelining, unified interface across
    Claude/Codex/OpenCode — all from the terminal

Reference
  → Links to lf.md, lfd.md, lfops.md, config.md, wave-authoring.md
```

**What to preserve from current page:**
- Install section (works as-is)
- Quick Fix section with the context token breakdown (strong)
- Inline prompts and context flags (useful reference)
- Feature Workflow with step chaining table (good teaching material)
- Setup paths routing table (expand slightly)

**What changes:**
- Framing shifts from flat feature list to progressive journey
- Waves section distinguishes lf (manual/local) from Concerto/lfq (wave management via lfd)
- Concerto introduced at the waves escalation point as the native wave experience
- Remote section added (brief, since auth/setup details are in flux)
- tmux as sidebar for CLI power users

### 2. New `docs/wave-authoring.md` — Wave Authoring Guide

The core deliverable. End-to-end guide from zero to auto-looping wave.

**Important distinction:** `lf` runs steps locally without lfd — it's manual mode. `lfq` and Concerto both talk to lfd, which actually manages and runs waves. Wave authoring is Concerto-native. `lfq` is the CLI equivalent (same lfd backend). `lf` commands are mentioned as the manual building blocks that waves automate.

**Structure:**

```
Wave Authoring
  → "A wave is a program of work that agents process autonomously."

Creating a Wave (Concerto-native, lfq equivalent)
  → In Concerto: create a wave, set its flow/area/direction
  → CLI equivalent: lfq create mywave .
  → Python API: loopflow.create_wave(...)
  → What this creates on disk:
    wave/<name>/
    ├── README.md        # Vision, strategy, goals, risks
    ├── <name>.yaml      # Flow config (what to run)
    ├── 01-item.md       # First piece of work
    ├── 02-item.md       # Second piece of work
    └── ...
  → The wave/ directory is the source of truth for what to build.
    lfd reads it; update-wave writes it.

Drafting Wave Content
  → Use lf design to explore and draft — it's a local conversation
    that can produce wave/ files. But lf doesn't register or run
    waves. Think of it as drafting sheet music, not conducting.
  → Or write wave items by hand — sometimes a text editor is faster.
  → Either way: once wave/ files exist, Concerto/lfq picks them up.

The Auto-Loop
  → Diagram: ingest → kickoff → build → update-wave → [loop]
  → ingest: picks lowest-numbered item, moves to scratch/
  → kickoff: elaborates into actionable design
  → build: implement → compress → lint → gate
  → update-wave: removes shipped items, folds context into remaining
  → Loop terminates when: backlog empty, max_iterations, or stopped

The Wave Directory (reference)
  The README
    → Vision, Strategy, Goals, Risks, Metrics, "Not here"
    → Show condensed example from wave/infra/README.md

  Writing Items
    → Numbered prefix = stage order (01, 02, ...)
    → Finish line: one sentence, verifiable
    → Scope: what's in, what's out
    → Items within same stage: ingest picks by priority
    → Stages process in order: all 01-* before any 02-*

  The Config YAML
    → flow: which flow to run (ship-wave, build, grind, etc.)
    → area: paths in scope
    → direction: quality lenses
    → agent: optional model override

Running and Monitoring
  → Concerto: visual wave dashboard — status, logs, PRs
  → lfq commands: run, list, logs, stop (same lfd backend)
  → Python API for programmatic control
  → Stimulus types: once, loop, watch, cron
  → Listen: inter-wave coordination (brief — link to waves.md)
  → Don't over-explain stimulus architecture — it's evolving (Chords wave)

Wave Lifecycle
  → Create → Active (items consumed) → Complete (directory removed)
  → update-wave is the only writer to wave/<wave>/
  → "Fold, don't drop" — context from shipped items folds forward

Worked Example
  → Walk through wave/infra/: README, one item, YAML, what happens on run
```

**Key distinction from original design:** Wave authoring is Concerto-native, not CLI-native. Concerto and lfq both talk to lfd (which manages waves). `lf` is manual mode — useful for drafting content and running individual steps, but it doesn't instantiate or run waves. The page teaches the Concerto/lfq path as primary, with `lf` as the drafting tool.

### 3. Update `docs/index.md` — Lead with Action, Keep Depth

The current index.md has strong content (especially "Why Flows?" with diagrams). Keep it — bias towards more info. Add progressive journey routing above the model reference.

**Structure:**

```
Loopflow
  → "Run prompts, hand off cleanly."

Try it (keep existing quick demo)

Query lfd (keep existing)

Python API (keep existing)

The Journey
  → Try it: lf debug -c, lf design → getting-started.md
  → Scale: waves automate your workflow → wave-authoring.md
  → Go remote: lfd on a server, Concerto mobile → getting-started.md#remote

Why Flows? (keep — strongest conceptual content)
  → Linear, parallel, fork diagrams
  → Synthesizer explanation

The Model (keep, tighten slightly)
  → Step, Flow, Direction, Area, Stimulus
  → "Author your own → Wave Authoring"

Where Files Live (keep)

Reference links (keep, add wave-authoring.md)
```

**What changes from current:** Add "The Journey" routing section between the quick-start examples and the model reference. Everything else stays — the "Why Flows?" diagrams and model reference are valuable and shouldn't be cut.

## Forward-looking wave awareness

13 active waves are changing the product. What this means for docs:

**Safe to document (stable):**
- Step/Flow/Direction/Area model (core primitives, not changing)
- `lf design` → wave creation workflow
- Wave directory structure (README + yaml + numbered items)
- Stimulus types: once, loop, watch, cron
- `lfq` commands (create, run, list, logs, stop)
- Auto-loop cycle (ingest → kickoff → build → update-wave)
- tmux plugin (ships independently via TPM)

**Document carefully (evolving):**
- Listen stimulus — Chords wave is reworking inter-wave coordination. Document current behavior, don't explain the signal architecture.
- Auth — OAuth broker and API key fallback are shipping. Keep to "connect your providers via `lfq auth`" without detailing token flow.
- Remote lfd — Studio auth, discovery, hosted SaaS in flight. Describe what's possible, keep setup specifics light.

**Don't document yet:**
- Direction aliases (Context wave)
- Cross-repo area resolution (Cross-Repo wave)
- Concerto context UI / cost analytics (Context + Cost waves)
- Sandbox executor details (Sandboxes wave)
- Voice input (Voice Control wave)
- `lfq usage` / token analytics (Cost wave)
- Hosted SaaS (Remote wave item 06)

## Alternatives considered

| Approach | Tradeoff | Why not |
|----------|----------|---------|
| Three parallel paths (Terminal, Visual, Remote) | Clear but misleading | Suggests equal weight. Reality is a progression with CLI/GUI as interface preference. |
| Fold wave authoring into waves.md | Less page count | waves.md is stimulus-focused (188 lines). Authoring is a different task. |
| Lead wave authoring with `lf` commands | Familiar to CLI users | `lf` doesn't talk to lfd — it's manual mode. Waves are a Concerto/lfq concept. Leading with `lf` would misrepresent the product. |
| Minimal updates — just add wave authoring | Lower risk | Getting-started needs the progressive framing to make the product journey legible. |
| Separate Concerto tutorial | Complete GUI coverage | Wave says "not Concerto features." Concerto appears naturally when relevant, not as its own section. |

## Key decisions

**Progressive journey, not parallel paths.** The docs follow how users actually adopt loopflow: try it → build features → waves → remote. CLI vs GUI is interface preference, not a separate path. Concerto appears at natural escalation points.

**Three layers: lf, lfq, Concerto.** `lf` runs steps locally without lfd — manual mode, good for tinkering and drafting. `lfq` talks to lfd from the terminal — creates and runs waves. Concerto talks to the same lfd — the visual native experience. Wave authoring docs are Concerto-native with lfq equivalents. `lf` is for drafting content and running individual steps.

**GitHub audience starts with `lf`, graduates to waves.** The getting-started page leads with `lf` (the CLI tinkering path). When users reach the "Scale with Waves" section, that's where Concerto and lfq enter as the way to actually run waves.

**index.md keeps its depth.** "Why Flows?" diagrams, model reference, file layout all stay. Add journey routing above, don't cut below.

**Forward-looking awareness.** Document stable primitives fully. Mention evolving features (listen stimulus, remote setup) briefly without committing to architecture that's changing. Don't document features that don't exist yet.

**tmux as a sidebar.** Power user CLI path, not a primary workflow. Installs independently via TPM — no repo clone needed.

## Scope

**In scope:**
- Rewrite `docs/getting-started.md` as progressive journey
- New `docs/wave-authoring.md` with `lf design` as primary path
- Update `docs/index.md` with journey routing, keep depth
- Update nav links across docs pages to include wave-authoring.md

**Out of scope:**
- Concerto feature docs or tutorials (wave: "not Concerto features")
- Gemini CLI documentation (wave: "not here")
- New steps, flows, or directions
- docs/ restructure beyond the three files above
- Content accuracy verification (sprint 03)
- README.md changes (sprint 01 already handled)
- Features from forward-looking waves that haven't shipped

## Done when

- `docs/getting-started.md` reads as a progressive journey: try it → build features → waves → remote
- `docs/wave-authoring.md` exists, Concerto-native with lfq equivalents, `lf` for drafting
- A reader can follow wave-authoring.md from "I have loopflow and lfd running" to "I have a wave auto-looping through my backlog"
- `docs/index.md` routes to the journey before showing the model reference, keeps existing depth
- Concerto is the native wave experience; `lf` is manual mode for tinkering/drafting
- No references to features that don't exist yet (or marked explicitly as coming soon)
- All docs pages that link to related content include wave-authoring.md where relevant
- No broken internal links between docs pages
