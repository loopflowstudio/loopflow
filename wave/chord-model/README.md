# Chord Model

Make chord-waves work. A chord-wave is a wave whose area is `wave/` — same data model, same infrastructure. No separate chord CRUD, no separate tables. The distinction is behavioral: chord-waves default to the `tend` flow, and the tend steps carry S2-S5 coordination concerns as built-in behavior.

This wave is recursive: it builds the tools that the redesign chord-wave will use to coordinate all four waves, including this one. Early items ship via existing `build` flow. Later items create `tend`. Then the chord-wave starts using what it built.

## Strategy

Bootstrap first. Get the redesign chord-wave running tend cycles against its own waves as fast as possible. Every item after that is informed by what tend reveals.

## The tend flow

Build creates. Tend maintains. Counterpoint — two voices moving independently but in harmony.

```yaml
# "tend" flow
- scan-waves    # read member wave state, run history, PR outcomes
- assess        # compare against directions, find drift
- propose       # suggest changes (new waves, config, pruning)
- apply         # make the changes (or flag for human review)
```

The tend steps map naturally to VSM concerns:

| Tend step | VSM concern | What it asks |
|-----------|-------------|-------------|
| scan-waves | S2 (Coordination) | What's happening across waves? Information flow, trigger state, shared files |
| assess | S3 (Optimization) | Are resources well-allocated? Are waves in conflict? Is work balanced? |
| assess | S4 (Intelligence) | Is the environment changing? Are we building the right things? What's emerging? |
| (directions) | S5 (Identity) | What are we building and why? Does current work serve the mission? |
| (block queue) | Algedonic | Urgent signals that bypass normal flow and go straight to the human |

This isn't configuration — it's what chord-waves *are*. Any wave with a tend flow is asking these questions about its area. Not in a strict hierarchy, but as facets of the same gardening attention.

Convention over configuration. Tolerant readers — wave state files should be readable even when they drift from ideal format. Agents fix what they find, not fail on it.

## Human intervention points

Three kinds, spaced between agent work, each a different attention:

**Build: design review** (forward-looking, single wave). Is this the right thing to build? The prompt shows the design, alternatives, risks. Verdict: go / rethink / scope down.

**Build: code review** (backward-looking, single wave). Is what we built good enough? The diff in context of design intent, not just "does this compile." Verdict: ship / iterate / reject.

**Tend: calibration** (meta, cross-cutting, panoramic). The highest-leverage human moment. The chord-wave presents:
- Are we making real, measurable progress?
- Are we lost in details that don't matter, or skipping details that do?
- Do agents have the tools to evaluate they're creating polished, reliable user experiences?
- Is the human still connected to what's being produced, or drifting?
- Proposed wave mutations with rationale.

The human approves mutations, writes trajectory notes (which become Letta core memories), or overrides.

## Wave mutation levers

When the chord-wave (or human at calibration) needs to change how a wave operates:

- **Direction** — shift what a wave optimizes for (add `care` if shipping sloppy, `simplicity` if over-engineering)
- **Area** — tighten scope if producing shallow work, widen if missing the point
- **Flow** — change the process (inject research step if building without understanding, remove gates if they're ceremony)
- **Work items** — re-prioritize, rewrite stale items, delete non-issues
- **Agent** — shift model (opus for research, haiku for cleanup)
- **Step agents** — different models for different steps in the flow
- **Triggers** — change frequency, add/remove trigger sources
- **Lifecycle** — pause, resume, split, combine, or prune a wave

## Letta memory

Thin integration. Letta is a memory service, not an agent runtime. Waves stay ephemeral with file-based state. The chord-wave is the only thing with persistent qualitative memory — the architectural boundary that makes chord-waves more than fancy cron jobs.

```
chord-wave tend cycle starts
  -> load from Letta:
      core:     design principles, key decisions, current priorities
      recall:   recent wave activity, conflict resolutions, human decisions
      archival: full redesign context, abandoned approaches, research
  -> run tend flow with memories in prompt context
  -> write to Letta:
      what was observed, what was decided, what was proposed
chord-wave tend cycle ends
```

Block resolutions feed into Letta. The chord-wave accumulates judgment — "last time we saw this pattern (stall after three PRs on the same item), narrowing scope and adding a research step worked." Patterns emerge from repeated resolutions and get applied to future similar situations.

## Depth over speed

The trap: systems that emphasize scale and speed end up with jagged polish. Some parts pixelated summaries, others handcrafted. The inconsistency makes the whole product hard to trust.

**It is better to go deep on fewer things than to leave unknown unknowns accumulating across a wide surface.**

## VSM expressibility

Two levels of VSM influence:

**Absorbed into the DNA.** Every chord-wave, by default, asks VSM-level questions via the tend flow. This isn't configuration — it's what chord-waves are.

**Expressible as wave configuration.** For users who want the full Moskov system — explicit S2 through S5 agents with distinct roles, formal escalation paths — they can build that as nested chord-waves. A chord-wave with five member waves, each focused on a specific S-level concern. Or a five-step flow where each step embodies a level.

The system must be expressive enough to represent VSM directly. If you can't build Moskov's architecture as a chord-wave configuration, the model isn't general enough. This is a design constraint, not a feature request.

## Two chord-waves

**Redesign chord-wave.** The first one. Coordinates four waves of this redesign. Recursive — builds its own tools, then tends its own construction.

**Default chord-wave.** Every project gets one. After the redesign chord-wave proves tend works, the default chord-wave absorbs the existing five waves (foundation, trust, context, concerto, scale) and restructures them through tend cycles. The chord-wave proposes, the human reviews.

Chord-waves can contain chord-waves (DAG, acyclicity enforced). The default chord-wave at the top, project chord-waves inside, waves at the leaves.

## Goals

- Tend flow runs against the redesign chord-wave's own waves
- Letta provides persistent memory across tend cycles
- Chord-wave can mutate wave configuration (direction, area, flow, agent, work items)
- Human calibration moments surface trajectory, not just status
- Default chord-wave exists as concept, ready to absorb existing waves after proven
- VSM expressibility: tend flow asks S2-S5 questions by default, AND a user can configure nested chord-waves that directly implement the full Moskov VSM hierarchy

## Risks

- Letta integration could be heavier than "thin wrapper" — watch for scope creep
- Tend flow could become ceremony if it doesn't surface genuinely useful observations
- Recursive bootstrapping means early tend cycles run on incomplete machinery

## Metrics

- Number of tend cycles that surface an actionable observation (target: >50%)
- Time from block detection to human awareness (target: <1 hour during working hours)
- Number of wave mutations proposed by chord-wave that human accepts (signal of useful judgment)
- Human-system drift: days since human engaged substantively with a wave's output
