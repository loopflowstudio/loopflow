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

## Still open

1. **MEMORY curation** — server-owned; don't hand-edit. Trim the dated progress
   narration to invariants + verified patterns via `lf memory update`.
2. **Linear roadmap** — text projects may be enough; decide whether to also
   mirror project status into Linear or leave `lf op pm` as the task-tier view.
3. **Process as shared default** — later: pull the generic discipline into an
   inherited default so waves declare only deltas.
```
