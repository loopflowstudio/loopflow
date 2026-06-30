# Goals

Turn a Wave from "a ticker spawning cold, stateless runs over a local roadmap
mirror" into **a persistent Looping Agent running 24/7 against a Goal, steered
by a live Asana Roadmap.** This is a chord — a wave whose members are waves.

## Vision

loopflow becomes a **language good enough that writing goals is a good way to
compute.** Goals are programs; loopflow is the language and runtime.

- **step** — an atom of the language; a well-chosen abstraction, a *word*.
- **flow** — a phrase: atoms composed into a sentence.
- **goal** — an utterance in that language, run *in a loop*. Supersedes
  `direction`. The user's primary authoring surface.

A **Wave** is the **overall system** for one looping unit of work — the loopflow
*process* (the orchestration runtime, the Looping Agent, the loop itself) *plus*
the *data structures* (the wave record in lfd, parent/child relations, area, the
Goal reference, the roadmap binding, the metrics). Not just an actor, not just a
config row — the entire running apparatus.

The Wave runs a **Goal** (a loop prompt) against its **Roadmap** (from Asana):
read roadmap → decide next move → dispatch a flow as inner work → re-measure →
repeat. The flow is the hands; the goal is the head; the Wave is the body that
holds both.

Three layers, cleanly separated:
- **Product developer** — *"I just want loopflow to be a good way to run my
  agents; I don't want to think about the internals."* Writes goals as intent,
  runs agents, never touches steps/flows.
- **loopflow language** — builtin steps/flows rich enough to execute those goals
  out of the box.
- **loopflow developers** — maintain and extend the vocabulary (rare, internal).

Two runtime backends, in priority order:
- **(a) Codex / Claude cloud — FIRST.** Adapt to the developer's existing
  workflow. They own session persistence + remote exec; lfd provisions the goal
  prompt, wires Asana, launches, deep-links. We rent persistence.
- **(b) Hosted lfd + embedded Ghostty — SECOND.** Ours to own and support well.
  lfd owns the long-lived session; Concerto shows full state.

Concerto surfaces, per repo, the looping sessions — a launcher + link for (a),
a real dashboard for (b).

### How a Wave operates (the orchestration contract)

**The Wave is the orchestrator; the Goal is the prompt it runs.** The Wave
(through its Looping Agent) is the actor — not "one flow on repeat." Every Wave
can:

1. **Establish a basic loop** over three handles:
   - **flow** — the inner work pipeline it dispatches each iteration.
   - **roadmap** — the Asana backlog it reads and steers by.
   - **metrics** — the signal in `goal/` it drives and re-measures.
2. **Create subwaves + launch subagents** — fan out. Spawn child waves (the
   chord structure) and launch agents to work them. Self-construction, concrete.
3. **Run adhoc `lf` flows** — beyond its standing flow, invoke any `lf` flow on
   demand.

The **Goal** is the looping prompt that *directs* this orchestration — the third
primitive (step/flow/goal), data not actor.

This contract dictates the capability surface the runtime must expose: read
roadmap + metrics, dispatch a flow, create child waves + launch agents (lfd
API), and run arbitrary flows.

**Where it lives — two layers in the seed:**
- The **LOOPFLOW operating prompt** — the universal Wave orchestration contract
  above, woven into the *initial prompt* of any looping session, so whatever
  agent picks it up (any repo, codex or claude) knows from turn one that it's a
  looping orchestrator with these three powers.
- The **Goal** — this wave's specific looping prompt, layered on top.

The LOOPFLOW operating prompt is *not* a reversal of 2026-06-24 (which pulled
the `LOOPFLOW.md` dev *manual* out of every session). That was loopflow's own
dev context; this is the narrow, universal *runtime contract for being a looping
orchestrator* — a small payload with a real reason to be in the seed. Keep them
distinct so this isn't "cleaned up" as the old manual sneaking back.

### Not here

- Heavy guardrail/permission machinery for self-construction. Self-extension
  (goals authoring steps) is the rare, mostly-internal path; the guardrail is
  default behavior, not a cage.
- Backwards compatibility with the old `direction` config. `direction` is
  superseded by `goal`; migrate, don't shim.
- Replacing codex/claude's runtime in backend (a). We fit into it.

## Goals

- A `goal/` prompt primitive that supersedes `direction`, with the standard
  `.lf/` override model.
- A Looping Agent runtime: backend (a) codex/claude cloud first, backend (b)
  hosted lfd second. Replaces the dumb loop ticker.
- Asana as the **live** roadmap — invert `pm.rs` from down-mirror to live read
  + write-back.
- Concerto per-repo looping sessions (launch + dashboard, two depths).
- Vocabulary expressive enough to build **the clients and the servers** (mobile
  client, CLI client, server) from goals with **zero step authoring**.
- **Cross-repo Goals.** A Goal can span repositories. Default model: a cross-repo
  Goal is a **chord whose children live in different repos** (parent coordinates,
  each child wave is repo-scoped with its own Looping Agent) — falls out of the
  chord-as-wave-with-children structure for free, no new field. Open: whether a
  single *leaf* Looping Agent may span repos directly (one agent, many worktrees,
  coordinated cross-repo PRs) for tightly-coupled changes (e.g. add a server
  endpoint + consume it in the client atomically). Lean: chord-spanning by
  default; add multi-repo leaf agents only if coupled changes need atomicity.

## Risks

- **Backend-(a) coupling.** codex/claude cloud APIs move; A1 (lfd launches
  their cloud) couples us to them. A2 (scaffold-and-hand-off) is thinner but
  cedes lifecycle ownership. Unresolved.
- **Persistence in backend (b).** A genuinely long-lived agent grows its
  context unbounded and loses everything on crash. Threading memory across
  ticker-spawned runs is safer but less "one master loop."
- **Asana as source of truth.** Network dependency in the hot loop; rate limits;
  write-back conflicts when humans and the loop both edit.
- **Self-construction runaway.** Goals spawning goals forever — mitigated by
  blocks→human and default non-extension, but needs a real stop condition.
- **Vocabulary gaps underestimated.** Greenfield scaffold + run + integrate are
  larger than they look; "zero step authoring" is a high bar.

## Metrics

- **Reference builds from goals, zero step authoring:** 3 (mobile client, CLI
  client, server) reaching a running, demoable state. Target: 3/3.
- **Product-dev step-authoring rate** on the reference builds. Target: 0 steps
  authored.
- **Unattended loop iterations** a Looping Agent completes without human
  intervention. Target: ≥ 20 consecutive.
- **Asana round-trip coverage:** % of roadmap items the loop both reads and
  writes status back to. Target: 100%.
- **Vocabulary gap count** against the clients-and-servers acceptance test.
  Target: 0 missing fundamental steps.
