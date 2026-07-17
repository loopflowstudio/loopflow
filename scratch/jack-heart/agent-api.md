# agent-api: the agent-equivalent of the Loopflow Mac app

Full design for review. Nothing filed in Linear; nothing implemented.

## Product intent

Loopflow has two equally real front doors:

1. Download the Mac app and get started.
2. Open an agent harness, install the Loopflow skill, and launch, observe, and
   steer Waves from there.

The harness agent is a Loopflow **User**: an external client acting for a person.
It is not a Wave peer, provider body, parent Run, new actor type, or new
transport. It uses the same public API as the Mac app.

This project completes the top-level composition of that API and deletes the
client-side implementations it replaces.

## Architectural position

Three in-flight cuts define foundations this project must consume.

### Architecture owns Work and status

The `architecture` worktree is cutting the runtime to:

```text
Work -> Epoch -> Run -> Launch -> optional Turn
                    \-> Wait

Steer advances Basis.
HomeId owns execution authority.
status(work) returns one status/attention projection.
```

Wave, Project, and Task are the only Work identities. The cut deletes the
Session/body/action/directive and interaction/handoff models in the current
tree. It also requires CLI JSON, Swift, monitoring, and reconciliation to use
one `WorkStatus` projection. One Home resident hosts many Work endpoints; Work
does not require one resident OS process per Wave, Project, or Task.

Agent-api does not add `WorkSnapshot`, `StatusTreeSnapshot`,
`OperationalLevel`, `WaveRuntime`, a second attention type, or another generic
Work record. It composes and renders the architecture-owned types.

### Product owns SSH account authority

The `product` worktree is making `lf ssh` carry one explicit foreground account
lease through nested processes:

```rust
enum AccountAuthoritySource {
    RemoteNative,
    LocalForwarded { lease_id: LeaseId },
}
```

Forwarded authority cannot detach or cross a second SSH hop. Remote Home
residents must therefore select the existing `RemoteNative` branch. Agent-api
does not create a credential mode beside this enum and does not weaken the
detachment guard.

### PR #1069 owns public agent packaging

PR #1069 (`lf-docs`) establishes the public delivery shape:

```text
docs/                         canonical documentation
docs/agent-api.md             human and agent long form
skills/loopflow/SKILL.md      installable external front door
website/docs/                 generated deploy copy
/docs/<slug>.md               raw Markdown page
/llms.txt                     curated machine index
/llms-full.txt                complete machine corpus
```

Agent-api extends these artifacts after its commands ship. It does not add a
second skill, agent guide, docs corpus, or site route. The current
`docs/agent-api.md` describes the Session-era API and is a semantic migration
target, not a contract to preserve through compatibility types.

### This project's boundary

Agent-api owns only:

- top-level Wave selection and lifecycle composition;
- grouping selected Waves by Home so each Home resident starts once;
- repository-scoped aggregation of architecture status;
- one Wave creation operation used by CLI and Mac;
- remote lifecycle routed as nested `lf ssh` commands;
- deletion of Mac filesystem, tmux, identity-merge, and status-fold logic;
- extension of PR #1069's Loopflow skill and two-front-door docs.

## Systems to consolidate

The current code has working pieces but too many product doors:

- `wave::run` runs a Wave foreground.
- `start_lf_session` owns safe local detachment.
- `ops::home::start_home` adds another lifecycle layer and constructs remote
  tmux-over-SSH itself.
- `MacLocalWaveAgentLauncher` constructs local tmux argv again.
- `PortfolioRepoState.createWave` writes GOAL.md and MEMORY.md directly.
- `WavesView` scans files, synthesizes ids, and merges authored Waves with
  registry rows.
- status, roadmap, and Swift currently project overlapping runtime views.

The end state keeps one implementation at each boundary:

```text
Wave creation       -> one Rust Work/Wave operation
Home resident        -> one internal foreground primitive per Home
detached lifecycle  -> lf start / lf stop
local detachment    -> one process primitive
remote transport    -> lf ssh
Home dispatch       -> one local/SSH command planner
status meaning      -> architecture WorkStatus
current planning    -> roadmap join referencing WorkStatus
presentation        -> CLI renderer or Swift view
```

The current public `lf home start|probe` suite is retired when `lf start` and
`lf status` cover those behaviors. It remains implementation, not a second
user API.

The retained implementation seam is a small operation plan, not a durable or
wire model:

```rust
struct HomeBatch {
    home: HomeId,
    waves: Vec<WaveId>,
}

fn group_waves_by_home(waves: &[(WaveId, HomeId)]) -> Vec<HomeBatch>;
fn run_home_command(batch: &HomeBatch, command: &[String]) -> Result<Output>;
```

`run_home_command` runs locally when the batch names the current Home and
otherwise invokes `lf ssh <HomeId>` with remote-native authority. `lf ssh`
alone resolves the mutable route and validates the remote's durable Home id.
Start, stop, status, and routed chat reuse this plan. It replaces `ops::home`
transport/status parsing; it does not introduce another Home record, registry,
route resolver, or command-specific router.

## Public command space

```bash
# daily User loop
lf start [WAVE ...] [--all] [--json]
lf stop WAVE [--json]
lf status [WAVE] [--json]
lf chat -w WAVE

# Wave definition
lf wave create NAME [--home HOME_ID]

# existing complementary views
lf roadmap [--json]
lf top [--json]
lf runs [--json]

# transport
lf ssh HOME_ID|SSH_DESTINATION [--remote-native] [--repo PATH] -- COMMAND...
```

Intentional migrations:

- `lf wave NAME` is retired with the per-Wave resident. `run` remains the name
  of the architecture's wake-to-wait execution authority, not a server command.
- `lf home start|probe` leaves the public surface.
- `lf ls` retires because bare `lf status` now lists the repository's Wave
  roots. Keeping an address-only list would require a parallel `WaveSnapshot`
  without adding a product capability.
- bare `lf status` is current-repo scope; `lf status WAVE` is the same status
  contract scoped to one Wave.
- roadmap retains its planning scope but references the same Work identities
  and statuses instead of copying runtime/action fields.

## `lf start`

`lf start` owns selection, Home dispatch, and result composition.

1. List stable Wave Work roots for the current canonical repo.
2. Explicit names select exactly those Waves.
3. With no names, select Waves whose owning Home is controllable by the current
   User. `--all` attempts every Wave in the repo. This relation comes from the
   Home authority model; it is never inferred by splitting an SSH address.
4. Group the selected Waves by `HomeId`.
5. Ensure each Home's one resident locally or through `lf ssh <HomeId>`.
6. For each selected Wave, record the typed `UserStart` event that resolves its
   exact `WaitOn::Event`, or reserve Ready Work with `RunTrigger::User`. A Wave
   waiting on some other fact stays waiting.
7. Attempt every selected Wave, even if one fails.
8. Return the architecture-owned status projection for the selected Waves.

The selected Wave's placement must come from the architecture-owned
`WorkRef -> HomeId` relation that exists before a Run is reserved. `Run.home_id`
is execution evidence, not configuration for a never-run Wave. Agent-api does
not preserve `WaveHome`, add another Wave-to-Home table, or recursively inspect
descendant Work to invent a placement plan. Cross-Home child dispatch belongs
to the shared Work controller; `lf start` only activates the selected Wave
roots on their authoritative Homes.

Already-running Waves are success. A Wave legitimately waiting on Child, Time,
Event, Input, Capability, or Effect is also a successful observation; `start`
does not erase its exact wait just to make it green. There is no separate
`StartResult` DTO: human output renders Work status and `--json` returns the
same status contract as `lf status`. The command exits nonzero when a Home
resident cannot be ensured, the expected Home does not match, or the requested
Start transition is rejected. A repo with no Wave Work fails with the next
concrete command, `lf wave create product`.

### Local placement

```text
selected waves = [product, intelligence]
both home_id = home_local
  -> ensure_home_resident(home_local) once
  -> make product and intelligence runnable
  -> read their WorkStatus values
```

Only `ensure_home_resident` spells detached process mechanics. The internal
foreground resident hosts every Work endpoint assigned to that Home. The Mac
app shells `lf start product`; it never constructs tmux argv.

### Remote placement uses `lf ssh`

```text
wave.home_id = home_buildbox
route(home_buildbox) = ssh://jack@buildbox

lf start product
  -> lf ssh home_buildbox --remote-native --repo src/my-repo --
       lf start product --json
```

The Wave configuration yields only the stable `HomeId`; `lf ssh` resolves that
Home's current route. Its existing raw SSH-destination form remains useful for
ad-hoc commands, but lifecycle routing always passes a `HomeId`. If several
selected Waves share the remote Home, the caller sends one nested `lf start`
containing those Wave names. `lf start` does not resolve SSH routes, implement
SSH, build a tmux-over-SSH command, or assume the Loopflow repository.

The nested command has four invariants:

1. `lf ssh <HomeId>` resolves the current destination and port from the one
   architecture-owned Home record. It creates no route cache or second Home
   identity in agent-api.
2. The same `HomeId` travels as a non-authoritative assertion. The remote
   compares it with its own durable Home id before mutation. A stale route
   fails instead of starting Work on the wrong machine.
3. A matching Home naturally takes the local branch, ensures its resident once,
   and makes the selected Wave Work runnable. Routed context may prevent a
   forwarding loop, but environment is never accepted as identity or authority.
4. `--remote-native` selects the product worktree's
   `AccountAuthoritySource::RemoteNative`, starts no foreground lease, and
   forwards no local provider, GitHub, Linear, or named-secret credential. The
   resident uses durable credentials installed on its Home. SSH connection
   authentication itself is unchanged.

The normal `lf ssh` default remains the foreground forwarded lease. A remote
start attempted from inside such a lease is still a forbidden second hop.

The first implementation may reuse the repo's path relative to the user's home
through `--repo`. A repo outside that root fails clearly until Home routing
owns an explicit remote repo mapping.

### Stop symmetry under one Home resident

`lf stop` selects the same Home and routes the same way, but it does not kill
the shared Home resident:

```text
lf stop product
  -> stop/fence product's active Wave Run
  -> record WaitOn::Event(UserStart) for product
  -> leave the Home resident and descendant Project/Task Work running

remote
  -> lf ssh <home-id> --remote-native --repo <repo> -- lf stop product --json
```

`UserStart` is an exact reference represented through the architecture's
existing `WaitOn::Event`, for example
`EventRef { source: "loopflow.user_start", id: <wait-cycle-id> }`. Stop creates
the reference and Start records the matching User fact. This adds no lifecycle
field, Event model, or Wait variant. It is not a generic Paused state or
unblock command. This is the minimal transition needed to preserve the
already-shipped per-Wave stop API after removing per-Wave resident processes.
Remote start does not ship without the symmetric stop transition.

The local domain transition is basis-checked:

| Current Wave status | `start` | `stop` |
| --- | --- | --- |
| Running | idempotent | fence the Run, then record UserStart wait |
| Ready | reserve with User trigger | record UserStart wait |
| Waiting(UserStart) | record event and reconcile current reality | idempotent |
| Waiting(other fact) | preserve the exact wait | supersede it with UserStart wait; retain history |
| Done / Abandoned | refuse with the explicit restart path | report already terminal |

Resolving UserStart does not blindly restore an old Wait or force a Run. The
controller re-reads current truth: it reserves only when useful, otherwise it
records the still-relevant Time, Child, Event, Input, Capability, or Effect wait
again. The operation takes the current `Basis`, so a concurrent Steer or
terminal transition cannot be silently overwritten.

## Status and roadmap

Agent-api does not define lifecycle or health again. It waits for the
architecture projection over:

- stable Work and current Epoch;
- active Run and fresh Run health;
- exact typed Wait;
- User or parent attention;
- Home/topology evidence;
- current Basis and legal controls.

The projection must be structurally sufficient for every client without a
second tree. At minimum, one `WorkStatus` node carries its `WorkRef`, current
Epoch/Basis, one derived Work state, optional attention, fresh Home/Run
evidence, legal controls, and child `WorkStatus` nodes. The exact type belongs
to architecture; agent-api adds only a repository scope wrapper around those
nodes.

The direct product states are more useful than a traffic light: Running,
Waiting on a named fact, Ready, Done, and Abandoned, with independent health
such as Working, Stalled, Dead, or Unobservable. Terminal and Swift may color
or glyph those values, but no green/red/black wire enum discards the reason.

Bare status adds only repository scope:

```text
lf status
  -> list root Wave Work in current repo
  -> status(wave) for each root, including child Project/Task status
  -> render or serialize the shared projection
```

Focused status uses the same DTO filtered to one Wave. `lf start --json` returns
that same shape after dispatch. The Mac app decodes those DTOs and does no
client-side lifecycle fold.

Status for remote Work uses the same transport composition:

```text
lf status product --json
  -> resolve product.home_id
  -> lf ssh <home-id> --remote-native --repo <repo> --
       lf status product --json
  -> lf ssh validates the remote HomeId
  -> decode WorkStatus, not terminal prose
```

This replaces Home-specific JSON inspection and `HomeRuntimeDto`
classification. Unreachable SSH is attached as fresh topology evidence by the
shared status projection; it is not converted into a Work lifecycle state.

Roadmap remains a current Linear planning view. It joins section/rank/plan
facts to `WorkRef` and `WorkStatus`; it does not clone Run, Wait, attention,
actions, or PR state into Roadmap-specific runtime structures. Historical Work
status remains queryable when a Project disappears from the current plan.

## Wave creation and discovery

```bash
lf wave create product --home <home-id>
```

One Rust operation creates stable Wave Work plus its GOAL.md and MEMORY.md in
the canonical main repo. It uses the architecture-owned Wave identity and
`HomeId`. It does not introduce `AuthoredWave`, an optional wire id, a second
registry, or a host string as identity.

Existing GOAL-only Waves receive stable ids through the architecture migration.
Afterward every creation path creates the id immediately. Swift no longer
synthesizes ids or merges filesystem and registry lists.

The operation does not create a Project implicitly. Planning setup stays
explicit because PM commands require a bound team:

```bash
lf pm init --wave product
lf pm project create ...
```

The Mac New Wave sheet shells `lf wave create` and refreshes from `lf status`.
It does not write files.

## Chat and steering

`lf chat` remains the User surface. The harness and Mac app are the same caller
kind; internal parent control remains `Run(RunId)`.

The architecture cut converges authored direction on `Steer`. Agent-api adds no
agent-specific message type or permission matrix. Remote foreground chat may be
run through `lf ssh <home-id> --remote-native -- lf chat ...`; it uses the owning
Home's durable state and needs no forwarded account lease. A direct Mac
transport to a remote loopback listener is outside this project's lifecycle
work.

## External skill and docs packaging

PR #1069 already publishes the conventional external skill:

```text
skills/loopflow/SKILL.md
```

Install it from the repository:

```bash
npx skills add loopflowstudio/loopflow --skill loopflow -g -y
```

It already uses standard `name` and `description` frontmatter, omits the
private `loopflow: true` builtin marker, and points agents to the raw Markdown
corpus. Agent-api modifies this file in place and teaches only verbs that have
landed.

The skill distinguishes caller authority explicitly:

- a Loopflow-launched Wave/Project/Task worker is an internal participant; it
  reports on an established radio channel and never impersonates the User in
  `lf chat`;
- an external harness invoked by a person is a Loopflow `User`; it may use
  `lf start`, `lf status`, and `lf chat` as the agent equivalent of the Mac
  app.

PR #1069 currently says both that the skill is for agents arriving on their own
and that agents never use chat. Agent-api replaces that role ambiguity; it does
not add an agent role or a second chat API.

The skill remains an executable user manual, not a client implementation. It:

1. Verifies or installs `lf`.
2. Runs `lf start` and relays exact Work status.
3. Reads `lf status`, `lf roadmap`, `lf top`, and Task detail.
4. Uses `lf chat -w <wave>` for User conversation and steering.
5. Lets `lf start` select configured Home ids and compose remote `lf ssh`
   itself.
6. Seeds a bare repo through `lf wave create`, `lf pm init`, then explicit
   Project and Task creation.
7. Never writes Wave files, launches tmux, synthesizes ids, derives status, or
   handles credentials itself.

## Delivery order

### 0. Adopt the in-flight contracts

Stack agent-api on the architecture Work/Home/status cut: it is the hard parent
for the entire project. Bring in the product SSH-authority cut after it lands,
or keep remote lifecycle as a later dependent slice; it is not the parent of
Wave identity, status, or local lifecycle.

PR #1069 is merged on main and is now the docs/skill packaging substrate.
Product still has open PR #1071. The architecture worktree is currently owned
by a running agent in a detached `HEAD` transition with local changes, so it is
not yet a valid stack target. Create the agent-api stack only after architecture
returns to a stable branch and publishes its checkpoint/PR. Then agent-api
inherits #1069 through main and updates its files in place. Bring PR #1071 into
the remote slice after it lands or as an explicit later dependency. Revalidate
this design against the final types and do not build compatibility DTOs around
the current Session/Home runtime shapes.

Done when agent-api code can name stable `WorkRef`, `HomeId`, `WorkStatus`, and
`AccountAuthoritySource::RemoteNative` directly, and stopped Wave Work uses an
exact typed `UserStart` event through the architecture's existing Wait model.

### 1. Repository status and Wave identity

Add repository-scoped status aggregation and stable Wave creation through the
architecture domain APIs. Move Mac discovery and New Wave to `lf`; delete
file scanning, writes, synthetic ids, merge logic, and client status folds.

Done when a newly created, never-run Wave has a stable id and appears in CLI
and Mac status without client repair logic.

### 2. Local lifecycle consolidation

Add `lf start`, group Waves by Home, ensure one local Home resident, route the
Mac launcher through `lf start`, and retire `lf wave NAME` plus public
`lf home start|probe`. Adapt `lf stop` to the typed UserStart wait instead of
killing a per-Wave server.

Done when repeated `lf start product` is idempotent, returns shared status, and
no Swift file constructs a tmux launch; two Waves on one Home start one
resident; stopping one Wave leaves the other and the resident running.

### 3. Remote lifecycle through `lf ssh`

Add the remote-native SSH path, HomeId destination resolution and validation,
repo routing, nested `lf start`, and symmetric `lf stop`. Delete direct
SSH/tmux construction from Home operations.

Done when a Wave assigned to a remote Home passes local start -> nested remote
start -> shared status -> routed stop without a forwarded lease, wrong-Home
mutation, or repository-path assumption.

### 4. Extend the #1069 skill and docs

Update `skills/loopflow/SKILL.md`, `docs/agent-api.md`, the docs index, and their
generated website copy. Add the install command and distinguish external User
harnesses from Loopflow-launched workers. Teach only delivered public verbs;
do not create another agent guide or retrieval surface.

Done when a fresh harness can install the skill in a waveless repo and reach
one stable Wave plus one initialized Project without inventing a command or
writing product state directly.

## Tests that prove the product

- Architecture WorkStatus Rust/Swift fixture remains the only status contract.
- Repo aggregation tests cover multiple Waves, exact Wave scope, unavailable
  Home evidence, and historical Work absent from the current plan.
- Wave creation/status tests prove stable identity, canonical-main files,
  duplicate refusal, and migration of an existing GOAL-only Wave.
- Start selection tests cover explicit names, User-controllable defaults,
  `--all`, already-running Waves, partial failure, and no-Wave guidance.
- Two-Waves/one-Home tests prove one resident start, independent Wave Run
  control, and exact UserStart-wait status.
- Stop transition tests prove a prior Time/Child/Event wait remains in history,
  Start re-evaluates current truth, and a stale Basis cannot overwrite a
  concurrent Steer or terminal transition.
- Placement tests prove a never-run Wave resolves its authoritative Home
  without reading historical `Run.home_id` or scanning descendant Work.
- SSH command-plan tests prove resolved route/port, expected HomeId, repo path,
  nested `lf start`, remote-native authority, and zero forwarded credentials.
- Wrong-route integration test proves the target Home refuses mutation.
- Remote lifecycle integration test proves start, status, and stop.
- Product lease tests continue proving forwarded detachment and second hops are
  rejected; remote-native start must not weaken them.
- Mac tests assert visible results after invoking `lf`, not tmux argv,
  filesystem merging, synthetic ids, or client-derived status.
- Skill install and bare-repo seed smoke test.
- Skill role tests prove an external harness may use User chat while a
  Loopflow-launched worker stays on radio.
- Packaging tests keep shared operating invariants aligned between the
  external skill and `LOOPFLOW.md`, and continue proving raw Markdown,
  `llms.txt`, and the generated website docs copy. PR #1069's focused packaging
  suite currently passes (8 tests).

## Open edges

- Architecture still owns the exact `HomeId` migration source and route
  observation schema. It must expose authoritative Work placement before a Run
  exists; this project consumes that decision.
- Architecture must accept the typed `UserStart` event that preserves
  `lf stop <wave>` under a shared Home resident. It uses existing
  `WaitOn::Event` plus `RunTrigger::User`, not another lifecycle or Wait enum.
- The no-argument start filter needs the Home-to-User control relation; do not
  reconstruct ownership from route text.
- Repos without a shared home-relative path need a Home-owned remote mapping.
- Cross-Home child Work must be dispatched by the shared Work controller. If
  architecture instead requires callers to start every descendant Home,
  it must expose that as one controller operation; agent-api will not build a
  second recursive scheduler.
- Agent chat can route over SSH; direct Mac attachment to a remote loopback
  listener remains a separate transport decision.
