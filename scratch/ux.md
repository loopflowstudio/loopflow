# Desktop product UX

## Seed

> I want to give my feedback on the Loop Flow desktop app. To this day I still
> mostly work in the terminal in Claude Code and Codex Sessions directly and I
> don't really find the wave chat to be useful. There's just a lot of cleanup
> and design work and polish and bug fixing. The scope of this session is to
> gather that all into a road map project and really try to kick start the
> product wave and especially the max surface UX project or maybe some new
> combination or new definition in that general space.

## Existing product tension

The Product Wave says Loopflow is not one interface. Its Mac Surface UX Project,
however, currently measures success partly by whether the app becomes the only
surface needed to drive waves for a full week. The user still chooses direct
Claude Code and Codex terminal sessions, and does not find Wave Chat useful.

Do not assume this is an adoption failure to repair. Explore whether the Mac app
should replace terminal work, frame and amplify it, or own a different part of
the work entirely.

## Discovery

- What the user opens the app hoping to see or do
- What pushes the user back to a direct terminal session
- Which parts of Wave Chat are redundant, awkward, unreliable, or conceptually
  misplaced
- Which rough edges are bugs or polish and which reveal a wrong surface model
- Whether Mac Surface UX remains one measured bet or should be redefined or
  split along a more truthful boundary

## Feedback log

### Repository and Wave navigation

> The first thing is that the sidebar—I don't like that the repos and the waves
> are in two separate sections. I think that the repo list should be a drop-down
> that sits at the top of the wave list. That should help with another problem,
> which is just that a lot of wasted space exists where we're basically trying
> to squeeze too much text into a single line. It has this very horizontal shape.
> And you can't actually read the sentences, so you're just left a little bit
> more confused. It's not actually helping.

Observed in the supplied screenshot:

- The permanent Repo column and permanent Wave column present a parent and its
  children as peer navigation regions.
- Repository selection belongs at the top of the Wave list as a scope control.
  It should not consume a full-height column.
- Wave rows spread a title, status pill, and prose preview across a wide single
  line. The preview truncates before it communicates useful meaning.
- The layout spends substantial width on navigation while still failing to make
  the Wave description readable. The empty lower portion makes the density at
  the top feel especially accidental.

Direction to explore: one navigation column, with a repository picker above a
more legible Wave list. Do not preserve prose excerpts merely because more width
becomes available; decide what information helps someone choose the next Wave.

### Main pane vocabulary and information hierarchy

> Once we get to the main pane then it's interesting that the most prominent
> thing is “no registered waves in this repository.” I'm not exactly sure even
> what a registered wave really means. The timestamp is really long. It just
> seems like the more illegible the concept, the more prominent the wording. I
> think this is where we need to be really ruthless about defining what are the
> concepts we're trying to represent on screen and just making sure that random
> stuff didn't creep into what is in the main UI.

> There is a lot of information that I want to display for a wave:
>
> - Projects
> - Tasks
> - Active sessions
>
> So there needs to be really good progressive disclosure work done here to
> figure out what gets revealed when.

The current pane elevates machinery over meaning:

- “Registered Wave” is not yet a user-legible concept and conflicts visually
  with the Wave list already shown at left.
- A full machine timestamp exposes freshness with maximum precision and minimum
  interpretability.
- The largest element is an implementation-shaped empty state, while the Wave's
  plan and live work have no visible representation.

The surface needs an explicit concept inventory. Every persistent element must
answer a user question. Projects, Tasks, and active Sessions are distinct kinds
of information and should not be collapsed into one status or dumped at equal
weight. Define what is visible at repository, Wave, Project, Task, and Session
levels, then reveal detail through selection or expansion.

### A running Wave is neither legible nor actionable

> When you look at a wave that is theoretically running, you see a lot that's
> wrong. The work map is unavailable. The chat is a whole bunch of failed
> inscrutable log lines. There are references in the chat to issues but those
> issues aren't linked. They don't have popovers. They're not well organized.
> This is not comprehensive.

Observed in the supplied screenshot:

- The Wave says `Running`, but its plan region says `Work map unavailable` and
  prints an internal Project UUID plus a CLI repair command into the primary UI.
- Wave Chat gives repeated `Attempt failed` records similar visual weight to a
  useful authored update. Failures crowd out the thread rather than rolling up
  into one diagnosis with detail on demand.
- Error copy exposes database schema compatibility and trace-capture machinery
  without translating the user impact, owner, or next action.
- References such as `W2-135`, `W2-141`, `PR #889`, and flow names are dead text.
  They neither navigate nor preview their target.
- A narrow Objective column renders the charter as oversized wrapped prose. The
  information is technically present but hard to scan.
- `Stop` is prominent, while attaching to active work, understanding what is
  running, and reaching referenced work are not apparent.

These are at least four distinct design obligations:

1. **Reliable projection:** a Wave detail view must still explain its plan and
   live work when one source is stale or broken.
2. **Failure curation:** repeated machine failures roll up; the primary surface
   states impact and recovery, with raw evidence behind disclosure.
3. **Typed references:** Tasks, PRs, Projects, Sessions, flows, and Waves render
   as interactive objects with consistent navigation and compact preview.
4. **Thread structure:** authored decisions, progress, requests for attention,
   and machine diagnostics need different hierarchy. A chronological transcript
   alone is not sufficient organization.

### Wave operational lights

> I want green/red/black light indicators for running / stopped but with some
> outstanding work / off.

Replace text pills such as `Running` and `Idle` in the Wave list with a compact
three-light operational signal:

| Light | Meaning |
| --- | --- |
| Green | The Wave is running. |
| Red | The Wave is stopped even though outstanding work expects attention or execution. |
| Black | The Wave is off; no process is expected to be running. |

This is not a generic health color. It combines desired intent with observed
liveness, preserving the difference between an intentionally off Wave and one
whose work has stopped. The exact reason, evidence freshness, and legal action
belong behind hover, selection, or disclosure rather than in the list label.

> I want to (subtly/tastefully) allude to HAL 9000 with the indicator.

Treat the light as a small recessed glass lens rather than a flat status dot:
a controlled inner glow when lit, a faint specular highlight, and reflective
near-black glass when off. Keep the reference implicit. Avoid an eye-shaped
housing, novelty copy, conspicuous pulsing, or other sci-fi decoration that
would turn an operational signal into a theme.

Placement remains an open design choice:

#### One minimal lens per Wave row

- Status stays attached to the Wave it describes.
- Wave ordering remains stable as processes start and stop.
- Repetition can become visual noise, so the lens must remain small and the row
  should not repeat the state as a pill.

#### Three grouped sections: Live / Blocked / Off

- The list becomes an operational triage view, and each section needs only one
  lens in its heading.
- Waves move between sections as state changes, weakening spatial memory.
- `Blocked` is stronger than “stopped with outstanding work.” Use it only if the
  shared state can prove a blocker; otherwise choose language that truthfully
  includes crashed, stalled, unreachable, intentionally paused, or awaiting
  attention states.

Decision criterion: is the Wave sidebar primarily for finding a known Wave or
for finding the Wave that needs attention next?

Decision for now: keep a stable, ungrouped Wave list with one minimal lens per
row. The expected scale is only a handful of Waves, and stable recognition is
more valuable than status-driven grouping. Revisit grouping only after real
Wave count or attention-triage behavior makes it necessary.

### Selected Wave: objective and Projects first

> I think a ~1 sentence objective would be nice to lead with. After that I think
> it should have Projects. For projects I want their most prominent qualities to
> be # of open tasks and their KR list.

Default reading order:

1. Wave name and operational lens
2. Approximately one sentence stating the Wave objective
3. Projects
4. Deeper Task, Session, evidence, and conversation detail on disclosure

Each Project should make these facts easiest to scan:

- Project name
- Number of open Tasks
- KR list and which KRs currently hold

Do not give Project summary prose more prominence than those facts. The Project
is a measured bet; open work shows its current load, and KRs show the proof that
defines completion.

Open contract question: should Wave Objectives be authored to fit this one-line
surface, or should the UI disclose a longer canonical Objective after presenting
a deterministic excerpt? Do not silently generate a second summary that can
disagree with `GOAL.md`.

### One operational grammar at every work level

> Again I want to be able to basically see for each Project and Task whether it
> is green/red/black.

Use the same minimal lens for Waves, Projects, and Tasks. The color must keep the
same meaning at every level; do not invent separate Project health and Task
status palettes.

A parent lens should describe that entity's own Session and intent, not silently
aggregate all descendants. A green Project may contain a red Task, and both
facts must remain visible. Otherwise a single parent color cannot explain which
work is alive or stopped.

Candidate shared semantics:

- **Green:** this entity's intended Session has a live body.
- **Red:** this entity still expects execution, but its body is stopped or
  otherwise not advancing.
- **Black:** this entity is off or terminal; no live body is expected.

This requires desired execution intent in addition to process liveness. Open PM
work alone cannot make every unlaunched backlog Task red, or the signal becomes
permanent alarm noise.

### Separate Chat from Supervision

> Then the next most important thing is to separate
>
> CHAT and SUPERVISION.
>
> CHAT should be the default UX. We want to make sure the chat stream that you
> get when you use the desktop app:
>
> - is robust to previous failures and changing bodies
> - is always fast (both to load and to respond), clear, simple
>
> SUPERVISION:
>
> This is something that takes some interactions to get to. Then this should be
> divided basically into:
>
> ACTIVE SESSIONS: See all running task, project, execs
>
> RUN HISTORY: leave alone for now but do it later

Chat and Supervision are separate product modes:

#### Chat — default

Chat is the default destination after selecting a Wave. It is the durable human
thread with that Wave, not a chronological dump of execution diagnostics.

- Thread continuity belongs to the Wave and survives agent/body replacement.
- Historical malformed events, failed trace capture, or unavailable machinery
  must not prevent the thread from loading or accepting a new message.
- Initial content paints quickly; sending and visible acknowledgement feel
  immediate; response streams without waiting for unrelated plan or supervision
  queries.
- The primary transcript is simple and readable. Operational failures appear
  only when they change what the user needs to know or do, with detail behind
  disclosure.

#### Supervision — secondary

Supervision requires deliberate navigation from the default Chat surface. It
contains operational truth and controls without turning Chat into a process
console.

1. **Active Sessions — now:** show all live Task, Project, and execution bodies,
   with their hierarchy, state, and route to attach or inspect.
2. **Run History — later:** retain the destination in the information
   architecture if useful, but do not implement or give it primary space in the
   current slice.

The distinction should also govern data loading: Chat does not block on the
Project map, process inventory, trace capture, or Run History.

### Working composition: persistent context + Chat

Two visual directions were compared:

- **Persistent context:** Objective and Project/KR context remain visible in a
  left rail beside default Chat.
- **Chat first:** Chat occupies the full pane; Objective and Projects live under
  a separate Overview destination.

The user prefers the persistent-context direction. Carry this forward as the
working composition:

```text
┌─ Wave navigation ─┬─ Objective + Projects ─┬─ Chat ────────────────┐
│ repo picker       │ one-sentence objective│ durable Wave thread   │
│ stable Wave rows  │ Project cards         │ linked references     │
│ status lenses     │ open Tasks + KRs      │ simple composer       │
└───────────────────┴────────────────────────┴────────────────────────┘
```

Supervision opens deliberately from the Wave header rather than occupying the
default composition. The mockup establishes hierarchy only; its sample KRs,
chat content, card styling, lens size, and spacing remain subject to refinement.

The reason this direction wins is not merely that Chat gets a narrower reading
measure. Objective and Projects deserve persistent focus as first-class Wave
content; they are not auxiliary metadata or a context list subordinate to Chat.
The composition should make plan and conversation feel like two parts of one
Wave rather than primary content plus a sidebar.

Mockup correction: do not render Task and PR labels as detached context bullets
or duplicate a plain reference with a separate `See W2-135` chip. Link the Task
or PR reference itself inline in the authored sentence. Hover can preview the
typed object; selection navigates to its detail.

### Preserve the basic structure while changing its meaning

> This is also more similar to what we already have, but there are enough other
> changes going on and I'm curious to see what it's like while keeping the same
> basic structure.

Use the current app's basic composition as the first redesign scaffold:

1. navigation,
2. Objective + Projects,
3. default Chat.

Do not combine the information-architecture correction with a wholesale shell
replacement. The meaningful changes are already substantial: fold repository
selection into Wave navigation; replace prose snippets and status pills with a
stable list and operational lenses; make Project proof legible; make Chat
durable and quiet; and move process inspection into Supervision. Keeping the
basic structure makes those changes easier to evaluate and ship incrementally.

### Future Wave hierarchy

> I also think the hierarchy of it is necessary eventually. I expect to have
> Waves with sub-Waves or another level of hierarchy even.

The first version may render today's handful of Waves as a flat stable list, but
the navigation model and row layout must be outline-capable. A resident child
Wave should be able to appear beneath its parent with the same operational lens
and selection behavior.

Do not add arbitrary folders or recursive Project trees to anticipate this. The
hierarchy should follow real ownership in the product model—such as a Project
promoted into a child Wave—so indentation communicates an actual operating
relationship. The exact depth, collapse behavior, and cross-repository behavior
remain future design work.

### Name the operational destination by its user contract

> I don't like the name Supervision really. Admin? Control?

`Supervision` exposes runtime vocabulary and implies watching subordinate
processes. Candidate names carry different promises:

| Name | Promise | Risk |
| --- | --- | --- |
| Control | Inspect live work and act on it | Overpromises if actions are missing or unreliable |
| Activity | See what is happening and what happened | Sounds passive; weak home for start, stop, attach, and recovery |
| Sessions | Reach current execution directly | Too narrow once Run History and other controls arrive |
| Operations | Live execution plus operational history | Accurate but jargon-heavy |
| Admin | Configure and maintain the system | Implies settings, permissions, and system administration rather than work |

Working preference: `Control` if Active Sessions exposes real, reliable actions;
otherwise use the narrower truth until those controls exist. Keep `Admin` for
actual product administration if such a surface appears later.

### Active Sessions: interactive handoffs, background visibility

> The main thing I want to be able to do is have agents launch interactive
> sessions that those agents are blocked behind. When the agent has an open
> interactive session, that should count as red for the blocked signal.

> Then there's the question of what we want to do with any non-interactive
> session, and whether we want you to be able to do anything with that. I don't
> think I really do. But I do want you to be able to see.

Active Sessions has two kinds of row with intentionally different capability:

#### Interactive handoff

An agent launches an attachable interactive Codex, Claude Code, or other vendor
session in the correct Home and worktree, then blocks behind it. The durable
parent Session records that it is waiting on this exact child. The app:

- turns the owning Task, Project, and affected Wave red;
- shows why the human is needed and which agent is waiting;
- makes **Open** or **Attach** the primary action;
- attaches to the exact interactive session rather than recreating it;
- detects completion or explicit hand-back, records a receipt, and unblocks the
  waiting agent.

Changing agent bodies must not orphan the handoff. The interactive Session has
durable identity, parentage, Home, worktree, provider, and attach identity.

#### Non-interactive body

Show live Wave, Project, Task, and direct execution bodies so the user can
understand what is running. Each row may disclose owner, provider/model, current
step, age, progress freshness, worktree, and reason. It is view-only in the
first product slice: do not expose attach, steer, interrupt, or stop merely
because lower-level CLI controls exist.

#### Current capability versus required contract

The existing API has useful pieces but not the desired handoff:

- `lf status --json` carries Project/Task runtime, process liveness, next owner,
  and reason.
- `lf runs --json` carries active agent launches, provider/model, worktree,
  surface (`headless` or `tui`), lineage, and capture state.
- `lf project attach` and `lf task attach` can attach read-write to existing tmux
  control terminals.

The missing shared primitive is an agent-launched interactive child Session.
Current run rows do not carry an attach descriptor or reliable Project/Task
parent; current Project/Task attach reaches the non-interactive control body,
not a deliberately launched human handoff. Build this in the shared CLI/store
contract, then let Swift consume it. Do not create a Mac-only terminal lifecycle.

### Refined green / red / black semantics

> Red should certainly include no live body though, but Off is no live body and
> everything is clean. Red is no live body but there is some specific definition
> of uncommitted/local progress.

The lens is a compact projection over body liveness, next owner, and recoverable
local progress:

| Light | Contract |
| --- | --- |
| Green | A live body is advancing and the next owner is not the human. |
| Red | Human attention or recovery is required: an interactive handoff is waiting, or no body exists while unsettled local progress remains. |
| Black | No live body is expected, no handoff is waiting, and the workspace has no unsettled local progress. |

Red includes “no live body,” but does not equate all absent bodies with failure.
A clean unstarted backlog Task is black. PM completion remains a separate fact.

Define `unsettled_local_progress` once in the shared API from durable evidence,
not Swift filesystem guesses. Candidate evidence includes dirty worktree changes,
Task-authored commits not yet settled into the recorded delivery state, or an
active non-terminal Session whose workspace requires recovery. Exactly how open
or submitted PRs participate still needs a product decision.

### Open an interactive handoff in several surfaces

> Then we want to be able to launch those sessions ideally in as many ways
> possible—at a minimum, embedded Ghostty, but then also possibly in Warp or a
> Cursor/Claude IDE if that's possible.

One durable interactive Session may expose several presentation targets. The
target changes where the human interacts; it must not create a second agent,
worktree, or lifecycle.

```rust
#[non_exhaustive]
enum SessionPresentation {
    EmbeddedTerminal,
    ExternalTerminal,
    ProviderIde,
    WorktreeOnly,
}

struct InteractiveAttach {
    session_id: String,
    worktree: PathBuf,
    home: String,
    provider: String,
    provider_session_id: Option<String>,
    terminal_argv: Vec<String>,
    presentations: Vec<SessionPresentation>,
}
```

Capability ladder:

1. **Embedded Ghostty — required.** Consume the shared attach descriptor and
   run the exact tmux attach command inside the existing Ghostty surface. The
   runtime owns creation and identity; Swift only presents it. Replace the
   current Swift-owned arbitrary Task terminal lifecycle rather than extending
   it into a second source of truth.
2. **Warp — useful terminal adapter.** The app already opens Warp at a Task
   worktree. Warp URI support can open a window/tab at a path; attaching the
   exact handoff also requires a command-bearing Warp Tab/Launch Configuration
   that runs the shared attach command. Treat plain “open worktree” as a weaker
   fallback and label it honestly.
3. **Claude in VS Code/Cursor — provider-specific adapter.** Claude's extension
   supports opening a tab and resuming a known session id in the current
   workspace, and is installable in Cursor. Offer this only when Loopflow has the
   provider session id and can prove the target workspace. Otherwise open the
   worktree without claiming to attach.
4. **Other IDEs — worktree fallback.** Open the correct worktree in the user's
   editor. This is useful context, not control of the interactive Session.

Each Active Session row should present one primary `Open` action using the
preferred available target, plus an `Open in…` menu listing only capabilities
the current provider, Home, and installed apps can satisfy. Embedded Ghostty is
always the safe fallback for a local attachable Session.

Remember presentation choice with this resolution order:

1. last successful surface for this provider, when available on the Session's
   current Home;
2. last successful surface overall;
3. embedded Ghostty.

Update preferences only after a launch succeeds. If a remembered surface is no
longer installed or cannot attach this provider/session, fall back visibly and
leave the durable Session untouched.

### Git archaeology: rehome proven launch contracts

This feature has several direct ancestors. Reuse their product decisions; do
not restore their retired runtime ownership.

| Commit | What it proved |
| --- | --- |
| `b39ad201f` | `lf open` reopened an existing worktree in Warp + Cursor. |
| `87a8b812e` | GhosttyKit can provide a production embedded Metal terminal with full keyboard, mouse, clipboard, and IME behavior. |
| `870f83a57` / `9613dece3` | An interactive flow step can pause, render inside embedded Ghostty, and expose explicit Continue/Cancel instead of relying on terminal folklore. |
| `c1d4adae8` | A headless flow can create an interactive Session, publish its `session_id`, block, let the app join it, and resume when the Session completes. This is the closest ancestor of the desired agent handoff. |
| `f0eafbb18` / `1267bf494` | Interactive attention, persisted terminal Sessions, embedded workspace tabs, and completion-driven unblocking can share one lifecycle. |
| `fbcf9f3af` / `c358e9cf4` | The attach boundary should return connection information (`session_name`, `host`, `cwd`, `status`, later explicit env); terminal bytes stay out of the API, and Swift constructs local or remote tmux attach from the descriptor. |
| `710bcd60c` | “Open Terminal” and “Open Internally” can target the same persistent tmux shell; `AppPreferences.defaultTerminal` already established remembered presentation choice. |
| `6d0c98d67` | External local/remote launchers existed for terminals, Cursor/VS Code, and Zed, including SSH remote targets. |
| `f0292d588` | The strongest lifecycle shape: runtime-owned terminal creation returned `{session, connection}`, Swift persisted only the Session id, reattached through the API, and the tmux pane survived flow exit. |
| `b2b388488` | Current `lf` can hand interactive skills to vendor TUI or IDE surfaces with compact provider-native skill seeds. |
| `309575f8e` | The lfd/session catalog was deleted during the Wave/Project/Task ontology collapse. The mistake to avoid is restoring lfd or a parallel Session ontology. |

The resulting direction:

- Move the proven terminal-session lifecycle into the current shared `lf` store
  as a child of durable Wave/Project/Task Session intent.
- Keep a stable Session id and a small attach descriptor; do not stream terminal
  bytes through Loopflow.
- Reuse the existing Ghostty surface and external launcher techniques.
- Preserve explicit completion/hand-back so the blocked parent agent resumes.
- Let Swift remember presentation preference, but never own Session creation,
  identity, or recovery.

Current `TaskWorkspaceView` already proves embedded agent attach, arbitrary
Ghostty shell tabs, changed-file inspection, and Warp worktree opening. Treat it
as UI material to consolidate, not as the durable lifecycle: its
`TaskTerminalStore` currently invents tmux names and owns those shells in Swift.

## Current portfolio context

Mac Surface UX currently promises glanceable wave state, navigation, launch,
reattach, steering, and audit around the shared product API. Its KRs combine
terminal replacement, one-action controls, attention ranking, surface-layout
stability, and elimination of a Swift-owned session lifecycle.

The Product Wave separately owns Loopflow API, Wave Chat, Auditability, iOS
Surface UX, Distributed Computing, and Product Performance. The current seams
may be contributing to the desktop experience feeling assembled rather than
coherent.

## Roadmap decision

Keep the existing **Mac Surface UX** Project, but replace its terminal-replacement
framing with one coherent desktop-product bet:

> The Mac app is Loopflow's legible daily front door: each Wave opens to its
> purpose and measured work beside a fast durable Chat, while attention and
> interactive handoffs remain reachable without making the app a second runtime.

Proof:

1. For one full week of real use, every selected Wave opens to a one-sentence
   objective, all Projects and KRs, open-Task counts, and durable Chat; raw
   implementation errors and internal vocabulary never dominate the first
   screen.
2. Across the shared fixture suite and 20 sampled live Wave, Project, and Task
   rows, every green, red, or black lens agrees with liveness, next owner, and
   unsettled local progress, and its disclosed reason explains the state.
3. In 20 cold opens and body replacements, Chat renders the latest durable
   thread before refresh, accepts a new message without waiting on trace
   capture, preserves conversation across prior failures, and rolls repeated
   operational failures into one actionable notice.
4. Ten of ten agent-launched interactive handoffs open the exact durable Session
   in embedded Ghostty, survive app or agent-body restart, and resume the blocked
   parent on hand-back without a duplicate body or Swift-owned lifecycle.
5. During a week of dogfood, Active Sessions accounts for every live Wave,
   Project, Task, and direct execution body; non-interactive bodies stay
   view-only, and interactive Open uses the last successful provider or overall
   surface with an honest embedded fallback.

### Task graph

The first four slices can run concurrently because they have separate product
owners and mostly separate code surfaces:

1. **Stable Wave surface (W2-178)** — repo dropdown, stable Wave rows, restrained lens,
   persistent Objective/Projects pane, and Chat as the default third pane.
2. **Durable Wave Chat (W2-174)** — bounded/cached load, quick send path, body-independent
   history, failure rollups, and inline linked references with popovers.
3. **Shared attention projection (W2-123)** — define green/red/black from shared
   liveness, next-owner, and unsettled-progress evidence; expose the reason and
   use the same fixture in CLI and Swift.
4. **Interactive human handoff (W2-175)** — build the child Session and attach descriptor
   on W2-135's current supervision PR, block the parent, and resume it on an
   explicit completion or hand-back.

Two integration slices follow those roots:

5. **Active Sessions (W2-176)** — add the progressive Control destination, interactive
   Open/Attach, and a read-only census for non-interactive bodies. Depends on the
   attention projection and interactive handoff.
6. **Presentation adapters (W2-177)** — embedded Ghostty first, then Warp and supported
   provider IDE targets, with provider/overall remembered preference and visible
   fallback. Depends on the shared attach descriptor.

Run History remains intentionally deferred.

W2-173 was superseded before implementation because its initial Task Session
pinned a development-provenance `lf` that could not open the production store.
W2-178 carries the unchanged work with the corrected execution context.

### Context Lab trace vocabulary

> The context lab also added an “evidence” as a frame on its traces which is
> something I think I want to remove.

> Simplify, simplify, simplify.

Context Lab may use captured traces to calculate source measurements, but the
surface should not make “Evidence” another object the user must learn. A trace
opens as **Trace**, with System prompt, Task prompt, and Conversation as its
three direct contents. Selecting a source revision shows its details in the
existing rail without an “Evidence” heading. Provenance remains attached to the
trace and revision data; it is not promoted into a separate product frame.

## Size check

This design is larger than one Task: it changes the main Swift information
architecture, Chat behavior, a shared status projection, and the runtime Session
contract. The task graph above keeps the model changes owned by Loopflow API and
Auditability while Mac Surface UX owns their presentation and dogfood proof.

## Repository Wave bootstrap

> The default user flow should basically be:
>
> - make sure there is one project for one wave asap so you can start executing
>   standard operating procedure tasks
> - you should look for opportunities to create a structure something similar
>   to these 4 at the root, and then to subdivide at will.
> - you need not spin up all waves all the time, but the first time you try to
>   solve a bug that rightly belongs to one of these waves, its worth iterating
>   on the Goals, KRs, etc while also solving the bug

> This is probably its own .md file. WAVES.md to go alongside LOOPFLOW.md as
> autoinclude. Maybe the empty screen chat has a selector that basically quick
> starts one of these specifically or allows you to name your own first wave.

The repository is the portfolio root. Do not manufacture a root Wave whose
Project and KRs merely summarize other Waves. That creates a second
representation of the repository without a concrete operating objective.

Split the always-on operating guidance by concern:

- `LOOPFLOW.md` explains how an agent works safely through Loopflow.
- `WAVES.md` explains how a repository grows its operating structure.
- Both are built-ins, but they have different inclusion scopes. Every Loopflow
  execution receives `LOOPFLOW.md`; `WAVES.md` enters only a high-level context
  that can create or reshape the Wave portfolio: repository bootstrap, a Wave
  resident, or an explicit Wave-shaping skill. Project/Task pursuit and direct
  execution do not receive portfolio doctrine after ownership is decided.

`WAVES.md` defines four useful root roles without requiring four empty records:

| Role | Default prefix | Owns |
| --- | --- | --- |
| Product | `PRD` | User value, behavior, interaction, and product quality |
| Infrastructure | `ENG` | Technical architecture, developer flow, reliability, and release machinery |
| Intelligence | `SCI` | Evaluation, learning, model behavior, and research |
| Operations | `OPS` | Recurring service operation, external coordination, and portfolio hygiene |

These are routing defaults, not a required org chart. Materialize a role when
the first real outcome belongs there. A repository may start with only Product,
or with a custom domain Wave when that is more truthful. Child Waves appear
when a domain needs durable memory, chat, cadence, budget, or independent
Project selection; Project nesting is never used as a substitute.

### Empty Chat bootstrap

When a selected repository has no authored Waves, the main pane stays a Chat
surface and replaces the disabled composer with a small first-run prompt:

```text
What kind of work are we starting?
[ Product ] [ Infrastructure ] [ Intelligence ] [ Operations ] [ Name a Wave ]
```

Choosing a default role creates that Wave with its standard name and prefix.
`Name a Wave` asks only for a name; it does not force the work into one of the
four roles. The app selects and starts the new Wave, then reveals the ordinary
Chat composer with one focused prompt:

```text
What should Product accomplish first?
```

The first send is a normal durable Chat message. The Wave resident receives
`WAVES.md` and makes the smallest executable structure its first move: create
one outcome-shaped Project with initial proof KRs, then pursue the work. This
keeps KR authorship in the high-level agent that can exercise judgment instead
of making Swift generate planning content.

Wave creation itself is one shared CLI operation, not a Swift sequence of
filesystem and Linear mutations.

```rust
#[derive(Debug, Serialize)]
pub struct WaveCreateRequest {
    pub role: Option<WaveRole>,
    pub wave_name: String,
}

#[derive(Debug, Serialize)]
pub struct WaveCreateResult {
    pub wave: WaveSummary,
    pub team_key: String,
}
```

The create operation:

1. writes `wave/<slug>/GOAL.md` with a concise initial objective;
2. connects or creates the Wave's Linear Initiative and role team;
3. registers the Wave and returns enough identity for the app to select it;
4. the app launches it through the existing Wave launcher and attaches Chat.

If launch fails, the authored Wave and PM binding remain valid and the selected
pane offers the normal Start Wave recovery. The first Wave turn must create a
concrete Project while answering the user's outcome; it must never create “Set
up Product,” “Root,” or another administrative placeholder Project.

Existing repositories skip first-run bootstrap. Their empty Chat means “no
messages yet,” not “no Wave structure,” and should show a normal composer.

### Bootstrap done when

1. Bootstrap and Wave-shaping sessions receive `WAVES.md`; ordinary Project,
   Task, and direct execution sessions do not.
2. A new repository opens to the five choices above, not “No registered
   Waves.”
3. Choosing Product produces exactly one Product Wave, selects it, and starts
   its normal durable Chat.
4. Sending the first outcome produces one outcome-shaped Project with
   proof-shaped KRs before implementation Tasks are delegated.
5. Infrastructure, Intelligence, and Operations remain suggestions until
   actual work materializes them.
6. Choosing a custom name produces the same one-Wave, one-Project result
   without assigning a false default role.
