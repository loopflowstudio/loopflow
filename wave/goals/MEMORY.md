# goals wave memory

Steers Loopflow toward persistent Goal-driven Waves: **writing a goal is a way
to compute.** GOAL.md is the authored charter, Linear the live roadmap, Concerto
the session surface. North metric (post-reframe): **waves that run consistently
— real work for a week straight — across both loopflow and Cadenza**, not a
one-off demo. Reference builds (CLI probe shipped, server + mobile remain) are
proof, not the goal.

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

## GOAL.md is now the wave charter (PR #843, this branch)

GOAL.md went from a frontmatter default-flow+metrics record to a **durable,
inward-facing charter re-injected every loop.** The anatomy:

- **## Objective** — one paragraph, the mind's judgment prior; a voice you could
  tell from any other wave's. **Exactly one objective per wave — a second forks a
  child wave**, it doesn't add a row.
- **## Measures**, split by lifecycle because "Key Result" jams three things
  together: **Key Results** (complete-this → retire when hit, graded for
  progress; OKR discipline — outcome not output, quantified target, ~3–5,
  ~70%=win) · **Quality** (hold-this → checked every loop for compliance) ·
  **Bounds** (never-exceed → checked for violation; the spend cap) · **Done
  means** (Scrum DoD, the durable bar). **Kill output/codename/dark-room
  metrics** ("iterations ≥ N" rewards busywork — delete, don't relabel).
- **## Cron** — scheduled recurring duties (mirrors `crons:` frontmatter).
- **## Process** — routing judgment: how to size a task, which flow when,
  decompose vs go direct. **This is what replaced `primary_flow`** — routing is
  now prose the mind reasons over, not a config knob that mechanically fires.

**The quality bar is one honest question, not a rubric** (a rubric scores
presence, an LLM ticks every box and stays mediocre): *"Do I have what I need to
know what work from me would be most impactful?"* Not a whole-body yes → fix the
charter before doing downstream work. It's ungameable (tests function, not
features) and subsumes the deleted "north" and "stop" clauses (those move to
LOOPFLOW.md if anywhere). Empirical failure modes and a **gold-set of exemplars**
are produced by phase A, not shipped as a canned list.

**The ladder:** P1 (this branch — anatomy + honest question + `primary_flow`
removal + all six waves retrofitted) → **A** retrofit/audit each wave to discover
empirical failure modes and produce the gold-set exemplars → **B** the
elicitation UX: a failed whole-body-yes sends the mind to *ask the human* (not
guess), with an opinionated groove that shapes raw intent into outcome-KRs. **B
is itself a KR of the goals wave** — the leading indicator of the
run-consistently metric.

## Shipped this wave

- **#796 (merged 2026-07-05) — the reactive server.** `lf wave <name>` = a
  sovereign per-wave server: persistent codex app-server mind (0.142.5,
  live-proven), append-only journal as runtime truth (thread/state/queue are
  folds; restart-safe), steer/interrupt with anti-wedge deadline, one-brain
  registration, `lf chat`/`lf memory` speech (attributed emissions; server
  holds MEMORY.md's pen), ambient context in every lf run. Replaced the
  loop_ticker/goal-loop brain; `lf goal` deleted.
- **#801 (merged 2026-07-05) — realignment + the lfd/lfq/lfdb collapse.**
  Brought the architecture-direction record; began the component reshaping.
- **#803 (open) — M1/M2 compression.** Dropped the Postgres backend (sqlite-only
  now), retired `lf q` for `--dispatch`/`--wave`, hoisted the conversation
  harness to top-level `harness` + `chat`, deleted the docker-compose lfd path,
  dropped the wave `mode` knob. ~3,400 lines removed.
- **#843 (open, this branch) — GOAL.md the charter.** The reframe above +
  `primary_flow` retired across all layers. Charters are the deliverable; the
  code migration is deliberately minimal.

## Foundational model (still true)

- **Two-file wave surface** — `wave/<name>/` is GOAL.md (intent) + MEMORY.md
  (this file); both injected into the wave loop each iteration.
- **Goal primitive** — `goal` is the third prompt primitive
  (step/flow/**goal**); the durable Wave carries `goal` (**`primary_flow`
  removed — #843**); `load_goal` resolves `.lf/goals/<name>.md`
  repo→home→builtin. Concrete flow names still live on `Run.flow` where work
  executes; `DEFAULT_WAVE_FLOW` only fills synthetic/default run rows.
- **Emission vocabulary is exec, one door** — `lf chat` / `lf memory` /
  `--dispatch`: the only door every process on the machine has. Speak locally,
  escalate deliberately (`--parent` walks store ancestry to the parent).
- **Wave ancestry** — parent/child is a wave tree (drop "chord"); reintroduced
  by #781 (merged). Nothing builds trees yet.

## Hard-won learnings (carry forward)

- **Schema-drift class:** the ontology collapse renamed tables/columns by
  editing historical CREATE migrations in place — fresh dbs fine, recorded dbs
  stranded. NEVER edit an applied migration; add a rename/drop migration + a
  convergence-tolerance list. Diff a fresh-migrated schema against the live one
  before whacking single moles.
- **#843 rebase gotcha (same class):** this branch removed `primary_flow`; main
  *independently* collapsed the wave model multi-repo→single-repo (migrations
  051/052). Every conflict was the intersection. Rule applied: **keep main's
  single-repo shape, drop `primary_flow`.** New `053_drop_wave_primary_flow.sql`
  runs after 052 (the column still exists to drop); added to
  `RENAME_CONVERGENCE_MIGRATIONS`. Column indices in `rows.rs::map_wave_row`
  shifted down one; catalog INSERT 17→16 params.
- **Vendor drift discipline:** conformance traces catch mapping bugs; only a
  live smoke catches protocol drift (codex 0.142.5: app-server subcommand,
  clientInfo required, client-sent `initialized`, `turn/start`, steer carries
  `expectedTurnId`) and process-tree bugs (nvm shim → grandchild binary →
  process-group kill; tmux kill-session sends SIGHUP, bypassing SIGINT-only
  cleanup hooks). Traces are still hand-authored — see follow-up #9.
- **Free-energy brief:** the design has the tradition's structure, not its
  dynamics; "unattended iterations" as a metric rewards the dark-room failure —
  pair it with a progress setpoint. Carried to M4. (The charter reframe already
  bans dark-room KRs.)

## Landed research (2026-07-03, for the record)

- **01 Live roadmap — shipped.** Loop reads the PM provider live each iteration;
  no local mirror; `--pr` link on `lf op pm update` (#780 merged). Provider is
  **Linear** (some old task titles still say "Asana" — cosmetic, they're done).
- **03 Wave ancestry — #781 merged.** `parent_wave_id` tree, `children_of()`
  query, DTO mirrored. Nothing constructs trees yet.
- **04 Wave spend budget — designed, deferred to M4.** Core hard floor
  (`spend_cap`, per-run cost accrual, at-cap pause) + userland policy below the
  ceiling. Needs a Money cents newtype.
- **02 Cloud backend — recommend A2 (lfd scaffolds; vendor owns the loop);
  deferred to M3.** Net-new piece is a `.mcp.json` Linear emitter.

## Next

- **Phase A (retrofit/audit):** run each wave through the honest question,
  discover empirical failure modes, and promote the best retrofits to the
  gold-set exemplars phase B teaches from. Filed on Linear (#843 item).
- **Finish the Swift flow-removal** — single-repo Wave, ~40 call sites; do NOT
  reapply stash@{0} (multi-repo-era). Filed on Linear.
- **Door auth hardening** (from `origin/lfd-exec` review) — shrink `/v0/exec`
  blast radius, token TTL + per-subagent minting, constant-time compare,
  peer-uid-primary. Ladders to M3. Filed on Linear.
- **Prove-the-language:** CLI probe shipped (#799 open); server + mobile
  reference builds remain.
- **Continue M1** charter extraction (`stream_events`, `find_repo_root`,
  worktree naming rule) and **M2** (finish cutting container mode).
