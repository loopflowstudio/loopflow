# Research: agent-native top-level Loopflow API

## System understanding

Loopflow's agent API is the `lf` CLI plus its JSON contracts. The missing layer
is top-level composition: a User needs to launch, observe, and steer the same
Wave portfolio the Mac app conducts. This project should not add an agent
transport, agent role, runtime store, or parallel status model.

Three sources remain authoritative:

- authored Wave goal and memory live under `wave/<name>/`;
- Project and Task planning truth lives in Linear, with local snapshots;
- execution authority and evidence live on the owning Home.

The harness agent is an external Loopflow **User**, not a Wave peer. `lf chat`
is therefore already its steering surface.

### Architecture

#### The active architecture cut changes this project's foundation

The `architecture` worktree is not a nearby refactor; it is replacing the
public runtime ontology. Its target contract is:

```text
Work -> Epoch -> Run -> Launch -> optional Turn
                    \-> Wait

Steer advances Basis.
HomeId owns execution authority.
status(work) returns one projection over these facts.
```

Its durable Work identities are exactly Wave, Project, and Task through
`WorkRef`. It explicitly deletes Project/Task Session identity, body-generation
state, copied directives, `TaskActionModel`, interaction/handoff records, and
the current status enum cross-products. It also requires one status/attention
projection for CLI JSON, Swift, monitoring, and reconciliation.

The target Home is a stable `HomeId`. Hostname, socket, SSH address, and
reachability are mutable routes or observations, not identity. An unreachable
route is fresh topology evidence and does not make Work stopped or dead.

The remaining placement contract matters to `lf start`: `Run.home_id` records
where one execution happened, but a never-run Wave still needs one
authoritative `WorkRef -> HomeId` placement. Architecture must own that
relation. Agent-api must not preserve the current `WaveHome` identity/address
shape or add another registry to fill the gap.

Consequences for this project:

- Do not add `WorkSnapshot`, `WaveRuntime`, `OperationalLevel`, or a new status
  tree hierarchy.
- Do not build on `ProjectRuntimeSnapshot`, `TaskRuntimeSnapshot`,
  `TaskAttentionSnapshot`, `HomeRuntimeDto`, or `WaveHomeDto`; the architecture
  cut is removing or replacing the concepts they expose.
- `lf status`, `lf roadmap`, the Mac app, and `lf start` must consume the
  architecture-owned `WorkStatus` projection and stable Work ids.
- Bare `lf status` should replace the current address-only `lf ls`; retaining
  both would require another Wave summary contract without adding a distinct
  product capability.
- Current-plan roadmap data remains a join with Work status, not a second copy
  of runtime facts. Historical Work lookup and current planning projection are
  deliberately separate.

#### The active product cut owns SSH account authority

The `product` worktree is implementing nested, multi-provider account authority
for `lf ssh`. It already defines:

```rust
enum AccountAuthoritySource {
    RemoteNative,
    LocalForwarded { lease_id: LeaseId },
}
```

A forwarded authority is a foreground lease. Descendants may narrow it but not
widen it; remote-native accounts are invisible while it is active; detachment
and a second SSH hop are rejected. Managed credentials remain behind a broker
and the remote receives a handle, not a token bundle.

That is the correct safety boundary for ordinary foreground remote work. It
cannot back a resident launched by `lf start`, because the resident must outlive
the SSH process. Remote start must deliberately select the existing
`RemoteNative` authority source and suppress all locally forwarded provider,
GitHub, Linear, and named-secret credentials. This project must extend that
system, not add a second credential enum or bypass its detached-work guard.

#### Current lifecycle duplication

Today lifecycle has three implementations:

- `wave::run` owns foreground Wave execution and first-run registry setup;
- `ops::home::start_home` owns detached start, but builds remote tmux-over-SSH
  itself and assumes the remote Loopflow repo path;
- `MacLocalWaveAgentLauncher` builds another local tmux command.

The Mac app also writes GOAL.md/MEMORY.md directly and separately scans authored
Wave files, synthesizes ids, and merges those entries with registry rows.

The target architecture also removes the assumption that every Wave needs its
own resident process. One Home resident may host many Wave, Project, and Task
Work endpoints. `run` remains the architecture's wake-to-wait authority noun;
a resident server is not a Run.

The target therefore has one public orchestration verb, `lf start`, which
groups selected Waves by Home and ensures each Home resident once. The
foreground Home-resident primitive stays internal/diagnostic. The Mac app and
harness shell the public verb. Remote placement composes `lf ssh`.

#### Wave creation and identity

An authored GOAL currently may exist before a registry row, which forced Swift
to invent synthetic ids. The architecture contract rejects that ambiguity:
Wave is stable Work and has a `WaveId`.

`lf wave create` should create the stable Wave identity and authored files
through one Rust operation. Existing authored Waves receive ids in the
architecture migration. Clients never synthesize, merge, or make ids optional.
The goal file remains authored truth; the durable id remains identity. Neither
replaces the other.

#### Skill distribution

PR #1069 already establishes the conventional skills.sh and agent-doc package:

```text
docs/                         canonical source
docs/agent-api.md             long-form agent API
skills/loopflow/SKILL.md      installable external skill
website/docs/                 generated deploy copy
/docs/<slug>.md               raw Markdown
/llms.txt                     curated machine index
/llms-full.txt                concatenated corpus
```

The upstream CLI discovers `skills/<name>/SKILL.md` and can install one named
skill from a repository. See the
[official skills CLI README](https://github.com/vercel-labs/skills#creating-skills).
PR #1069's skill has standard public frontmatter and correctly omits the
private `loopflow: true` builtin marker.

This is the packaging substrate for agent-api, not a sibling implementation.
Agent-api must update `docs/agent-api.md` and `skills/loopflow/SKILL.md` in
place, let the existing sync step materialize `website/docs/`, and retain the
existing raw Markdown and `llms` routes. The current long-form page documents
Session-era commands that architecture is removing, so its meaning changes
after the runtime cut even though its delivery path stays.

The current external skill contains a role contradiction: its comment says it
teaches agents that arrived outside Loopflow, while its body says agents never
use `lf chat`. The target distinction is authority, not whether the caller is
software: Loopflow-launched workers use radio; an external harness acting for
a person is a `User` and uses the same chat/start/status surfaces as the Mac
app.

The skill is a manual compression of the injected canonical `LOOPFLOW.md`.
PR #1069 landed `python/tests/test_loopflow_skill_alignment.py` to pin their
shared doctrine anchors and public frontmatter, but #1091 then deleted the
doctrine-anchor half as a low-value test; only frontmatter and
self-containment guards survive on main. Agent-api extends that surviving
file when it changes caller authority; it does not add another sync system or
restore the anchor list.

Focused PR #1069 packaging verification passed: 8 tests cover README/docs-index
alignment, raw Markdown delivery, content negotiation, agent-readable 404s,
`llms.txt`, `llms-full.txt`, sitemap, and the Markdown link.

### Data flow

#### Start

The desired operation is:

```text
lf start [waves]
  -> list Wave Work in the current canonical repo
  -> resolve each Wave's owning HomeId
  -> group selected Waves by HomeId
  -> ensure each Home resident once, locally or remotely
  -> make selected Wave Work runnable when it is Ready or waiting for Start
  -> return the architecture-owned WorkStatus projection
```

Local dispatch:

```text
Wave.home_id == current HomeId
  -> ensure_home_resident(current HomeId)
  -> record UserStart to resolve WaitOn::Event, or reserve Ready Wave with RunTrigger::User
```

Remote dispatch:

```text
Wave.home_id = home_buildbox
route(home_buildbox) = ssh://jack@buildbox

lf start product
  -> lf ssh home_buildbox --remote-native --repo <relative repo> --
       lf start product --json
  -> lf ssh resolves the mutable route from HomeId
  -> carry that HomeId plus the existing route-break context
  -> remote validates expected HomeId == its local HomeId
  -> remote invocation takes the local branch and starts the resident
```

`lf start` owns selection and orchestration. `lf ssh <HomeId>` owns Home route
resolution, identity validation, and SSH transport. Its raw SSH-destination
form remains available for ad-hoc use. Home code does not build SSH or tmux
argv, and agent-api adds no route resolver.

Selection is intentionally root-scoped. Agent-api starts the selected Wave
Work on its authoritative Home; it does not walk Project/Task descendants and
construct a second cross-Home schedule. The shared Work controller must own
child placement and dispatch. If the architecture requires explicit activation
of every descendant Home, that must still be exposed as one controller
operation rather than reimplemented in this client composition layer.

`--remote-native` is a command spelling for the product worktree's existing
`AccountAuthoritySource::RemoteNative`, not a new authority type. It starts no
foreground account broker and forwards no local credentials. SSH connection
authentication is unchanged.

The HomeId check matters. A route is mutable and can be stale or misconfigured;
a recursion-break environment variable alone must not authorize starting Work
on the wrong Home. Passing `HomeId` as the `lf ssh` destination makes route
resolution and target validation one transport operation instead of two
loosely coupled arguments.

#### Stop after the shared-Home cut

Current `lf stop <wave>` shuts down one Wave listener. The target has no
per-Wave listener process to kill: stopping the Home resident would also stop
unrelated Waves and their recovery keeper.

The smallest semantics that preserve the public Wave lifecycle are:

```text
lf stop product
  -> stop/fence product's active Wave Run
  -> supersede its current scheduling Wait with WaitOn::Event(UserStart)
  -> retain prior Wait history
  -> leave the Home resident and descendant Project/Task Work alone

lf start product
  -> record the typed UserStart event and resolve that exact Wait
  -> reconcile current reality
  -> reserve with RunTrigger::User only when useful, else record the exact Wait
```

`UserStart` uses the architecture's existing `WaitOn::Event` with an exact
reference such as
`EventRef { source: "loopflow.user_start", id: <wait-cycle-id> }`. Stop creates
the reference and Start records its matching User fact. This adds no Event
model or Wait variant. The invariant matters: this is not a generic Paused
Work state, a generic unblock operation, process absence, or GOAL.md rewrite.
The typed wait makes status say exactly what can wake the Wave and allows other
Work on the Home to continue.

Superseding a Time/Child/Event wait does not delete history. Starting later does
not blindly restore it: the controller re-evaluates current truth and either
reserves useful work or records the still-relevant exact wait. Both mutations
are basis-checked so concurrent input cannot be overwritten.

The remote repo is currently expressible only as a path relative to the remote
user's home. Reusing the same relative path is acceptable for the first slice;
a local repo outside the home must fail clearly until Home routes own an
explicit repo mapping.

#### Status and roadmap

The architecture worktree owns:

```text
status(work) -> WorkStatus
```

where status is derived from Epoch, active Run, typed Wait, attention, and
fresh health/topology evidence. The product states are direct: Running,
Waiting on an exact fact, Ready, Done, or Abandoned, plus independent Run
health. A route that cannot be observed remains unknown evidence.

This project only adds scope and rendering:

- bare `lf status` gathers root Wave Work for the current repo;
- `lf status <wave>` scopes the same projection to one Wave;
- `lf start --json` returns the same status projection after dispatch;
- the Mac app decodes and renders the same DTOs;
- `lf roadmap` joins current Linear plan placement to those statuses without
  copying runtime, attention, or action fields.

Remote status is another nested
`lf ssh <HomeId> --remote-native -- lf status --json` call. The caller decodes
`WorkStatus` directly;
it does not parse a command-specific Home DTO. SSH failure becomes topology
evidence, not Work lifecycle.

A separate green/red/black enum would discard why Work is waiting and would
compete with Run health. Color and glyph are presentation over the exact typed
status, not another wire model.

#### Chat

`lf chat` already reads the journal while stopped and uses the Wave listener
when live. `POST /messages` already distinguishes send, steer, and interrupt.
The architecture target further converges authored direction on `Steer` from a
`User` or parent `Run`. The harness is the User case. No new agent chat API is
needed.

Remote User operations follow the same Home routing rule. Until the Mac app has
a remote listener transport, an agent can compose
`lf ssh <HomeId> --remote-native -- lf chat`.

### Key abstractions

This project should reuse these abstractions rather than define peers:

- `WorkRef::Wave(WaveId)`, `Project(ProjectId)`, `Task(TaskId)` for identity;
- `Epoch`, `Basis`, `Run`, `Wait`, `Launch`, and `Steer` for execution and
  control;
- architecture-owned `WorkStatus` for every status consumer;
- `HomeId` for execution authority and `lf ssh <HomeId>` for route resolution;
- one non-wire Home command planner reused by start, stop, status, and chat;
- product-owned `AccountAuthoritySource` for SSH credential lifetime;
- one Wave definition/create operation joining stable identity with GOAL and
  MEMORY files;
- one Home-resident process primitive beneath `lf start`.

The project needs no new durable entity and no new generic runtime DTO.

## Tensions

- **Current code versus target code**: the current branch's most convenient
  DTOs are deletion targets in `architecture`. Implementing against them would
  guarantee a second migration and temporarily create two public truths.
- **Status versus roadmap**: they share Work status but not scope or purpose.
  Status is durable operational truth; roadmap is current planning placement.
  Forcing both into one mega-DTO would duplicate less code but blur ownership.
- **Home identity versus SSH route**: current `WaveHome` combines owner and
  address. Target `HomeId` is stable while routes change. Start passes identity;
  `lf ssh` must resolve, validate, and use the route without making it identity
  again.
- **Foreground forwarding versus detached residency**: normal `lf ssh` should
  keep the foreground lease. Remote `lf start` must select remote-native
  authority or it is unsafe by construction.
- **Per-Wave stop versus one Home resident**: the existing stop command cannot
  keep its process-kill implementation. It needs an exact User-start Wait (or
  the API must intentionally change); stopping the shared resident is wrong.
- **Authored Wave files versus durable identity**: both are real. The fix is one
  creation/migration operation, not an optional id or another authored-Wave
  registry in Swift.
- **Cross-worktree ordering**: agent-api depends on architecture's status/Home
  contract and product's SSH lease work. It should rebase onto those contracts,
  not independently reproduce them.
- **External User versus internal worker**: PR #1069 describes all software
  callers as “agents,” but only internal Loopflow participants are barred from
  User chat. The public skill must name authority or it blocks its own second
  front door.
- **Canonical docs versus deploy copies**: PR #1069 correctly makes root
  `docs/` canonical and generates `website/docs/`. Agent-api must use that sync
  path, not edit both copies or add another corpus.
- **Injected contract versus installed skill**: both must carry shared safety
  rules, but #1091 removed the doctrine-anchor test that pinned them together.
  Nothing now catches `LOOPFLOW.md`/`SKILL.md` wording drift automatically, so
  agent-api must re-check that pairing by hand when it changes caller
  authority.

PR #1069 is merged on main and is now the docs/skill packaging substrate.
Product still has open PR #1071. Architecture remains the hard stack parent,
but its worktree is currently owned by a running agent in a detached `HEAD`
transition with local changes, so it is not yet a valid stack target. Wait for
architecture to return to a stable branch and publish; agent-api will then
inherit #1069 through main. Product remains the narrower dependency for the
remote-lifecycle slice after PR #1071 lands.

## Observations

### Complexity

`lf/commands/waves.rs` is the current status hotspot, but the architecture
worktree already claims its replacement. Agent-api should not refactor the old
file into a new hierarchy in parallel.

Remote Home start currently mixes transport, credentials, repo placement,
process detachment, liveness parsing, and command output. The clean split is:

- architecture owns Home identity and route records;
- one short-lived Home batch planner groups Wave ids and chooses local/SSH;
- `lf ssh <HomeId>` resolves and validates the route, then transports one
  command;
- `lf start` selects and orchestrates;
- the detached primitive starts locally;
- `WorkStatus` reports the result.

### Quality

Current Task attention fixtures preserve unavailable evidence well. The
architecture status projection should retain that property, but the old
`TaskAttentionSnapshot` shape should not survive merely to preserve tests.

The product worktree's SSH lease tests are a strong foundation: detachment,
second hops, remote fallback, connection loss, and redaction are already
explicit constraints. Remote-start tests should extend that matrix with the
remote-native branch.

Swift tests currently prove its duplicate behavior: raw tmux argv, authored
file writes, synthetic ids, merge logic, and client-side status folds. Those
tests are a deletion map. Replacement tests should assert visible results after
calling `lf` and decoding the shared DTOs.

### Potential

Most of the desired feature is composition:

- stable Work and one status projection are being built in `architecture`;
- remote-native versus forwarded authority already exists in `product`;
- `lf ssh` already owns remote command transport;
- `start_lf_session` already owns local detachment;
- the architecture target already consolidates per-Wave residents into one
  Home resident;
- `lf chat` already owns User steering;
- Swift already consumes `lf --json` for most surfaces.

The net implementation can shrink the codebase if it waits for and uses those
boundaries.

## Open questions

- The architecture worktree still names the exact `HomeId` migration source and
  route-observation contract as open. It must also expose authoritative Work
  placement before the first Run and own cross-Home child dispatch. Agent-api
  should consume those decisions, not settle a competing Home schema or
  scheduler.
- Architecture must accept a typed `UserStart` event through `WaitOn::Event`,
  or explicitly replace the shipped `lf stop <wave>` behavior. There is no
  coherent per-Wave process stop once a Home resident hosts many Work endpoints.
- The default `lf start` selection needs the Home ownership relation: explicit
  names are unambiguous; the no-argument form should select Waves whose owning
  Home is controllable by the current User without parsing ownership from an
  SSH string.
- Remote repo placement needs a Home-owned mapping once repos cannot be assumed
  to have the same home-relative path.
- A remote Wave's loopback listener is not directly attachable by the local Mac
  app. Agent chat can route through SSH; Mac remote chat needs a separate
  transport decision.

## Recommendations

### Make agent-api a composition project

**Observation**: architecture already owns Work identity and status; product
already owns SSH account authority.

**Cost**: sequence or stack this work after those contracts instead of coding
immediately against current DTOs.

**Benefit**: no parallel model, no throwaway migration, and a substantially
smaller implementation.

**Verdict**: required.

### Route remote start through `lf ssh` with remote-native authority

**Observation**: the current Home launcher reimplements SSH and tries to detach
under a transport designed for foreground credentials.

**Cost**: add a remote-native `lf ssh` path, HomeId validation, repo routing,
and nested start/stop tests.

**Benefit**: one transport, correct credential lifetime, and a command plan
that exactly follows Wave configuration.

**Verdict**: required. Do not bypass the forwarded-lease detachment guard.

### Express Wave stop as an exact Wait

**Observation**: one Home resident hosts many Work endpoints, while the shipped
API stops one Wave.

**Cost**: add one typed UserStart event and transition through the existing
architecture Wait/status/controller contract.

**Benefit**: preserves `lf start`/`lf stop` without a Paused lifecycle state,
without killing unrelated Waves, and without another process registry.

**Verdict**: preferred coordination change. If rejected, the command contract
must change; the old implementation cannot survive the Home-resident cut.

### Let stable Work identity own Wave creation

**Observation**: authored files without ids cause Swift discovery and synthetic
identity.

**Cost**: one Rust create/migration operation plus Mac deletion.

**Benefit**: every Wave has one stable id from creation while GOAL remains the
authored source of purpose.

**Verdict**: required.

### Preserve exact status instead of adding traffic-light state

**Observation**: the architecture projection distinguishes lifecycle, Wait,
attention, health, and topology evidence. A light enum collapses them.

**Cost**: terminal and Swift render the richer type directly.

**Benefit**: clearer APIs, honest unknowns, and no client-side state fold that
can drift.

**Verdict**: required.

### Keep the external skill declarative

**Observation**: PR #1069 already supplies the external skill and agent-readable
docs delivery. Every required operation has or will have an `lf` verb.

**Cost**: extend those sources in place, clarify external User versus internal
worker authority, add the install smoke test, and protect shared
`LOOPFLOW.md` invariants from drift.

**Benefit**: one packaging system and one executable user manual, not another
client or docs corpus.

**Verdict**: worth it.
