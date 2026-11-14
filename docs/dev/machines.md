# Machines and processes

`lf machine` manages execution destinations: connections, defaults, pools and
rentals. `lf --machine` runs a command at its destination. Client access and
server installation have separate owners; see the accepted
[server boundary](server.md).

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

SSH aliases, `user@host` and SSH URIs are passed to OpenSSH. Add reads the remote
identity itself; `--label` overrides the host name. The label is Loopflow's, not
OpenSSH's: `ssh mini` resolves only if `~/.ssh/config` names it too.

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
login through SSH stdin: a fresh login the Machine refreshes itself, or, on a
rental, a Claude access token the laptop refreshes from a login it keeps for
rentals. An access token cannot refresh, so no machine rotates another's login;
the rental registers the identity the sender asserts. Account-selected launches
connect missing or expiring logins in the foreground. See [subscriptions](../subscriptions.md#use-subscriptions-over-ssh).

Remove forgets the connection without contacting the remote. Historical machine
identity and routes remain in SQLite, and ownership entries keep naming the Machine. Previously observed routes
need an explicit add before they can receive a login.

## Local by default

```bash
lf session history list                        # Processes recorded on this Machine
lf process list --json                           # OS-live processes on this Machine
lf wave status product                 # current plan, Task conditions and Session evidence

lf --machine build-home session history list   # run the same reader on build-home
lf --machine build-home --wave product wave-operate
```

`lf --machine` is transport, not a second API. The target runs its own `lf`, verifies
its Machine identity, resolves its own files and store, and returns the result.
Planning intent, including Task ownership, has one owner of record: the planning
provider, or Loopflow's journal when the plan is local. Execution facts stay with
the Machine that produced them, and inventories remain local. SSH command
routing needs no resident API server. Desktop uses a separate local client API
server; LOO-718 adds authenticated remote client access. Neither moves execution
ownership into a central service.

The Machine and ownership types live in
[`durable.rs`](../../rust/loopflow/src/durable.rs). SSH routing is exposed by
the CLI under [`lf/`](../../rust/loopflow/src/lf).

## Own Tasks

```bash
lf task claim LOO-123               # you, on this Machine
lf task move LOO-123 --machine mini # same person, mini's Machine
lf task unassign LOO-123             # unowned, history intact
lf task show LOO-123 --json      # ownership: {owner, superseded_by, log}
```

A Task is owned by (person, Machine) or unowned. Its append-only ownership log
holds `claim` and `unclaim` entries; the later entry wins in provider order, and
no current owner is stored beside it. Each entry names its person, the Machine
it puts the Task on and the Machine that wrote it, so every Machine stores the
same rows. Moving a Task is a claim by the same person naming another Machine;
it needs only the target's identity, not a connection. Entries an earlier `lf`
wrote as `handoff` read as such claims. `lf task run` and `lf task checkout`
claim an unowned Task when they allocate its checkout. Completion, abandonment
and reopening write no entries; a Done Task shows its last owner as history.

A person is named once, in `people`: by the planning provider's user
(`linear:<user>`, learned when Linear first shows the Task's assignee), else by
the Git journal's planning user key (`journal:<key>`), with a display name.
Ownership entries and Machines refer to that row; the Git email names nobody new.
Entries written before a Machine knew its person still carry only the Git email.
A claim naming your person is yours; one naming a different person is still
yours when its Git email is, so connecting Linear keeps journal-era claims.

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
Placement preferences are authored in personal configuration files. The private
repository file is shared across linked Task worktrees; independent clones can have
different defaults. Machine alias/identity references are portable declarations;
this installation's own identity and observed state remain runtime facts. Connection
addresses may require host-local bindings. [Settings ownership](settings.md) defines
the catalogue, file writer and migration boundary.

Resolve the selected machine to its durable ID at the initiating LF instance. If
it is the current identity, execute locally. Already assigned work does not choose
placement again on arrival. The executor provides local tools and credentials;
the originating profile supplies portable preferences. Retaining those preferences
for later direct commands is a separate, inspectable file update.

Saving a preference selects future placement. Moving an existing Task or Session
is an explicit domain operation that preserves its identity and native history;
it must not be hidden in a generic file edit. CLI and Desktop expose the same
placement resolution and movement operations.

A Task's Machine is captured in its claim, not inherited. Changing the Wave Machine
moves no existing Task; `lf task move <task> --machine <machine>` moves one (see
[Move a Task between your Machines](#move-a-task-between-your-machines)).

Forwarding stays within one person's Machines. Claim, move, unclaim,
`task run`, `task checkout` and global `--task` pick their writer and re-check
inside one IMMEDIATE transaction:

| Task is | Writer |
|---|---|
| Yours, on another of your Machines | That Machine, over `lf --machine` |
| Unowned | Your Wave Machine |
| Someone else's | Your own Wave Machine; your later claim wins |

Nothing routes to another person's Machine, and nobody moves or unclaims
another person's claim; moving a Task to another person is deferred. The Machine in a claim
comment is informational. Routing reports its source: `owner`,
`owning_machine:wave:<name>` or `owning_machine:this_machine`. Ownership does not move
a checkout or launch a process. `lf wave status` reads planning, Task conditions,
metrics and Session history; no process needs to be running.

`RepositoryId` identifies the selected repository plan. `repository_plans` binds
that ID to the Machine-local repository locator; it contains no copied Work or
execution. Existing repository paths receive distinct opaque IDs; new Wave writes
use the same binding, and `lf repo` covers repositories without Work.
`lf config repo --bind ID` explicitly associates even an established checkout.
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
are parsed into the log with the Machine that wrote them. In the journal each
entry sorts after every entry its writing Machine had seen. An unknown Machine remains an entry reference, never an
invented SSH route.
The journal does not carry RepositoryId: unbound import creates a local repository
ID. Explicit association converges established roots on a selected ID without
rewriting Work; Desktop reassociates retained windows on opening/restoration. This associates repository roots, not divergent
Task/Wave IDs for one provider object; provider correspondence and uncertain
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

An entry another Machine wrote never ends this Machine's own open claim; only a
later entry this Machine wrote does. If another Machine claims later, every other Machine shows the
later claim, while the earlier claimant still owns it locally, reports
`superseded_by` ("Also claimed later by Kim on mini" in `lf task status`) and
steers its live Sessions. Nothing is cancelled. If the log stops showing this
Machine as owner for any other reason, such as a stale Machine's unclaim, the
owner claims again.

With Linear, a claim sets the assignee to that Machine's Linear viewer and moves
backlog, triage or `unstarted` to `started`; unclaim clears the assignee; a move
keeps the person, so it leaves the assignee. The assignee is not a second ownership store: assignee changes write no
entries, and the claim's Machine re-asserts its assignee while its claim is
unsuperseded.

Where a Task runs and whether this Machine has it checked out are two
questions with one answer each. The ownership log answers the first. A
`task_checkouts` row answers the second: worktree path, branch, base commit and
parent PR, stored only on the Machine that has the checkout. Planning never
carries a path, so no Machine holds another Machine's path:

```console
$ lf --machine mini task status MTG-321      # mini never checked it out
MTG-321  Runs on laptop since Nov 12 09:15
  Not checked out on mini
```

Ownership changes move no checkout and start no process. A Machine that loses a
Task keeps its row after `lf wt prune` removes the worktree, so the Task's
history on that Machine still finds its Sessions and Processes. Status, file access
and checkout-based Session/Process membership read this Machine's row. Explicit
Session bindings retain membership without one.

## Move a Task between your Machines

Start on the laptop, send the Task to the always-on mini when the laptop is busy
or about to close, and bring it back when you sit down again:

```console
$ lf task move MTG-321 --machine mini
Checked mini: it can take MTG-321
Stopped MTG-321 at `implement`
Committed and pushed MTG-321 on mtg-321
Arrived on mini (Graph at `implement`); its Session moved with it
Running on mini as Graph process 0199a213-81c0-7800-8aa1-bbab2a035a55
  follow: lf task status MTG-321
```

The Graph is running on mini when the command returns, from the step it was on.
Moving back is the same command, run from either Machine: on the laptop
`--machine` takes its host name, on mini the name you registered it under
(`lf machine add laptop`). Mini then says `MTG-321  Runs on laptop since …`;
asked from the laptop (`lf --machine mini task status`), it reads the laptop's
preferences, which don't name the laptop, and shows its Machine id.

**Reopen the conversation there.** The terminal that was showing the Task's
Session ends it with `This conversation moved to mini: lf resume --task MTG-321`;
that command opens it again from mini.

**The conversation goes only with its Task.** `lf --machine mini resume <session>`
on a Task's conversation moves the Task, exactly as `lf task move` does, then
resumes the conversation there. A conversation outside any Task moves alone.
`lf resume <session>` opens a Task's conversation wherever the Task runs now:
when a rental ends and its Tasks come back to the laptop, their conversations
resume on the laptop.

**A running step starts over from the checkpoint.** The Task's primary Session
travels with its transcript. A headless step that was mid-run, such as
`implement`, is stopped, and mini runs that step again from the commit the move
pushed: the step's edits arrive, its conversation does not. Steps the run had
already finished are not repeated. Programs the agent
had started, such as a training job or a dev server, end on the laptop and are
not restarted.

**Nothing to prepare on mini.** The check before the move clones the repository
on a Machine that has none and takes the Task in from planning. A personal Git
plan (`planning: {provider: git, personal: true}`) is read under the laptop's
planning key, which a Machine without one adopts; a Machine already planning
under another key fails the check. A provider login mini lacks is asked for in
the foreground, as account-selected launches do. Both Machines need an `lf` that
knows how to take a Task in; an older one fails the check, and nothing moves.

**The laptop afterwards.**

```console
$ lf task status MTG-321
…
MTG-321  Runs on mini since Nov 12 10:42
$ lf task list
MTG-321 · Jack · Runs on mini since Nov 12 10:42 · Resume a Session anywhere
$ lf machine list
mini  mini.local  home_7c0d…
  MTG-321 since Nov 12 10:42
$ lf wt prune
removed mtg-321  /Users/jack/src/project.mtg-321  (Task MTG-321 runs on mini)
```

`lf task status`, `lf task list` and `lf machine list` read where the Task runs
from its ownership, not from the laptop's stopped records. The laptop's worktree
stays until `lf wt prune`, which removes it once the Task is owned elsewhere and
it has no source changes beyond what the move pushed; moving back checks the
branch out again.

**Failures.** A move refuses before anything stops when mini can't be reached or
can't take the Task, or the worktree is mid-merge or mid-rebase; nothing changes.
The move hands the Task to mini before it stops the Graph, so nothing on the
laptop restarts it meanwhile. If the checkpoint can't be pushed, the Task is
handed back: the log shows both claims, its conversation stays on the laptop,
and the Graph stays stopped at the step `lf task status` names.
The Machine the Task leaves records the move as `leaving` with the claim,
keeps the step it stopped, records it as `sent`, with what it sent, before it
answers, and as `arrived` once mini has taken the Task in. A move killed while
it was leaving, or one whose answer was lost on the way back, shows in status:

```console
$ lf task status MTG-321
MTG-321  Moving to mini since Nov 12 10:42: not sent
  retry: lf task move MTG-321 --machine mini
  take back: lf task claim MTG-321
```

Once sent, the stage after the colon reads `sent, not arrived`; `since` stays
when the move began.

On mini, status shows its recorded check separately from ownership and checkout:

```console
$ lf task status MTG-321
…
MTG-321  Checked move from laptop at Nov 12 10:42
  retry: lf task move MTG-321 --machine mini-host
MTG-321  Not checked out on mini-host
```

The retry names mini's own hostname. This is a past check: the laptop may have
claimed the Task back since then. It establishes no ownership or active arrival;
retrying asks the source for the current move. Once the Task arrives, the check
line disappears.

Running the same move again, from any Machine, asks the Machine the Task left.
Run on mini, it asks the Machine mini checked the move with, so mini needs that
Machine added (`lf machine add`). Mini may not know itself by the laptop's
label; then the refusal prints the same check and retry. Addressed to mini from the laptop
(`lf --machine mini task move MTG-321 --machine mini`), a move the laptop began
runs on the laptop, which holds the move. A leaving move carries on: it
commits, pushes and takes the conversation, which are safe to repeat. A sent
move is delivered as it was sent, conversation included, without committing or
pushing again. Either way the Graph continues on mini at the step it was on. If
the Graph does not start on mini, the move still stands and the last line prints
`lf task run MTG-321 --from 'implement'`, which starts it at that step on the
Task's owner.

`lf task claim MTG-321` on the laptop first asks mini whether the move arrived.
If it did, its answer lost, the Task runs on mini and the claim is written
there. If it did not, or mini is gone, the laptop takes the Task back and the
move ends.

Auto-Ship approval travels with the move, as the Graph position does. Each
Machine keeps approvals and stops as a log (who, when, which process). The continued run is a plain
`lf task run`; when it stops at an interactive step, mini's ship process carries
the Task on, as it would have on the laptop. `lf task hold MTG-321` withdraws the
approval on either Machine. A Machine ships only Tasks it owns: once a Task has
left, its old checkout starts nothing.

## Own and rent Machines

A **pool** names machines you own (`pool_machines`: pool, Machine, `cost_usd`
and `expires_at`); it knows no provider. A `machines` row is only identity: id,
public name and the person it belongs to. An **on-demand** declaration in personal configuration is a
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
and installs each Claude account's rental access token (and, when the repository
plans in Linear, a freshly refreshed Linear access token), clones and binds the
repository, then hands off. A sandbox that never answers SSH is deleted at once.
An unanswered create is `unconfirmed` until its name finds it (then deleted,
since it has no route) or the create's window passes (`lost`); it is never
resent. With `LF_RENT_LINUX_BIN` naming a Linux build, the laptop uploads that
binary instead of the published `lf`, so a development build rents sandboxes
running its own code.

`lf [--repo REPO] [--wave WAVE] set machine.capacity R`, naming a pool or on-demand, sets the
scope's personal Tasks capacity preference in place of a Tasks
Machine; the nearest scope that sets either wins. `lf task run` places an
unowned Task by that default as `--pool` or `--rent` would. Its Session stays
where it runs. Pool and on-demand bindings remain host-local; transporting a
preference does not create that capacity on another machine. File-backed setting
resolution must distinguish the authored capacity choice from its runtime binding.

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

Before it deletes, the laptop pulls `lf session history export --since <rental start>`:
the worker's Process, Session, session-event and Graph rows as JSON lines, origin
first, plus capture files. They land in `machine_records` keyed by origin and
row, and captures under `~/.lf/machines/<origin>/runs`, never in this Machine's
own tables, so no reconciler, attachment or Task judgment acts on a copy. An
export from any Machine but the one expected is refused; a failed copy deletes
nothing. `lf session list` and `lf task status` name copies "from <machine>"
after the sandbox is gone, and `lf session history show <session>` opens one. The give-back
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
lf process list --json
lf process top
lf process prune --dry-run
```

The outer command journal records command receipts. `lf process list` and `lf process top` join
those receipts to current OS process facts. Completed processes disappear from
the live view. This is observation, not a durable lifecycle model.

`lf process prune` removes dead command receipts only after their terminal outcome
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

Relocation fences the locator, moves authored files, commits the new locator transactionally, and keeps planning and Work
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
Machine selection and Session attribution stay intact. Installation quits and
reopens a running Desktop. It does not restart other processes or wait for their
exit; they may continue or fail naturally. A Session owner on a superseded lf
keeps serving: `lf resume` moves one Session onto the installed lf, and `lf
process rollover` moves every eligible owner at once. Clients and hooks launched before this
behavior may retain their old executable paths.

Private-Home development and tests retain explicit `LF_BIN` selection. Store
schema validation and migration authority still apply to every build; ordinary
main-Machine opens never initialize or advance its schema.

`LF_HOME` explicitly selects a private, disposable Home. Its database is
always `$LF_HOME/loopflow.db`; no other variable selects a store. Every variable
Loopflow sets or reads is listed in [Environment](environment.md). A private Home
migrates itself on open, including a branch's unmerged migration; editing that
migration afterwards needs a fresh `LF_HOME`. Loopflow never copies main data into
it or promotes its contents.

Published installation verifies immutable artifacts, validates the candidate on
a temporary database snapshot, and advances only the main database under the
installation promotion lock. The snapshot is validation input, never a second live
Machine. Published switch receipts support interrupted release installation;
they do not choose ordinary command data directories. Every published switch
makes its target the published fallback, so a settled install retains the active
build and the fallback it replaced, normally the previous build. Other
content-addressed binaries and app bundles are removed unless a live process
executes them.

Retained bytes are not a rollback path. `lf self install rollback` refuses
receipt-managed installs, so going back means switching to a stream whose release
still opens the store. After a release raises the store's reader floor, older
binaries, retained or released, fail their own preflight and no rollback path
exists.

`lf self` owns published updates and skill exports; `lf install`
remains the short update command. Its directory retains the stored
spelling `~/.lf-machine/install`: released entry gates, receipts and scheduled
jobs pin that path. The installation promotion lock likewise stays at
`~/.lf/promotion.lock`. Neither path is relocated.

Artifact switching lives in
[`installation.rs`](../../rust/loopflow/src/installation.rs) and the
install command implementation under [`lf/commands/`](../../rust/loopflow/src/lf/commands).

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
