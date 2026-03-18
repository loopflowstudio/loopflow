# Chord Model

## Vision

Every chord is a viable system. Each chord maintains its own identity, boundary, and all five VSM systems for its members. The root chord is special only because it has no parent — its algedonic signals go to a human.

A chord-wave is an ordinary wave whose `area` points at `wave/<name>/` directories. No parallel runtime model. Coordination, self-healing, and policy all grow out of the existing wave/flow system.

## The VSM mapping

### Any chord (S2–S5)

Every chord is responsible for its own five systems:

- **S5 (Identity/Policy)** — what is this chord? what's its boundary? what autonomy levels do members get? what gets escalated vs handled locally?
- **S4 (Intelligence)** — what changed in the environment? what's coming? scanning for relevant signals.
- **S3 (Control)** — are members performing? where to allocate attention? resource management. Algedonic signals from members land here first.
- **S2 (Coordination)** — are members conflicting? oscillating? duplicating work? resolve interference.
- **S1 (Operations)** — the member waves themselves, running their own flows.

### Root chord (S5 especially)

The root chord's S5 holds the most consequential policy — the identity and boundary of the whole system. Its decisions cascade to every nested chord. It answers: what is this project? what matters? what risk tolerance do agents operate under?

The root chord is also the terminal escalation point. Algedonic signals that no nested chord can handle surface here, and from here to the human.

### Algedonic channel

The pain signal that bypasses normal hierarchy. When a wave fails and self-healing fails, the algedonic signal goes to the nearest chord's S5. If that chord can't handle it (per its policy), it escalates to its parent. The root chord escalates to the human via the attention queue.

Pattern: detect failure → classify error → headless repair in same branch → if repair fails, escalate.

## Two granularities

### VSM flow (single pass)

One agent walks through all five systems in descending order. One PR.

```yaml
# flow: vsm
- s5   # identity: still who we say we are? boundary right?
- s4   # intelligence: what changed? what's coming?
- s3   # control: members performing? allocate attention
- s2   # coordination: conflicts? oscillation? duplication?
```

S1 is absent — it's the member waves running their own flows. The chord doesn't do S1 work.

The existing `tend` flow is a specific arrangement of the same ideas for interactive use (scan → assess → branch to chord or reorg). `vsm` is the general-purpose builtin.

### VSM chord (five waves)

For projects complex enough that each system needs its own persistent wave, memory, and cadence:

```yaml
# wave/root/root.yaml
flow: vsm
area:
  - wave/s5-policy/
  - wave/s4-intelligence/
  - wave/s3-control/
  - wave/s2-coordination/
  - wave/s1-operations/    # or just the leaf waves directly
```

Each system wave runs on its own rhythm. S5 slower (weekly, on-demand). S4 scans frequently. S3 tends daily. S2 reconciles as needed. Same model, different scale.

## Strategy

### Algedonic signals first

The first atom to get right is the pain signal. When something goes wrong — CI fails, agent crashes, step produces garbage — the system tries to fix it headlessly. When it can't, it creates an algedonic signal routed to the parent chord. The root chord surfaces it to the human.

Error classification drives repair strategy:

| Error class | Detection | Repair |
|-------------|-----------|--------|
| CI failure | GitHub webhook | `ci-fix` flow |
| Agent crash | Non-zero exit | `debug` with error log |
| Step contract violation | Post-step check | Re-run with guidance |
| Branch router ambiguous | No valid path | Re-run with stricter prompt |
| Rebase conflict | Git exit code | Auto-resolve attempt |

### Wave discovery from disk

Waves exist as YAML in `wave/`. lfd discovers them, reconciles against the store, starts running. No manual `lfq create`. The root chord gets auto-created when Concerto launches, with membership derived from what's on disk.

### Self-healing before coordination

Once waves are self-starting and self-healing, the chord's VSM flow has real operational data. It can read algedonic history, see which waves are healthy vs struggling, and make coordination decisions grounded in observed behavior.

The old standalone `signals` wave folds back here. Stall detection, repeated repair failure, and later signal memory are chord concerns, not a second block system beside attention items.

### Recursion

Chords can contain chords. Each is a viable system with its own S5. Acyclicity enforced. The area-derived membership model handles nesting naturally — a chord's area entries that match `wave/<name>/` are its members.

## Goals

- Algedonic signals: failure → headless repair → escalate if stuck, end-to-end
- VSM flow as the default chord flow (s5 → s4 → s3 → s2)
- Wave discovery from disk, root chord auto-created
- Each chord holds its own identity and policy
- Root chord as terminal escalation point to human
- Recursive viable systems: chords containing chords
- Repeated stall/algedonic patterns feed later mutation and memory work without inventing a parallel block model

## Risks

- Self-healing could mask root causes if repair succeeds superficially
- VSM steps could become formulaic checklists instead of genuine system assessment
- Wave discovery could fight with manual wave management
- Algedonic signals could be noisy if thresholds are too sensitive
- Recursive chords could create deep escalation chains that delay human awareness

## Metrics

- Time from failure to either auto-fix or human notification: <10 minutes
- Repair success rate by error class: tracked, trending upward
- Algedonic signals that resolve without human intervention: tracked
- VSM flow cycles that produce at least one actionable change: >50%
