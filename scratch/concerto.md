# Concerto wave restart — GOAL.md onto the flowloop model

## What we're doing

Restart the Concerto wave with a **radically leaner GOAL.md**, built on the
flowloop tiering (`scratch/flowloop.md` in the `loopflow.flowloop` worktree):

- **wave owns the Objective only** — mission · vision · vibe. *Not* the KRs.
- **project owns a KR set (the Measures)** — the durable middle tier. Status
  (core / experiment / parked / killed) is what distinguishes the spine from an
  abandoned prototype. This is the thing the flat Linear roadmap couldn't say.
- **task owns a design-doc → one small PR** — ephemeral.

**Crons live on the wave only.** The wave is the thing with a heartbeat — the
eternal gardener that never stops (daily dogfood pass). A project is just a
durable measure-bucket: `{ status, KRs (measures) }`. No per-project cron; the
wave's loop is the one clock.

The simplification Jack wants *is* flowloop's locked decision: "KRs move out of
GOAL.md." The heavy Measures block (Key Results / Quality / Bounds / Done-means)
leaves the charter entirely and descends to project level.

Projects are **text files for now** ("text files will handle it fine whatever"
— Jack). The project-flowloop runtime is flowloop v2; we don't wait on it.

## The reshape-vs-rebuild resolution

The wave's loudest learning is "reshape proven code, don't rebuild beside it."
That's about *code*, where a rewrite loses hard-won correctness. It does **not**
apply to the charter. "Start fresh" costs differently per layer:

- **GOAL.md** — rewrite freely. Stale framing is a liability, not an asset.
- **Roadmap** — the actual restart. Re-express as projects; nothing valuable lost.
- **MEMORY.md** — curate, don't blank. Keep the invariants + verified patterns
  (frame-don't-render, terminal-session ownership, DTO no-defaults, remote-TLS-via-
  Tailscale); drop the dated progress narration.

## Draft: the new concerto/GOAL.md

```markdown
---
crons: []
pm:
  provider: linear
  linear_project: '9ee88f2a-ef37-46c7-b201-d197db3ccae0'
---

## Mission
Make Concerto the daily surface for conducting waves — without stealing the
vendor's instrument.

## Vision
Open the app and land immediately in the right wave, the vendor's own TUI alive
in the terminal, just enough state around it to pick the next move. Frame, don't
render: navigation, launch, reattach, attention, and repo context are Concerto's;
assistant turns and agent protocol stay with the CLI that made them.

## Vibe
<TBD — the felt quality. A conductor's podium, not a cockpit? Calm, glanceable,
the app disappears into flow?>

## Process
Dogfood before guessing. Reshape the working surface; don't rebuild beside it.
Prefer lfd-owned sessions to Swift-owned tmux. (Most of this is generic loopflow
discipline — candidate to inherit from a shared default rather than re-declare.)
```

## Draft: concerto's projects (the durable tier)

Represented as text for now — one block per project, shape `{ status, KRs
(measures), crons }`. Status is the legibility knob; KRs are the Measures that
left GOAL.md; crons are the project's own rhythm.

- **session-lifecycle** · core
  - KRs: a running wave session survives app restart and reattaches cleanly 5/5
    dogfood trials; launch-or-attach the right vendor session in one action.
  - cron: `daily` → reattach smoke test against a live wave; file the first break.
  - The spine; nothing works without it.
- **attention & navigation** · core
  - KRs: open the app, land in the right wave; list ranked by attention
    (failed → waiting → running → idle), each wave carrying its reason.
- **wave conducting** · core
  - KRs: create, start, and observe a new repo wave from Concerto without opening
    a separate terminal.
- **remote connection** · maintain (shipped)
  - KRs: reach a native remote lfd over HTTPS via Tailscale; token rotation
    without re-paste.
  - cron: `weekly` → tailnet round-trip check (the untested-in-CI gap).
- **⌘K palette** · experiment
  - KRs: keyboard-first launch beats the glanceable list in UX research — or it's
    killed.
- **native multiplexer / native chat** · killed
  - Replaced by lfd-owned terminals and frame-don't-render. Recorded so it reads
    as *dead*, not *pending*.

## Decisions (this session)

- **Objective is one thing.** mission / vision / vibe / purpose collapse into a
  single `## Objective` paragraph — no labeled triad. (Jack: "those are all the
  same.")
- **Measures leave GOAL.md, descend to projects.** Confirmed.
- **Crons on the wave only.** Projects are just `{ title, KRs }`; the wave holds
  the one heartbeat. (Jack: "put the crons on only the wave actually.")
- **No status field.** A project file exists ⇒ it's alive. A dead bet is
  *deleted* (git is the tombstone), matching the repo's "keep one implementation,
  use git for history" rule. Core-vs-experiment is prose, not a field.
  (Jack: "lets drop the status field.")
- **Layout = file-per-project, no frontmatter.** `wave/concerto/projects/*.md`,
  just a title and KRs.
- **Five live projects seeded** from the retired Measures: session-lifecycle,
  attention-navigation, wave-conducting, remote-connection, palette. The old
  native-multiplexer/native-chat direction is deleted, not tombstoned.

## Next: the new-world wave viewer

** THIS IS THE MOST IPORTANT THING TO BUILD WELL IN THIS BRANCH **

Jack: *"I want a wave viewer that shows me the GOAL.md content and for now the
tasks in the associated project"* → *"make it work in the new world … build front
end data structures to model Waves the right way"* → *"Wave will also have execs
which are sessions you can directly take over or steer."*

The current Swift `Wave` carries `goal: String` + `metrics: [String]` — the *old*
world (objective + flat measures). The viewer must render the *new* ontology, so
the frontend domain model mirrors flowloop's tiers.

### The model (Swift, LoopflowCore)

Three surfaces: **the aim** (objective), **the plan** (projects → KRs → tasks),
**the ledger** (runs across it all).

```swift
struct Wave {
    let name: String
    let objective: String       // GOAL.md prose. `goal` renamed; `metrics` retired.
    let projects: [Project]      // the plan: read / steer by chat
    let runs: [Run]              // the ledger: chart/history across every project & task
}

struct Project: Identifiable {
    let id: String               // slug from projects/<id>.md
    let title: String
    let summary: String?
    let krs: [String]            // the Measures that left GOAL.md
    let tasks: [Task]            // Linear issues (empty until wired)
}

struct Task: Identifiable {      // a Linear issue
    let id: String
    let title: String
    let status: TaskStatus       // todo | inProgress | done
    let pr: URL?
}

// Run  = lfd's EXISTING Run DTO (wave.rs ~L330): id, wave_id, flow, task, status,
//        worktree, branch, started_at, ended_at, error, parent_run_id, … Reuse it.
// Session = the live, attachable face of a running Run (TerminalSession, /attach).
//        "A session is the live face of a run you can take the wheel of."
```

**Two interaction halves:** objective/projects/tasks = the conducting surface
(read, steer by chat); runs = the ledger (chart/history), its live rows attachable
as sessions (frame don't render). Vocabulary locked: **Run** = ledger entry
(reuses lfd `Run`); **session** = a live run's attachable tmux; **exec** retired
from the frontend (stays flowloop's word for *how* a run is born).

### Architecture direction (2026-07-07) — `lf` / lfd / pubsub

The spine Concerto builds toward (Jack, this session):

- **`lf` is the single implementation.** It queries lfdb directly — *daemon-less*
  local reads — and runs/executes commands. "Runs become an `lf` API" is the first
  instance: `lf` gains a runs query surface over lfdb. Reads and actions are `lf`.
- **lfd demotes to proxy + pubsub.** It (a) proxies `lf` over HTTP so remote looks
  like local, and (b) subscribes to / streams new runs (pubsub). It is **not** how
  things execute and **not** a parallel implementation. Concerto's *bundled* lfd
  earns its keep solely as the pubsub pipe feeding the ledger.
- **Why:** one implementation (`lf`), matching the repo's "keep one
  implementation" law at the daemon layer; kills the two-code-path / three-mirror
  drift the DTO rule fights; shrinks local-vs-remote to "which lfd proxies."

**The infra is moving that way (started, not complete); this branch is Swift
catching up.** (Jack: "the infra is already going that direction, but we haven't
really redone Swift's code to match" … "it's not complete, but started for
sure.") The primitives that exist on the Rust side:

- `lf runs` (`lf/commands/runs.rs`) — `RunSummary`, trace, event-folding. The
  daemon-less runs query is already there.
- `lf sub` (`lf/commands/sub.rs`) — the pubsub subscribe (live events until
  killed).
- lfd exec door (`http/routes/exec.rs`, #825).

Swift is one generation behind: `LocalWaveService` = *"load Wave and Run data
from lfd daemon (HTTP)"* — `GET /waves` against `baseURL`, the old lfd-is-the-API
model, on the old `goal`/`metrics` `Wave`; its `shellCommandRunner` hook sits
unused. The gap to close:

| | infra (there) | Swift (old) |
|---|---|---|
| query | `lf runs`, daemon-less lfdb reads | HTTP `GET` to lfd-as-API |
| live | `lf sub` pubsub | polling / none |
| model | Run + wave/project/task tiers | `goal` + `metrics: [String]` |

Scope boundary: this branch redoes **Swift** to match. It does **not** migrate
lfd's remaining executor into `lf` — that collapse is its own effort.

### Data sources (under the new spine)

- objective, projects, KRs → `GOAL.md` + `projects/*.md`. Daemon-less: read the
  files directly (slice 1) → later a `lf wave show` query surface.
- runs (the ledger) → a **`lf` runs query** over lfdb (pull) + **lfd pubsub** for
  live new-run updates (push). Reuses lfd's existing `Run` shape; live rows carry
  a `TerminalSession` for `/attach`.
- tasks → Linear via `lf op pm` (which `lf` owns; lfd only proxies it).

### Open modeling question (flowloop R1)

Tasks map to *a project*, but a wave has one `linear_project` today, so tasks are
one flat bucket, not yet per-`projects/*.md`. Slice 1 decision: model `tasks` on
`Project` (right shape) but hang the flat Linear list at wave level / leave
`Project.tasks` empty until R1 resolves — don't invent a fake per-project split.

### Slices

1. **Model + parser + viewer: the aim + the plan (local).** The `Wave/Project/
   Task` structs + a reader turning `GOAL.md`/`projects/*.md` into `Wave/Project`;
   viewer renders objective + projects + KRs. Reads files locally — no wire work.
   **Demo: open Concerto, click the concerto wave, see the objective and five
   projects with their KRs, rendered from the files we committed today.**
2. **The ledger.** A `lf` runs query over lfdb (daemon-less pull) renders the
   wave's `Run` records as a chart/history grouped by origin (project/task) and
   time; the bundled lfd's pubsub pushes live new-run updates; live rows attach as
   sessions. This is where "runs become an `lf` API" lands.
3. **`lf` query surface for the aim + plan** — promote slice 1's direct file read
   to a `lf wave show` query so remote (lfd-as-proxy) works, not just local files.
4. **Tasks from Linear** — surface the wave's project issues via `lf op pm`;
   render under the project.

## Still open

1. **MEMORY curation** — server-owned; don't hand-edit. Trim the dated progress
   narration to invariants + verified patterns via `lf memory update`.
2. **Linear roadmap** — text projects may be enough; decide whether to also
   mirror project status into Linear or leave `lf op pm` as the task-tier view.
3. **Process as shared default** — later: pull the generic discipline into an
   inherited default so waves declare only deltas.
```
