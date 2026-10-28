# Machines and processes

A machine is one OS user's Loopflow authority in one data directory. It owns local processes,
credentials, planning storage, command and conversation records, and OS locks.
Two users on one host, or two data directories, have separate machine identities.
Its SSH route may change without changing its identity. `LF_HOME` still selects
the data directory; provider and account homes remain filesystem locations.
Opaque `home_…` IDs, existing scheduled-job keys and historical evidence retain
their stored spelling. Live commands and DTOs use `machine` and `machine_id`.

```bash
lf machine add mini
lf machine list
lf machine status mini
lf --machine mini session list
lf --machine mini --task LOO-123 implement
lf --machine mini machine add builder
lf machine rename mini builder
lf machine remove builder
```

`--machine <label-or-id>` transports the entire command. Unscoped commands use
its saved repository; Work-addressed commands carry a repository plan ID instead.
`--forward-agent` requires `--machine`; `machine add` sets the saved default.

Quote a remote `~/` path (`--repo '~/src/project'`) to avoid local shell expansion.
Without `--repo`, add uses the local checkout's path relative to the local home.
Absolute paths are accepted too. SSH aliases, `user@host` and SSH URIs are passed
to OpenSSH. Add reads the remote identity itself; `--label` overrides the host name.
The repository must already exist on the remote.

List reads saved connections. Status checks without prompting and shows remote
and local versions. Different versions produce an explicit remote update command;
Loopflow does not maintain old-version compatibility or replace an existing peer.
When `add` finds no `lf`, it offers the published installer with a default of yes,
then reads the new identity. Batch, JSON and nonterminal commands never install.

SSH key authentication and a known host key must already work. Failures identify
sign-in, unknown or changed host keys, unreachable hosts, or missing `lf`, with a
command to resolve each. Host-key repairs require verifying the fingerprint.
Connections share a private OpenSSH control socket and expire after 60 idle
seconds. Personal SSH control sockets are never reused or closed. Explicit
agent forwarding uses a separate connection. Add and status
transfer no credentials. SSH requires an added connection and checks its identity
before dispatch. Every receiving invocation validates identity through read-only
storage before help, repository lookup or Process admission; no shell-side identity
probe duplicates that check. Launch retains its sender probe and credential
preparation. `--context`/`--explain` skip both, and neither side admits a Process.
Names and paths resolve at the destination. Missing or changed identity fails without
initialization, repair or local fallback. `lf machine connect mini <provider> [email]` installs a separate
login through SSH stdin. Claude's is the account's long-lived subscription token
(`lf account token claude <email>`, minted on first use in a foreground run):
it cannot refresh, so no machine rotates another's login, and it cannot read
its profile, so the receiving machine registers the identity the sender asserts; account-selected launches connect missing logins in the
foreground. See [subscriptions](../subscriptions.md#use-subscriptions-over-ssh).

Remove forgets the connection without contacting the remote. Historical machine
identity and routes remain in SQLite, and ownership entries keep naming the Machine. Previously observed routes
need an explicit add before they can receive a login.

## Local by default

```bash
lf history list                                # Processes recorded on this Machine
lf ps --json                           # OS-live processes on this Machine
lf wave status product                 # current plan, Task conditions and Session evidence

lf --machine build-home history list   # run the same reader on build-home
lf --machine build-home --wave product wave-operate
```

`lf --machine` is transport, not a second API. The target runs its own `lf`, verifies
its Machine identity, resolves its own files and store, and returns the result.
Planning intent, including Task ownership, has one owner of record: the planning
provider, or Loopflow's journal when the plan is local. Execution facts stay with
the Machine that produced them, and inventories remain local. There are no
daemons or listeners.

The Machine and ownership types live in
[`durable.rs`](../../rust/loopflow/src/durable.rs). SSH routing is exposed by
the CLI under [`lf/`](../../rust/loopflow/src/lf/).

## Own Tasks

```bash
lf task claim LOO-123               # you, on this Machine
lf task move LOO-123 --machine mini # same person, mini's Machine
lf task unclaim LOO-123             # unowned, history intact
lf task status LOO-123 --json       # ownership: {owner, superseded_by, log}
```

A Task is owned by (person, Machine) or unowned. Its append-only ownership log
holds `claim`, `handoff` and `unclaim` entries; the later entry wins in provider
order. A handoff moves the claim to its target Machine and keeps the person; it
needs only the target's identity, not a connection. `lf task run` and
`lf task checkout` claim an unowned Task when they allocate its checkout.
Completion, abandonment and reopening write no entries; a Done Task shows its
last owner as history.

Which Machine runs a person's Wave work is that person's choice, never
repository content:

```bash
lf --wave infrastructure set machine mini            # this Wave's new Tasks and its Session
lf --wave infrastructure set machine.session laptop  # only the Wave Session
lf --repo loopflow set machine.tasks mini            # only the repository's new Tasks
lf --wave infrastructure unset machine               # back to the repository's, else this Machine
lf machine list                                      # each Machine with the defaults naming it
```

`lf set` without `--wave` sets the repository, even in a Wave's Task worktree;
`lf task move <task> --machine mini` moves one Task.
A Wave or repository has two settings, where its new Tasks run and where its
primary Session runs. `machine.tasks` or `machine.session` sets one; `machine`
sets both. Each resolves on its own, with no fallback from one to the other.
A default lives in Machine-local SQLite `scope_defaults`, keyed by Wave or
repository identity, and is copied to the
person's other added Machines over `lf --machine`, so the laptop launches there and
mini receives the runs. An unreachable Machine gets a warning; rerun later.
Resolution for an unowned Task walks its Wave, its parent Waves, its repository, then this Machine,
so single-Machine use needs no setup. `lf wave show`'s `machine` field reads the
same resolver.

Setting the Session Machine moves the Wave Session with its native provider
history, as `lf --machine <machine> resume <session>` does. `lf session connect --wave <w>`,
a bare connect resolving to a Wave or the repository, and Desktop's Wave Session
route to the Session Machine over `lf --machine`. `lf open` reads a primary on
another of your Machines through that connect, never from this Machine's copy.

A Task's Machine is captured in its claim, not inherited. Changing the Wave Machine
moves no existing Task; `lf task move <task> --machine <machine>` moves one. It
stops the Task's Graph and its step Processes, commits and pushes its worktree,
hands it off, and carries its Workflow position, history and primary Session; the
new Machine checks out the branch and `lf task run` there runs the stopped route
again. A worktree mid-merge or mid-rebase refuses to move.

Forwarding stays within one person's Machines. Claim, handoff, unclaim,
`task run`, `task checkout` and global `--task` pick their writer and re-check
inside one IMMEDIATE transaction:

| Task is | Writer |
|---|---|
| Yours, on another of your Machines | That Machine, over `lf --machine` |
| Unowned | Your Wave Machine |
| Someone else's | Your own Wave Machine; your later claim wins |

Nothing routes to another person's Machine, and nobody hands off or unclaims
another person's claim; cross-person handoff is deferred. The Machine in a claim
comment is informational. Routing reports its source: `owner`,
`owning_machine:wave:<name>` or `owning_machine:this_machine`. Ownership does not move
a checkout or launch a process. `lf wave status` reads planning, Task conditions,
metrics and Session history; no process needs to be running.

`RepositoryId` identifies the selected repository plan. `repository_plans` binds
that ID to the Machine-local repository locator; it contains no copied Work or
execution. Existing repository paths receive distinct opaque IDs; new Wave writes
use the same binding, and `lf repository` covers repositories without Work.
`lf repository --bind ID` explicitly associates even an established checkout.
The same table retains prior IDs as local locators and exactly one selected ID
per path; Work, provider mappings, pending effects and execution are untouched.
An ID already bound to another local path is not reassigned. Binding selects no
planning destination and publishes nothing. A shared code remote
or equal clone name supplies no identity. `--repository ID` resolves at the target
before Process admission, so SSH need not change the Machine's saved default.
LOO-412 supplies user/shared destination selection and portable Work exchange.
Ownership entries travel as marked Task comments ("Jack Heart claimed this on
mini" plus a `loopflow-ownership` HTML comment carrying the entry) through the
ordinary comment delivery to Linear or the Git planning journal; inbound comments
are parsed into the log. In the journal each local append sorts after every entry
its Machine has seen. An unknown Machine remains an entry reference, never an
invented SSH route.
The journal does not carry RepositoryId: unbound import creates a local repository
ID. Explicit association converges established roots on a selected ID without
rewriting Work; Desktop reassociates retained windows on opening/restoration. This associates repository roots, not divergent
Task/Project IDs for one provider object; provider correspondence and uncertain
effect ownership remain with the common planning writer.

Read execution separately from planning:

```bash
lf task location LOO-123 --peers --json
```

The local read uses one store snapshot. `--peers` addresses added Machines by
exact repository/Task identity through bounded, noninteractive SSH reads. Each
reply echoes a fresh request ID and its observing Machine; a cached, mismatched,
unreachable or unassociated reply is unavailable. A negative reading says only
that its Machine has no record. No connections, execution records or credentials
are imported or created by observation.

Peer observation routes nothing; the ownership log alone selects the Machine.
Added peers are not a complete global inventory, and an observation reserves
nothing. No execution-location cache exists.

Task-run, checkout and Desktop opening explanations share one location
projection, including local/peer provenance and unavailable checkout paths for
retained execution. Desktop opens recorded remote Tasks in the existing local
repository window through the owner's Session/Files transport. Shells recheck the
recorded Task/Machine/checkout before launch; file invalidations use the same
owned-line transport and retain drafts on disconnect. Missing or stale ownership
prepares nothing and never falls back to local paths. Headless fixtures cover
these paths; configured SSH and mounted native usability remain unproved.

An inbound entry never ends this Machine's own open claim; only its own unclaim
or handoff does. If another Machine claims later, every other Machine shows the
later claim, while the earlier claimant still owns it locally, reports
`superseded_by` ("Also claimed later by Kim on mini" in `lf task status`) and
steers its live Sessions. Nothing is cancelled. If the log stops showing this
Machine as owner for any other reason, such as a stale Machine's unclaim, the
owner claims again.

With Linear, a claim sets the assignee to that Machine's Linear viewer and moves
backlog, triage or `unstarted` to `started`; unclaim clears the assignee; handoff
leaves it. The assignee is not a second ownership store: assignee changes write no
entries, and the claim's Machine re-asserts its assignee while its claim is
unsuperseded.

Task checkout location is recorded separately: `tasks.checkout_machine_id` and
`worktree` identify the Machine and path prepared for execution. New checkouts
record the preparing Machine. Existing checkouts retain their recorded location
when ownership changes; no process or checkout is moved. Status, file access
and checkout-based Session/Process membership use that location. Unknown historical
Machines stay unknown, and file access cannot treat their paths as local. Explicit
Session bindings retain membership independently of checkout location.

## Own and rent Machines

A **pool** names machines you own (`machines.pool`, with `cost_usd` and
`expires_at`); it knows no provider. An **on-demand** (`ondemands`) is a
provider (Daytona, Modal, RunPod or AWS), hardware with at most one GPU
(RunPod, AWS and Modal rent them; `--vcpus`/`--memory-gb` are floors its machine
meets) and optionally spot (AWS and RunPod), and a budget: machines at once and the
most per hour. They share one namespace. A **rental** (`rentals`, `rent_…` ids)
is one paid window: hardware, price, cap, deadline, state and cost, separate
from the Machine it becomes once reachable. Task execution never learns a
Machine is rented.

`lf task run --pool P` hands the Task to the first machine in P that holds no
unfinished Task and has not expired; there is no queue. `--rent R` admits
(limit, then the price from the provider's published per-second rates), writes
the rental as `requested` with its sandbox name `lf-<id>` before the create,
creates once, waits for SSH, installs `lf`, adds the Machine, forwards GitHub
and installs every Claude account's long-lived token (and, when the repository
plans in Linear, a freshly refreshed Linear access token), clones and binds the
repository, then hands off. A sandbox that never answers SSH is deleted at once.
An unanswered create is `unconfirmed` until its name finds it (then deleted,
since it has no route) or the create's window passes (`lost`); it is never
resent. With `LF_RENT_LINUX_BIN` naming a Linux build, the laptop uploads that
binary instead of the published `lf`, so a development build rents sandboxes
running its own code.

`lf [--repo REPO] [--wave WAVE] set machine R`, naming a pool or on-demand, sets the
scope's Tasks capacity (`scope_defaults.tasks_capacity`) in place of a Tasks
Machine; the nearest scope that sets either wins. `lf task run` places an
unowned Task by that default as `--pool` or `--rent` would. Its Session stays
where it runs, and pools and on-demands are this Machine's own, so the default
is not copied to your other Machines.

Providers are reached through one embedded bridge
(`rust/loopflow/src/rental/sandbox.py`) under `uv`, with the provider's pinned
SDK where it has one (RunPod speaks plain HTTP). It creates, finds and
deletes machines by the name lf gave them. Daytona and Modal build an image (an
`lf` user, git, gh, Node and Claude Code, Rust, uv); RunPod and AWS boot
stock Ubuntu 24.04 and install the same tools before admitting `lf`. Modal runs
key-only sshd behind a TCP tunnel; Daytona's SSH gateway takes a token valid for
the sandbox's life, which rides in the Machine route; RunPod maps port 22 on a
public IP, and AWS opens 22 in a `loopflow-rentals`
security group.

The hard deadline never needs the laptop. Daytona (`ttl_minutes`) and Modal
(`timeout`) hold it themselves; elsewhere the guardian
(`rust/loopflow/src/rental/guardian.py`) does, starting before anything else on
the machine. On RunPod it is the main process and the setup its child: when it
exits it deletes its pod with
the pod-scoped key RunPod injects, after reading the price charged so the
budget can shorten the deadline. On AWS it powers the instance off, which
terminates it. A guardian only ever shortens the deadline. A machine found
ended is still deleted, freeing what it leaves (a pod's disk).
The laptop gives back
when a Task rental has drained and its last Task finished 15 minutes ago, at
the soft deadline (15 minutes before the hard one), or on `lf machine end`; any
of them refuses while the sandbox holds uncommitted or unpushed work. Owned
machines are never deleted.

Before it deletes, the laptop pulls `lf history export --since <rental start>`:
the worker's Process, Session, session-event and Graph rows as JSON lines, origin
first, plus capture files. They land in `machine_records` keyed by origin and
row, and captures under `~/.lf/machines/<origin>/runs`, never in this Machine's
own tables, so no reconciler, attachment or Task judgment acts on a copy. An
export from any Machine but the one expected is refused; a failed copy deletes
nothing. `lf session list` and `lf task status` name copies "from <machine>"
after the sandbox is gone, and `lf history show <session>` opens one. The give-back
pass runs at the end of a forwarded `lf task run` for its rented Machine and in
lazy recovery (any lf read of this Machine's work) for every rental; `lf ondemand list` only
refreshes provider state and never ends a machine.

## Process topology

```text
shell / automation / Loopflow.app
               |
               lf ---- Linear / GitHub / provider auth
               |
        store + repository
               |
          Graph process --> step Process --> LfSession <--> AgentProcess
```

Wave operations are finite attributed conversations. Each Task Graph
runs through the common Graph runner. Cron invokes commands on schedule;
local PR supervision watches and repairs delivery in the invoking process.

The process that directly spawns a child owns its child handle. Cross-process
recovery requires exact saved process identity and the applicable lock. A PID, tmux name, parent Process, Work identity or telemetry row alone grants
no signal authority.

## Observe processes

```bash
lf ps --json
lf top
lf mon prune --dry-run
```

The outer command journal records command receipts. `lf ps` and `lf top` join
those receipts to current OS process facts. Completed processes disappear from
the live view. This is observation, not a durable lifecycle model.

`lf mon prune` removes dead command receipts only after their terminal outcome
is recorded, and may reap only AgentProcesses whose recorded identity is live
and whose attached LfProcess is provably dead. Receipts use Process IDs, so PID reuse cannot overwrite an
unfinished Process’s identity. Failed terminal writes and interrupt cleanup retain
that identity without inventing an outcome. An unclaimed provider PID is
never killed merely because it resembles a Loopflow child.

Cross-process control requires exact PID/start identity and the applicable
conversation and AgentProcess record. Revalidate native scope or exclusive process
group before signaling. An attached LfProcess may disappear while its AgentProcess survives;
recorded endpoints alone do not prove liveness.

## Independent bridges

`lf discord serve <wave>` is a foreground bridge from a configured channel to
bounded conversations. It has no Wave cursor or inbox authority. Cron and Task execution
do not require a daemon or bridge. See [Discord](../work.md#talk-to-it-from-discord).

## Move a Wave without changing its identity

Wave identity is a UUID. The readable Wave locator is `(canonical repository, slug)`.
A bare slug may be ambiguous across repositories and is not mutation authority.

```bash
lf relocate <wave-id> --repo <target> --name <slug>
```

Relocation fences the locator, moves authored files, commits the new locator transactionally, and keeps PM and Work
joined to the unchanged UUID. A local receipt bridges the filesystem
and SQLite commit boundary so retry can finish verified cleanup after a crash.

## One main Machine

```bash
lf install                            # update the installed release and main Machine
uv run python scripts/install.py local # build an experimental CLI
LF_HOME="$(mktemp -d)" local-bin/lf wave list --json
```

Ordinary commands, provider children, hooks, Session connections and Graph steps
use the stable installed entry. Each new execution selects the current installed
CLI; main-Machine dispatch ignores inherited `LF_BIN` selections. The caller's
Machine selection and Session attribution stay intact. Installation does not
restart existing processes or wait for their exit; they may continue or fail
naturally. Clients and hooks launched before this behavior may retain their old
executable paths.

Private-Home development and tests retain explicit `LF_BIN` selection. Store
schema validation and migration authority still apply to every build; ordinary
main-Machine opens never initialize or advance its schema.

`LF_HOME` explicitly selects an empty, disposable experiment. Its database is
always `$LF_HOME/loopflow.db`; no other variable selects a store. Every variable
Loopflow sets or reads is listed in [Environment](environment.md). Its schema is
initialized once and must match on subsequent opens. A schema mismatch requires
a new experiment. Loopflow neither copies main data into it nor upgrades,
repairs, restores or promotes its contents.

Published installation verifies immutable artifacts, validates the candidate on
a temporary database snapshot, and advances only the main database under the
installation promotion lock. The snapshot is validation input, never a second live
Machine. Published switch receipts support interrupted release installation;
they do not choose ordinary command data directories. A settled install retains
its published fallback and the one it replaced; superseded content-addressed
binaries and app bundles are removed unless a live process executes them.

`lf self` owns published updates and skill exports; `lf install`
remains the short update command. Its directory retains the stored
spelling `~/.lf-machine/install`: released entry gates, receipts and scheduled
jobs pin that path. The installation promotion lock likewise stays at
`~/.lf/promotion.lock`. Neither path is relocated.

Artifact switching lives in
[`installation.rs`](../../rust/loopflow/src/installation.rs) and the
install command implementation under [`lf/commands/`](../../rust/loopflow/src/lf/commands/).

## Boundary contracts

- Machine identity is stable; network route is replaceable.
- Commands act locally unless routed with `lf --machine` or retained Task
  execution. Shared-Task location discovery observes added peers without merging
  their planning or claiming their execution.
- Placement selects where Work belongs, not whether it is currently running.
- Detached processes use credentials installed on their Machine.
- Direct child handles are local capability; inferred process ownership is not.
- Promotion owns artifact selection and app replacement, not conversation
  lifecycle.
- Published preview uses a temporary snapshot; all ordinary writers share the main Machine.

## Next

[Data and persistence →](data.md) maps the stores on each Machine.
[Codebase map →](codebase.md) maps CLI and process entrypoints.
