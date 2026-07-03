# Waves, one level outward — Concerto rebuild

> **Scope shift.** This started as "slice 1: a repo-filtered wave list on the
> stubbed single-repo model." It has grown into a **rebuild of the Concerto
> product UI**, on the real Wave/RepoWork schema. The slice-1 list already
> landed (see `jack-heart.waves-outward-review.md`); it's the first tile of the
> new surface, not a standalone deliverable.
>
> **This is now the `desktop` wave's roadmap.** The durable arc — vision,
> `lfd-owned-wave-identity`, `wave-surface-ux-exploration`, `ux-iteration-loop`
> — lives in `wave/desktop/` (GOAL.md + roadmap items 2–4). This scratch doc
> drives only **this diff: slices A + B** — the Wave/RepoWork model split and
> the simplest three screens that prove the architecture. It gets wiped on land;
> the wave carries what's durable.

## The rebuild

Start the exposed product UI over. **Keep the code in tree — don't delete** —
but nothing is wired into the product until it earns its spot in the new
surface. Menus, windows, everything: re-derive from what the three screens
actually need.

### The three screens

1. **Portfolio** — all active waves (name · repo chips · rollup status).
2. **Repo view** — waves filtered to the current repo, plus **quick-start**: spin
   up a new wave seeded from the repo's roadmap or from **Asana** data.
3. **Wave screen** — the wave's home as a terminal multiplexer (see below).

## Thesis: terminal-first, not native-chat-first

This is the cut the rebuild commits to. Today Concerto is a native SwiftUI
**chat client** — it renders agent turns, reply queues, assistant-message text,
voice input. The new wave screen hosts the agent in a **tmux pane** (the
goal-loop harness) alongside yazi + ad-hoc terminals. Once the agent lives in
the terminal, the native-chat stack stops earning its place. Concerto becomes a
**window manager over tmux/ghostty sessions the daemon runs**, not a chat UI.

## Model: identity vs execution

`Wave.repo: String` today conflates the agent's intent with one repo's
execution. Split into an outer identity and an inner per-repo stream. One intent
→ N per-repo streams sharing the wave's id/name.

```
Wave (outer — identity, singular, lfd-owned)
  { id, name, GOAL, MEMORY, agent, flow, direction, area, triggers, crons,
    repos: [RepoWork] }

RepoWork (inner — execution, per-repo)
  { repo, worktree, branch, status, iteration, activeRun, commits,
    diffStat, openPRCount, pr }
```

Repo is a **filter, not a container**. Many-to-many: a wave references a repo
set; a repo is touched by many waves. `waves.filter { $0.repos.contains(repo) }`.
Wave status = rollup over its RepoWork statuses (rollup rule still open).

## GOAL / MEMORY: lfd-owned master copy

The identity's GOAL and MEMORY are a **single master copy stored by lfd, outside
any repo**. Editable and personal — this is *your* identity, not the repo's.
The repo copies are materialized projections that reconcile *to* the master;
they are never the source of truth.

- **Export (push):** on a PR to repo X, merge the master GOAL/MEMORY with X's
  in-repo `wave/<name>/` copy and write the result into X, so the PR carries
  `wave/<name>/{GOAL,MEMORY}.md`.
- **Pull-in:** subscribe to repos to pull their `wave/<name>/` edits back into
  the master.
- Master is authoritative; both directions reconcile against it.

Consequence: the wave screen's file browser (yazi) is rooted at the **master
wave home**, not any repo — which is exactly why the identity layer is singular
and sits above the fan-out. It also keeps the multi-user door open: identity is
personal + shareable (clone the master into another user's space); *firing
state* (runs you launched) stays personal. Don't build multi-user now; just
keep firing-state separable from identity.

## Harness engine & serialization

The engine is the existing agent CLI, launched with `/goal`. `/goal` + the
existing prompt generation assemble the loop; the universal **LOOPFLOW operating
prompt** (the orchestration contract) is woven into the seed, with the wave's
GOAL layered on top. No new engine to build.

**Wave-home file contract (the serialization).** The lfd master materializes to
the wave home as:
- `wave/GOAL.md` — YAML frontmatter (`primary_flow`, `mode`, `metrics`) + body
  (the loop prompt).
- `wave/MEMORY.md` — sectioned markdown (`Shipped` / `Model` / `Next`); the
  accreting one.
- `scratch/` — working files.

**File-as-master.** The file is the editing surface (yazi edits it); lfd parses
frontmatter into the Wave record. One artifact, three consumers — yazi edits,
lfd stores, the loop injects. The Wave DTO carries parsed `goal` + `memory` +
config; the file is authoritative because it's what the human edits.

**Limits via read-on-demand, not compaction.** Preloading MEMORY into the
assembled context is an optimization, not a requirement. When it exceeds budget,
the LOOPFLOW operating prompt points the agent at `wave/MEMORY.md` (and
`scratch/`, roadmap) to read on demand — the agent already has a shell in the
wave home. No summarization/compaction engine to build.

## Wave screen composition

The wave's **home directory** (lfd-managed: `wave/GOAL.md`, `wave/MEMORY.md`,
`scratch/`) is the cwd everything roots at. Three surfaces onto it:

1. **Goal-loop harness** — tmux session running the chat-agent harness that
   drives the loop. cwd = wave home.
2. **Files** — yazi in ghostty over GOAL.md / MEMORY.md / scratch/. Same cwd.
3. **Ad-hoc terminals** — launch arbitrary CLI sessions in the wave context.
4. **RepoWork strip** *(lean)* — compact per-repo rows (chip · status · iter ·
   PR). Click → drill into that repo's worktree as its own terminal workspace
   (the worker session). Execution lives one level down from the director.

Primitives already exist: `TmuxSession.ensureBaseSession()`
(`tmux new-session -d -s <name> -c <cwd>`), `GhosttyTerminalView`,
`MultiplexerView` / `TerminalWorkspaceView`, `TmuxSessionRegistry`.

## Kept / dropped (the map)

Current product-view surface ≈ 11k LOC (macOS Views 9.3k + Concerto/Views 1.6k).
The three screens need ~a third of it.

**Keep (reshaped):**
- `PortfolioWindow` → all-waves screen
- `ContentView` (already slice-1'd) → repo view + quick-start form
- `MultiplexerView` / `TerminalWorkspaceView` / `GhosttyTerminalView` /
  `TmuxSession` → wave screen panes + worker workspaces
- Plumbing (not product UI): `RepoState`/snapshots, lfd client,
  connection/setup, palette/fonts, `ScreenshotWindow` (review infra)

**Drop (doesn't earn a spot).** *Drop means unwire from the product — remove
from app/menu/window wiring, leave the source in tree. Recover via git if a
later screen needs it. Do not delete files.*
- *Native-chat stack (~1.9k):* `WaveSessionView`, `SessionContextView`,
  `VoiceInputButton`, `InteractiveSessionView`, `ReplyQueue`,
  `SelectableAssistantMessageTextView`
- *Structured wave-detail (~2.4k):* `WaveDetailPanel`, `WaveSidebar`,
  `StepRunner`, `WaveRunsTab`, `FlowProgressPills`, `WaitingStateCard`,
  `IterationTimeline`, `WaveWorkspaceView`, `WaveDetailLiveUpdates`
- *Flow-config browser (~1.3k):* `FlowsView`, `FlowTypeahead`, `AreaTypeahead`,
  `DirectionTypeahead`, `TypeaheadComponents` (quick-start may reuse a typeahead)
- *Superseded:* `CatchWaveView` (→ quick-start), `CommandPalette` (defer),
  `AttentionQueueView` + `NextActionsBar` (pending waiting-nudge decision)

## Open questions

- **RepoWork stream location.** Lean: drill-down (b) with a summary strip on the
  wave screen. Alt: tile RepoWork panes directly (a) if you usually work 1–2
  repos per wave. **Undecided.**
- **Who owns the harness lifecycle?** Lean: **lfd** — the daemon spawns the
  goal-loop harness so waves survive the UI (triggers + crons require it);
  Concerto attaches to the daemon's session. Alt: Concerto-owned (UI-bound
  waves) as a stepping stone. **Undecided.**
- ~~**What command is the harness?**~~ **Decided:** existing agent CLI launched
  with `/goal`; existing prompt generation + LOOPFLOW operating prompt assemble
  the loop. See "Harness engine & serialization."
- ~~**Serialization / limits.**~~ **Decided:** file-as-master (yazi edits, lfd
  parses); read-on-demand instead of a compaction engine.
- ~~**Voice input.**~~ **Decided: dropped from wiring for now.** Unwire
  `VoiceInputButton` and the launch-time `VoiceInputService` prewarm; leave the
  code in tree.
- **Waiting-nudge.** How does a terminal-hosted wave signal it needs you? Rollup
  status says `waiting`, but the interaction is inside the tmux pane. Is
  "status chip → open the pane" enough, or does the director need a native nudge
  (the one piece of the chat stack that might have to survive)?
- ~~**Rollup status rule.**~~ **Decided:** `Wave.status` is derived over
  `repos[].status` — any `running` → running; else any `failed` → failed; else
  any `waiting` → waiting; else all `paused` → paused; else idle. Same shape for
  a rollup `iteration` (max, or the running repo's) if the UI needs one.
- **Merge semantics.** Semantic/LLM consolidation vs structural append for
  GOAL/MEMORY export + pull-in. MEMORY across repos wants consolidation; GOAL is
  likely authored-once, top-down.
- ~~**Wire-type split.**~~ **Decided: A2 (full core split), this diff.** Not the
  DTO-only stub — `Wave.repo: String` → `repos: Vec<RepoWork>` through the Rust
  store + executor, both DB backends, then the Python/Swift DTO mirrors +
  fixtures. Field boundary: identity (goal, agent, flow, direction, area,
  triggers, crons, mode, workers) stays on `Wave`; execution (repo, worktree,
  branch, status, iteration, activeRun, commits, diffStat, openPRCount, pr) moves
  into `RepoWork`; `Wave.status`/`iteration` become rollups. A sequenced,
  build-green plan is being produced before implementation; identity *storage*
  (lfd-owned GOAL/MEMORY) stays out of scope — that's roadmap item 2.

## Build order

1. **Repo-filtered wave list** — done (slice 1, stubbed model).
2. **Wave/RepoWork model split** — wire-type change; the real schema.
3. **Wave screen** — harness pane + yazi + ad-hoc terminals + RepoWork strip.
4. **Repo-view quick-start** — seed a wave from repo roadmap or Asana.
5. **Worker workspace** — drill from a RepoWork into its worktree terminal.
