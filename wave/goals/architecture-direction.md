# Architecture direction (2026-07-05)

The technical architecture the wave-agent work is converging toward, and the
roadmap to it. Durable record of the design conversation whose working notes
lived in `scratch/{components,collapse,realign,post-m2}.md` (wiped on land).
Decisions marked **[R]** are Jack-ratified; **[~]** are proposed/open. This
document records the target direction; PR #801 still carries explicit staging
debt where called out below.

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

## The waves-outward claims (C1–C7)

Falsifiable engineering translations of the philosophy. C1–C7 started as
Claude coinages and were reshaped in the July 5 design walk.

1. **C1 — Route-around [R]:** every local capability works with zero
   daemons; a daemon may accelerate/aggregate, never enable.
2. **C2 — Coordination uses durable facts and explicit commands [R]:**
   speech is a worker-output/reporting primitive, not the universal bus.
   Coordination state should be inspectable: sqlite for operational facts,
   journals for conversation, git/GitHub for PR truth, Asana for roadmap truth,
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

- **M0 — True (this branch):** the fix wave lands; the demo claims hold
  again (worker reports reach the mind, no wedges, no message loss); PR
  ships. *Exit: the live two-process demo runs clean.*
- **M1 — Shaped (the conversion):** the component charter enforced — harness
  → `crate::harness`; engine consolidation (config/naming/util,
  the cycle-breaks); dispatch extracted from the executor's corpse;
  gatekeeper sheds in-process mutations; grammar (`skill | flow | op | :` +
  `--dispatch`); `step`→`skill` sweep. One worktree per move-set. *Exit:
  cargo dependency direction matches the charter — commands import
  components, never the reverse; `crate::wave` extracts cleanly.*
- **M2 — Substrate:** postgres + dual-backend machinery deleted (~2.5k
  lines); container mode cut as a product/deployment shape; sqlite narrowed to
  the operational scratchpad; run lifecycle and conversation on their proper
  tiers; the query plane. *Exit: `grep -r rusqlite::` in wave/ returns
  nothing; postgres and `mode: container` gone.*
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

## The M1 conversion work-list (from the 2026-07-05 review's confirmed cycles)

1. `stream_events` (SSE client): `lf::commands::sub` → `wave`.
2. `find_repo_root`: `lf::commands::util` → `engine`.
3. `wave_config` / `read_wave_config`: `lfd::http::routes` → `engine`.
4. Worktree/naming: `channel.rs`'s private path math + executor's
   `run_worktree_path` → one rule in `engine/worktrees` (fixes the
   4×-authored divergence).
5. `ensure_wave_worktree` + placement helpers: `lfd::executor` → `dispatch`.
6. `process_alive` / tmux probes: one home; both `wave` and `dispatch` call.
7. Primary-channel predicate: one function (name-equality vs dot-absence
   unified), used by the listener and `wave_context`.
8. `lfd`'s in-process mutations (`/land`, `/next`, `/combine`, `/stop`,
   rename) → exec `lf` under client authority.

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

This branch is M0, not the fully shaped architecture. Known mismatches with the
target above:

- `lfd` still has remote-bind/token scaffolding. Target: local capability token
  through M2, with real remote identity/auth in M3.
- Container mode still exists. Target: cut it in M2 along with postgres. Preserve
  useful mechanisms from the container work (env hardening, named credential
  mounts, health checks, redaction, service-file hygiene) by moving them to the
  future deploy/SSH story if they still apply.
- `lfd` still has hand routes (`/land`, `/next`, `/combine`, `/stop`, rename)
  that call ops or tmux in-process. Target: exec `lf` argv, then remove the
  private hands.
- `stream_events`, `find_repo_root`, `wave_config`, worktree naming, and
  placement helpers still sit in pre-charter homes. M1 owns these moves.
- `harness` still lives under `lfd::conversations`; M1 moves it to
  `crate::harness`.
- Postgres remains in `lfdb`; M2 deletes postgres and narrows sqlite to the
  operational scratchpad.

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

## Open, needing Jack

- Transport-contingency as a narrow wave-listener-only claim: keep or drop.
- Channel vocabulary: how much should user-facing CLI expose channels directly?
- Cross-repo model: wave tree of single-repo children, `Wave { repos:
  [RepoWork] }`, or both as orthogonal axes?
