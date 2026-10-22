# LOO-526: Rent a machine from a pool and give it back

Status: draft design, 2026-10-21, written by the Task conversation for Jack
Heart's review before `lf task run`. Jack's brief (Task description) is the
problem statement; this doc chooses mechanisms. Unresolved choices are in
`scratch/questions.md`.

## Experience

```sh
doppler run -p etude -c prd -- \
  lf pool add code --provider runpod --cpu cpu3g --vcpus 8 --max 1 --max-usd-per-hour 0.40
lf machine use code --task LOO-570          # rents code-1, adds it, hands the Task over
lf task status LOO-570                      # Owner: Jack on code-1
lf task run LOO-570                         # runs on code-1 over lf --machine, as today
lf pool show code                           # code-1 · running · $0.32/h · 0.4h · $0.13 · LOO-570
# the Task's PR lands; its work drains
lf pool show code                           # code-1 · ended 11:42 · copied 3 Sessions, 14 Processes
lf session list --task LOO-570              # Sessions from code-1, kept on the laptop
```

By hand, outside any Task's lifetime:

```sh
lf pool rent code --hours 2                 # lasts until 2h or `lf pool end code-2`
lf pool end code-2
```

## What exists, and what Etude teaches

- A **Machine** is `durable::Machine { id, label, route, created_at, observed_at }`
  in the `machines` table; `lf machine add` probes `lf --version && lf machine id`
  over SSH and installs lf with the published installer when missing. SSH over
  `lf --machine` is the only transport. The repository must already exist on the
  remote and be bound to the same `repo_…` id (`lf repo identity --bind`).
- **Task ownership** (LOO-523) is an append-only log; `lf machine use <machine>
  --task X` hands a Task to a Machine, and `lf task run X` on the laptop forwards
  to the owner over SSH (`work_route::dispatch` → `ssh::run`), which allocates the
  checkout there. Logins arrive with `lf machine connect <m> <provider>` (LOO-413),
  approved once in Jack's Chrome per machine.
- **Upward copies are planned, not built**: no origin stamping on Process/Session
  rows, no export command, and `lf machine remove` forgets a worker's records.
- **Etude** (~/src/etude at 2b3fe66) separates Machine / Rental / RunAttempt,
  checks template, limit, availability, price ceiling and dollars before spending,
  writes the machine's identity before the create so an unknown reply keeps it,
  and puts safety on the worker: a standard-library guardian that deletes the pod
  at min(deadline, budget) regardless of what else fails. Its queue, controller
  loop, Jobs/Runs/Cohorts, S3 storage and program-archive bootstrap exist for
  training runs and are not copied.
- **LOO-554** rented RunPod CPU pods by hand on 2026-10-21: v1 REST `POST /pods`
  with image `runpod/base:1.0.2-ubuntu2404`, `ports: [22/tcp]`, `PUBLIC_KEY`,
  US data centers (direct SSH to EU-SE-1 timed out; proxy SSH is PTY-only and
  cannot carry `lf --machine`). 16/8/4-vCPU cpu3c/cpu5c/cpu3g had no capacity;
  a 2-vCPU/4 GB pod came up and OOM-killed `cargo build --release`. Key:
  Doppler `etude`/`prd` `RUNPOD_API_KEY`.

## Model

Three records, as in Etude, mapped onto what Loopflow already has.

| Record | Question it answers | Where |
|---|---|---|
| **Machine** | What is it and how do I reach it? | existing `machines` row; a rented one is an ordinary added Machine whose route is the pod's SSH destination |
| **Rental** | What paid window, at what price and cap, what has it cost, and is it still there? | new `rentals` table; `rent_<uuid>` ids, separate from `home_…` |
| **Allocation** | Which Task is being done on it? | existing Task ownership log: a `handoff` to the rented Machine |
| **Pool** | What do I rent, how many at once, at what ceiling? | new `pools` table |

```text
pools
  name TEXT PRIMARY KEY          -- "code"
  provider TEXT                  -- "runpod"
  shape TEXT                     -- JSON: {cpu: "cpu3g", vcpus: 8, disk_gb: 40, image: "...@sha256:..."}
  max_machines INTEGER
  max_usd_per_hour TEXT          -- decimal as text; above this no machine is taken
  hours REAL                     -- hard deadline for machines this pool makes (default 4)
  created_at INTEGER

rentals
  id TEXT PRIMARY KEY            -- rent_…
  pool TEXT REFERENCES pools     -- NULL once the pool is removed; history stays
  provider TEXT
  provider_name TEXT             -- the pod name we sent: "lf-<rental id>"; lookup key for an uncertain create
  provider_id TEXT               -- RunPod pod id once confirmed
  machine_id TEXT REFERENCES machines  -- once probed
  by_hand INTEGER                -- 1: lasts until deadline/explicit end; 0: ends when work drains
  usd_per_hour TEXT              -- quoted at admission, replaced by the pod's actual price
  cap_usd TEXT                   -- usd_per_hour × hours; the guardian's budget
  deadline INTEGER               -- hard deadline the guardian enforces
  state TEXT                     -- requested | unconfirmed | booting | running | ending | ended | lost
  created_at INTEGER
  ended_at INTEGER
  cost_usd TEXT                  -- whole billed hours from /v2/billing/pods, updated on each pass
  copied_at INTEGER              -- when the laptop last copied its records
```

A Machine's kind is derived: rented when a Rental names it, added otherwise.
Nothing is duplicated onto `machines`. One draft migration
(`scripts/new_migration.py rental_pools`).

## RunPod adapter (`rust/loopflow/src/rental/runpod.rs`)

Etude's `api.py`/`http.py` reduced to five calls against `https://api.runpod.io/v2`
(the v2 REST that RunPod's current docs name; LOO-554 used v1 and the skill's
migration table maps the fields):

| Call | Use |
|---|---|
| `GET catalog/cpus?include=AVAILABILITY&product=POD` | price per vCPU-hour and availability for the shape |
| `POST pods` with `cpu: {id, vcpuCount}` (never `gpu`), `disk`, `ports: ["22/tcp"]`, `env`, `dockerStartCmd` | create, sent once, never retried |
| `GET pods` (no server filters; match `name` client-side) | recover an uncertain create |
| `GET pods/{id}` | status, `publicIp`, `portMappings["22"]`, actual `costPerHr` |
| `DELETE pods/{id}` after re-reading name + createdAt | give back exactly this pod |
| `GET billing/pods?podId=…&bucketSize=hour` | whole billed hours (`totalAmount`) |

Replies are narrowed to checked fields; nothing is logged whole. `vcpuCount` must
be a power of two within the flavor's range (catalog says). `mounts` is never
sent (CPU pods refuse persistent mounts). The reqwest client the crate already
uses carries `User-Agent: loopflow` (RunPod 403s default agents).

Key: `RUNPOD_API_KEY` from the environment of the lf process that rents,
i.e. `doppler run -p etude -c prd -- lf pool …`. The pod never receives it; it
uses the pod-scoped key RunPod injects as `RUNPOD_API_KEY` inside the container
(Etude's `pod_key()`), which can only delete the pod itself. The first ~$0.10
probe verifies this on a CPU pod.

## Admission: check before spending

`lf pool rent` and `lf machine use <pool> --task` share one path:

1. A free pooled Machine (running Rental in this pool owning no unfinished Task,
   no live Processes) is used first; nothing is rented.
2. Otherwise: live Rentals in the pool `< max_machines`, else refuse naming the
   limit. `lf pool max code 0` keeps existing machines and permits only `rent`.
3. Catalog: flavor available, `price × vcpus ≤ max_usd_per_hour`, else refuse
   with the price seen. `cap_usd = price × vcpus × hours`; `--max-usd` lowers it.
4. Write the Rental as `requested` with `provider_name = lf-<id>` **before** the
   POST. A 2xx moves it to `booting` with `provider_id`; a timeout or unreadable
   reply moves it to `unconfirmed`. Every later pass resolves `unconfirmed` by
   listing pods and matching the name: found → adopt; absent after the create's
   own deadline → `lost`. The create is never resent.

## The pod

The template's start command replaces the image's start script, so it starts
what the machine needs, in this order, as Etude does:

1. **Guardian** (`rental/guardian.py`, Python standard library only, embedded with
   `include_str!` and passed base64 in `LF_GUARDIAN`): writes a ready marker,
   shortens the deadline to the budget using the pod's own key and `costPerHr`,
   sleeps on both wall-clock and monotonic time, then `DELETE pods/{self}`
   forever until it succeeds. Nothing downstream starts until the marker exists.
2. **sshd** with the account's `PUBLIC_KEY`, key-only, root login prohibited;
   a non-root `lf` user owns the work (LOO-554 found root hides Linux bugs).
3. **Tools**: git, gh, Node (Claude Code), the published lf via `install.sh`.
   Everything is apt/npm/curl; the image stays stock `runpod/base:1.0.2-ubuntu2404`
   pinned by digest at pool add.
4. `sleep infinity`. The laptop drives everything after this over SSH.

Before this lands, the demo pod runs this branch's Linux build (Docker
`linux/amd64`, `scp`, `mv` over `.new`) instead of step 3's installer, as LOO-554
did. After landing the installer is enough.

Laptop side, after `booting`: poll `GET pods/{id}` until `publicIp` and the port
mapping for 22 exist, wait for SSH, then the ordinary path: `lf machine add
lf@IP:PORT --label code-1` (probe + identity), `lf --machine code-1` clone of
the repository's origin and `lf repo identity --bind <repo_id>`, then
`lf machine connect code-1 github` and `… claude` (Jack approves each in Chrome;
`-b` reports which connects are still needed instead of waiting). The Rental is
`running` once `machine_id` is set. Then the handoff, exactly as to any Machine.

`lf task run` then forwards to the owner as it does today. Nothing in Task
execution learns about rentals.

## Giving back

**Drain** (pool-made machines): the rented Machine owns no uncompleted Task and
`lf --machine code-1 ps --json` shows no live Process. **Deadline** (by-hand
machines, and every machine's hard stop): `rentals.deadline`. **Explicit**:
`lf pool end code-1`.

The give-back pass runs, in order:

1. Refuse while the pod holds the only copy of development work: any Task
   worktree on it with uncommitted changes or commits not on its upstream
   (`git status --porcelain`, `git rev-list @{u}..HEAD` over SSH). Report what
   is unpushed and leave the machine running. `lf pool end --force` is the only
   override and names what it discards.
2. **Copy up**: `lf --machine code-1 history export --since <rental start>`
   emits the Machine's Sessions, Processes, FlowProcesses, session events and
   capture payloads for the window as JSON lines plus files; the laptop imports
   them as read-only rows carrying `origin_machine_id` and `observed_at`
   (new columns on those tables, NULL for rows this Machine wrote itself).
   Copies never gain authority while the origin exists; after the pod is deleted
   they are the final record. `lf session list`, `lf history show` and Task
   status read them like local rows and say "from code-1".
3. `DELETE pods/{id}` with Etude's identity re-check; `state = ended`,
   `ended_at`, final `cost_usd`. The Machine row stays (ownership history reads
   "Jack on code-1"); `lf machine status` shows "rental ended" instead of
   unreachable. `lf machine remove code-1` is allowed afterwards and loses
   nothing, because the copies exist.

**Who runs the pass.** There is no daemon. The pass runs at the end of a
forwarded `lf task run` (the laptop already waits for it), in every `lf pool`
command, and in the installed periodic Task check (`lf task automation`), which
gains this as one more read. A soft deadline 15 minutes before the hard one
turns the pass from "when drained" into "now": copy up and delete. The guardian
is the backstop for a laptop that never comes back; what it deletes at the hard
deadline has either been copied on an earlier pass or was unreachable for the
whole window. Pushing from the pod to the laptop needs a route into the laptop
that does not exist today (questions.md, Q1).

## Commands

```text
lf pool add <name> --provider runpod --cpu <flavor> --vcpus N [--disk-gb 40] [--hours 4]
                   --max N --max-usd-per-hour USD
lf pool list                      pools, live machines, spend this month
lf pool show <name>               machines, rentals (state, price, cost, deadline, Task), last pass
lf pool max <name> N              change the limit; current windows stay
lf pool rent <name> [--hours H] [--max-usd USD]      a machine by hand
lf pool end <machine> [--force]   give one back now
lf pool remove <name>             forget the pool; its history stays
lf machine use <pool> --task X    a free pooled machine or a new rental, then the handoff
lf history export [--since]       the export the copy reads (also useful alone)
```

`lf machine use` resolves its target as Machine label, hostname, Machine id, and
now pool name; the pool branch is the only new behavior. Setting a Wave's or
repository's default to a pool (so new Tasks rent automatically) is the natural
next step and a separate Task.

## Deleting what this replaces

Nothing in the repository rents machines today; `website/home.html` mentions
RunPod only as prose. LOO-554's hand procedure was never checked in. Etude's
code is copied, never shared as a package.

## Verification

- Rust unit tests: admission arithmetic and refusals; Rental state machine
  including uncertain create → adopt/lost by name; drain judgment; refuse-delete
  with unpushed work; RunPod adapter against a closure-stubbed HTTP send
  (envelopes, power-of-two vCPU, identity re-check on delete, billing hours).
- Guardian: `python/tests/test_guardian.py` imports `rental/guardian.py` by path;
  Etude's cases: deadline, budget shortening, backwards clock, delete retry.
- Export/import round trip in a two-store fixture: origin stamping, no authority
  for copies, final record after the origin Machine is gone.
- Live probe, ~$0.10, before pursue finishes: a 2-vCPU cpu3g pod boots the
  guardian, sshd and lf; `lf machine add` succeeds; the pod-scoped key deletes
  the pod at a 10-minute deadline. Then the demo as Jack wrote it, at ≤ $0.40.

## Demo

Jack's, from the brief: `lf pool add code --provider runpod --cpu cpu3g --vcpus 8
--max 1 --max-usd-per-hour 0.40`, run a Task in that pool, watch the machine
appear, the Task read "Jack on code-1", the PR push, and the machine get deleted
with its Session history retained on the laptop. Capacity note: 8-vCPU cpu3g had
none on 2026-10-21; the demo may need `--vcpus 4` or `cpu3c`.

## Sequence

1. Draft migration, `rental` module: records, pool admission, Rental states,
   RunPod adapter with stubbed tests. `lf pool add/list/show/max/remove/rent/end`.
2. Guardian and pod start command; live probe.
3. `lf machine use <pool> --task`; provisioning (add, clone+bind, connect).
4. `lf history export`, import with origin stamping, give-back pass and its
   three callers.
5. Docs: `docs/architecture/machines.md` rentals section, `lf-reference.md`
   regeneration, README machine section. Demo.
