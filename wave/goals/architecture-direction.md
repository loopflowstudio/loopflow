# Architecture direction (2026-07-05, reconciled 2026-07-06)

The technical architecture the wave-agent work is converging toward, and the
roadmap to it. Durable record of the design conversation whose working notes
lived in `scratch/{components,collapse,realign,post-m2}.md` (wiped on land).
Decisions marked **[R]** are Jack-ratified; **[~]** are proposed/open. This
document records the target direction. PR #801 (merged) reshaped the components;
PR #803 (M1/M2 compression) then executed a large chunk of the debt below;
PR #796 (merged 2026-07-06) landed the reactive server.

**Reconciled against the merged tree on 2026-07-06** (a top-down four-layer
read: wave core, lfdb substrate, engine+harness, lfd+lf+DTOs). The headline:
the tree is *truer than this doc was* — ~80% of the M1/M2 debt list below was
already executed and only survived here on paper. Resolved items are marked
**[done]** with the PR that closed them. The "Current state" section is the
ground-truth map the next full architecture review should start from; it
supersedes the stale-debt narrative that used to lead this file.

## Current state (2026-07-06 audit) — read this first

### The kernel is small

The whole system pivots on a handful of types and functions. Understand these
and you understand loopflow.

**Data structures.** `Event`/`EventKind` (`wave/journal.rs`) — the append-only
per-wave log, the one truth. `ChatTurn` + `absorb_item` (`chat/turns.rs`) — a
turn, with one growth rule shared by fold/adapter/live-snapshot. `WaveRuntime`
(`wave/runtime.rs`) — the live in-memory materialization of the journal.
`MindState` + `can_transition` (`wave/state.rs`) — the mind lifecycle, exactly
four states. `ChildChannel`/`ChannelFrame` (`wave/channel.rs`) — a named stream
under a wave. `ConversationEvent` (`chat/types.rs`) — the vendor-normalized
event. The `Harness` trait (`harness/mod.rs`) — the vendor abstraction.
`Run`/`RunEventRow`/`Session` (`lfdb`) — the operational rows.

**Load-bearing APIs.** `Journal::append` — the single writer; journal order ==
cache order == broadcast order because everything routes through it under one
lock. `fold_thread`/`fold_workers` — pure folds; the store is truth, memory is
a projection. `run_mind` (`wave/mind.rs`) — the scheduler `select!` loop (the
"outer loop owns scheduling" claim, made real). `WaveRuntime::open` — the boot
janitor that makes the journal closed-consistent before serving. The session
env contract (`LFD_SESSION_ID` + `LFD_SESSION_INHERITED`) — the identity
mechanism: present-without-inherited = "I own this row"; present-with-inherited
= "ancestor's row, chain under it."

### What the audit found: tiers of "unnecessary"

**Tier 1 — already deleted (this doc was stale).** All confirmed gone in the
merged tree: `loop_ticker` + the activation queue, `build_wave_agent_command` /
`InFlightDispatch` / the old goal-agent launch path, `roadmap_item` plumbing,
`LOOPFLOW_OPERATING_PROMPT` and the "Looping Agent" double-identity (single
"mind" identity now), `--pool` / `Placement::Pool`, `LFD_DISABLE_TMUX`,
`alias="loop"`, all postgres residue, and `render_goal`'s roadmap/metrics
ceremony. The naming rule is authored **once** (not 4×); `find_repo_root` and
`stream_events` are already in their charter homes.

**Tier 2 — genuinely dead, being cut now** (PR: dead-code sweep):
`secrets_provider_config` (0 refs) and `wave_pr_merge_events` (insert-only, no
reader) tables dropped; the `worker_dispatched`/`worker_finished` serde back-
compat aliases removed. Still live and deferred: migration-tolerance healing
(048/049/050), deletable only after every store converges.

**Tier 3 — the one real remaining charter gap: lfd is still a hand.** `/land`,
`/next`, `/combine`, `/stop`, rename, `DELETE /waves`, and session
create/cancel still mutate in-process (`crate::ops`, `git`, `tmux kill-session`)
from route handlers. The code self-flags it (`waves.rs:29`). `hooks.rs` already
shows the target pattern — it plans `LfExec`s and spawns detached `lf`. The fix
is making the other routes look like `hooks.rs`. Adjacent placement smell:
`lfd/pm/linear.rs` and `lfd/queue.rs` (vendor + git code) sit under the `lfd::`
namespace though only `lf` calls them. **This is what M1 is now about.**

**Tier 4 — shape questions for the full review (judgment, not cleanup).** Do
not pre-resolve these; they are the review's payload. (a) `journal/mod.rs`
double-writes run telemetry to both `events.jsonl` and sqlite `run_events` —
the jsonl exists only for lfd's poller. (b) Two disjoint journal subsystems
(`crate::journal` CLI-run telemetry vs `crate::wave::journal` the wave log)
share one append+fold pattern built twice; hazard today is two `read_events`
with the same name and different signatures. (c) Three shapes for "the agent
said something": `ConversationEvent → EventKind → ChatTurn`. (d) `MEMORY.md`
carries truth + progress + mailbox + cache at once.

The DTO discipline is clean: no `serde(default)` in the mirrored wire types, and
a real 3-language fixture round-trip test that treats an absent required field
as a parse error. Only blemish: request-*body* DTOs (`LandWaveRequest`,
`UpdateWaveRequest`) derive `Default` — inbound, not in the fixture set.

## The anatomy

Four roles, one verb surface, distance as the only difference:

- **`lf` — the hands.** Does the work, on the filesystem. Every capability
  has a daemonless path.
- **`lfd` — the ear-and-voice (the face), never a hand [R].** Four quadrants:
  listens inward (scan/index/bridge over files + sqlite), listens outward
  (webhooks and, later, authenticated remote clients), speaks outward (read
  routes, `/ws` push, event relay), speaks inward by **execing `lf`**. For
  M1/M2 it is local-only with a machine-local capability token, not user auth;
  remote identity is M3. Hand-ban: no pens, no git, no tmux, no vendors.
- **`lfq` — the future reach.** `lf`-through-HTTP: mirrors `lf`'s argv to a
  gatekeeper. This is M3, not current scope. One exec door, no per-verb API to
  drift.
- **The resident — the mind.** `lf wave <name> --mind-only`, spawned by its
  listener; owns the vendor harness; its input is its own subscription.
- **The filesystem — the body.**

## The substrate: two write-tiers + a query plane

- **Journals (filestore) [R]:** one-pen-by-hierarchy. The narrative substrate
  — primarily channel conversation streams. One-pen because *narrative needs
  order*. MVP stays simple append-only JSONL; rotation/segmentation is a later
  journal-engineering workstream, not M2.
- **sqlite / lfdb [R]:** the many-writers tier — parallel WAL writes from
  every `lf` on the machine. The operational substrate:
  runs/sessions registry, `run_events` ledger, repos root-list, tokens.
  Many-pens because *operations need concurrency*. **A file, not a center**
  — engines ≠ centers; postgres (a server, shared-state coordination) dies,
  sqlite stays. (Correction of the earlier "both engines gone," which
  conflated engines with centers.)
- **The query plane [R]:** one query *system*, not one required query
  *process*. A canonical fold/query module answers machine questions; `lf`
  links it locally, `lfd` serves it to local viewers, and future `lfq` proxies
  it remotely. Centralized code, not daemon-required reads.

Not in any store: wave identity (GOAL.md/MEMORY.md markdown), conversation
(journals), queue truth (git/gh), vendor threads.

## Ratified principles

- **Waves outward [R]:** zero centralized control; the wave is the unit of
  sovereignty; nothing sits above the waves; the ban is on centers *between*
  waves — *within* a wave the listener is deliberately a local center.
- **One pen, by hierarchy [R]:** the nearest running listener holds the pens
  for its channel family; pens follow the tree. This is a concurrency rule:
  journal writes go through the listener instead of direct appends.
- **Human input has practical algedonic priority [R]:** VSM/Friston are useful
  lenses, not a data model. There is no `AlgedonicSignal` and no "algedonic
  channel" in the technical architecture. Humans can jump to a worker through
  tmux/today's terminal surface, future Cadenza/Concerto can link to the
  relevant worker, and the mind has its own steering harness. Worker
  urgency-gated interjection is deferred to M4 dynamics, if the dynamics prove
  it.
- **Channels [R]:** a channel is a named stream (journal + thread +
  subscribability); every wave has one, every work line gets one (ownership
  name = channel name); names are topics, dots are the tree, subscription by
  prefix; the wave tree is the subscription topology. Wave stays an identity;
  promotion (a work line grows a GOAL) is the vocabulary.
- **`lf wave` spawns the mind [R];** the resident's own command is a role
  flag (`--mind-only` / `--no-mind`), not a `mind` noun.
- **A wave targets exactly one repo [R] (2026-07-06).** Cross-repo is a
  nice-to-have, not a shaping constraint. The `Wave { repos: [RepoWork] }`
  multi-repo model (the `wave_repos` table, `RepoWork`/`RepoWorkDto`) is being
  stripped — the single repo and its execution state belong directly to the
  wave. If cross-repo ever returns, it is a **wave-tree of single-repo
  children** over channels, never a repo-list on one wave. This resolves the
  code's prior split-brain: the substrate carried the multi-repo model while
  the reactive server (#796) was already single-`repo_root`.

## The waves-outward claims (C1–C7)

Falsifiable engineering translations of the philosophy. C1–C7 started as
Claude coinages and were reshaped in the July 5 design walk.

1. **C1 — Route-around [R]:** every local capability works with zero
   daemons; a daemon may accelerate/aggregate, never enable.
2. **C2 — Coordination uses durable facts and explicit commands [R]:**
   speech is a worker-output/reporting primitive, not the universal bus.
   Coordination state should be inspectable: sqlite for operational facts,
   journals for conversation, git/GitHub for PR truth, Linear for roadmap truth,
   and explicit `lf` commands for action. Private daemon control loops are
   suspect; commands at authority boundaries are real.
3. **C3 — Execution converges on `lf` invocations [R]:** the ideal execution
   unit is an attributed `lf` command. This is an uptime-style goal: 100% is
   impossible, but every extra nine matters. Exceptions are vendor boundaries,
   transport wrappers, or debt.
4. **C4 — Aggregation is derived and disposable [R]:** `lfd` may scan, index,
   cache, and relay. It must not become authority.
5. **C5 — Current `lfd` is local-only with a capability token [R]:** no OAuth,
   accounts, or remote identity model in M1/M2. Local clients may read
   `~/.lf/session-token`; this is effectively local machine authority, not user
   auth. Remote HTTP identity is M3; self-hosted ops are SSH-first.
6. **C6 — Hierarchy is subscription, not supervision [R]:** parents listen to
   and fold child channels. Starting/gating work is allowed; private live
   command over child workers is suspect.
7. **C7 — Sovereignty is fractal [R]:** no global center above waves. Inside a
   wave, the listener is deliberately sovereign over its channel family.

## Roadmap: M0 → M4 (ordered by when reviewing stops hurting)

- **M0 — True [done #796]:** the reactive server landed; worker reports reach
  the mind, no wedges, no message loss; the live two-process demo runs clean.
- **M1 — Shaped (the conversion):** *mostly done, one gap left.* Harness →
  `crate::harness` [done #803]; the `step`→`skill`/`--dispatch` grammar
  [done #803]; the naming rule, `find_repo_root`, `stream_events` all already
  in their charter homes [done]. **The one remaining M1 move: lfd stops being a
  hand** — convert `/land`, `/next`, `/combine`, `/stop`, rename, `DELETE
  /waves`, session create/cancel to exec `lf` argv (pattern: `hooks.rs`), then
  delete the private hands. Secondary: hoist `lfd/pm` + `lfd/queue` out of the
  `lfd::` namespace (only `lf` calls them). *Exit: no route handler calls
  git/tmux/ops in-process; the dependency arrow already matches the charter
  (verified — nothing imports `lf::commands` but `bin/lf.rs`).*
- **M2 — Substrate:** *mostly done.* Postgres + dual-backend deleted [done
  #803]; container mode + `mode` knob cut [done #803]. Remaining: narrow sqlite
  to the operational scratchpad, and the Tier-4 shape calls (the run-telemetry
  double-write, the two journal subsystems) if the review greenlights them.
  *Exit: run telemetry has one home; sqlite holds only operational facts.*
- **M3 — Faced:** auth/identity, exec-under-client-identity door; `lfq` the
  proxy; `loopflow` the python viewer library; remote Concerto via relay;
  federation. Self-hosted operation stays SSH-first. *Exit: drive a Mac-Mini
  wave from the laptop, same verbs, own identity.*
- **M4 — Alive:** predictions/setpoints/precision (free-energy adopt-nows);
  sovereign spend cap; Decisions/HITL; possible urgency-gated worker
  interjection if the dynamics prove it. *Exit: the wave notices its own
  surprises.*

Standing campaign, throughout: prove-the-language — three reference builds
from goals.

## The M1 conversion work-list (reconciled 2026-07-06)

Most of the 2026-07-05 list turned out already done in the merged tree. Verified
status:

1. ~~`stream_events`: `lf::commands::sub` → `wave`~~ — **[done]** lives at
   `wave/subscription.rs`; `lf/commands/sub.rs` imports it.
2. ~~`find_repo_root`: → `engine`~~ — **[done]** at `engine/repo.rs`;
   `lf::commands::util` is a one-line delegate.
3. ~~`wave_config` → `engine`~~ — **[done #803]** shim dropped; `WaveConfig`
   lives in `engine/wave_config.rs`.
4. ~~Worktree/naming: one rule~~ — **[done]** authored once (`engine/naming.rs`
   + `engine/worktrees.rs`). The only residue: `wave/channel.rs`
   `child_worktree_path` re-inlines the `{repo}.{suffix}` format — fold into
   `engine::worktrees` (small).
5. `ensure_wave_worktree` + placement helpers → `dispatch` — still in
   `lfd::executor/helpers.rs`; the `dispatch` component isn't extracted yet.
6. `process_alive` / tmux probes: one home — minor, still scattered.
7. Primary-channel predicate: one function — minor.
8. **`lfd`'s in-process mutations (`/land`, `/next`, `/combine`, `/stop`,
   rename, `DELETE /waves`, session create/cancel) → exec `lf`.** *This is the
   live M1 gap.* Pattern to copy: `hooks.rs` (`LfExec` + detached spawn).

## Component charter (role · data · API · IO)

- **`wave` (listener):** serve a channel family — hear/check/fold/tell; holds
  every pen; vendor-free; extractable. Owns `Event`/`EventKind`, `ChatTurn`,
  `Channel`, the resident wire DTOs. IO: journals (sole writer), MEMORY.md,
  endpoint/token files, its socket, sqlite reads.
- **`resident` (mind):** owns harness + scheduler + seed + home worktree;
  publishes deltas, subscribes for input. Not a library — a process.
- **`harness`:** normalize vendor CLIs → `ConversationEvent`; the `Harness`
  trait; consumed by the resident alone. (`crate::harness` after M1.)
- **`engine`:** prompts, flows, ambient context, worktrees + THE naming rule,
  wave file conventions (`WaveConfig`), repo-root/util. The material layer.
- **`dispatch`:** mint a work line — placement → worktree → rows → channel
  journal → detached tmux `lf`. Survives the executor's death.
- **`lf` (commands):** thin verbs; NOTHING imports from here.
- **`lfd` (gatekeeper):** see anatomy. Reads + push + webhook-ingress-as-exec
  + boot hygiene. Local-only with a capability token through M2. Never
  git/tmux/vendors/pens in the target shape.
- **`lfdb`:** the machine scratchpad (see substrate).
- **`ops`:** git/GitHub/PM verbs (`lf op` stays — the local deterministic
  sibling of prompted work: `skill | flow | op | :`). Exec'd by the gate,
  never called in-process by route handlers.

## Known staging debt in PR #801

The #801 branch was M0. PR #803 (M1/M2 compression) closed the substrate and
harness items below; the remainder is still open. Known mismatches with the
target above:

- **[done #803]** `harness` moved out of `lfd::conversations` to top-level
  `harness` + `chat`; vendor adapters no longer depend on the daemon.
- **[done #803]** Postgres deleted; `lfdb` is sqlite-only (the
  tokio-postgres/deadpool tree is gone). Narrowing sqlite to the operational
  scratchpad is the remaining M2 work.
- **[done #803]** `wave_config` re-export shim dropped and the route pruned;
  the docker-compose lfd path deleted and the OS ServiceManager folded into
  direct config dispatch. The `mode` knob is gone from `GOAL.md`.
- **[done #803]** `lf q` retired; dispatch flows through `--dispatch`/`--wave`
  on the flow commands (decision-question #1 resolved — one dispatch door).
- Container mode's *deploy* mechanisms (env hardening, named credential mounts,
  health checks, redaction, service-file hygiene) still need a home in the
  future deploy/SSH story if they still apply.
- `lfd` still has remote-bind/token scaffolding (an `AuthConfig.token` override,
  `LFD_HTTP_TRUSTED_PROXY_CIDRS`, IP-source machinery) pre-wired for M3. The
  runtime today meets the M2 target (local capability token only); the M3 knobs
  are dormant. Self-flagged `TODO(M3)` in `lfd/auth.rs`.
- **`lfd` still has hand routes — the live M1 gap.** `/land`, `/next`,
  `/combine`, `/stop`, rename, `DELETE /waves`, session create/cancel call ops
  or tmux in-process. Target: exec `lf` argv (pattern: `hooks.rs`), then remove
  the private hands. See Current-state Tier 3 and M1 item 8.
- **[done]** `stream_events`, `find_repo_root`, worktree naming — all already
  in their charter homes (see the reconciled M1 work-list). Only `child_worktree_path`
  (`wave/channel.rs`) still re-inlines the naming format.

Mechanisms to preserve while moving or cutting old organs:

- auth hardening: bearer parsing, query-token rejection, throttling, local
  capability token, owner-only token files
- webhook reliability: signature verification, plan-then-exec tests,
  dedupe-after-success, bounce replay
- push bridge mechanics: silent boot seed, fingerprint diffing, bounded scans,
  duplicate-tolerant UI updates
- deploy hygiene from container mode: env validation, named credential mounts,
  health checks, redaction, service-file secret hygiene
- process hygiene: tmux/session reconciliation, worktree janitor, graceful
  shutdown hooks, interrupt deadlines

## Decisions ratified 2026-07-06

- **Cross-repo model → wave = 1 repo [R].** `Wave { repos: [RepoWork] }` and
  `wave_repos` are being stripped; cross-repo is a future wave-tree, not a repo
  list. (See Ratified principles.)
- **Interrupt grace window → accept immediate kill [R].** The design promised
  cooperative→grace→kill; the code does immediate SIGKILL. Simpler is fine —
  this doc now matches reality. A grace stage is an M4-if-ever note, not work.
- **Offline waves → accept the bounce; fix the *workers*, not the transport
  [R].** A down/absent wave server should be a shrug — workers degrade, note,
  and keep going — not a source of concern. That's mostly an operating-
  instructions adaptation (`<lf:loopflow>` already says "note it and move on";
  agents don't yet act like it). The real goal is keeping the server *up*.
  Queue-for-offline-waves stays a named follow-up, not a commitment.
- **Conformance traces → hand-authored + a live smoke gate [R].** A live smoke
  test is what actually catches vendor protocol drift (codex hardcodes 0.142.5
  wire strings in `harness/codex.rs` — the drift point). Recorded real traces
  are heavier for less; skip them.

## Still open, needing Jack

- Transport-contingency as a narrow wave-listener-only claim: keep or drop.
- Channel vocabulary: how much should user-facing CLI expose channels directly?
- The Tier-4 shape questions (Current state) are the full review's agenda, not
  pre-decided here.
