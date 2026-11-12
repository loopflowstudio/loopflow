# Simplification Opportunities: remote work and Task sharing

Scope: how one person's Machines (laptop, mini, rentals) reach each other, hand
Tasks between them, and show each other's work. Companion to
`scratch/remote-machines.md`, which records the MTG-321 snags and current model.

## Product intent

Work runs on whichever of your computers has room, and you keep using it from
wherever you are. Moving a Task is one act: it keeps its conversation, branch
and Graph position, it continues where it lands, and every surface (CLI,
Desktop, phone) can see it and type into it there.

## What the server and holder change

The remote-work design dates from when every conversation had its own detached
owner and a Machine had no resident process. `docs/dev/machines.md` says so:
"SSH command routing needs no resident API server". So cross-Machine work became
SSH running the remote's ordinary CLI. That premise no longer holds.

| Then | Now |
|---|---|
| No resident process; SSH runs a fresh `lf` | Every install runs a Machine server; Task, Wave and repository launches fail without it (`transport/README.md` Held terminals) |
| The wire is the CLI's argv; compatibility means equal version strings | The server speaks a typed, schema'd protocol with `x-revision`/`x-min-revision` negotiation (`server/README.md`, `lf server describe`) |
| Remote viewing is an SSH-wrapped `lf` viewer relay | `lf server mobile` already admits the owner's other Tailscale nodes and attaches terminals with tickets; the laptop is such a node |
| An owner process per Session holds the agent | Holders keep PTYs and agents through server restarts; any number of views attach |
| Nothing could resume a moved Session on arrival | `session.continue` starts `lf session resume` of a Session in a held terminal |

Code that crosses Machines does not use any of this yet. Served writes for
remote Work run `lf … --json` over SSH and parse what it prints
(`engine/task.rs:300-316` through `work_route::run_remote`), even though the
server contract says "Nothing parses standard output". The move protocol from
the MTG-321 fix is spelled as hidden CLI flags:
`session receive --check --task --issue --repository`. A released peer
rejects new flags, and the operator reads that as "cannot receive it".

## Opportunity 1: One hub; every Machine dials it

**Misalignment.** A Machine has two APIs: the typed server protocol for its own
clients, and an untyped CLI-over-SSH for its peers. Peer calls lose typed
results, revision negotiation, subscriptions, ticketed attach and request
attribution. The peer design also asks every Machine to reach the others: a
move from rental to mini goes through whoever can SSH to both. The design's
intent is one-way reachability: A reaching B must not require B to reach A.

**Symptom.**
- `ssh::request` and `ssh::run_with_io` apply preferences differently (the
  "Remote execution uses … preferences" notices).
- `REMOTE_PATH` PATH patching.
- The string-compared version probe.
- `resident_args`' argv rewriting.
- The hidden `session send`/`receive [--check]` commands.
- `run_remote` parses stdout.
- Remote resume relays SSH-wrapped viewers.
- There are two Machine lists: `machines:`/`connections:` in each Machine's
  `config.yaml`, plus `mobile.yaml` for the phone's endpoint.

**Realignment.** Two roles that today's word "primary" blurs:

- **Hub:** the most reliably on Machine. It is the one address every other
  Machine and the phone connect to. Here: the mini.
- **Desk:** where the person sits and starts work. Here: the laptop. It is an
  ordinary Machine that also runs Desktop.

"Desk" may need no record: it is whichever Machine a client is attached to.
"Hub" is the one stored fact, one address per Machine.

The hub is also where outside events arrive. The always-on, reachable Machine is
the one that can take Linear and GitHub webhooks and turn them into `lf`
commands, replacing polling. That is the self-hosted event server wanted since
2026-11-02 for red-main automation (LOO-704); it and the hub are the same
Machine and the same server process.

Jack, 2026-11-12: "I kind of like the terms 'hub' and 'desk.' One thing that
I've looked at before was the idea of also having a machine that does things
like subscribing to Linear webhooks … 'hub' is a word I've used for that. I
think that makes sense and then I do also like the idea of a desk. I'm not sure
how much it matters".

Rules:
- **Machines dial the hub.** Each Machine's server keeps one outbound
  connection to the hub's server and speaks the existing protocol over it, as a
  client that also answers requests sent back down that connection. The hub
  never dials out, so NAT'd rentals and a roaming laptop both work.
- **Clients talk to their own server first.** Desktop connects to the laptop's
  server over the Unix socket, as today. That server answers for local Work
  itself and asks the hub for everything else. The phone has no local server,
  so it connects to the hub.
- **The hub relays and keeps the Machine list.** "What Machines exist" is the
  set that has dialed in, with last-seen times. A request for another Machine's
  Work (`task.run`, `session.attach`, `task.move.*`) goes down that Machine's
  connection. The hub executes nothing for others.
- **The hub is also a Machine.** It runs Tasks placed on it like any other.
  Being hub adds routing, not authority: planning still owns Task facts, and
  each Machine keeps its store, holders and Graph runners.
- **Where work runs stays a personal setting** (`machine.tasks`,
  `machine.session`, `lf task move`). Nothing defaults to the hub because it is
  the hub.
- **SSH is bootstrap only:** install `lf`, pass a tailnet key or login, name
  the hub; the Machine dials in from then on.

**The case here: laptop desk, mini hub.**

| Situation | What works |
|---|---|
| Laptop awake, on the tailnet | Desktop shows laptop and mini Tasks live; move either way is one leg (laptop ↔ hub); phone sees both |
| Laptop asleep or lid closed | Its agents are suspended with it. Mini Tasks keep running; the phone sees them live and sees laptop Tasks as "laptop asleep since …" from the hub's last reading |
| Laptop awake, off the tailnet (plane) | Everything local works unchanged: Desktop, Graphs, holders, bare runs. Mini Tasks show their last reading, marked unreachable. Moves and remote steers wait |
| Mini down | Each Machine works locally. No cross-Machine view, move or phone access until it returns. Nothing is lost: no Work lives only on the hub unless it runs there |
| Rental | Dials the hub at bootstrap; reachable from laptop and phone through it, with no route from laptop to rental |

Sketch of setup, one command per Machine:

```bash
lf machine hub                 # on the mini: this Machine is the hub; prints its address
lf machine join mini-heart     # on the laptop or a rental: dial that hub
lf machine list                # anywhere: every Machine the hub knows, and when it last answered
```

Demo: on the laptop, MTG-321 is mid-`implement` and the fans are on.
`lf task move MTG-321 --machine mini` returns with the Task running on the
mini. Close the lid. The phone shows the Session live and takes a typed line.
Open the lid: the laptop's Desktop shows the same terminal as an ordinary pane.

**Cascade.**
- Hidden plumbing commands, argv rewriting, `REMOTE_PATH` and the version probe
  go; `hello`/`welcome` revision negotiation covers compatibility.
- Per-Machine `machines:`/`connections:` and the phone's `mobile.yaml` collapse
  into one hub address per Machine.
- `lf task status` on any Machine asks the owner through the hub, live, and
  says so when the owner is unreachable.
- Remote attach is `session.attach` with tickets relayed through the hub, so
  Desktop panes for mini Sessions are ordinary views.
- A move ends in `session.continue` on arrival: the Task is running when it
  lands.
- The phone gets one address that is always on. Today it pairs with whichever
  Machine runs `lf server mobile`.

**Tension.** Kept open:
- **Stale readings.** Showing a sleeping laptop's Tasks on the phone means the
  hub keeps each Machine's last reading. That is a cache of another Machine's
  execution facts, which the current design avoids ("execution facts stay with
  the Machine that produced them"). Alternative: show only "laptop: offline,
  3 Tasks" from planning ownership, with no cached detail.
- **Relay cost.** Terminal bytes for a mini Session viewed on the laptop pass
  through the hub, which here is the mini itself. For a rental viewed from the
  laptop they take two hops.
- **No hub.** A person with one Machine needs none of this. A Machine with no
  hub named behaves exactly as today's local-only install.
- **Unverified:** whether the laptop and mini share a Tailscale owner, which
  the carrier's admission requires.

**Bare runs on another Machine.**
- A bare Session is defined as the agent being your terminal's own child, with
  no server.
- Over another Machine that is `ssh -t mini lf implement`: your terminal owns
  it, and Loopflow adds no network.
- What it needs from Loopflow is `lf` on the remote's PATH for non-login shells
  (the MTG-321 snag) and a way to name the host. `lf --machine mini <skill>`
  could be exactly that `ssh -t` exec with nothing forwarded.
- Anything more (outliving the terminal, phone view, steers) is a managed
  Session started on that Machine through the hub, which may be a
  Task-less one.
- That keeps one notion of Machines and one protocol. The ad hoc path is SSH
  with a name, not a second network.
- Open: does `--machine` for bare runs earn its place, or is
  "`ssh mini`, then `lf`" enough once PATH works?
- Jack, 2026-11-12, on this section as worded above (with "primary" for hub):
  "I think I agree with that".
- Jack, 2026-11-12, on the flag: "i think lf --machine mini is still fine
  idk", then "maybe not". Undecided. LOO-861 deletes the SSH transport under
  the flag either way and lists the flag's fate as open.
- Jack, 2026-11-12, on "the Machine that runs work uses its own preferences,
  with nothing sent from the laptop": "Not sure about that. i think thats
  probably wrong if we keep it". Open in LOO-861, not in its deletion list.
- Jack, 2026-11-12: "coudl make sense if lf --machine basically means: (1) use
  known machine routes instead of needing to otherwise know a name for te
  machine [although is there somewhere global we should be "installing"
  machine names so this isnt necessary) (2) prioritize this machine's config
  rather than the other machine's config or maybe even (3) bring some
  credentials along the way". Written into LOO-861 as the sketch for the flag.
- Jack, 2026-11-12, on the roles: "your primary machine, as a user, is a
  laptop. Some other client may or may not be where you want work to happen …
  you have another machine that's sort of like the most reliably always-on
  server". And: "it would be weird to have two different kinds of networking
  protocols or notions of what machines exist and how you connect to them side
  by side. We should prioritize having the one that gives the best experience
  for the work model".

## Rex, mapped

| Rex | Loopflow today | Loopflow with a hub |
|---|---|---|
| Server per host; clients connect; servers never talk to each other | Server per Machine; peers talk CLI-over-SSH | Server per Machine; each connects to one server, the hub |
| Listeners: SSH (`-S ssh://…`), built-in Tailscale node, Serve, tailnet address; same protocol on all | Unix socket + `lf server mobile` WebSocket on the tailnet; peer calls use a different (CLI) protocol | One protocol, local socket and tailnet carrier; SSH only to bootstrap |
| Identity from the connection: SSH login, Tailscale owner | Tailscale `whois` owner for the phone; `LF_EXPECTED_MACHINE_ID` for peers | Tailscale owner plus Machine identity in `hello` |
| Host registry is per client (`rex hosts`, Mac "Add Remote Host…"), unsynced | `machines:`/`connections:` per Machine, plus `mobile.yaml` | Presence on the hub; each Machine knows one address |
| Login server: a front door that identifies the caller and hands the connection to that account's server; account servers survive it restarting | none | Hub: a front door that routes to each Machine's server; their Sessions survive it being away |
| Personal server stops when idle; sessions end with it | Holders outlive server restarts and upgrades | Same, per Machine |
| `--tailscale` server joins the tailnet itself with a one-time auth key | Rentals reached by SSH | Rentals join the tailnet and dial the hub |
| `/llms.txt` API reference; old clients keep working; `features` announced | `lf server describe` schema; `x-min-revision` | Same, now covering Machine-to-Machine traffic |

Where Rex differs on purpose: Rex has no work model, so it never needs to know
where a session should run. Loopflow does, which is why its registry lives in
one place (the hub) rather than per client as Rex's does.

## Opportunity 2: One transfer: a move

**Misalignment.** "This Task now runs over there" has three mechanisms with
different guarantees:
- **`lf task move` / `resume --task --machine`:** stops the Graph, checkpoints,
  pushes, carries the transcript and Graph position (`MovedCheckout`,
  `MovedTask`, `MovedHistory`).
- **Pool/rental placement:** `rental::hand_off` writes only a claim and a
  handoff. A forwarded `lf task run` then carries a `TaskSource` in the
  `LF_TASK_SOURCE` env var (branch, commit, id) and requires the source
  worktree to be clean and pushed.
- **Return from a rental:** `lf machine end` copies records home read-only and
  `hand_back_in` writes a handoff. No conversation, branch or Graph position
  comes back.

`TaskSource` and `MovedCheckout` describe the same thing: the pushed commit a
Task continues from.

**Symptom.**
- `ops/task/remote.rs`: `TaskSource`, `source_for_task`, `require_checkout`,
  `require_pushed`.
- `ssh.rs`: `resolve_task_source`, which pre-publishes planning so the remote
  can find the Task. That is the same problem `arriving_task` now solves on the
  receiving side.
- `prepare_task`'s `source` parameter.
- `rental.rs`: `hand_off`, `hand_back_in`.

**Realignment.** A handoff entry is written only by a move. Placement on a pool
Machine or a rental is: pick or provision a Machine, then move. Ending a rental
is a move back, then the records copy. The receiver acquires the Task from
planning and clones on demand (this branch), so the sender pre-publishes
nothing.

**Cascade.**
- `TaskSource`, `LF_TASK_SOURCE`, `source_for_task` and `resolve_task_source`'s
  planning push go.
- `resident_args`' Linear-id rewriting goes.
- Rental give-back returns the live conversation.
- One test family (`machine_commands::task_move_*`) covers every placement.

## Opportunity 3: A Task's location is its owner; elsewhere is history

**Misalignment.**
- Where a Task is lives in several records: the ownership log (`machine_id`),
  `TaskExecutionRoute.checkout` (a local path), the sender's `tasks.worktree`
  and Graph row, and `sessions.moved_to_machine_id`.
- After a move, the laptop keeps a worktree, a checkout record and stopped Graph
  rows for a Task it doesn't run.
- Routing reconciles these with special cases.

**Symptom.**
- `work_route::moved_session` and `moved_task_conversation` (a Task that stayed
  while its conversation moved).
- `lf task status` on the sender shows its own stale Graph.
- `lf wt prune` doesn't know the Task left.
- Desktop splits local and peer Tasks (`open/workspace.rs:443-470`) and refuses
  to place a remote Task's checkout.

**Realignment.**
- Sending retires the sender's live rows. The worktree is removed once the push
  is confirmed, and the Graph and Session become history marked "moved to
  mini".
- Every live question about a Task goes to its owner (Opportunity 1).
- A Task's conversation moves only with its Task. Moving a Session alone stays
  for non-Task Sessions.

**Cascade.**
- `moved_task_conversation` goes.
- `TaskExecutionRoute.checkout` is meaningful only on the owner.
- Prune and status need no move awareness.
- Desktop shows one Task list whose rows say where each Task runs.

## Aligned areas

- **Stable ids.** TaskId derives from the Linear UUID, and a Machine's id
  survives route changes. Identity crosses Machines without translation.
- **Per-Machine stores, with planning as the shared channel.** There is no
  replicated database, and ownership travels as marked comments in the planning
  provider. Keep this; Opportunity 1 changes how Machines talk, not what they
  share.
- **The append-only ownership log** with "nothing routes to another person's
  Machine".
- **Identity checks on every received call** (`LF_EXPECTED_MACHINE_ID`), and no
  local fallback when the destination is unreachable.
- **Holders.** A held agent belongs to the Machine that runs it, and moving
  means ending it and resuming from the transcript there. The move path already
  works this way.
