# One plan at every size

Design for LOO-938, 2026-11-18. Guide page: `scratch/plan-design.html`.
Exploration it came from: `scratch/explore.html`, `scratch/explore.md`.
It replaces "the hub owns the plan", which the branch currently implements.
`scratch/plan.md` builds this design with three differences listed at its
top (what a ref holds, one carrier shape, the repository id); where they
differ the plan holds.

## What you get

The same plan and the same save at every size of deployment. One Machine
keeps its plan in its store. A second Machine, a phone, teammates, a hub and
Linear are each added by one person doing one thing, and none replaces what
was there before.

## What was said

Jack Heart, 2026-11-18:
- "the plan syncing was written in a pre-machine world"
- Five people, no Linear: "for now let's assume there is no such thing as a
  shared central hub ... They need to somehow sync through Git."
- "Lets say that we wnat to have the best reasonable behavior without a hub,
  and then also support saying that there is a hub (which could b a separate
  machine or possibly just running on one person's machine)"
- "is there a design that would naturally graduate through all the tiers of
  possible deployment"
- Run state between people: "I think mostly or exclusively no." Work that
  passes between people "means a new task".
- A hundred people on Linear, on Loopflow: "Lets say literally 50%".
- First adopter on a team's Linear: "Me using Loopflow to sort of interact
  with my team's Linear doesn't get in the way."

Earlier the same day "The hub owns it" was chosen for the plan owner. On
whether this design replaces that: "Why woudd it not? it seems like it does".
The hub survives as a connection, not as the home.

## The rule

1. **A save writes this Machine's store and stamps each changed field.** A
   stamp is `(counter, machine id)`. The counter is one more than the highest
   this Machine has written or seen. So an edit made after seeing yours beats
   yours, whatever either clock says; ties go to the machine id.
2. **The plan is the newest stamp for each field**, across every Machine whose
   values this Machine has received.
3. **Things that only append never conflict**: comments, ownership entries,
   created records. Deletion is a field (`deleted_at`).
4. **A claim is first-wins**: of two claims for one Task that never saw each
   other, the lower stamp holds it. The other Machine learns on its next
   exchange and stops its run, keeping the work.

Nothing else decides. There is no conflict state between Loopflow Machines,
no causal parents, no observed heads, no routes.

## How values travel

Three ways, all carrying the same stamped values. Receiving a value twice,
late or by two ways gives the same plan.

**Git, always.** Each Machine writes one ref,
`refs/loopflow/plan/<machine id>`, holding the newest value it has written
for each field, one file per record. Only that Machine pushes it, so a push
is a fast-forward and never races. A Machine drops its own value from its
ref once it has seen a newer stamp for that field, so refs stay near the size
of what that Machine currently holds the newest word on. Reading is one
`git fetch 'refs/loopflow/plan/*'`, run by the Machine's server on one timer
per repository and at command boundaries, not per Session.

**A hub, when named.** `lf machine hub <machine>` (on the branch) names one
reachable Machine. Machines send their stamped values over the server
connection (`plan.apply`) and are sent others' (`plan.changes`). The hub
applies rule 2 like everyone else and decides nothing. It may be a separate
box or one person's Machine. When it is unreachable, Git carries everything;
nothing is configured to fall back. Several people may name the same hub.

**Linear or Asana, between people, per repository.** A repository's people
meet through Git or through a tracker, never both. The tracker carries what
it has fields for (title, state, assignee, PR link, comments) through
`tracker_actions` and its own rule (keep theirs / use mine), as today. A
person's own Machines still exchange everything through Git and the hub.

## The phone

The phone holds no plan. Every one of a person's Machines holds the whole
plan, so the phone asks whichever it can reach: the laptop, the mini, either.
A hub matters to the phone only because it is the one that stays on. A Task's
live Session is different: that comes from the Machine running it.

## The tiers

| Deployment | Action | Added |
|---|---|---|
| One Machine | install | nothing sent; the ref exists locally |
| Second Machine | push, clone | each reads the other's ref |
| Phone, always-on Machine | `lf machine hub mini` | live exchange; a Machine the phone can always reach |
| Five people, no hub | teammates clone | their refs arrive in the same fetch |
| Team hub | everyone names one Machine | seconds, not a fetch interval |
| Team on Linear | link a Wave to a Project | your slice travels through Linear |
| 100 people, half on Loopflow | nothing | same, per person |

Stepping down works the same way: a hub that disappears leaves Git.

## A team's Linear

- **Your slice.** A Wave links to a Linear Project. Loopflow stores issues
  assigned to you and the ones you create (created assigned to you).
  Everything else in the Project is read when someone looks and is never
  written. `lf task claim` assigns an issue to you and makes it a Task.
- **Whole Project** stays for a Project one person owns alone.
- **Assignment is the handoff and the lock** between people who use Loopflow
  and people who don't.
- Run state (Machine, step, Graph) is not written to Linear.

### Loopflow-only plan data on a tracker

Linear has no field for a Task's Wave or its order in the Wave. Proposed:

- **Wave membership comes from the link.** A Task's Wave is the Wave linked
  to its Linear Project, so everyone derives the same answer and nothing is
  stored.
- **There are two orders.** Jack Heart: "you shoudl be able to reorder tasks
  for other people. if you think a task is highest priority..." and "your
  order of your claimed tasks is NOT the proejct order, ut your order of hte
  backlog IS".
  - *Backlog order is the Project's order, shared.* Reordering unclaimed
    Tasks writes Linear's own manual order, so the whole team sees it,
    Loopflow or not. In a Git repository it is a stamped field like any other.
    This applies to issues outside your slice too: the reorder is written
    straight to Linear. (Linear's manual-order field in the API is unverified;
    check it before building.)
  - *The order of the Tasks you have claimed is yours.* It is not written to
    the Project and nobody else's view changes. It travels between your own
    Machines by Git and the hub.
- **Nothing Loopflow-only is stored on a tracker item.** With membership
  derived and claimed order private, no field needs it. If one appears:
  Asana has `external.data` (hidden, 32 KB, shared by users of the same OAuth
  app); Linear has only attachments, which every teammate sees as a link.

Found by research (`scratch/tracker-research.md`, 2026-11-18; vendor claims
carry sources there and are not all re-checked):
- Linear's manual order (`Issue.sortOrder`) belongs to the issue, not the
  Project. Writing it moves the issue in every manually sorted view in the
  workspace. Asana's order is per Project (`addProject` with
  `insert_before` / `insert_after`). Jack Heart: "i think backlog reorders
  still make sense probably, and i think we can accept both linear and
  asana's view of it". So a backlog reorder is written to each tracker in the
  tracker's own terms: workspace-wide in Linear, per Project in Asana.
- `linear/actions.rs` writes `prioritySortOrder` with `sortOrder` today, which
  moves Priority-sorted views; Asana order is retired as unsupported.
- A Wave links to a Project on both: a Linear Initiative and an Asana
  Portfolio hold Projects, never Tasks, and Portfolios need a paid tier.
- **Teams and Initiatives matter only when a new Wave is made.** Jack Heart:
  "i think we also should just need a default initaitive or team for creating
  NEW waves, but we shouldnt need to care about the team/initative in any
  other way". A repository names one default container for new Waves' Projects
  (Linear: a Team, optionally an Initiative; Asana: a Team, which in an
  organization every Project must be created in). Linking a Wave to an
  existing Project asks about neither. The research missed Asana Teams: they
  hold Projects, never tasks, and are not behind the paid tier (from general
  knowledge, not re-checked).
- **A Task with no Wave is an issue with no Project.** Jack Heart: "sounds
  good." Created in Loopflow: Linear puts it in the repository's default
  Team with no Project; Asana gives it no Project, so it sits in the person's
  My Tasks and the shadow repository project goes. Assigned to you from
  outside: it goes to the repository when exactly one of your repositories
  could own it (Linear: one repository names that Team as its default; Asana:
  one repository uses that workspace), and otherwise shows under
  `lf task list --available` until `lf task claim` from the repository you
  want. Jack Heart: "how about if it is in the team that is the default team
  for a repo, it goes into that repo...though i guess that becomes a problem
  if two repos have the same team".
- Four things on `main` disturb a shared workspace and block "your slice":
  the repository marker written into a Linear Team's description; refusing
  Projects with several Teams or not exactly one Initiative; ingesting every
  open issue of a Team; writing `external.gid` onto every open Asana task in
  the Project during a read.
- Polling "assigned to me" alone never sees an unassignment; the query also
  names the issues already held.

Passed over: a label group per Wave (membership only, and it fills the team's
labels); markers in the description (people edit it); a Git ref
beside the tracker (two meeting points for one repository).

## What is deleted

- `planning_peer_changes`, `planning_peer_observed`, `planning_peer_routes`,
  field causality, destinations, the shared document on `shared/main`, and the
  "lost values" view for Git.
- `PlanningSync`'s per-Session thread and tick.
- LOO-899 (peer edit without full field history) has nothing left to fix.

## What the branch has, and what changes

Kept: `lf machine hub`, `plan.changes` / `plan.apply` (protocol revision 9),
`hub_sends`, `lf repo tracker`'s hub line, Desktop's "where the plan lives".

Changes:
- The hub is no longer the plan's home. A Machine with a hub is still
  authoritative for its own writes; the "first contact pulls before it
  pushes" rule goes, because stamps make order of arrival irrelevant.
- "The change sent last wins" becomes "the newest stamp wins". `plan.apply`
  carries stamps; `plan_changes.seq` stays as the hub's cursor only.
- Git plan sync comes back in the per-Machine form above. The deleted journal
  does not.

## Moving what exists

etude is the only repository planning through Git (397 records on
`shared/main`, 22 on a personal ref). Each Machine's first exchange stamps
its store's current values from the journal clock it already holds and writes
its ref. The old refs are left; a released `lf` keeps reading them and does
not see new changes, so the first exchange says which Machines still run it.

## Demo

1. On the laptop, in etude with the network off: `lf task create 'x'`.
   `lf repo tracker` shows `origin · 1 unsent`.
2. Network on. On the mini: `lf task list` shows it within one fetch.
3. `lf machine hub mini` on the laptop; retitle on the mini; the laptop's
   open list changes without a command.
4. Stop the mini's server. Retitle on the laptop. A third checkout (another
   person's identity) fetches and sees the title: Git carried it.

## Tradeoffs

1. **A losing edit vanishes from the plan.** Two people change one field; the
   later stamp wins and nothing marks the other. It is in the loser's ref
   history (`git log`), not in any view.
2. **Without a hub, two people can start the same Task.** The window is one
   fetch interval. The later claim's run stops when it learns.
3. **Nobody is in charge of the plan.** Anyone who can push to the repository
   can write any field of any Task. A hub does not check or refuse.
4. **The plan is as private as the repository.** Every clone can fetch the
   plan refs.
5. **One ref per Machine, for as long as it holds a newest value.** Rented
   Machines add refs. A Machine that is gone keeps its ref until another
   adopts its values.
6. **Fetching on a timer returns**, once per repository per Machine server.
   Fresh within the interval, not at once, unless there is a hub.
7. **Two conflict rules stay in the product.** Stamps between Loopflow
   Machines; keep theirs / use mine with a tracker.
8. **Reordering the backlog reorders it for everyone**, including teammates
   not on Loopflow. Chosen. The order of claimed Tasks is per person.
9. **Teammates do not see each other's runs.** Only what the plan holds.
10. **Part of today's build is undone.** The branch deleted Git plan sync;
    this brings a smaller one back.

## Open

- Fetch interval without a hub, and whether `lf task claim` and `lf task run`
  fetch first (they shrink tradeoff 2 to the length of one fetch).
- Who adopts a gone Machine's values, and when.
- Whether a teammate needs anything beyond network reach to use someone's hub.
- A non-user reassigns or closes an issue while its Graph runs: stop and keep
  the work, or keep running and say so on the issue.

## Slices

1. Stamps in the store and on `plan.apply` / `plan.changes`; newest-stamp
   apply; hub no longer authoritative. Demo steps 1–3 with the hub only.
2. Per-Machine Git refs: write, fetch, prune; the server's one timer; journal
   tables marked leaving. Demo step 4. etude moved.
3. First-wins claims across Machines, and fetch-before-claim.
4. Your slice of a Linear Project (its own Task if this one is full).
