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
├── wave: clear-the-deck        4 items — cuts, deletions, collapses
├── wave: agent-embedding       5 items — Concerto as conductor
├── wave: chord-model           7 items — tend flow, Letta, mutations
└── wave: signals               5 items — block taxonomy, cascade, memory
```

All four waves use existing `build` / `ship-wave` flows until the
chord-model wave produces `tend`. Then the chord starts using what
it built.

---

## Phasing

### Phase 1: Bootstrap

Build enough to run the first tend cycle. Uses existing flows.

- **chord-model/01** — register waves, create chord, verify queryability
- **chord-model/02** — tend flow steps (scan-waves, assess, propose, apply)
- **signals/01** — block taxonomy (types, data model, API)
- **chord-model/03** — Letta integration (stand up, wire into tend)

First tend cycle runs. The chord observes its own bootstrap commits.
Letta records its first memories. The recursive loop is live.

### Phase 2: Build and Tend in Counterpoint

Waves produce real PRs. The chord tends them. Build and tend
alternate — waves create, chord observes, chord proposes, waves
adjust.

**clear-the-deck** runs fast — four independent cuts, each a PR.

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

---

## Existing Waves

The five existing waves (foundation, trust, context, concerto,
scale) keep running as-is. They aren't manually reshuffled into
the new structure. When the default chord is ready (chord-model/07),
it absorbs them and proposes restructuring through tend cycles.

Some existing work items are already covered by the new waves:
- scale/04 (chord UI) → agent-embedding/01 + 03
- scale/05 (cross-repo UI) → agent-embedding/03
- trust/04-05 (sandbox) → clear-the-deck/03
- foundation/01 (code cleanup) → clear-the-deck energy

These overlaps resolve naturally when the default chord runs its
first tend cycle and proposes consolidation.

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
