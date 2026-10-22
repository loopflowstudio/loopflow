# LOO-526: Rent a machine from a pool and give it back

Status: implemented except the live probe and demo (2026-10-21); design revised after Jack Heart's review in the Task conversation. Jack's brief (Task description) is the problem statement; this
doc chooses mechanisms. Decided in conversation: records are pulled by the
laptop, not pushed (Q1); routing lives on `lf task run --pool | --rent` (Q2;
`--machine` already exists as the root forwarding flag); vocabulary follows Etude's 2026-10-21 split (#23, #19) of
**pool** (machines you own) from **on-demand** (a template and a budget).
Remaining choices are in `scratch/questions.md`.

## Experience

```sh
lf pool add lab                                        # machines you own
lf machine add jack@mini.local --pool lab              # cost $0, never expires
doppler run -p etude -c prd -- \
  lf ondemand add code --provider runpod --vcpus 8 --memory-gb 16 --max 1 --max-usd-per-hour 0.40
lf ondemand availability code        # cpu3c×8 · $0.24/h ≤ $0.40 · 0/1 in use · observed 20:51
lf task run LOO-570 --rent code --quote     # would rent cpu3c×8 at $0.24/h, ≤ $0.96 for 4h; runs nothing
lf task run LOO-570 --pool lab --rent code  # a free lab machine first, else rent code-1, then run there
lf task status LOO-570                      # Owner: Jack on code-1
lf ondemand list                            # code · cpu3c×8 · 1/1 · code-1 running · $0.24/h · $0.13 · LOO-570
# the Task's PR lands; its work drains
lf ondemand list                            # code · 0/1 · code-1 ended 11:42 · 3 Sessions, 14 Processes copied
lf session list --task LOO-570              # Sessions from code-1, kept on the laptop
```

By hand, outside any Task's lifetime:

```sh
lf ondemand rent code --hours 2 [--max-usd 0.50]   # lasts until 2h, the cap, or `lf machine end`
lf machine end code-2                              # give it back now
```

## What exists, and what Etude teaches

- A **Machine** is `durable::Machine { id, label, route, created_at, observed_at }`
  in `machines`; `lf machine add` probes `lf --version && lf machine id` over SSH
  and installs lf with the published installer when missing. SSH over
  `lf --machine` is the only transport. The repository must exist on the remote,
  bound to the same `repo_…` id (`lf repo identity --bind`).
- **Task ownership** (LOO-523) is an append-only log; `lf machine use <machine>
  --task X` hands a Task to a Machine, and `lf task run X` forwards to the owner
  over SSH (`work_route::dispatch` → `ssh::run`), which allocates the checkout
  there. Logins arrive with `lf machine connect <m> <provider>` (LOO-413),
  approved once in Jack's Chrome per machine.
- **Before this Task, no upward copies**: no export command, and `lf machine
  remove` forgot a worker's records. Copy-up below adds both sides.
- **Etude at HEAD (5c18aca)**. #23: a *pool* is only machines you own, each with
  a committed cost and an expiry, and it knows no provider; an *on-demand* is a
  `MachineTemplate` (provider, gpu, vcpus, memory_gb, disk_gb, image,
  data_center) plus a budget (`max_machines`, `max_usd_per_hour`). They share one
  namespace. A launch names `--pool P`, `--rent R` or both; both uses a free owned
  machine before renting. `availability --rent R` asks the provider what it would
  rent now; `launch --quote` prints the first wave's machines and maximum spend
  and rents nothing. A Machine names its pool or its on-demand, exactly one.
  #19: a template with no GPU quotes RunPod's CPU catalog for the cheapest
  in-stock flavor at the smallest power-of-two vCPU count covering `vcpus` and
  `memory_gb`; the Quote carries the hardware, create asks for exactly that, and
  the receipt records it. Verified live on two cpu3c×8 pods at $0.24/h.
  Unchanged since 2b3fe66: admission writes identity before the create so an
  unknown reply keeps it, and the worker-side guardian deletes the pod at
  min(deadline, budget) regardless of what else fails. Etude's queue, controller
  loop, Jobs/Runs/Cohorts, S3 storage and program-archive bootstrap serve
  training runs and are not copied.
- **LOO-554** rented RunPod CPU pods by hand on 2026-10-21 with v1 REST, image
  `runpod/base:1.0.2-ubuntu2404`, `ports: [22/tcp]`, `PUBLIC_KEY`, US data
  centers (direct SSH to EU-SE-1 timed out; proxy SSH is PTY-only and cannot
  carry `lf --machine`). 4 GB OOM-kills `cargo build --release`. Key: Doppler
  `etude`/`prd` `RUNPOD_API_KEY`.

## Model

| Record | Question it answers | Where |
|---|---|---|
| **Pool** | Which machines do I own, and do they take new work? | new `pools` table |
| **OnDemand** | What do I rent, how many at once, at what most per hour? | new `ondemands` table |
| **Machine** | What is it and how do I reach it; owned (cost, expiry, pool) or rented (its on-demand)? | existing `machines` row + `pool`, `cost_usd`, `expires_at` |
| **Rental** | What paid window, at what price, hardware and cap, what has it cost, is it still there? | new `rentals` table; `rent_<uuid>` ids, separate from `home_…` |
| **Allocation** | Which Task is being done on it? | existing ownership log: a `handoff` to the Machine |

```text
pools
  name TEXT PRIMARY KEY           -- "lab"
  off INTEGER                     -- takes no new work; what it holds stays
  created_at INTEGER

ondemands
  name TEXT PRIMARY KEY           -- "code"; pools and ondemands share one namespace (checked on add)
  provider TEXT                   -- "runpod"
  template TEXT                   -- JSON: {gpu, vcpus, memory_gb, disk_gb, image, data_center}
  max_machines INTEGER            -- rented at once
  max_usd_per_hour TEXT           -- decimal as text; above this nothing is rented
  hours REAL                      -- hard deadline for machines it rents (default 4)
  created_at INTEGER

machines (+)
  pool TEXT REFERENCES pools      -- an owned machine's pool; NULL for rented or unpooled
  cost_usd TEXT                   -- committed when added, default 0
  expires_at INTEGER              -- NULL: never; past it the machine takes no new work

rentals
  id TEXT PRIMARY KEY             -- rent_…
  ondemand TEXT                   -- the on-demand it came from (kept after removal)
  provider TEXT
  provider_name TEXT              -- pod name sent: "lf-<rental id>"; lookup key for an uncertain create
  provider_id TEXT                -- pod id once confirmed
  machine_id TEXT REFERENCES machines   -- once probed
  by_hand INTEGER                 -- 1: lasts until deadline/cap/explicit end; 0: ends when work drains
  hardware TEXT                   -- JSON: {cpu: "cpu3c", vcpus: 8} or {gpu, min_vcpus, min_memory_gb}; what was priced and asked for
  usd_per_hour TEXT               -- quoted at admission, replaced by the pod's actual price
  cap_usd TEXT                    -- usd_per_hour × hours, or --max-usd; the guardian's budget
  deadline INTEGER                -- hard deadline the guardian enforces
  state TEXT                      -- requested | unconfirmed | booting | running | ending | ended | lost
  created_at INTEGER
  ended_at INTEGER
  cost_usd TEXT                   -- whole billed hours from /v2/billing/pods, updated each pass
```

Copy-up adds a `machine_records` table (origin, kind, key, row JSON,
observed_at) in the same draft. Each copy re-exports from the rental's start
and upserts; when it last ran is `observed_at`, so `rentals` keeps no watermark.

A Machine is rented when a Rental names it, owned otherwise; `pool` and a
Rental never both apply. One draft migration (`scripts/new_migration.py
pools_and_rentals`). An owned machine past `expires_at` leaves placement;
Loopflow never deletes it and Tasks already owned there stay owned (Etude's
`machine_lost` retry has no Loopflow equivalent and is not needed).

## RunPod adapter (`rust/loopflow/src/rental/runpod.rs`)

Etude's `api.py`/`http.py`/`machines.py` reduced, against `https://api.runpod.io/v2`:

| Call | Use |
|---|---|
| `GET catalog/cpus?include=AVAILABILITY&product=POD&vcpuCount=N` | Etude's `_cpu_quote`: smallest power of two ≥ vcpus, grown until `size × ramGbPerVcpu ≥ memory_gb`, price `securePerVcpu × size`, cheapest in-stock flavor; a `gpu` template uses `catalog/gpus` |
| `POST pods` with `cpu: {id, vcpuCount}` or `gpu: {...}` (never both), `disk`, `ports: ["22/tcp"]`, `env`, start command | create, sent once, never retried; result must carry our name, a positive price and a live status |
| `GET pods` (no server filters; match `name` client-side) | recover an uncertain create |
| `GET pods/{id}` | status, `ssh.direct` (host, published port for 22; `runtime.ports` carries the same), actual `cost` |
| `DELETE pods/{id}` after re-reading name + createdAt | give back exactly this pod |
| `GET billing/pods?podId=…&bucketSize=hour` | contiguous whole billed hours (`totalAmount`) |

Replies are narrowed to checked fields; nothing is logged whole. `mounts` is
never sent (CPU pods refuse persistent mounts). `User-Agent: loopflow` (RunPod
403s default agents). Disk defaults to 40 GB or 10 GB per vCPU when smaller,
as Etude does.

Key: `RUNPOD_API_KEY` from the environment of the lf process that rents
(`doppler run -p etude -c prd -- lf …`). The pod never receives it; it uses the
pod-scoped key RunPod injects as `RUNPOD_API_KEY` inside the container (Etude's
`pod_key()`), which can only delete the pod itself. The first ~$0.10 probe
verifies this on a CPU pod.

## Routing and admission

```text
lf task run X [--pool P] [--rent R] [--quote]
```

- A named Machine needs no new flag: `lf machine use M --task X` hands off, and
  the root `lf --machine M` forwards a command.
- `--pool P`: a free machine in P (owns no unfinished Task, not expired, pool
  not off), else refuse naming the busy machines. Live Processes are not
  probed over SSH; ownership alone decides. Loopflow has
  no queue; waiting is the person's choice.
- `--rent R`: admission, then rent, then hand off and run.
- `--pool P --rent R`: a free owned machine first, else rent. Etude's rule.
- `--quote`: print what it would take and the most it could cost; run nothing,
  rent nothing. Without capacity it fails as the run would.
- None of these: today's behavior, the owner or the Wave/repository Machine
  default. A Wave or repository default naming a pool and an on-demand (Etude's
  `default --pool --rent`) is the natural follow-up, a separate Task.

Admission for `--rent` and `lf ondemand rent` is one path (Etude's order):

1. Live Rentals of R `< max_machines`, else refuse naming the limit.
   `lf ondemand max code 0` keeps existing machines and permits only `rent`.
2. Quote: hardware in stock and `usd_per_hour ≤ max_usd_per_hour`, else refuse
   with the price seen. `cap_usd = usd_per_hour × hours`; `--max-usd` lowers it.
3. Write the Rental as `requested` with `provider_name = lf-<id>` and the quoted
   hardware **before** the POST. A 2xx moves it to `booting` with `provider_id`;
   a timeout or unreadable reply moves it to `unconfirmed`. Every later pass
   resolves `unconfirmed` by listing pods and matching the name: found → adopt;
   absent after the create's own deadline → `lost`. The create is never resent.

`lf ondemand availability R` is the quote alone, with in-use/max and when the
provider answered.

## The pod

The start command replaces the image's start script and starts, in Etude's order:

1. **Guardian** (`rental/guardian.py`, Python standard library only, embedded
   with `include_str!`, passed base64 in `LF_GUARDIAN`): writes a ready marker,
   shortens the deadline to the budget using the pod's own key and actual price,
   sleeps on both wall-clock and monotonic time, then `DELETE pods/{self}` until
   it succeeds. Nothing downstream starts until the marker exists.
2. **sshd** with the account's `PUBLIC_KEY`, key-only, root login prohibited;
   a non-root `lf` user owns the work (LOO-554: root hides Linux bugs).
3. **Tools**: git, gh, Node (Claude Code), the published lf via `install.sh`.
   apt/npm/curl only; the image stays stock `runpod/base:1.0.2-ubuntu2404`
   pinned by digest at `ondemand add`.
4. `sleep infinity`. The laptop drives everything after this over SSH.

Before this lands, the demo pod runs this branch's Linux build (Docker
`linux/amd64`, `scp`, `mv` over `.new`) instead of step 3's installer, as LOO-554
did. After landing the installer is enough.

Laptop side, after `booting`: poll `GET pods/{id}` until `ssh.direct` names a
host and published port for 22 (the proxy endpoint is PTY-only and never counts), wait for SSH, then the ordinary path: `lf machine add
lf@IP:PORT --label code-1` (probe + identity), clone the repository's origin and
`lf repo identity --bind <repo_id>` over `lf --machine code-1`, then `lf machine
connect code-1 github` and `… claude` (Jack approves each in Chrome; `-b`
reports which connects are still needed instead of waiting). The Rental is
`running` once `machine_id` is set. Then the handoff and the forwarded run,
exactly as to any Machine. Nothing in Task execution learns about rentals.

## Giving back

**Drain** (machines rented for a Task): the Machine owns no uncompleted Task and
`lf --machine code-1 ps --json` shows no live Process. **Deadline** (by-hand
machines, and every rental's hard stop): `rentals.deadline`. **Explicit**:
`lf machine end code-1`; owned machines refuse ("owned machines are never
deleted; `lf machine remove` forgets one").

The give-back pass, in order:

1. Refuse while the pod holds the only copy of development work: any Task
   worktree on it with uncommitted changes or commits not on its upstream
   (`git status --porcelain`, `git rev-list @{u}..HEAD` over SSH). Report what is
   unpushed and leave the machine running. `lf machine end --force` is the only
   override and names what it discards.
2. **Copy up** (pull; Jack's decision for now): `lf --machine code-1 history
   export --since <rental start>` emits the Machine's Sessions, Processes,
   GraphProcesses, session events and capture payloads for the window as JSON
   lines (origin first) plus capture files; the laptop imports them into
   `machine_records(origin_machine_id, kind, key, row, observed_at)` and
   captures into `~/.lf/machines/<origin>/runs`. Decided in implement
   2026-10-21: copies live apart from `processes`/`agent_sessions`, not as
   stamped rows in them, because `session_events.seq` is a local integer
   referenced by `current_capture`, `captured_event` and
   `attachment_exit_seq`, and every local reconciler (unfinished Processes,
   attachment, availability, Task work) would need an origin filter. Apart,
   a copy cannot gain authority. An export from another Machine id (a reused
   pod address) is refused. `lf session list` and `lf task status` say "from
   code-1"; `lf history show <id>` opens a copy as text (`--events` prints its
   raw event rows; `--json`, `--final`, `--context` refuse).
3. `DELETE pods/{id}` with the identity re-check; `state = ended`, `ended_at`,
   final `cost_usd`. The Machine row stays (ownership history reads "Jack on
   code-1"); `lf machine status` shows "rental ended" instead of unreachable.
   `lf machine remove code-1` is allowed afterwards and loses nothing.

**Who runs the pass.** There is no daemon. `lf machine end` gives back its
one rental; the end of a forwarded `lf task run` reconciles its Machine's rental
(`rental::settle`), which also strips `--pool/--rent/--quote` before
forwarding; the scheduled per-minute `lf task reconcile` runs the pass over
every live rental (without `RUNPOD_API_KEY` it reports only rentals already
past their soft deadline). `lf ondemand list` refreshes from RunPod (adopt or
lose unconfirmed creates, notice pods already gone, update cost) and never
copies or deletes: a listing, including `--json` polled by a client, must not
end a paid machine (concept review 2026-10-21). A soft deadline 15 minutes
before the hard one turns the pass from "when drained" into "now". The guardian
is the backstop for a laptop that never comes back: what it deletes at the hard
deadline was copied on an earlier pass or was unreachable for the whole window.

## Commands

```text
lf pool add <name>                      machines you own; `lf pool off|on <name>`; `lf pool remove`
lf pool list                            each pool's machines, cost, expiry, busy/free
lf machine add <dest> [--pool P] [--cost-usd USD] [--expires-at RFC3339]   (new flags)
lf machine end <label> [--force]        give a rented machine back now
lf ondemand add <name> --provider runpod [--gpu G] --vcpus N --memory-gb N [--disk-gb N]
                 [--image REF@sha256] [--data-center DC] [--hours 4] --max N --max-usd-per-hour USD
lf ondemand list                        template, budget, machines (state, hardware, price, cost, deadline, Task)
lf ondemand availability <name>         what it would rent now, at what price; rents nothing
lf ondemand max <name> N                change the limit; current windows stay
lf ondemand rent <name> [--hours H] [--max-usd USD]      a machine by hand
lf ondemand remove <name>               forget it; its rentals' history stays
lf task run X [--pool P] [--rent R] [--quote]
lf history export --since T             the export the copy reads
```

`lf machine use <machine> --task X` keeps today's meaning (hand off). The Wave
and repository defaults it sets stay single-Machine until the follow-up.

## Credentials on a rented machine (Jack, 2026-10-22)

Jack: no browser approval per rented machine; mint "as auto as possible";
several Claude accounts per machine, one token each.

**Token.** One long-lived subscription token per Claude account, minted once a
year. `claude setup-token` mints it: prefix `sk-ant-oat01-`, refused as an API
key (401), accepted on the OAuth subscription path (200), honoured by Claude
Code as `CLAUDE_CODE_OAUTH_TOKEN` and as a `.credentials.json` holding only
`accessToken` (probe on `probe-token-1`, 2026-10-22). It lacks `user:profile`,
so Anthropic's profile endpoint and usage polling refuse it.

**Minting.** `lf account token claude <email>` runs `claude setup-token`
through the existing login harness (`start_auth_command` with the `$BROWSER`
handoff and `AuthCommandInput::AuthorizationCode`): the authorize URL opens in
the account's verified Chrome profile, the person approves and pastes the
one-time code, lf captures the token from the command's output and stores it
on the account (new `provider_accounts.long_lived_token`, encrypted like other
tokens, with `minted_at`). It refuses anything whose prefix is not an `oat`
token: an API key bills per token, a subscription token does not. Rental
`prepare` and `lf machine connect <m> claude <email>` mint when the account
has none and the run is foreground; a background run with no stored token
fails naming `lf account token`. `lf account token claude <email> --file F`
or stdin takes a token minted by hand; never argv.

**Installing.** `Credential::Claude` carries the token in a credentials file
(`accessToken` only, `expiresAt` a year from minting) plus the laptop's
`login` and `subject`. The pod's `install_managed` skips the identity probe
for a long-lived token and registers the account from the laptop's assertion,
which comes over the added Machine's SSH channel. Usage for that account on a
rented machine reads "unavailable" rather than erroring.

**Several accounts.** `prepare` connects every Claude account the laptop has a
token for, not one; the Task's account selection on the pod works as on any
Machine, so an exhausted account falls through to the next (Q11 revised).
`lf machine connect <m> claude <email>` still adds one by hand.

**Built 2026-10-22.** `lf account token claude <email> [--file F|-]
[--chrome-profile P]`; draft columns `provider_accounts.long_lived_token`
(encrypted) and `long_lived_token_minted_at`; the login harness keeps parsing
provider output after the URL, so `start_claude_setup_token` captures the
printed token; `Credential::Claude` now carries `{login, email, subject,
credential}` with the laptop's verified email and subject, and the receiver
refuses a refresh token or a non-`oat` access token, installs without a probe,
and records the file's digest so the offline identity check holds;
`connect_claude_accounts` in `prepare` connects every token-holding account
(none stored: mints for each connected account in the foreground). Claude no
longer uses `fresh_machine_login` (Codex still does). Usage polling already
maps the 403 to "unavailable".

**Verification.** Unit: refusal of non-`oat` prefixes; the credentials file
shape; install path skipping the probe only for long-lived tokens; prepare
iterating accounts. Live: Jack's next `--rent` opens Chrome zero times when
tokens are stored, and `claude -p` on the pod answers under each account.

## Deleting what this replaces

Nothing in the repository rents machines today; `website/home.html` mentions
RunPod only as prose. LOO-554's hand procedure was never checked in. Etude's
code is copied, never shared as a package.

## Verification

- Rust unit tests: CPU quote selection (power of two, memory growth, cheapest
  in-stock, flavor max); admission refusals; Rental state machine including
  uncertain create → adopt/lost by name; pool free-machine selection with off,
  expiry and busy; drain judgment; refuse-delete with unpushed work; RunPod
  adapter against a closure-stubbed HTTP send (envelopes, identity re-check on
  delete, contiguous billing hours).
- Guardian: `python/tests/test_guardian.py` imports `rental/guardian.py` by
  path; Etude's cases: deadline, budget shortening, backwards clock, delete retry.
- Export/import round trip in a two-store fixture: origin stamping, no authority
  for copies, final record after the origin Machine is gone.
- Live probe, ~$0.10, before pursue finishes: a 2-vCPU CPU pod boots the
  guardian, sshd and lf; `lf machine add` succeeds; the pod-scoped key deletes
  the pod at a 10-minute deadline. Then the demo at ≤ $0.40.

## Demo

Jack's, from the brief, in today's vocabulary: `lf ondemand add code --provider
runpod --vcpus 8 --memory-gb 16 --max 1 --max-usd-per-hour 0.40`, run a Task
with `--rent code`, watch the machine appear, the Task read "Jack on code-1",
the PR push, and the machine get deleted with its Session history retained on
the laptop. Etude rented cpu3c×8 at $0.24/h on the 21st.

## Delete — do not maintain

Nothing: no earlier renting code exists (LOO-554's procedure was never checked in).

## Concept review (2026-10-21, headless)

Pool / OnDemand / Machine / Rental / ownership handoff map one-to-one to what
a person does. Its two directions (passive listing, readable copied history)
are built. Recorded for Jack, not blocking the interactive demo: `--rent`
under `-b` (Q7), logins left on a deleted pod (Q8).

## Concept review (2026-10-22, headless)

The model still maps one-to-one. Found one lifecycle gap: a rental could end
(`lf machine end`, soft deadline, guardian) while its Machine still owned an
open Task, so the next `lf task run` forwarded to a deleted pod. Settled: an
ordinary `Handoff` back to the Machine running the pass, keeping the person's
claim and the code-1 history (built; see Status). Q7–Q9 stand as recorded.

## Status and remaining work

Built 2026-10-21 (implement, compress, copy-up implement, compress, post-review implement): the draft
`pools_and_rentals` (pools, ondemands, machine columns, rentals,
machine_records); the `rental` module (admission, CPU quote, unconfirmed
resolution, free-machine selection, give-back judgment); the RunPod adapter
behind a send closure; guardian and pod boot; `lf pool`, `lf ondemand`, `lf
machine add --pool/--cost-usd/--expires-at`, `lf machine end [--force]`, `lf
task run --pool/--rent/--quote`; `lf history export --since`; copy-up into
`machine_records` before every delete (a failed copy keeps the machine unless
`--force`); copied Sessions in `lf session list`, `lf task status` and `lf
ondemand list`; `lf machine status` "rental ended"; the give-back pass in `lf
machine end`, the end of a forwarded `lf task run` (`rental::settle`) and the
scheduled `lf task reconcile` (`rental::check`; `lf task automation` only
inspects); `lf ondemand list` refreshes provider state and ends nothing; `lf
history show <id>` opens a copied Session from `machine_records` and its
copied captures. Built 2026-10-22: `SqliteStore::set_rental_state` hands the
Machine's open Tasks back to this Machine in the same write whenever the rental
stops being live, so every path to `ended` returns them, and the pass prints "LOO-570 back on <host>".

Deviations from the first draft of this design, recorded in
`scratch/questions.md`: copies live in `machine_records` rather than stamped
rows in `processes`/`agent_sessions`; `rentals` has no copy watermark (each
copy re-exports from an hour before the rental's start and upserts);
`lf session list --json` and the `TaskStatus` DTO omit copies by choice.

Live probe, 2026-10-22 (Task conversation, disposable `LF_HOME`, ~$0.03 total):

- Probe 1 (`probe`, cpu3c×2, $0.06/h, EU) rented but reported no SSH address
  for 10 minutes: the adapter read `publicIp`/`portMappings`, which v2 does not
  return. Its guardian deleted the pod within a minute of the 10-minute
  deadline. Fixed in `1a40cf60e5`: `ssh.direct` (host, port), falling back to
  `runtime.ports` for private 22/tcp; the proxy endpoint never counts.
- Probe 2 (`probe-us`, cpu3c×2 in US-KS-2, 12-minute deadline, $0.05 cap):
  `ssh.direct` appeared 35 s after create; the rent returned in 60 s with
  `probe-us-1` added. On the pod: non-root `lf` user, git, gh, Node, Claude
  Code 2.1.308 and published lf 0.14.14 on the login PATH, `/run/lf/ready`
  set; guardian running as root since boot with "ends by 08:26:19Z or $0.012",
  so `RUNPOD_POD_ID` and the pod key were injected. `lf --machine probe-us-1
  machine id` returned the pod's Machine id. The pod was gone 15 s after the
  deadline; `lf ondemand list` then read "ended 01:26" and `lf machine status`
  "rental ended". Billing showed no records for a sub-hour pod, as expected.
- Long-lived tokens (2026-10-22): unit and harness tests pass with a fake
  `claude setup-token`; the real command's output through the piped harness,
  and a token-only pod answering under each account, are left to the demo.
- Not exercised: `lf history export` over SSH (the pod ran published lf, which
  lacks the command) and the Task path (`--rent`, connect, clone, hand-off).

Demo attempt, 2026-10-22: Jack ran the first demo command with the installed
lf 0.14.14, which has no `ondemand` and took it for a skill name. The branch
binary could not open his home either: main's unreleased `rename_flow_to_graph`
draft left 81 `task_events` rows as `flow_finished`, and the semantic check
refused the copy. Fixed here in that draft (rows read as `graph_finished`),
proved in `released_flow_records_read_as_graphs_after_the_rename`. With that
passing, the build still re-executed the installed lf after making its copy
(#1612 never taught `dispatch_default_cli` about copies), so `ondemand` reached
0.14.14. Fixed: a build with a selected copy keeps running, and its children
resolve to the same binary. The demo runs `target/release/lf` directly; it
works on `~/.lf/copies/<drafts>/loopflow.db`, a snapshot of the real store, so
the Task is there and laptop-side PR and push effects are deliberately inert.
The first quote then refused: cpu3c at 8 vCPUs was in stock in seven data
centers but not US-KS-2, where the 2-vCPU probe had run. The refusal now names
the pinned data center; the demo on-demand was re-added in US-IL-1 and quotes
cpu3c×8 at $0.24/h. `--data-center` is now a full id or a region prefix (`US`):
the quote keeps the data centers in stock there and the create sends them as
RunPod's preferred placement, so a pin names a region, not a guess at a hall.
Jack's `--rent code` then rented `code-1` (cpu3c×8, $0.24/h, 4h).

The "flash": `lf task run` delegates to an execution owner and attaches to it;
the attach snapshot began with clear-scrollback, home, clear-screen before
replaying an owner screen that was still empty, so a quick refusal wiped the
terminal and left two lines at the top. A snapshot of an untouched screen now
sends nothing, and output flows from the prompt as any command's does
(`attaching_before_the_owner_draws_leaves_the_viewer_terminal_alone`). The
copy banner still prints twice, once per lf process; cosmetic.

Jack, during the demo: "I don't really want to have to approve my credentials
every time I rent a machine." Q10 in `questions.md` has the options and the
probe: a `claude setup-token` subscription token works on a pod as an env var
and as an access-token-only credential file, is refused as an API key, and
fails the profile scope check. Next pursue pass: `lf account token claude
<email>` stores one per account (read from a file or stdin, never argv), connect
installs it with the laptop asserting identity, one account per rented machine
(Q11, revised by Jack to several accounts per machine).

Remaining:

1. **Jack's demo (≤ $0.40)**: the Task path end to end, with Chrome approvals
   for the pod's GitHub and Claude logins and this branch's Linux build on the
   pod for `lf history export`. The scheduled reconcile only gives back when
   its environment carries `RUNPOD_API_KEY` (Q9).

Last check (demo repair 2026-10-22): `cargo clippy -p loopflow --all-targets -D warnings` and `cargo fmt --check` clean; `cargo test -p loopflow --lib -- os_process::tests store::copy released_flow_records_read_as_graphs_after_the_rename` 18 passed; `target/release/lf ondemand list` runs on the copy of the real store.
