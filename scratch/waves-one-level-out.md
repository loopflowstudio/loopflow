# Waves, one level outward

## This PR builds — Slice 1 only

**Scope of this run:** the blank repo-filtered wave list. Nothing else.

Gut `swift/Concerto/Platform/macOS/Views/ContentView.swift` down to a bare list:
active waves touching the current repo, rendered as rows (name · repo chips ·
rollup status). Remove the sidebar+detail / multiplexer / session-takeover surface
from the exposed path — start from blankness.

- **UI-first.** Build against today's single-repo `Wave`. Stub the repo set as
  `repos = [wave.repo]`; do NOT split `Wave`/`RepoWork` across the wire mirrors
  yet (that's a later slice — see Build order).
- Repo-filter = `waves.filter { $0.repos.contains(currentRepo) }` (with the stub,
  that's `wave.repo == currentRepo`).
- No create-wave flow, no open-a-wave detail, no context injection yet.

Slices 2–4 (create-wave, open-a-wave, in-repo workspace) and the `Wave`/`RepoWork`
model split are OUT of scope for this run. See "Build order" below and
`scratch/questions.md` for deferred forks — do not resolve them here.

## The move

Not an inversion — a **new outer layer**. Today `Wave.repo: String` conflates the
agent's intent with a single repo's execution. Split them:

- **Wave (outer)** — the agent's *identity*: GOAL, MEMORY, agent config, flow,
  direction, triggers. Singular and coherent. Persists across runs and across
  the repo fan-out. This is what the wave list shows.
- **RepoWork (inner)** — where the agent *acts*: per-repo worktree, branch,
  status, iteration, activeRun, PR, commits. This is basically today's `Wave`.

One intent → N per-repo streams that share the wave's id/name.

```
Wave { id, name, GOAL, MEMORY, agent, flow, direction, area, triggers, crons,
       repos: [RepoWork] }

RepoWork { repo, worktree, branch, status, iteration, activeRun, commits,
           diffStat, openPRCount, pr }
```

## Repo is a filter, not a container

Many-to-many. A wave references a set of repos; a repo is referenced by many
waves.

- Portfolio root → all waves (+ repos).
- Click a repo → wave list filtered to `waves.filter { $0.repos.contains(repo) }`.
- Click a wave → its per-repo streams; inside a repo, the wave-keyed work as today.
- Wave status = rollup over its RepoWork statuses.

## Two things feed the agent (distinct axes)

1. **Injected context** — the *world* it works on. Per-repo, ambient, external:
   the repo's roadmap (on disk in `main/`) and `main/wave/` history (prior waves'
   GOAL/MEMORY). Anything with a roadmap can have it injected; same for `main/wave/`.
   This is context injection, NOT a menu of pre-shaped waves.
2. **Agent identity / integrity** — the *self* doing the acting. GOAL (directive),
   MEMORY (accumulated self), agent config. Singular. One coherent self, many hands.
   Identity does not fragment per repo.

### Consequence: GOAL/MEMORY move UP

The earlier concerto-wave-thread design wired GOAL.md/MEMORY.md to *each repo's*
worktree. That was wrong under this model. GOAL/MEMORY are identity → they belong
to the **wave**, singular. Each repo injects its own local roadmap + `wave/`
history as context, but there is one GOAL and one MEMORY above the fan-out.

## Multi-user falls out for free

Singular identity means a wave's agent context is shareable:

- **Agent identity (GOAL/MEMORY)** — shareable. A wave can be solo, or a
  team/project several people work in.
- **What you've fired** (runs, streams you launched) — personal to you.

Solo is the one-person case of the same structure. Don't build multi-user now —
just keep firing-state separable from identity so the team case stays open.

Cross-user sharing, when it comes, is likely **cloning** — copy a wave's identity
(GOAL/MEMORY/agent) into another user's space, their fired state fresh — not live
shared session state. The identity/firing split is what makes a clean clone
possible. Way down the line; don't build it.

## Build order (blank outward)

Strip the exposed UI to blankness, grow from the wave list up.

1. **Repo-filtered wave list** — gut `ContentView` to a bare list: active waves
   touching the current repo (name, repo chips, rollup status). Nothing else.
2. **Create-wave** — pick a repo set; each repo contributes its on-disk context.
3. **Open a wave** — its per-repo streams.
4. **In-repo workspace** — reintroduce the multiplexer, wave-keyed, under a
   selected repo.

## Deferred (deliberately)

- **Shared vs per-repo flow.** Identity is singular either way. A wave might run
  one flow across all repos, or each repo diverge. Decide when we open a wave.
- **Multi-user.** Model admits it; don't build it.
- **The concerto-wave-thread multiplexer layout** (agent | GOAL·ROADMAP·MEMORY)
  is superseded as a *default* — GOAL/MEMORY aren't per-repo panes anymore. The
  in-repo workspace returns in step 4 in a form that fits the outer identity.
