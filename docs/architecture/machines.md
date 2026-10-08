# Machines and processes

A machine is one OS user's Loopflow authority in one data directory. It owns local processes,
credentials, planning storage, command and conversation records, and OS locks.
Two users on one host, or two data directories, have separate machine identities.
Its SSH route may change without changing its identity. `LF_HOME` still selects
the data directory; provider and account homes remain filesystem locations.
Opaque `home_…` IDs, existing scheduled-job keys and historical evidence retain
their stored spelling. Live commands and DTOs use `machine` and `machine_id`.

```bash
lf machine add mini --repo '~/src/project'
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
before dispatch. `lf machine connect mini <provider> [email]` installs a separate
login through SSH stdin; account-selected launches connect missing logins in the
foreground. See [subscriptions](../subscriptions.md#use-subscriptions-over-ssh).

Remove forgets the connection without contacting the remote. Historical machine
identity, routes and Work placements remain in SQLite. Previously observed routes
need an explicit add before they can receive a login.

## Local by default

```bash
lf mon list                                # Processes recorded on this Machine
lf ps --json                           # OS-live processes on this Machine
lf wave status product                 # current plan, Task conditions and Session evidence

lf --machine build-home mon list   # run the same reader on build-home
lf --machine build-home --wave product wave/operate
```

`lf --machine` is transport, not a second API. The target runs its own `lf`, verifies
its Machine identity, resolves its own files and store, and returns the result.
There is no implicit fan-out and no central execution database.

The Machine and placement types live in
[`durable.rs`](../../rust/loopflow/src/durable.rs). SSH routing is exposed by
the CLI under [`lf/`](../../rust/loopflow/src/lf/).

## Place Work

`Placement` maps one `WorkRef` to one `MachineId`. It records where Work belongs,
not whether a process exists. `lf wave place` sets the Machine used by Wave schedules.
The current LOO-427 working tree prototypes nearest-assignment resolution through
Task, Project and ancestor Waves; this inheritance rule is still under design,
not an accepted routing contract. New children store no assignment copy, so later
delegation applies to existing
unassigned children as well as future ones. An explicit narrower assignment wins,
even when it selected the same Machine as its parent. The read exposes `source`
and `provenance`; historical rows remain `legacy` because an explicit choice cannot
be distinguished from a copied value. With no assignment, the root Wave supplies
a `local_default`. Delegation does not move a checkout or launch a process. `lf wave status` reads
planning, Task conditions, metrics and Session history; no process needs to be running.
The current routing prototype uses recorded checkout Machine before delegation
for known Task launches and checkout preparation. `task run`, `task checkout` and
global `--task` share that read before Workflow/checkout effects. Allocation checks
the effective destination again in its transaction; unknown historical checkout
Machine is unavailable, never a local default.

`RepositoryId` identifies the selected repository plan. `repository_plans` binds
that ID to the Machine-local repository locator; it contains no copied Work or
execution. Existing repository paths receive distinct opaque IDs; new Wave writes
use the same binding, and `lf repo identity` covers repositories without Work.
Peer binding is explicit and cannot replace an existing plan. A shared code remote
or equal clone name supplies no identity. `--repository ID` resolves at the target
before Process admission, so SSH need not change the Machine's saved default.
LOO-412 still owns user/shared selection and importing the same identity.

This is not complete distributed routing: a peer's absence of a checkout is not
proof of an unstarted Task. Planning exchange excludes execution. An observed
execution-location read, including unavailable/freshness state, must precede
applying delegation to imported Work. The prototype handles recorded locations;
remote observation and alias acquisition/rerouting remain unfinished. Do not infer
a global execution destination from the current delegation alone.

Task checkout location is recorded separately: `tasks.checkout_machine_id` and
`worktree` identify the Machine and path prepared for execution. New checkouts
record the preparing Machine. Existing checkouts retain their recorded location
when Work assignment changes; no process or checkout is moved. Status, file access
and checkout-based Session/Process membership use that location. Unknown historical
Machines stay unknown, and file access cannot treat their paths as local. Explicit
Session bindings retain membership independently of checkout location.

## Process topology

```text
shell / automation / Loopflow.app
               |
               lf ---- Linear / GitHub / provider auth
               |
        store + repository
               |
       Flow driver Process --> step Process --> AgentSession <--> native engine
```

Wave operations are finite attributed conversations. Each Task Flow
runs through the common driver. Cron invokes commands on schedule;
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
is recorded, and may reap only registered orphan OpenCode process groups whose
ownership is known. Receipts use Process IDs, so PID reuse cannot overwrite an
unfinished Process’s identity. Failed terminal writes and interrupt cleanup retain
that identity without inventing an outcome. An unclaimed provider PID is
never killed merely because it resembles a Loopflow child.

Cross-process control requires exact PID/start identity and the applicable
conversation/provider generation. Revalidate native scope or exclusive process
group before signaling. A driver may disappear while its engine survives;
recorded endpoints alone do not prove liveness.

## Independent bridges

`lf discord serve <wave>` is a foreground bridge from a configured channel to
bounded conversations. It has no Wave cursor or inbox authority. Cron and Task execution
do not require a daemon or bridge. See [Discord](../waves.md#discord-bridge).

## Move a Wave without changing its identity

Wave identity is a UUID. The readable Wave locator is `(canonical repository, slug)`.
A bare slug may be ambiguous across repositories and is not mutation authority.

```bash
lf relocate <wave-id> --repo <target> --name <slug>
```

Relocation fences the locator, moves authored files, commits the new locator transactionally, and keeps PM, Work, and Machine
placement joined to the unchanged UUID. A local receipt bridges the filesystem
and SQLite commit boundary so retry can finish verified cleanup after a crash.

## One main Machine

```bash
lf install                            # update the installed release and main Machine
uv run python scripts/install.py local # build an experimental CLI
LF_HOME="$(mktemp -d)" local-bin/lf wave list --json
```

All ordinary commands use the installed CLI and `~/.lf`. Source CLI commands
forward there before opening a store. Task Flows, Flow steps, sessions and
agent tools inherit the same Machine; no source-specific or installed-development
Machine exists.

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
- Commands and read surfaces act locally unless explicitly routed with
  `lf --machine`.
- Placement selects where Work belongs, not whether it is currently running.
- Detached processes use credentials installed on their Machine.
- Direct child handles are local capability; inferred process ownership is not.
- Promotion owns artifact selection and app replacement, not conversation
  lifecycle.
- Published preview uses a temporary snapshot; all ordinary writers share the main Machine.

## Next

[Data and persistence →](data.md) maps the stores on each Machine.
[Codebase map →](codebase.md) maps CLI and process entrypoints.
