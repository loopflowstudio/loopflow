---
priority: medium
---

# Wave identity vs per-repo work (one level out)

**Finish line:** The durable model splits a Wave's singular *identity* (GOAL,
MEMORY, agent, flow, triggers) from its per-repo *execution* (worktree, branch,
run, PR). One wave holds `repos: [RepoWork]`; the wave list is repo-filtered;
GOAL/MEMORY live once on the wave, above the repo fan-out.

## Context

Not an inversion — a **new outer layer**. Today `Wave.repo: String` conflates
the agent's intent with a single repo's execution. The move splits them:

- **Wave (outer)** — the agent's *identity*: GOAL, MEMORY, agent config, flow,
  area, triggers, crons. Singular and coherent. Persists across runs and across
  the repo fan-out. This is what the wave list shows.
- **RepoWork (inner)** — where the agent *acts*: per-repo worktree, branch,
  status, iteration, activeRun, PR, commits, diffStat. This is basically today's
  `Wave`.

```
Wave { id, name, GOAL, MEMORY, agent, flow, direction, area, triggers, crons,
       repos: [RepoWork] }

RepoWork { repo, worktree, branch, status, iteration, activeRun, commits,
           diffStat, openPRCount, pr }
```

One intent → N per-repo streams that share the wave's id/name.

### Repo is a filter, not a container

Many-to-many. A wave references a set of repos; a repo is referenced by many
waves.

- Portfolio root → all waves (+ repos).
- Click a repo → wave list filtered to `waves.filter { $0.repos.contains(repo) }`.
- Click a wave → its per-repo streams; inside a repo, the wave-keyed work as today.
- Wave status = rollup over its RepoWork statuses.

### Two things feed the agent (distinct axes)

1. **Injected context** — the *world* it works on. Per-repo, ambient, external:
   the repo's roadmap (on disk in `main/`) and `main/wave/` history (prior waves'
   GOAL/MEMORY). Context injection, NOT a menu of pre-shaped waves.
2. **Agent identity / integrity** — the *self* doing the acting. GOAL
   (directive), MEMORY (accumulated self), agent config. Singular. One coherent
   self, many hands. Identity does not fragment per repo.

**Consequence — GOAL/MEMORY move UP.** The earlier concerto-wave-thread design
wired GOAL.md/MEMORY.md to *each repo's* worktree. That was wrong under this
model. GOAL/MEMORY are identity → they belong to the **wave**, singular. Each
repo injects its own local roadmap + `wave/` history as context, but there is
one GOAL and one MEMORY above the fan-out. Today they live once in
`wave/<name>/GOAL.md` + `MEMORY.md` and are injected into the loop — keep that
shape as the identity anchor when the split lands.

### Multi-user falls out for free

Singular identity means a wave's agent context is shareable:

- **Agent identity (GOAL/MEMORY)** — shareable. A wave can be solo, or a
  team/project several people work in.
- **What you've fired** (runs, streams you launched) — personal to you.

Solo is the one-person case of the same structure. Don't build multi-user now —
just keep firing-state (runs/sessions) separable from identity so the team case
stays open. Cross-user sharing, when it comes, is likely **cloning** — copy a
wave's identity into another user's space, their fired state fresh — not live
shared session state. Way down the line; don't build it.

## The cross-repo fork (hold open)

This item and `2-wave-ancestry` give **two different answers to cross-repo
Goals** — resolve the tension deliberately, don't let it drift:

- **Chord-spanning** (README default, `2-wave-ancestry`): a cross-repo Goal is a
  *chord whose children live in different repos*; each child wave stays
  single-repo (`wave.repo` stays single on each leaf). No new field.
- **Multi-repo leaf** (this item): one wave holds `repos: [RepoWork]` and spans
  repos directly. This is the concrete design for README's open question —
  "whether a single leaf Looping Agent may span repos directly." Better for
  tightly-coupled changes (add a server endpoint + consume it in the client
  atomically); costs a bigger reshape of the durable `Wave` type.

These aren't mutually exclusive — ancestry (wave↔wave nesting) and RepoWork
(wave↔repo fan-out) are orthogonal axes and can coexist. The fork is only about
*which one carries cross-repo*. Lean: land ancestry first (it's the leading edge
and unblocks the chord model regardless); adopt RepoWork when coupled cross-repo
changes prove the chord-of-single-repo-children model too coarse.

## Build order (blank outward)

Strip the exposed UI to blankness, grow from the wave list up. This reshapes the
Concerto surface in `3-concerto-looping-sessions`, so sequence them together.

1. **Repo-filtered wave list** — gut `ContentView` to a bare list: active waves
   touching the current repo (name, repo chips, rollup status). Nothing else.
2. **Create-wave** — pick a repo set; each repo contributes its on-disk context.
3. **Open a wave** — its per-repo streams.
4. **In-repo workspace** — reintroduce the multiplexer, wave-keyed, under a
   selected repo. The old concerto-wave-thread multiplexer layout
   (agent | GOAL·ROADMAP·MEMORY) is superseded as a *default* — GOAL/MEMORY
   aren't per-repo panes anymore; the workspace returns in a form that fits the
   outer identity.

## Deferred (deliberately)

- **Shared vs per-repo flow.** Identity is singular either way. A wave might run
  one flow across all repos, or each repo diverge. Decide when we open a wave.
- **Multi-user.** Model admits it; don't build it.

## Done when

- The durable `Wave` carries `repos: [RepoWork]`; `Wave.repo: String` is gone.
- The Concerto wave list is repo-filtered (`waves.filter { repos.contains(repo) }`)
  and wave status rolls up over RepoWork statuses.
- GOAL/MEMORY resolve once per wave (identity), with per-repo roadmap + `wave/`
  history injected as context — not per-repo GOAL/MEMORY.
- The cross-repo fork above is resolved (in `scratch/` or the README), not left
  implicit.
