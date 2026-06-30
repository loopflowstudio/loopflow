# Vocabulary: the Loopflow stack

The product is its own model: a **Loop** runs **Flows** → **Loopflow**. The
runtime noun is **Loop** (renamed from Wave). This doc fixes the MVP nouns and
the behavior that matters: the Loop steers, aligns, and schedules.

## The stack (MVP)

```
Loopflow   the product / surface — where you watch and steer Loops
  └─ Loop      always-running interactive session: steers (human), aligns (Goal +
               metrics), schedules (parallelism, order, budget) — running Flows
               on Tasks as Workers
       └─ Worker   runs one Flow on one Task   (a Flow may block on a human, or run headless)
            └─ Flow    the grain — dispatched whole, never cracked open
                 └─ Step   the internal atom — one prompt → one agent action
─────────────────────────────────────────────── grain line ───────────────
  lf        the engine that executes Steps (the internals)
```

It reads in the name: **Loop** + **flow**. The new vocabulary is **Loop** (was
Wave), **Goal**, **Worker**, **Task**.

## What a Loop does

The Loop is the human-facing brain. Three jobs, one always-running interactive
session:

- **Steer** — takes live steering instructions from the human, mid-flight.
- **Align** — keeps the work pointed at the **Goal** and the **metrics**;
  corrects drift.
- **Schedule** — parallelism, prioritization, order, **budget**; dispatches
  Flows on Tasks as Workers.

Human intent, the Goal, and the metrics all meet here and turn into dispatched
work. Below the grain line, Workers just execute.

## What each noun is

- **Loop** — the live, steerable unit (above). Directed by a **Goal**.
- **Goal / Loop Prompt** — what directs a Loop. The conceptual successor to the
  already-removed `direction`: a prompt that steers a Loop, not a perspective
  fragment injected into one. Third prompt primitive (step=once, flow=composed,
  goal=looped).
- **Worker** — a Loop-spawned executor: a hosted session (tmux, per lfd's
  session infra) running one **Flow** on one **Task**. Dumb-ish: it executes; it
  does not schedule. Its Flow may have blocking interactive parts or run fully
  headless.
- **Task** — the *what* a Flow is applied to. Worker = (Flow × Task). Tasks live
  in **Asana** (the durable store) and are generated in the Loop's conversations.
- **Flow** — the grain a Loop dispatches: handed to a Worker whole, never opened
  into Steps. Altitude, not "smallest unit."
- **Step** — the internal atom, below the grain line. What `lf` executes.

## The grain line

Above it a Loop reasons in Flows and Tasks; below it `lf` runs Steps. Worker→Flow
is where work crosses from "what the Loop dispatches" to "what the engine
executes." A Loop never reasons in Steps.

## Map onto today

| Today | MVP noun | Notes |
|-------|----------|-------|
| Wave (mode `loop`) | **Loop** | the always-on, steerable, interactive unit |
| Wave's goal / former `direction` | **Goal** / **Loop Prompt** | what directs the Loop |
| pool `workers` | **Worker** | already the word; promoted to first-class |
| (new) | **Task** | the unit a Flow runs on |
| Flow (chain of steps) | **Flow** | unchanged; the grain a Loop dispatches |
| Step (prompt → agent) | **Step** | unchanged — the internal atom |
| product / Concerto app | **Loopflow** | done (rename shipped) |
| `lf` / `lfd` | the internals | unchanged framing |

## Blocks are joinable sessions

A Worker is a hosted tmux session (lfd has the roots of this). When its Flow
hits an interactive part, the session doesn't die — it **waits**, and surfaces
in Concerto as a **block**: "this one needs you." The human joins through
embedded Ghostty, attaches to the live session, handles the interactive part,
and the Flow continues.

So the Loop **holds the slot** (the session persists) and **frees the budget**
(idle, no token spend) while it waits. "Block" is not a new primitive — it's the
state of a Worker session paused on a human. This is the redesign's
queue-of-decisions: the system keeps running and surfaces what's stuck.

## Tasks live in Asana

The Loop is a planner *and* scheduler. It **generates** Tasks in its
conversations — reasoning against the Goal, roadmap, and metrics, steered by the
human — and persists them in **Asana**, the durable Task store. Then it schedules
Flows on them as Workers.

Asana being the store means one shared backlog: the Loop writes Tasks there, the
human can shape them there. Aligns with Asana already being the roadmap backend
in the goals model.

## Parked — not MVP vocabulary

Good workflows, not core nouns. Keep thinking about them; don't put them in the
MVP model.

- **Chords** — a Loop whose children are Loops (cross-repo, conductor over member
  Loops). A *workflow* you run, layered on later — not an MVP primitive.
- **Gardening** — the watch-and-tend pattern. A way of using Loops, not a noun.

## If adopted: blast radius (separate migration, not this PR)

Wave→Loop is a wide rename, and it *supersedes* the committed goals-wave design,
which kept "Wave" as the noun. Touches: `loopflow.api` (`create_wave` →
`create_loop`), `lfq` wave commands, `wave/<name>/` convention + frontmatter,
Concerto UI ("waves" throughout), DTOs (wire types mirrored 3 ways), config keys,
README, docs, goldens. Its own wave. The Loopflow product rename (shipped) is
step one; this Wave→Loop rename is the rest.
