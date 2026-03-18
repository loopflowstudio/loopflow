# Redesign Roadmap

Build and tend. Two voices in counterpoint.

A wave builds — code, tests, PRs. A chord tends — scan, assess,
propose, apply. Same flow engine, same infrastructure. The difference
is area: files vs waves.

The redesign is the first chord. Its first job is building itself.
Its second job is tending the waves that build everything else. The
recursive case — a chord that tends its own construction — is the
real test. If it can't coordinate its own creation, it can't
coordinate anything.

```
chord: redesign
│
│  build: creates chord machinery, signals, Letta integration
│  tend:  observes its own waves, surfaces blocks, remembers
│
├── wave: clear-the-deck        6 items — cuts, deletions, collapses, cleanup
├── wave: agent-embedding       8 items — Concerto as conductor
├── wave: chord-model           7 items — tend flow, Letta, mutations, APIs
└── wave: signals               5 items — block taxonomy, cascade, memory

wave: dogfood                   3 items — Mac Mini server, phone deploy, team workflow
```

All four waves use existing `build` / `ship-wave` flows until the
chord-model wave produces `tend`. Then the chord starts using what
it built.

---

## Phasing

### Phase 1: Bootstrap

Build enough to run the first tend cycle. Uses existing flows.

- **chord-model/02** — tend flow steps (scan-waves, assess, draft-chord, review-chord, apply-chord)
- **signals/01** — block taxonomy (types, data model, API)
- **chord-model/03** — Letta integration (stand up, wire into tend)

First tend cycle runs. The chord observes its own bootstrap commits.
Letta records its first memories. The recursive loop is live.

### Phase 2: Build and Tend in Counterpoint

Waves produce real PRs. The chord tends them. Build and tend
alternate — waves create, chord observes, chord proposes, waves
adjust.

**clear-the-deck** runs fast — two remaining cuts, each a PR-sized decision.

**agent-embedding** starts with block queue view (01) — the chord
needs somewhere to surface blocks. Terminal embedding (02) and
portfolio (03) follow.

**chord-model** continues with triggers (04), area model (05),
wave mutation (06). Each informed by what tend reveals.

**signals** builds cascade (02), stall detection (03), quality
signals (04) as real blocks emerge from the other waves' work.

### Phase 3: The Chord Earns Its Keep

Enough tend cycles to answer the open questions with evidence.

- chord-model/07 — DAG enforcement and default chord
- signals/05 — signal memory (patterns from resolutions)
- agent-embedding/05 — calibration view (trajectory review UX)

The default chord absorbs the existing five waves (foundation,
trust, context, concerto, scale) and restructures them through
tend cycles. Not manual reshuffling — the chord proposes, the
human reviews.

---

## Rhythm

A chord's execution alternates between tending and building.

```
tend (1 global update)
  scan-waves → assess → branch:
    chord: draft-chord → review-chord → apply-chord
    reorg: update-wave (coherence pass, no human review)

build × N (parallel, one per active wave)
  ingest → branch:
    build: kickoff → review-design → build → review → land
    reorg: update-wave (coherence pass, no human review)
```

One tend cycle, then N parallel build cycles — one per member wave that
has compelling work. Silent waves don't participate in the build round.
The chord naturally focuses on the work that matters most.

This is recursive. A chord whose members include sub-chords:

```
tend (top-level chord)
  └── apply mutations to sub-chords and leaf waves

build (sub-chord A)           build (sub-chord B)        build (leaf wave C)
  └── tend (own update)         └── tend (own update)      └── ingest → build
      └── build × M                └── build × K
```

The ratio — 1 tend per N builds — is the pulse. Tend observes what
built, adjusts, then build runs again. The chord doesn't micromanage;
it sets direction and checks in.

### Beats

Not every cycle produces a PR. A beat is the smallest unit of wave
activity:

- **Build beat**: ingest finds a compelling item, full flow runs, PR lands
- **Reorg beat**: ingest finds nothing compelling, or items are stale —
  wave reorganizes its own plans, updates item coherence, possibly goes
  silent. Single step, no human review.
- **Tend beat**: scan + assess finds pressure points, chord is composed
  and reviewed. Or: nothing to tend, waves reorganize internally.

Reorg beats are how waves maintain their own coherence between tend
cycles. The codebase evolves — other waves ship code, designs diverge,
value diminishes. A wave that notices its items are stale and
reorganizes itself is doing useful work, even though no code ships.

### Silence

Most waves in a large chord should be silent at any given time.

A silent wave has no items (or its items didn't survive coherence
review). It keeps its README — vision, strategy, goals, risks — as
its identity and sensor on the area. Silence signals two things:

- To the chord: "nothing compelling to build right now"
- To the human: "add items here if you want work in this area"

Silence shrinks the blocking queue. Fewer waves competing for review
means faster throughput on the waves that are actually building. A
chord that keeps all waves active simultaneously drowns the user.

The chord can wake a silent wave (add items) or close it entirely.
The human can seed any silent wave's backlog directly.

---

## Human Intervention Points

Three kinds, spaced between agent work, each a different attention:

**Build: design review** (forward-looking, single wave)
Is this the right thing to build? The prompt shows the design,
alternatives, risks. Verdict: go / rethink / scope down.

**Build: code review** (backward-looking, single wave)
Is what we built good enough? The diff in context of design intent.
Verdict: ship / iterate / reject.

**Tend: calibration** (meta, cross-cutting, panoramic)
The highest-leverage moment. The chord presents:
- Are we making real, measurable progress?
- Are we lost in details that don't matter, or skipping details
  that do?
- Do agents have tools to evaluate user experience quality?
- Is the human still connected to what's being produced?
- Proposed wave mutations with rationale.

The human approves mutations, writes trajectory notes (→ Letta
core memory), or overrides.

---

## Wave Mutation Levers

When the chord (or human) needs to change how a wave operates:

| Lever | When to pull |
|-------|-------------|
| Direction | Wave optimizing for the wrong thing |
| Area | Scope too broad (shallow) or narrow (missing the point) |
| Flow | Process wrong (needs research step, or fewer gates) |
| Work items | Backlog stale, wrong priorities |
| Agent | Wrong model for current work phase |
| Step agents | Different steps need different models |
| Triggers | Wrong frequency or trigger sources |
| Lifecycle | Pause, split, combine, or prune |
| Rhythm | Wrong execution pattern — change beat count, edit the grid |
| Silence | Nothing compelling to build — remove items, wave watches its area |
| Wake | Something compelling emerged — add items to a silent wave |

---

## Triage

The five old waves (foundation, trust, context, concerto, scale)
have been triaged. Useful items integrated into redesign waves,
the rest dropped or saved in `wave/backlog.md`.

**Integrated:**
- foundation/01 (code cleanup) → clear-the-deck/05
- foundation/03 (API expansion) → chord-model/08
- concerto/01 (queue management) → agent-embedding/08
- concerto/02 (API key entry) → clear-the-deck/06
- foundation/02 (Mac Mini dogfood) → dogfood/01

**Dropped** (built on things not yet solid):
- trust/04-05 (sandbox) — paused pending Daytona eval
- scale/02-03, 05 (cross-repo) — needs single-repo foundations first
- foundation/04 (container hardening) — no container to harden
- context/02 (UI polish) — context system shape may change

**Saved for later** (see `wave/backlog.md`):
- scale/01 (FlowRun container), scale/04 (chords UI)
- concerto/03 (release UI), concerto/04 (auto-send)
- context/01 (direction aliases)

## Dogfood

The dogfood wave runs alongside the redesign — not as a member,
but as the proving ground. Mac Mini server, phone deploy, team
workflow. Features only count when they work in the real
environment.

---

## What This Proves

If the redesign chord works — coordinates four parallel streams,
remembers across runs, surfaces what matters, reshapes waves based
on accumulated judgment — then chords work.

The second chord is whatever the default chord proposes when it
absorbs the existing waves. The third is Cadenza (../cadenza).

The recursive test is the hardest test. A chord that can build
itself, tend itself, and remember what it learned is a chord that
can do anything.
