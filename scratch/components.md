---
requires: collapse.md, realign.md, the review fleet's confirmed findings (2026-07-05)
produces: the component charter — role, data, API, IO per component; the conversion map
---
# Components: what each piece is, owns, and speaks

The reorg's target. Rule of the map: **every component names its one role,
its owned data structures, its API (what others may call), and its IO (what
it alone touches).** Anything in a component that isn't in its charter moves
or dies. The review's confirmed import cycles are the proof the current
tree doesn't match this map; the conversion makes it match.

## The map

### `wave` — the listener
- **Role**: serve a channel family: hear (doors), check (observation),
  fold (journal → thread/state/queue), tell (events, ambient reads). Holds
  every pen. Vendor-free. Extractable as a crate.
- **Data**: `Event`/`EventKind` (journal vocabulary), `ChatTurn` +
  `ConversationItem` (the thread — shared wire types), `Channel`/family,
  the resident wire DTOs, `MindState` (journaled; owned by the resident).
- **API**: HTTP doors (`/messages`, `/events`, `/memory`, `/health`,
  `/resident/*`); in-crate: journal folds, endpoint/registry probes,
  **the events-following client** (moves IN from lf::commands::sub — the
  cycle-break; sub.rs becomes a thin caller).
- **IO**: journal files (sole writer), MEMORY.md (sole writer),
  `.wave-endpoint`/`.wave-resident-token`, its HTTP socket, store reads
  (observation only — dies with lfdb).

### `resident` — the mind (inside `wave/` today; its own module, maybe crate later)
- **Role**: the wave's special citizen: owns the vendor harness, the
  scheduler, seed rendering, its home worktree. Publishes turn deltas
  through the door; its input is its subscription.
- **Data**: `MindState`, the event adapter, `MindConfig`.
- **API**: none — it's a process (`lf wave --mind-only`), not a library.
- **IO**: the vendor subprocess; HTTP client to its listener; GOAL.md /
  MEMORY.md / crons reads (via `engine`'s config reader, not lfd's).

### `harness` — vendor drivers (TODAY `lfd::conversations`; moves to `crate::harness`)
- **Role**: normalize vendor CLIs (codex app-server, claude resume-chain,
  opencode serve) into `ConversationEvent`s; steer/interrupt per capability.
- **Data**: `ConversationEvent`, `Capabilities`, `ApprovalPolicy`,
  conformance traces.
- **API**: the `Harness` trait + `default_create_harness`.
- **IO**: vendor subprocesses only. Consumed by `resident` alone.

### `engine` — the material layer
- **Role**: prompts, flows/steps, ambient context, **worktrees + the
  naming rule (the ONE implementation — channel↔path, ownership names)**,
  agent launch for batch runs, **wave file conventions (GOAL.md
  frontmatter — `wave_config` moves here from lfd::http::routes)**,
  repo-root/util resolution (takes `find_repo_root`).
- **Data**: `PromptComponents`, `AgentConfig`, `StreamEvent`, `WaveConfig`.
- **API**: gather/render fns, worktree builders, config readers.
- **IO**: the repo filesystem; agent subprocesses.

### `dispatch` — placement machinery (TODAY buried in `lfd::executor::helpers`)
- **Role**: mint a work line: placement → worktree (via engine's naming) →
  rows (transitional) → channel journal → detached tmux running `lf`.
- **Data**: `Placement`, the env contract consts, the tmux wrapper.
- **API**: `dispatch()` — called by `lf q` and (exec-wrapped) by anything.
- **IO**: git worktrees, tmux, store rows (transitional).
- **Note**: the executor dies; this survives it under its own name.

### `lf` (commands) — the hands
- **Role**: thin verbs. Parse, resolve, call a component, render. NOTHING
  imports from here — the confirmed cycles all break by moving the shared
  machinery down (stream client → wave, find_repo_root → engine).
- **API**: argv. That's the point (the gate mirrors it).

### `lfd` — the gatekeeper: ear-and-voice (Jack, 2026-07-05)
- **Role**: the machine's face — an ear AND a voice, both directions,
  never a hand. Listens inward (scan/index/bridge over files+sqlite);
  listens outward (webhooks, remote clients at the door); speaks outward
  (read routes, /ws push, event relay); speaks inward (**exec lf** —
  attributed speech through the same public doors as anyone: webhook →
  `lf chat --from github`, remote argv → exec under client identity).
  The hand-ban is the constraint: no pens, no git, no tmux, no vendors. Loses its remaining in-process
  mutations (land/next/combine/stop/rename → exec lf under client
  authority). Loses `wave_config` (→ engine).
- **Data**: wire DTOs, the push `Event` vocabulary, the bridge snapshot
  (in-memory, derived).
- **IO**: its HTTP socket, store reads, `lf` execs. Never git, never gh,
  never tmux-kill, never vendor processes.

### `lfdb` — the machine scratchpad (REVISED 2026-07-05: sqlite STAYS)
- **Role**: machine-local operational index, written directly by any lf —
  a FILE, not a center (Jack: postgres was the center; sqlite got swept up
  in the radicalism; corrected). Owns: runs/sessions registry, run_events
  ledger, repos root-list, tokens. NOT: wave identity (markdown),
  conversation (journals), queue truth (git/gh).
- **M2 becomes**: delete postgres + the dual-backend/dialect machinery
  (~2,500 lines); narrow sqlite to this charter; journals own conversation.

### `lfq` — the proxy (python, reborn)
- **Role**: lf-through-HTTP: mirrors lf's ARGV to a gatekeeper, executing
  under CLIENT identity. One endpoint (exec argv → streamed output), zero
  per-verb API to drift.

### `loopflow` (python) — the viewer library
- **Role**: the python twin of Concerto's read layer: waves/channels,
  live sessions under a channel, outstanding PRs, follow /events. The
  scripting surface for dashboards, tests, notebooks. Reads only.

### `ops` — git/GitHub/PM verbs
- **Role**: pr, land, next/advance, queue reconcile, pm (Asana), auth.
- **IO**: git, gh, Asana, token store. Called by `lf op` and exec'd by the
  gatekeeper — never called in-process by route handlers (the fix).

## The confirmed cycle-breaks (review findings → moves)
1. `stream_events` (SSE client): lf::commands::sub → **wave** (resident + sub both call it there).
2. `find_repo_root`: lf::commands::util → **engine**.
3. `wave_config` / `read_wave_config`: lfd::http::routes → **engine** (registry, resident, pm, routes all call engine).
4. Worktree/naming: `channel.rs`'s private path math + executor's
   run_worktree_path → **engine/worktrees as the one rule**; channel.rs
   and dispatch both call it (fixes the 4×-authored divergence).
5. `ensure_wave_worktree` + placement helpers: lfd::executor → **dispatch**.
6. `process_alive` / tmux probes: wave::registry ↔ lfd::executor → one home
   (engine or a small proc util); both import it.
7. Primary-channel predicate: one function in **wave** (name-equality vs
   dot-absence unified); wave_context imports it.

## Ratified during review (Jack, 2026-07-05)
- **Waves outward is the reorganizing principle, radically applied** — the
  review found basics wrong because structure still encodes centralized
  execution; the conversion is not cleanup, it is the philosophy landing.
- **Dispatch is a flag, now**: `lf flow build "task" --dispatch [z]`
  (from x.y, --dispatch z mints x.y.z); `lf q` retires; the explicit
  grammar (lf step/lf flow) joins the conversion as its own worktree.
- **Python reborn** as `lfq` the proxy + `loopflow` the viewer library.
- **`lf op` stays** — the deterministic local sibling of prompted work.
- **`step` renames to `skill`** (the ecosystem's word; sync_skills already
  emits SKILL.md): local invocation = `lf skill <name>` | `lf flow <name>`
  | `lf op <verb>` | `lf : "text"`, with `--dispatch [z]` as remote-ness.
  The rename sweeps engine dirs (.lf/steps→.lf/skills), prompts, docs —
  rides the grammar conversion worktree.

## Sequencing
1. **Fix wave first, this branch** — the review's confirmed behavior bugs
   land before any file moves (fixes are semantic, moves are mechanical;
   mixing them makes both unreviewable).
2. **Conversion worktrees, one per move-set, from the fixed tip** —
   disjoint by file ownership, mechanical, parallel: (a) harness →
   crate::harness; (b) engine consolidation (config, naming, util moves —
   breaks cycles 2,3,4,6,7); (c) dispatch extraction (5) + executor
   residue; (d) gatekeeper honesty (mutations → exec lf) + stream-client
   move (1).
3. Merge back in dependency order (b before c/d; a independent).
