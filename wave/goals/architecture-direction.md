# Architecture direction (2026-07-05)

The technical architecture the wave-agent work is converging toward, and the
roadmap to it. Durable record of the design conversation whose working notes
lived in `scratch/{components,collapse,realign,post-m2}.md` (wiped on land).
Decisions marked **[R]** are Jack-ratified; **[~]** are proposed/open.

## The anatomy

Four roles, one verb surface, distance as the only difference:

- **`lf` — the hands.** Does the work, on the filesystem. Every capability
  has a daemonless path.
- **`lfd` — the ear-and-voice (the face), never a hand [R].** Four quadrants:
  listens inward (scan/index/bridge over files + sqlite), listens outward
  (webhooks, remote clients at the door), speaks outward (read routes, `/ws`
  push, event relay), speaks inward (**exec `lf`** — attributed speech
  through the same public doors as anyone: webhook → `lf chat --from
  github`, remote argv → exec under client identity). Hand-ban: no pens, no
  git, no tmux, no vendors.
- **`lfq` — the reach.** `lf`-through-HTTP: mirrors `lf`'s argv to a
  gatekeeper, executing under CLIENT identity. One exec door, no per-verb API
  to drift.
- **The resident — the mind.** `lf wave <name> --mind-only`, spawned by its
  listener; owns the vendor harness; its input is its own subscription.
- **The filesystem — the body.**

## The substrate: two write-tiers + a query plane

- **Journals (filestore) [R]:** one-pen-by-hierarchy, rotated. The narrative
  substrate — primarily leaf workers' streams. One-pen because *narrative
  needs order*.
- **sqlite / lfdb [R]:** the many-writers tier — parallel WAL writes from
  every `lf` on the machine. The operational substrate:
  runs/sessions registry, `run_events` ledger, repos root-list, tokens.
  Many-pens because *operations need concurrency*. **A file, not a center**
  — engines ≠ centers; postgres (a server, shared-state coordination) dies,
  sqlite stays. (Correction of the earlier "both engines gone," which
  conflated engines with centers.)
- **The query plane [~ — needs Jack]:** "all queries go through a
  centralized query system." Two readings: **(a)** one query *system* (a
  canonical fold/query module) with three doors — `lf` links it locally,
  `lfd` serves it to viewers/remote, `lfq` proxies it — centralized *code*,
  not a centralized *process* (route-around survives); **(b)** all queries
  literally through the daemon (reads become a service; repeals route-around
  for reads). Claude's read: (a). **Unconfirmed.**

Not in any store: wave identity (GOAL.md/MEMORY.md markdown), conversation
(journals), queue truth (git/gh), vendor threads.

## Ratified principles

- **Waves outward [R]:** zero centralized control; the wave is the unit of
  sovereignty; nothing sits above the waves; the ban is on centers *between*
  waves — *within* a wave the listener is deliberately a local center.
- **One pen, by hierarchy [R]:** the nearest running listener holds the pens
  for its channel family; pens follow the tree. Why speech routes through
  servers, not direct appends.
- **The interjection dial [~ open, held]:** bottom row is
  hearable-never-interrupting today (attributed speech queues at the
  boundary; only unattributed human speech steers). Urgency-gated
  interjection (surprise-weighted escalation) lands with M4; don't build it
  before the wave can rank urgency.
- **Channels [R]:** a channel is a named stream (journal + thread +
  subscribability); every wave has one, every work line gets one (ownership
  name = channel name); names are topics, dots are the tree, subscription by
  prefix; the wave tree is the subscription topology. Wave stays an identity;
  promotion (a work line grows a GOAL) is the vocabulary.
- **`lf wave` spawns the mind [R];** the resident's own command is a role
  flag (`--mind-only` / `--no-mind`), not a `mind` noun.

## The waves-outward claims (C1–C7)

Falsifiable engineering translations of the philosophy. C1–C7 proposed by
Claude; **C2 amended by Jack**.

1. **C1 — Route-around [R]:** every local capability works with zero
   daemons; a daemon may accelerate/aggregate, never enable.
2. **C2 — Coordination is facts + speech, never command [R, amended]:**
   processes observe shared state and hear messages; no component RPCs
   another into acting. **But there IS an algedonic channel — reserved
   mostly for humans, and maybe some of the mind** (Jack). So: command is
   banned between peers; a narrow, precision-weighted escalation path exists
   for humans (and select mind use), not for the general bottom row.
3. **C3 — All execution is `lf` invocations [R]:** only listeners and
   residents are long-lived; everything that does work is an `lf` run
   someone started. *Tension:* pure speech-translation of triggers gave the
   CI-swallow / lost-main-moved regressions; the honest fix is durable speech
   (the write-queue), which makes the write-queue load-bearing, not optional.
4. **C4 — Aggregation is derived and disposable** [~ Claude, vibe-ratified].
5. **C5 — Authority flows from clients through the middle, never from the
   middle [R]:** remote exec under caller identity; needs a real identity
   model (review showed identity-by-absence already leaks steer privilege).
6. **C6 — Hierarchy is subscription, not supervision** [~ — the
   "never command" edge is Claude's; gates-on-starting vs command-over-
   running is the line to watch].
7. **C7 — Sovereignty is fractal** [~ Claude's framing of the ratified
   within-wave-center point].

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
  lines); sqlite narrowed to the operational scratchpad; run lifecycle and
  conversation on their proper tiers; the query plane. *Exit: `grep -r
  rusqlite::` in wave/ returns nothing; postgres gone.*
- **M3 — Faced:** exec-under-client-identity door; `lfq` the proxy;
  `loopflow` the python viewer library; remote Concerto via relay;
  federation. *Exit: drive a Mac-Mini wave from the laptop, same verbs, own
  identity.*
- **M4 — Alive:** predictions/setpoints/precision (free-energy adopt-nows);
  sovereign spend cap; Decisions/HITL; the real identity model + the
  interjection dial's notch 3. *Exit: the wave notices its own surprises.*

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
  + boot hygiene. Never git/tmux/vendors/pens.
- **`lfdb`:** the machine scratchpad (see substrate).
- **`ops`:** git/GitHub/PM verbs (`lf op` stays — the local deterministic
  sibling of prompted work: `skill | flow | op | :`). Exec'd by the gate,
  never called in-process by route handlers.

## Open, needing Jack

- The query plane: (a) one system three doors, vs (b) reads-as-a-service.
- Concerto's local fleet reads: via the query plane (Claude's lean) — depends
  on the above.
- C4/C6/C7 (Claude coinages) — ratify or reshape.
- Transport-contingency as a narrow wave-listener-only claim: keep or drop.
