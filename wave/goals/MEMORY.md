# goals wave memory

Steers Loopflow toward persistent Goal-driven Waves: goals as the authored loop
prompt, Asana as the live roadmap, Concerto as the session surface. Standing
campaign: writing goals becomes a way to compute — three reference builds from
goals (mobile, CLI, server) with zero step authoring.

## Where the architecture is going (2026-07-05)

The design conversation's durable record lives in two files beside this one:
`wave/goals/architecture-direction.md` (target anatomy, ratified principles
C1–C7, component charter, M0→M4 roadmap, M1 conversion work-list) and
`wave/goals/wave-agent-follow-ups.md` (10 open decision questions, accepted
risks, a reading list for the merged code). Read those for detail. The spine:

- **Waves outward [R] (Jack):** zero centralized control; the wave is the unit
  of sovereignty; nothing sits above the waves. Within a wave the listener is
  deliberately a local center (sovereignty is fractal). Test every feature:
  "does this create a center?"
- **Four roles, distance the only difference:** `lf` the hands (daemonless,
  does the work) · `lfd` the face (listens/relays/execs `lf`, never a hand —
  no git/tmux/vendors/pens) · `lfq` the future remote reach (M3) · the resident
  mind (`lf wave <name>`, owns the vendor harness).
- **Two write-tiers + a query plane:** append-only JSONL journals
  (one-pen-by-hierarchy, narrative order) + sqlite/lfdb (many-writers,
  operational facts) + one canonical fold/query module linked locally, served
  by lfd, proxied by future lfq. A file, not a center.
- **M0→M4:** M0 the demo holds (shipped) → M1 the component charter enforced
  (harness/engine/dispatch extraction, `--dispatch` grammar) → M2 substrate
  (postgres + dual-backend deleted, container mode cut, sqlite narrowed) → M3
  faced (auth/identity, lfq proxy, remote Concerto) → M4 alive (free-energy
  dynamics, sovereign spend cap).

## Shipped this wave

- **#796 (merged 2026-07-05) — the reactive server.** `lf wave <name>` = a
  sovereign per-wave server: persistent codex app-server mind (0.142.5,
  live-proven), append-only journal as runtime truth (thread/state/queue are
  folds; restart-safe), steer/interrupt with anti-wedge deadline, one-brain
  registration, `lf chat`/`lf memory` speech (attributed emissions; server
  holds MEMORY.md's pen), ambient context (`<lf:wave-chat-recent>` +
  `<lf:wave-memory>`) in every lf run. Replaced the loop_ticker/goal-loop
  brain; `lf goal` deleted.
- **#801 (merged 2026-07-05) — realignment + the lfd/lfq/lfdb collapse.**
  Brought the architecture-direction record; began the component reshaping.
- **#803 (OPEN, this branch) — M1/M2 compression.** Dropped the Postgres
  backend (lfdb is sqlite-only now; ~1.6k lines + the
  tokio-postgres/deadpool tree gone), retired `lf q` in favor of
  `--dispatch`/`--wave` on the flow commands, hoisted the conversation harness
  out of `lfd::conversations` into top-level `harness` + `chat` (vendor
  adapters no longer depend on the daemon), deleted the docker-compose lfd path
  and folded the OS ServiceManager, dropped the wave `mode` knob from GOAL.md.
  ~3,400 lines removed. Closes several #801 staging-debt items (harness →
  top-level, postgres gone, `wave_config` shim dropped) and decision-question
  #1 (one dispatch door — `lf q` retired).

## Foundational model (still true)

- **Two-file wave surface** — `wave/<name>/` is GOAL.md (intent) + MEMORY.md
  (this file); both injected into the wave loop each iteration.
- **Goal primitive** — `goal` is the third prompt primitive
  (step/flow/**goal**); the durable Wave carries `goal` + `primary_flow`;
  `load_goal` resolves `.lf/goals/<name>.md` repo→home→builtin.
- **Emission vocabulary is exec, one door** — `lf chat` / `lf memory` /
  `--dispatch`: the only door every process on the machine has; worker reports
  ride it. Speak locally, escalate deliberately (`--parent` walks store
  ancestry to the parent's endpoint).
- **Wave ancestry** — parent/child is a wave tree (drop "chord"); reintroduced
  by #781 (merged). Nothing builds trees yet.

## Hard-won learnings (carry forward)

- **Schema-drift class:** the ontology collapse renamed tables/columns by
  editing historical CREATE migrations in place — fresh dbs fine, recorded dbs
  stranded. NEVER edit an applied migration; add a rename migration + a
  convergence-tolerance list. Diff a fresh-migrated schema against the live one
  before whacking single moles.
- **Vendor drift discipline:** conformance traces catch mapping bugs; only a
  live smoke catches protocol drift (codex 0.142.5: app-server subcommand,
  clientInfo required, client-sent `initialized`, `turn/start`, steer carries
  `expectedTurnId`) and process-tree bugs (nvm shim → grandchild binary →
  process-group kill; tmux kill-session sends SIGHUP, bypassing SIGINT-only
  cleanup hooks). Traces are still hand-authored — see follow-up #9.
- **Free-energy brief:** the design has the tradition's structure, not its
  dynamics; "unattended iterations" as a metric rewards the dark-room failure —
  pair it with a progress setpoint. Carried to M4.

## Landed research (2026-07-03, for the record)

- **01 Asana live roadmap — shipped.** Loop reads Asana live each iteration; no
  local mirror; `--pr` link on `lf op pm update` (#780 merged).
- **03 Wave ancestry — #781 merged.** `parent_wave_id` tree, `children_of()`
  query, DTO mirrored. Nothing constructs trees yet.
- **04 Wave spend budget — designed, deferred to M4.** Core hard floor
  (`spend_cap`, per-run cost accrual, at-cap pause) + userland policy below the
  ceiling. Needs a Money cents newtype.
- **02 Cloud backend — recommend A2 (lfd scaffolds; vendor owns the loop);
  deferred to M3.** Claude Routines have no create API; Codex has no
  server-side schedule. Net-new piece is a `.mcp.json` Asana emitter.

## Next

- **Reconcile the roadmap** (blocked 2026-07-05: Asana token expired): file the
  M1 conversion work-list and the 10 decision questions from
  wave-agent-follow-ups.md into Asana; close #780/#781/#796/#801 tasks; file
  #803.
- **Prove-the-language:** CLI probe shipped (#799 open — GOAL.md-only Rust CLI
  built and gated `hello, Loopflow` with 0 authored `.lf/steps`); server and
  mobile reference builds remain.
- **Continue M1** charter extraction (`stream_events`, `find_repo_root`,
  worktree naming rule) and **M2** (finish cutting container mode).
