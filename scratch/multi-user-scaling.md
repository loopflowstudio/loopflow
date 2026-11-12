# Does the hub design scale to a team or a company-hosted setup?

Not being built now. Jack, 2026-11-12: "These are things I sort of explored
and denied later on and I don't think I want to pursue this right now. At the
same time I think it might be instructive as sort of just a scalable model. If
we're doing something that kind of rules that out, then we should think about
it for sure."

Standing constraint (2026-11-02): "stay within the grounds fully free, open
source, self-hosted, no such thing as a Loopflow Studio public server." So
"hosted" here means a company running its own hub and Machines.

Companion to `scratch/simplification-opportunities.md` (hub and desk).

## Direction

Jack, 2026-11-12: "I dont care too much about sharing sessions between team
members. Its an interesting curiosity but not something we need to ensure works
now. However, what I do want to build towards is (a) DEFINITELY having
consistent task management when multiple people are using linear or the git
based planning, so that its easy for people to create tasks and others to take
them or see tha others have taken them (b) MAYBE some notion of a shared hub
and shared machines for work so that (even if we dont have a vendor cloud)
teams can self deploy dev cluds".

### (a) Consistent Task management across people

Built and covered by `tests/two_people.rs` (LOO-555), on Git planning: one
person's new Task appears for the other after a sync; owners show in every Task
list; the claim log reads as dates and names; a steer from one reaches the
other's Task; a private Machine shows only its id.

Gaps, from reading the code (none reproduced with two real people):

1. **Two "who has it" fields.** Linear's assignee is a planning value
   (`tasks.planning_assignee`). Ownership is a separate log built only from
   `loopflow-ownership` marker comments. A claim writes the assignee
   (`assert_assignee`), but an assignment made in Linear's own UI writes no
   ownership entry. A teammate who assigns themselves in Linear reads as
   unowned to routing.
2. **Taking someone's Task is silent.** `docs/dev/machines.md`: for a Task that
   is "Someone else's", the writer is "Your own Wave Machine; your later claim
   wins". `lf task run` on a teammate's Task takes it with no word that it was
   theirs.
3. **Others' claims arrive on sync.** A claim made elsewhere shows after the
   next planning sync. Two people can claim inside that window; the log settles
   it afterwards. A hub that receives Linear webhooks closes the window.
4. **One person, two identities.** Ownership names a person by Git
   `user.email` (`person_key`); the Linear assignee is the Linear viewer id.
   Nothing ties the two, so Linear and `lf` can show different names for one
   owner.
   Jack, 2026-11-12: "This should definitely be cleaned up".
   A fourth identity exists too: `store/sqlite/github_people.rs` caches the
   GitHub user behind each owner email (`task_owner_emails`).
5. **Linear has no two-person test.** `two_people.rs` exercises Git planning
   only.
   Jack, 2026-11-12: "yeah this is a problem with me being a solo dev. cant
   really do much authentic here, though i suppose we could add a second
   account to linear or whatever just to play around eventually".

Sketches:
- With Linear planning, the assignee is who has the Task, and the marker adds
  only which Machine. An assignee with no marker is "taken, Machine unknown",
  not unowned.
- Running a Task another person holds says whose it was and when they took it,
  then proceeds. It doesn't refuse.
- Add a two-person Linear World beside the Git one.

### (b) Shared hub and shared Machines

A "shared Machine" can't mean several people in one Machine: a Machine is one
OS user's authority, and its provider logins are that person's. Paradev's
shared hosts hand every member the host's agent sign-ins.

Shape that fits what exists: the **pool** is shared and the Machine is personal.
A team hub admits several people. A pool declared for the team names hardware.
Placing a Task there gives the person their own Machine on it (an OS user or
container), which dials the hub; they connect their own provider logins with
`lf machine connect`. Pools and on-demand rentals already do this per person;
the new parts are the declaration living with the team and the hub admitting
more than one person.

Of the six items under "What in the hub design would rule a team out", (b)
needs 1, 2, 3 and 6. Item 5 (terminals readable by the hub operator) matters
once the hub isn't your own machine. Session sharing is out of scope.

### Person identity, in levels

Jack, 2026-11-12, on the Git email as routing key: "This is probably somethign
that should be fixed, though i think we might need multiple versions to keep
single player simple".

Today a person has four unrelated identities: Git `user.email` in ownership
entries (`person_key`), the GitHub user cached behind that email, the Linear
viewer id as assignee, and the Tailscale login the carrier admits. Sketch of levels, each adding one fact and nothing to configure below
it:

| Level | Who is "me" | Authenticated by | Needed when |
|---|---|---|---|
| Alone | The OS user who owns this Machine. Git name and email are display only | The OS | One person, any number of Machines |
| Shared planning | The planning provider's person: the Linear user, or the Git author under Git planning | Linear's token; under Git planning, nothing unless commits are signed | Two people share a repository's Tasks |
| Shared hub | The identity the hub's connection proves, bound once to the planning person | The carrier (Tailscale login today) | Several people dial one hub |

Consequences:
- Ownership entries name the planning person, not the Git email, wherever
  planning has people. That removes gap 4 under (a): Linear and `lf` show the
  same owner because it is the same id.
- Under Git planning with unsigned commits, a claim is a statement, not a
  proof. That is the trust a shared Git repository already has.
- Alone, nothing is asked: no account, no binding, no person field to fill.

### Servers are personal; the hub may be shared

Jack, 2026-11-12: "i think like rex allows multiple servers, we should allow
multiple servers on a machine per person. not clear those need to be shared.
but the hub might need to be shared".

Naming: one computer can already hold several Loopflow Machines.
`docs/dev/machines.md`: "Two users on one host, or two data directories, have
separate machine identities", each with its own server (development Machines
run as separately named launchd jobs). So "multiple servers per person on a
computer" is several Machines on one host, which exists.

Mapped to Rex's server modes:

| Rex | Loopflow |
|---|---|
| Personal server: runs as you, only you use it | A Machine's server. Stays personal: its store, holders and provider logins are one person's |
| Several account servers on one host, each as its own OS account | Several Machines on one host, one per person (or per data directory) |
| Shared server: everyone in a group sees and types into every session | Not wanted. Session sharing is out of scope |
| Login server: a front door that identifies the caller and hands the connection to that account's server; holds no sessions itself | The hub: identifies the person, routes only to that person's Machines, holds no Work |

One difference makes Loopflow's version smaller. Rex's login server must run
on the host and listen there, because clients dial in. Loopflow Machines dial
out, so a shared host needs no front door of its own: each person's server on
it dials the hub directly.

What that asks of the hub's shape, so it can be personal now and shared later:
- **The hub part reads no Work.** It holds presence, routes requests and
  receives webhooks. It never opens Tasks, Sessions or provider logins of the
  Machine it happens to run on.
- **Personal hub** = your always-on Machine's server with the hub part turned
  on. This is the mini.
- **Shared hub** = the hub part running as its own OS user with no person's
  Work beside it, admitting several people.
- The same code either way; the single-person case is a shared hub with one
  person in it.

One hub per Machine. Jack, 2026-11-12: "I think you can only have one hub, but
you can run a hub in multiplayer mode, because the hub doesnt actaully launch
work. it relates to a server somewaht the way a server relates to a holder".

The layers, each keeping what the one above may lose:

| Layer | Keeps | Opens | When the layer above is gone |
|---|---|---|---|
| Holder | PTYs and their agents | No store | Agents keep running; a new server reattaches |
| Server | One person's store, Work, hooks, Graph launches | Its holders | Local work continues; a new hub connection restores reach |
| Hub | Connections: who is reachable, and routes between them; webhook intake | No Work, no logins | — |

- Nothing a layer holds has to be rebuilt by hand: a hub's presence comes back
  as Machines redial, as a server's terminal registry comes back from its
  holders.
- The direction differs in one place. A server starts its holders; a hub starts
  nothing, and servers come to it.
- Multiplayer is a mode of the one hub: it admits several people and routes
  each only to their own Machines. Single-player is that with one person.

Follows from one hub:
- Someone on a team dials the team's hub from every Machine, including the
  ones at home, and their phone reaches all of them through it. A second,
  private hub for home Machines isn't available.
- That makes item 5 below (the hub operator can read relayed terminals) apply
  to personal Machines too, so direct routes or end-to-end encryption matter
  sooner in multiplayer mode than they would with two hubs.

Open:
- On a multiplayer hub, who may see that a Machine exists? Rule so far: a
  person sees only their own Machines; teammates see Task ownership through
  planning, as now.

## What the others do

Read 2026-11-12 from public repos and docs at these commits; nothing was run
and no code is used. Paseo `b5c25e5c`, T3 Code `63d5c6df`, Paradev `e5bac946`
(formerly Superset), HumanLayer `2d41fbc6` (now the "riptide" stack). Paseo and
T3 Code were read from their checked-in docs, not implementation source.

| | Paseo | T3 Code | Paradev | HumanLayer |
|---|---|---|---|---|
| Central piece | Relay (E2E-encrypted, untrusted) plus optional **Hub** control plane, hosted or self-hosted | T3 Connect: hosted broker; traffic rides a per-machine Cloudflare tunnel | Hosted API, relay, realtime, sandbox gate; relay off by default | Hosted API + Postgres + sync; required |
| Who dials | Daemon dials relay and Hub | Machine runs `cloudflared` outbound | Host dials relay; a fresh dial-back socket per stream | Daemon dials out only; no tunnel to a host |
| Host-to-host | Daemon A saves host B; one-way, no second hop | "Peer links", 30 days, OAuth by pairing code | Through relay | None |
| Where agents run | Your machines | Your machines | Your machines and vendor sandboxes (Vercel Sandbox) | Your machines; an agent skill provisions third-party compute |
| Team layer | Only in Hub: orgs, seats, API keys. Daemons have no accounts, only principals with permissions | None: personal accounts, 24-hour guest links | Org owns hosts; roles owner/member; cloud workspaces shareable | Org with roles; **Tasks** carry ACLs and are shared by default; hosts are per user |
| Isolation on a shared host | None: a connection is the daemon's OS user | Same | "Anyone you grant access to can reach everything the host can, including its agent sign-ins" | Daemon only receives its own user's sessions |
| Source of truth | Per daemon (JSON + provider transcript) | Per environment (SQLite) | Per-host SQLite and PTYs; cloud Postgres for orgs, tasks, chats | Cloud Postgres for tasks, sessions, events |
| Host offline | Hub dispatch fails; "nothing is queued" | Threads unreachable; cached projections readable | Workspaces unreachable; automations fail | History still readable; session marked failed after ~2 min silence |
| Moving work | No. Delegates a child agent to another host | No. "Existing threads stay where they started"; delegates | Yes: one branch plus a re-homed Claude transcript bundle, driven by the desktop | No. Pushes the branch, writes `handoff.md`, starts a new session |
| Credentials on a remote | Each host's own; never carried | Stay with the environment | Own hosts: sign in yourself. Sandboxes: injected into outgoing requests, never copied in | Log in again on the new host |

Three things they agree on:
- Machines dial out to a central piece. Nobody requires inbound reachability.
- The isolation unit is the OS user, container or VM. Nobody isolates people
  inside one daemon.
- Provider credentials are not carried between people.

Where they split is what the team shares: the **host** (Paradev) or the
**Task** (HumanLayer). Paradev's version means sharing sign-ins. HumanLayer's
keeps hosts personal and puts access rules on the work.

## Where Loopflow already stands

- A Machine is one OS user's authority. Two people on one host are two
  Machines. This is the isolation unit the others converged on.
- Tasks are already shared between people through planning (Linear, or Git
  planning). Ownership is (person, Machine), and "nothing routes to another
  person's Machine". `tests/two_people.rs` covers two people on one
  repository. This is HumanLayer's split: share the Task, keep the host
  personal.
- The team's database already exists and isn't Loopflow's: the planning
  provider. No cloud store has to be built for people to see each other's
  Tasks.
- `lf task move` carries branch, transcript and Graph position. Only Paradev
  does the same.
- Rentals give a person their own Machine on other infrastructure. A
  company-hosted setup is that, with the company's hub: each developer gets a
  Machine (OS user or container) on company hardware, and it dials the company
  hub.

## What in the hub design would rule a team out

Each is cheap to avoid now and expensive to undo later.

1. **Admission by "the owner".** The phone carrier admits "another untagged
   node of the approved owner". If the hub treats every connection as that one
   person, the Machine list has no person in it.
   Keep open: every hub connection is (authenticated person, Machine), and
   routing checks that both ends are the same person. With one person the
   check always passes.
2. **Identity fixed to Tailscale.** A company may use something else. The
   carrier already authenticates before upgrade.
   Keep open: one function answers "who is this connection", with Tailscale
   `whois` as today's only answer.
3. **Person identity is the Git email.** `person_key` in ownership entries is
   unauthenticated. Among your own Machines that's fine. On a team hub, the
   hub would bind `person_key` to the authenticated person.
   Keep open: don't use `person_key` as the hub's routing key.
4. **The hub keeping other Machines' readings.** This is the open "stale
   readings" choice. For one person it's a convenience. For a team it's a
   second copy of everyone's execution facts that needs access rules, which is
   the cloud database HumanLayer runs.
   Keep open: the hub stores presence only (who is connected, last seen). Task
   facts come from planning and live detail from the owning Machine. Paseo's
   Hub made the same split: connecting grants presence, and executing is a
   separate permission.
5. **The hub relaying terminal bytes in the clear.** On your own mini that's
   your own machine. On a company hub it makes the hub operator able to read
   every terminal. Paseo encrypts end to end; T3 calls its broker trusted.
   Keep open: a ticket names where to connect. Today that's the hub; later it
   can be the Machine itself when a direct route exists.
6. **Webhooks assumed to be one person's.** A team has one Linear workspace
   and one webhook endpoint.
   Keep open: the hub routes an event by the Task's owner (person, Machine),
   which the ownership log already records. One person is the same code.

Nothing else in the hub-and-desk design assumes one person: Machines dial the
hub, each keeps its own store and holders, and placement stays a personal
setting.

## What a team would add later, and where

Listed so the single-person design doesn't grow into these by accident.

- **Sharing a Session** (two people typing into one terminal). Views already
  allow any number of typists. What's missing is letting another person's
  client attach, which is an access rule on `session.attach`.
- **Cross-person handoff** of a Task. Deferred today
  (`docs/dev/machines.md`). The ownership log supports it; routing refuses it.
- **Shared pools** of Machines no one person owns. Pools exist per person.
- **Reading a conversation while its Machine is off.** Only HumanLayer has
  this, by storing every event centrally. Loopflow's answer today is the
  provider transcript on the Machine, plus Task thread reports in planning.

## Note on the word

Paseo also calls its control plane "Hub", with the same shape (daemons dial it,
self-hostable, team layer on top). Its Hub dispatches triggers and doesn't
relay terminals.
