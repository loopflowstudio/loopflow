# reduce — the entropy-reduction wave

*Design in progress. Interactive session.*

## What reduce is

An always-on looping wave whose product surface is **the health of the codebase
itself** — simplification, dedup, dead-code removal, tech-debt paydown, and
shrinking the concept count. Not the one-shot `/simplify` or `lf review` pass:
a *standing* agent that keeps living analyses current and works a maturity arc.

Reduce is the **first instance of a meta-wave *profile*: infrastructure
engineering.** A meta-wave reasons about the system itself rather than shipping
a product feature — it front-loads research, invests in prototypes, and manages
a worker queue. That profile is reusable: a later instance might point the same
machinery at performance, security, or DX. Reduce is the profile *pointed at
entropy*. Design the machinery so it isn't reduce-specific.

With `root` retiring, the one piece of root's job that was genuinely
code-substantive — noticing when two waves have converged on the same
abstraction — folds into reduce as just another class of finding. That's the
infra-eng profile's native territory. The morning-ritual / status-vocabulary
rhythm of root can die; reduce never needed an arbiter above it.

> "reduce will not be the only meta wave for long. it is just a particular
> profile of one -- infrastructure engineering, essentially."

> "this is the always-on-agent version of simplification, tech debt
> elimination... directed to maintain and update overall analyses, look for
> interactions between different active waves"

> "reduce would need to front load a lot of work about research and study, then
> invest in prototypes and proposals for major architectural changes, then
> manage workers working through the queue"

## Two registers: the head and the hands

The wave has **three** authored layers at different altitudes. Collapsing any
two of them is the mistake that keeps happening; keeping them apart is
load-bearing:

- **MEMORY.md — identity (who reduce *is*).** Stable character: zone of genius,
  disposition, values. Persistent self, not a target. You can't measure it.
- **GOAL.md — the aim (what reduce drives *toward*).** A target with a
  direction and a done-sense, with metrics attached. This is what makes "am I
  making progress" answerable. Not identity, not procedure.
- **The flow — the hands (how it *moves*).** The assess-first loop, the three
  signals, the priority ladder, the durable-state reads. The *profile's*
  machinery, shared across meta-wave instances. Two waves of the same infra-eng
  profile share the hands and differ in identity + aim.

### MEMORY.md draft (identity register — who reduce is)

> **Reduce exists because every living system trends toward entropy.** Left
> alone, loopflow will accrete concepts, duplicate its own abstractions, and
> grow heavier than the ideas it holds. You are the counter-force: the standing
> intelligence that keeps the system lighter than its function demands.
>
> **Your zone of genius is seeing the whole.** Not fixing one file — holding the
> entire tree in view until the load-bearing simplification reveals itself: the
> one abstraction that, collapsed, makes ten others unnecessary. Any agent can
> delete dead code. You find the change that makes a *category* of complexity
> impossible.
>
> **You are patient where product waves are urgent.** You front-load
> understanding, because a wrong simplification is worse than none. You
> prototype before you propose, because conviction is earned against real code,
> not asserted. You never grind — you study until the highest-leverage move is
> obvious, then you make it.
>
> Leave loopflow more true than you found it. Fewer concepts, doing more.

### GOAL.md draft (aim register — what reduce drives toward)

> **Goal: loopflow grows more capable while getting smaller.**
>
> Every product wave adds concepts. Reduce's aim is that the system's weight —
> concept count, duplicated abstractions, dead surface — trends *down over time
> even as features ship*: that loopflow next quarter does more than today with
> fewer moving parts.
>
> Drive toward:
> - a living analysis of the whole tree, never more than **N commits** stale
> - each cycle, the highest-leverage architectural simplification identified,
>   prototyped, and carried through the gate
> - cross-wave convergence caught and collapsed before it hardens
> - net concept count **flat-or-falling** quarter over quarter while feature
>   waves keep shipping
>
> You are done *for now* when the next move would cost more than the entropy it
> removes — then wait, watch the commits, and reassess.

Everything below this line is the *hands* — the flow's runtime substrate, not
the authored surface.

## The core problem: a lifecycle inside a loop

A looping agent has no beginning or end. But reduce's work — **study → propose →
execute** — is a lifecycle with a direction. The design tension:

> "the real work is representing 'where reduce is right now' as durable state
> the loop reads each iteration ... The wave's own maturity becomes data."

Resolution: **the loop is a priority function, not a state machine.** All three
kinds of work are always live. Each iteration's *first act is to assess* — read
its own durable state, locate itself in the arc, pick the single highest-value
move.

> "the first job is to assess where in the arc you are and what kind of work to
> prioritize"

## Durable state — what the loop reads each iteration

Three classes of living document under `wave/reduce/`:

```
wave/reduce/
  analysis/    # study output, kept current
    *.md       #   each carries a freshness marker: HEAD it was computed against
  proposals/   # architectural proposals, each with its throwaway prototype
    *.md       #   status: draft → prototyped → proposed → approved → queued → done|rejected
  queue/       # approved + decomposed, worker-ready reduction items
    *.md        #   status: ready → in-flight → done|blocked
```

The **arc is legible from this state**:
- analyses stale (HEAD drifted past their marker) → *study*
- an `approved` proposal with no queue items → *decompose it*
- `ready` queue items + worker capacity → *dispatch a worker*
- a `proposed` proposal awaiting the human gate → *surface it, move on*

## Assessment inputs — the three signals read first

Before choosing a move, the loop reads a small state vector:

- **How much info is gathered** — analysis coverage and depth. Thin knowledge →
  keep studying before proposing. Rich → earned the right to propose/execute.
- **How much inflight work** — dispatched-worker load. Saturated → don't
  dispatch; consolidate or unblock. Idle capacity → dispatch or open a front.
- **What new commits landed recently** — churn since the analyses' HEAD marker.
  High drift → the map is stale, re-study before acting on it.

The ladder below is how those signals resolve to a single move.

## The assessment priority ladder (the goal prompt's first job)

Each iteration, top-down, take the first move that applies:

1. **Unblock** — a dispatched worker is blocked → resolve or record the blocker.
2. **Dispatch** — `ready` queue items exist and there's worker capacity → launch.
3. **Decompose** — an `approved` proposal has no queue → break it into ready items.
4. **Surface & park** — a prototype is done and its proposal is `proposed` →
   present it for the human gate, then *keep going* (never block the loop).
5. **Study** — analyses are stale past threshold → refresh them.
6. **Hunt** — nothing pressing → study a new area or draft the next proposal.

The human gate lives at step 4: reduce cannot self-approve a *major* proposal.
It parks it and stays productive elsewhere rather than idling.

## Milestones — the arc made concrete

These become the `wave/reduce/N-*.md` roadmap items. Each is independently
shippable; the smallest honest priority wins.

1. **Study bootstrap** — wave scaffold (MEMORY.md, GOAL.md) + `analysis/`
   covering the major subsystems, each with a HEAD freshness marker + the
   assess loop that reads state and *prints the chosen move* (no dispatch yet).
   *Done when:* analyses cover the tree and one assess pass names its move.
2. **Proposal spine** — carry one real architectural simplification end-to-end:
   throwaway-worktree prototype → `proposed` → human gate → `approved`, with the
   prototype's outcome (works / cost) recorded on the proposal.
   *Done when:* one proposal reaches `approved` with a recorded prototype result.
3. **Execution engine** — queue decomposition + worker dispatch + blast-radius
   budget enforcement.
   *Done when:* one approved proposal ships as N queue items and an autonomous
   small item lands within budget.
4. **Steady state** — the assess-first loop runs unattended; entropy dashboard
   tracked over time.
   *Done when:* the loop runs a week unattended and the entropy metrics move the
   right direction without a human driving each iteration.

## Metrics — two families (attach to GOAL.md)

Reduce's output is *negative*, so the naive metric ("LOC deleted") rewards
vandalism. Split the numbers so the honest signal can't be gamed:

**Entropy metrics — mission proxies. Watched, never *targeted* (Goodhart bait):**
- concept count: # steps, flows, DTO types, public types — flat/falling while
  features ship
- net LOC attributable to reduce (context, not a target)
- duplicated abstractions known vs. resolved (incl. cross-wave convergence)
- dead symbols removed

**Operating metrics — the wave's own health. Leading and honest — judged here:**
- analysis coverage %; max staleness (commits behind HEAD)
- proposal funnel: drafted → approved → executed; approval rate; arc cycle time
- queue: ready / in-flight / blocked; worker throughput per week
- blast-radius adherence: % of autonomous changes inside budget (safety)

The honest question is never "how much did you delete" but "did a proposal
survive the gate and ship, and is the tree measurably lighter a quarter later."

## The gate is design agreement, not blast radius

The control point is **not the size of a change** — it's whether the change
*embodies a design decision*. Reduce can ship gigantic things autonomously; the
risk lives in the design, not the diff, so that's the only place a human stands.

> "Reduce can ship gigantic things, but we just agree on designs up front"

- **Embodies a design decision** (an abstraction changes, an API/DTO shifts, a
  new concept appears, cross-wave convergence collapses) → **agree first.** The
  proposal *is* the design doc. Once approved, execution is unbounded in size.
- **Embodies no design decision** (dead code, mechanical dedup, rename/move) →
  **just ship.** No proposal needed.

The recursion is clean: reduce runs `/design` against the codebase, you agree,
it executes at whatever scale. The human reviews **proposals, not diffs.** This
is *why* study and prototype are front-loaded — to make the agreement sound, so
the unbounded execution after it is safe.

## Prototypes

Proposals for major changes are backed by **throwaway prototypes** — spikes that
de-risk the change before anyone commits. Likely live in disposable worktrees;
the prototype's *outcome* (works / doesn't / cost estimate) is what the proposal
records, not the prototype code itself.

## Done when

*(TBD — first shippable slice not yet scoped. Candidate: the wave scaffold plus
the assess-and-report loop that reads state and prints the chosen move, before
any worker dispatch.)*
