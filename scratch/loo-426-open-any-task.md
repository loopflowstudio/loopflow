# LOO-426 — Open Work wherever Jack is

Status: design accepted, 2026-10-18; implementation built. Jack Heart
accepted the product direction, the full vocabulary/availability
recommendations, the public Work URI shape, the simplicity/integration boundary
and agent-initiated opening below, then closed design review and authorized
implementation through `pursue`. Three implementation passes (October 18) built
everything under Remaining implementation; no build items remain. Rendered
activation and the parity exercise remain demo evidence.

## Problem and accepted direction

Jack Heart wants an agent's Task handoff to put the existing conversation in
front of him, in its checkout, with a useful name and input intact. The October 7
four-conversation cmux exercise established the experience; the October 18 brief
requires Desktop to be at least as useful.

Accepted by Jack Heart on October 18:

- Ordinary navigation addresses Work: `loopflow://loopflow/task/LOO-507`, without
  an absolute repository path or Session identity.
- Prefer installed Loopflow Desktop, then another terminal/workspace in the
  current cmux, hrdr/Herder or Rex host, then a plain terminal. Explicit host
  selection wins. Desktop launch/control is macOS-only; generic opening is not.
- Keep `[LOO-489 · Enable the multitask navigation experience](destination)` as
  one clickable label. Disposition and an optional PR link sit beside it.
  Provider issue navigation remains a separate action.
- Reopening preserves the Work conversation and draft, with no second worker.
  A review already in its destination conversation is presented inline.
- Open conversations available for Jack to do his work with agents, raising focus
  at most once. Availability does not require a question, review or action owed.
  LOO-427's one repository window across machines and execution ownership remain.
- Prefer simpler architecture and understandable behavior over brittle
  environment-specific automation. Use sustainable host integration patterns
  where they improve working with Loopflow across setups; integration breadth
  does not justify coupling correctness to rapidly changing host internals.

Jack Heart clarified in the design conversation: use **Available** throughout
internal and external vocabulary. Agents are available for Jack to do his work
with them; they are not waiting on him. This supersedes the brief’s old batch
flag and the draft’s obligation-based filter. The accepted vocabulary contract
and repository-wide audit are incorporated below as part of this design.

Source: `lf task status LOO-426 --json`, revision 2, including its October 18
Workflow direction.

The Product GOAL calls for understandable Work and native Sessions through shared
owners. Its October 7 direction retired numeric performance/soak acceptance;
this design records comparison measurements without inventing a latency KR.
The Project's sustained-use KRs need later usage evidence, not a passing test.

## Vocabulary and availability — accepted

Jack Heart accepted the vocabulary audit and its recommendations during LOO-426
review on October 18, 2026, and requested their inclusion in this design.
This section owns the accepted internal/external frame, source findings,
consumer changes and proof. The URI shape was accepted separately below.

### The contract

**Loopflow presents work and agents that are available for the person's purposes.**

The person chooses what matters and when to engage. Agents carry out authorized
work, retain the result and context, and make useful next actions accessible.
Describe the person's work first, the available action second, and actual
execution/dependencies where relevant. Do not present an agent's unmet need as
the reason to act.

“Available” describes an interaction opportunity. “Design review” describes what
can be done there. “CI running” describes a dependency. None means Jack owes the
system attention. Availability does not require a question or pending review,
and it is not proof of successful implementation, authorization, or completion.

Do not turn this into euphemism. Keep consequential decisions specific:
“Choose whether to publish the site” or “Publishing requires confirmation” is
honest. “The agent is blocked on you” assigns the person a fault. A real failure
still says what failed and the effect; a real deadline names its source and date.

### What the source shows

1. **The existing shared availability owner largely supports this frame.**
   [`available_sql`](../rust/loopflow/src/store/sqlite/sessions.rs) derives
   availability from current process/attachment evidence. Reported interactive
   `idle`, reported `blocked`, pending input, a successful interactive hand-back,
   and quiet with no open tools can qualify. Quiet is currently 120 seconds;
   it is an inference, not proof of a question or an input-ready screen.
   [`SessionActivity`](../rust/loopflow/src/session.rs) nevertheless describes
   quiet as “waits on a person.” The
   [provider tracker](../rust/loopflow/src/harness/attention.rs) uses the internal
   name `Attention` and test names about turns that “wait.”
2. **CLI documentation contradicts the existing CLI.**
   [`lf` arguments](../rust/loopflow/src/lf/mod.rs) expose `session list --available`.
   [README](../README.md) still advertises `--waiting` and “waiting on you.”
   [Glossary](../docs/glossary.md) calls the conversation state “Waiting.”
   [Conducting](../docs/conducting.md) contains both vocabularies and frames
   multiple agents as things the person must keep track of. Its Session section
   later documents the correct flag. This is more than a planned new CLI spelling.
3. **“Available” already has conflicting domains.**
   [`RoadmapSection::Available`](../rust/loopflow/src/lf/commands/waves.rs) means
   filed work without a runtime, not an available conversation. `Waiting` groups
   non-Task next owners, including User, Wave, CI and External. A blanket rename
   would merge distinct meanings and falsely label dependencies available.
   The old LOO-426 proposal repeated that mistake by filtering next-action owners.
4. **Swift reinterprets the shared signal.**
   [`SessionProgramStatus`](../swift/LoopflowMac/SessionProgramStatus.swift)
   prioritizes explicit reports; its label is the report's raw state, such as
   Idle or Blocked. [`WorkProjection.attention`](../swift/LoopflowMac/WorkProjection.swift)
   includes inferred Available and reported Blocked, but not reported Idle.
   Rust includes interactive reported Idle. The projection also limits the
   section to nonterminal Tasks, one Session per Task; repository/Wave Sessions
   and secondary conversations are not represented there.
   [Tests](../swift/LoopflowTests/TaskRowMetadataTests.swift) explicitly preserve
   Wave exclusion and use a fixture named `waiting` for Available. The source
   proves these selection rules; no live comparison was run in this audit.
5. **Presentation still couples availability to trouble.**
   The [navigator](../swift/LoopflowMac/Views/WorkNavigator.swift) says AVAILABLE,
   while its internal values are `WorkAttentionItem` and `openAttention`.
   [WorkTone](../swift/LoopflowMac/Design/WorkStyle.swift) maps availability,
   blocking and stalling through `waiting`; errors elsewhere use the same palette.
   [Theme keys](../swift/Loopflow/Design/ThemeFile.swift) and the
   [theme editor](../swift/LoopflowMac/Views/ThemeEditor.swift) expose Waiting names.
   This is evidence of semantic coupling, not proof that a particular color
   makes people feel pressured. Visual evaluation remains necessary.
6. **Prompts actively regenerate the old framing.**
   [task-operate](../rust/loopflow/src/builtins/task/skill/task_operate.md) requires
   “waiting on a person”; [wave-session](../rust/loopflow/src/builtins/wave/skill/wave_session.md)
   and [repo-session](../rust/loopflow/src/builtins/ops/skill/repo_session.md) order
   reports by “waiting on the user.”
   [wave-operate](../rust/loopflow/src/builtins/wave/skill/wave_operate.md) already
   uses an Available heading, but defines it only through reviews, decisions
   and merges. Renaming the heading has not removed the obligation model.
   LOO-426's October 18 primary Session received these instructions and produced
   the same framing in its review handoff. Website copy alone cannot hold the frame.
7. **Some “waiting” has no interpersonal meaning.**
   [`ActivityState::Waiting`](../rust/loopflow/src/lf/commands/top.rs) comes from
   OS scheduling state. [`FlowOutcome::Waiting`](../rust/loopflow/src/flow/runner.rs)
   can mean execution stopped or delivery remains unsettled. Locks, child-process
   waits and `lf task wait` are mechanical operations. None proves a conversation
   available. [Program Status](../rust/loopflow/src/program_status.rs) implements
   externally defined idle/blocked/error vocabulary; preserve protocol values.
8. **The public story imposes a management role.**
   The [website](../website/home.html) says “2 waiting on you” and asks the person
   to stay in sync with an agent swarm. Its strongest sentence is already
   “design, implement, demo, release and study software with agents.” Lead with
   that experience. Orchestration can remain a capability without requiring
   everyone to identify as a manager of agents.

### Keep four facts separate

Use existing authorities; these distinctions do not justify four new stored
models or a new umbrella status enum.

| Fact | Owner/evidence | Presentation |
| --- | --- | --- |
| Conversation availability | Shared Session reader | Available; unknown evidence remains unknown |
| Execution activity | Processes and provider reports | Working, idle, stopped, failed; retain precise diagnostics |
| Work stage | Stored Task Workflow and artifacts | Design review, demo, implementation, completed |
| Dependency or authorization | CI, delivery, action policy and actual decision | CI running; sign-in required; confirmation before publishing |

A conversation can be available while its Task's CI runs. A design can be
reviewable even if its previous provider process is closed. A live process can
be sleeping without being available. A completed Task can still have a useful
conversation. Do not force these combinations into one linear urgency queue.

Keep the shared Available inference and its evidence limits visible. The audit
has not established that quiet detection should change. Resumable is also useful
but is a different capability: ordinary Open can resume history without claiming
a currently observed Available state. Product judgment remains over how broad
the prominent Available section should be; the opener should not silently add a
review-only requirement.

### Vocabulary to use

| Current pattern | Accepted wording or name | Meaning preserved |
| --- | --- | --- |
| Waiting on you / needs you | Available · Design review, or Available · Demo | What the person can do |
| Agent needs your answer | “Which deployment target should this use?” | The actual choice, without emotional debt |
| Unblock the agent | Choose deployment target; sign in; retry | The concrete action |
| Human intervention required | Deployment failed: credentials expired. Sign in to retry. | Cause, effect and recovery |
| Attention queue / needs-me | Available conversations / `--available` | Access to work with agents |
| Nothing waiting | No available conversations | A factual empty result |
| Clear your queue / catch up | Open your work; continue a conversation | Choice without a completion quota |
| Waiting on reviewers | Review requested | Delivery fact without blame |
| Agent is idle | Available, when supported by shared evidence | User-level interaction; retain idle in diagnostics |
| Roadmap Available meaning unstarted | Not started (label), `NotStarted` (section) | Planning lifecycle, not Session availability |
| Waiting on CI | CI running / checks queued | Specific dependency state |

Not every sentence needs “available.” “The draft is ready to review” and “The
preview is ready to try” can be natural prose without adding Ready as another
canonical conversation state. Avoid mechanical “available for you” repetition.
Keep collaboration language such as “we can explore” or “I found”; the problem
is assigning agents needs that become the person's duties.

Accepted internal naming direction:

- `SessionAttention` → `SessionAvailability`; DTO `attention` → `availability`;
  `session_attention` → `session_availability`. Preserve the existing single
  Available interpretation, not a second “needs response” signal.
- `WorkAttentionItem` → `AvailableConversation` only after fixing its projection
  and cardinality. If it remains one item per Work, name it `AvailableWork` and
  carry its actual conversation target. Do not rename a Task-only subset as all
  conversations. `attention(...)` → `availableConversations(...)` or the matching
  Work projection name.
- Provider `Attention` → `SessionActivityTracker`; tests describe the evidence
  and resulting availability, not an agent waiting for someone.
- `pendingHuman` should name the actual review stage; a future diagram node is
  `reviewNotStarted`, not Available. “Design review” at the current node is enough.
- Separate the presentation roles `available` and `warning`. Reusing an old
  warning tone with a new name does not resolve the conceptual coupling. Existing
  custom theme keys need explicit migration under the repository's compatibility
  policy; do not silently reinterpret an externally used warning role.
- `NextMoveOwner` describes execution responsibility and must not become the
  availability filter. A possible later simplification is action/required-authority
  terminology, but preserve actor attribution, routing and authorization.
- Mechanical `ActivityState::Waiting` can be described as sleeping/idle when
  that matches evidence; `FlowOutcome::Waiting` should become a precise held or
  stopped outcome after all producers are classified. Never rename these Available.

Wire changes require coordinated Rust/Swift/fixtures/schema consumers. Internal
mirrors move together; published external contracts need a deliberate transition.
Released migrations, provider payloads, original quotations and historical receipts
retain their original text. Source examples and maintained docs must be current.

### Keep the frame from drifting back

1. Put a short governing rule in AGENTS.md's Voice section; make the glossary the
   public semantic definition. Put self-contained report rules in the operator
   skills that generate reports, not every injected prompt. In those rules,
   reports lead with work and options. A finished Flow makes a review accessible;
   it does not create an impatient agent. Preserve authored review boundaries and
   named authority. Required decisions remain explicit, without asking again
   where permission already exists.
2. Cut the source path together: shared availability → CLI/DTO → app projection
   → labels/accessibility → operator reports → docs/website. Keep raw execution
   and external protocol evidence separately inspectable. Builtin source changes
   must reach installed skills and newly assembled context; an old conversation's
   captured prompt cannot be assumed rewritten.
3. Add a small terminology check for maintained product copy and builtin prompts:
   flag “waiting on you,” “needs you,” `--waiting`, `needs-me`, and generic agent
   “attention required.” Review matches in context. Do not ban the substring
   `wait` across source or maintain a giant exemption list for archived evidence.
4. Add behavioral coverage, not just string assertions: a completed turn with no
   question appears; explicit idle appears consistently in Rust and Swift;
   CI state does not remove an available conversation; a running tool does not
   become available merely because its process sleeps; cleared/stale evidence
   does not invent availability; opening preserves drafts and authority.
   Exercise repository, Wave, Task and secondary Sessions at their intended scope.
5. Evaluate reports using ordinary and difficult cases. Ask for a status overview,
   review a draft, leave three conversations untouched, encounter a real failure,
   and return later. Observe questions, defaults, sorting, badges and notifications.
   “3 available” is an inventory count, not unread debt. Do not make a normal
   available conversation increasingly urgent just because the person left it.
   Actual deadline/incident priority must come from the work or explicit settings.

Example report:

> Three conversations are available: LOO-426 has an opening design to explore,
> LOO-507 has a provider integration to try, and LOO-489 has navigation work to
> continue. LOO-497's checks are running. Deployment failed because credentials
> expired; sign in to retry.

Governing rule to install:

> Describe agents as available for people to do their work with. Name the work,
> the available interaction, and actual constraints. Never portray people as
> blockers or as owing agents attention. Availability is independent of a pending
> question, review or Task dependency. Preserve real authorization and failures.

### Remaining semantic detail and evidence limits

Carry the vocabulary changes through the implementation and consumer cutover
below. Planning section names and process outcome names require classification
of their actual meanings; cosmetic renames cannot substitute for that work.
The shared availability and Swift projection disagreement needs a composed
regression case. Color and notification behavior need native interaction review;
palette names alone do not establish experienced urgency. Quiet inference has
not been revalidated by this source audit. Preserve these limits during cutover.

## Demo

From outside the Task conversation, activate the readable LOO-507 label or run:

```sh
lf open 'loopflow://loopflow/task/LOO-507'
lf open --available
```

The Task command opens its primary conversation. Type a draft, switch away, and
reopen that Work: the same pane and draft return. Ask for review there: the
question appears inline, without a self-opening instruction. From another Work,
the report supplies the readable Task link.

With three Task conversations available for Jack to work with, the second command opens three named
conversations in Desktop's sidebar and raises one final destination. Repeat the
original four-Task exercise in Desktop and cmux; record actions required, time to
first usable prompt, all-four readiness, and visible names/reasons. Repair app
regressions in this Task. Repeat Desktop-absent opening in Herdr, Rex and a plain
terminal, including an existing destination and a failed route. Click actual
rendered links as well as executing the fallback command. These are demo checks,
not unattended build prerequisites.

## Findings that determine the architecture

1. **Conversation ownership already exists.**
   `ops/human_session/primary.rs::{ensure_task,task_conversation}` reuses the
   persisted Task primary, otherwise selects an existing interactive conversation
   by human input/opening evidence. Only a Task without a conversation gets one.
   `select_connection` and `connection_scope` also own repository/Wave selection.
   Reuse their scope lock, checkout preparation and admission. Do not select the
   latest Flow Session or build a second conversation picker in Swift.
2. **Current links under-deliver.** `lf/commands/desktop.rs::opening_url` emits
   `loopflow://task/...` with a path query. `WorkDestination.swift::TaskLink` and
   `LoopflowApp.handleDeepLink` mirror that grammar. In
   `WorkModel.prepareLinkedTask`, an ordinary Task link can return at details;
   conversation preparation depends on Session/diff/internal intent. Replace
   that split for ordinary Task opening; retain the sidebar's details action.
   The app's unqualified lookup also enumerates Wave-contained Tasks: standalone
   Tasks must use the shared repository Task lookup.
3. **There is no generic opener to extend.** `DesktopCommand::Open` invokes
   macOS `open -a Loopflow`, reports only opening, and performs no availability
   dispatch. `lf help open` currently fails because PR and Desktop both own it.
   Add an exact root `open`, remove Desktop's opening subcommand, and retain
   explicit `lf pr open`. No current `home desktop` implementation was found.
4. **Folder names locate; they do not identify.**
   `repository/location.rs` and `store/sqlite/repositories.rs` own registered
   planning identity and Git locators. Resolve folder names against those
   registrations, collapse equivalent locators, and retain ambiguity across
   distinct repositories. Never enroll a repository or bind a locator on click.
5. **Availability belongs to the conversation.** Shared Session availability
   already covers completed turns, explicit input opportunities and quiet
   conversations. Select that evidence, not `next_move.owner == user` or Task
   dependency conditions. A pending review is optional context, never required
   for inclusion. CI/dependency status alone neither includes nor excludes an
   otherwise available conversation. Unknown is not an empty inventory.
6. **Draft continuity forbids implicit takeover.** Existing attachment actions
   expose Move here; retained Swift panes preserve drafts. Reopening a matching
   destination focuses it. If the conversation is controlled elsewhere and no
   matching host destination is reachable, show that fact and the existing
   explicit move action. Opening must not restart the provider to look ready.
   Cross-client unsent-draft migration is not provided by native resume and is
   not promised here.
7. **Links need both rendering and a receiver.** OSC 8 gives terminal output a
   readable clickable label; Markdown alone does not establish activation.
   OS scheme dispatch delivers a URL, not the originating shell environment.
   Therefore a bare custom URL cannot independently recover the current host.
   Use the opening mechanism and bounded presentation provenance below.
   [OSC 8 specification](https://iterm2.com/feature-reporting/Hyperlinks_in_Terminal_Emulators.html),
   [Apple URL registration](https://developer.apple.com/documentation/xcode/defining-a-custom-url-scheme-for-your-app),
   [Launch Services](https://developer.apple.com/library/archive/documentation/Carbon/Conceptual/LaunchServicesConcepts/LSCTasks/LSCTasks.html).
8. **Host mechanisms exist, but native proof is distinct.** Herdr's installed
   0.9.3 CLI exposes no-focus workspace creation, stable workspace/pane IDs and
   pane command execution. Its documented environment identifies the originating
   pane/socket. Rex documents contextual IDs, JSON inspection, window creation
   with a command argv/cwd, labels and explicit focus. These support dispatch;
   they do not prove hyperlink activation or the installed Rex version.
   [Herdr CLI](https://herdr.dev/docs/cli-reference/),
   [Rex context](https://www.superlogical.com/rex/docs/automate/cli),
   [Rex layout API](https://www.superlogical.com/rex/docs/reference/lua).
   For cmux, use observed behavior and Loopflow's existing integration contract
   in `terminal_title.rs`; do not copy, translate or paraphrase cmux source,
   tests, help, skills or configuration. Exact host command payloads and native
   focus/reuse behavior require disposable implementation probes before a host
   branch is considered complete.

## Chosen approach

### Simplicity and integration boundary

Jack Heart set this priority after the native probes on October 18: simpler
architecture and understandable behavior beat extensive environment-specific
code. The following is the proposed application of that accepted priority.

Keep three responsibilities small: resolve Work through existing owners, choose
a destination using one policy, and ask that destination to open or focus it.
URI parsing, conversation selection, attachment ownership, batching and errors
stay shared. A terminal integration translates only documented host context and
open/focus operations. It owns no alternate Work model or routing policy.
Use concrete functions; no plugin framework or generalized capability registry.

The generic path must remain useful with every optional host integration absent:
the registered URL receiver invokes the shared opener, which selects Desktop
when available or the supported terminal fallback. An interactive direct command
can use its current terminal. A narrow integration can improve placement within
cmux or Herdr when explicit, current context is supplied. Keep the same Work URI
and explicit host choice in every setup. A browser click and a terminal command
can choose different presentation destinations because they supply different
context; both select the same Work and preserve its attachment ownership.

An integration earns its place when it uses a documented public interface,
provides a concrete benefit such as exact-pane context or retained-destination
focus, and leaves the generic path usable when unavailable. Prefer argv, OSC 8,
OS URL registration and small host callbacks. Do not add focus heuristics,
process-tree reconstruction, window-title matching, private configuration
inspection, version-specific workarounds or background discovery to simulate
missing context. A newer host release should at worst remove that convenience,
with a clear fallback; it must not change Work identity or launch duplicate work.

Herdr's documented link handler is a promising optional adapter, not a
prerequisite for opening Work. cmux's observed shell context supports direct
opening; its native click path remains unproved and does not justify a special
inference mechanism. Rex remains deferred. These are integration candidates and
evidence limits, not a requirement to install or enable plugins during review.

### One command, one Work address

`lf open [WORK_URL]` opens the selected scope; existing `--repo`, `--repository`,
`--task`, `--wave`, `--machine` selection continues through shared context.
`--host auto|desktop|cmux|herdr|rex|terminal` selects presentation.
`--available` selects conversations with shared availability evidence, across
registered repositories unless explicit scope narrows it. `--session ID` is an explicit Session operation, not
an ordinary Work address. Preserve `--diff` for Task opening. Unsupported
terminal companion views are reported separately; never claim Changes opened.

`lf open --link --json` is read-only: returns label, Work URL, copyable fallback
argv, review relationship and route evidence. Text output
uses OSC 8 on a supporting terminal; `--format markdown` emits the approved
Markdown label. Piped text has no escape controls. `--link` never prepares a
checkout, creates a Session or dispatches. Opening JSON reports per-item
`opening`, `usable`, `elsewhere` or `failed`, with target, chosen host and reason;
printing a link has a separate description and is never an opening receipt.
Extend existing Desktop opening status/readiness reporting rather than storing
another authoritative readiness state.

### URI schema and handler routing

The accepted Task address is `loopflow://<repo-short-name>/task/<identifier>`.
Jack Heart accepted the following Work address shape in the URI review:

| Address | Destination |
| --- | --- |
| `loopflow://loopflow` | Repository's primary conversation |
| `loopflow://loopflow/wave/product` | Wave's primary conversation |
| `loopflow://loopflow/task/LOO-426` | Task's primary conversation |

Addresses select existing Work; resolution obtains its conversation, checkout
and execution owner from shared records. An ordinary link does not pin a Session,
provider, workflow position or presentation host. Reopening follows that Work's
current primary. Explicit Session selection remains available through the CLI.
Jack Heart then asked whether the handler can detect the local OS and terminal
host at activation. Opening-time dispatch (below) answers that; no terminal
identity appears in these public addresses.

Escape components using
URI encoding, not form encoding. Validate cardinality, controls, encoded slashes,
credentials, port, fragments and duplicate/unknown query fields in shared Rust
parsing. Swift consumes the same fixtures and Rust resolution; it does not
repeat repository selection.

For same-name repositories, append `?repository=<registered-identity>` to the
same grammar, optionally qualified by the existing Machine identity where needed.
Unqualified ambiguous links return candidate labels and qualified links without
changing selection. A local ID alone is not portable: use existing locator
correspondence on another Machine, and report unknown correspondence. No path,
Session ID or newly invented repository identity is emitted. Renames can make an
unqualified short-name link unavailable; explicit identity remains authoritative.

Identity qualifiers (`repository`, and `machine` where required) identify Work
or its existing owner. Presentation context belongs to opening the address.
Do not emit `origin` or another process token in ordinary clickable or copied
links. The earlier origin-query proposal is withdrawn as the default mechanism;
missing activation context uses the generic route rather than adding process
tokens to public addresses. No additional scheme or command-bearing URL.

### Source of truth and consumers

Shared SQLite Work, repository locators, Task placement, the Task primary Session
and the attachment claim remain authoritative. `ops/task_location` determines
execution Machine/checkout; presentation Machine is independent. Rust produces a
resolved Work target and a read-only opening explanation, then dispatches it.
Swift's `WorkLinkRouter`, retained workspace registry and `WorkModel` install or
focus that target in LOO-427's repository window. Session preparation stays on
the execution owner through existing remote routing, even when Desktop is local.

Required consumers: Clap navigation/explanation/remote routing/help schemas;
Rust opening and link emission; Swift URL entry, palette, Task/sidebar links,
retained panes and opening receipts; terminal host/title integration; CLI
installation/URL registration; Task/Wave operational lists, review instructions
and examples. Any new wire fields have explicit types and Rust/Swift fixture
coverage, without DTO defaults.

### Dispatch and reuse

Use concrete host branches behind shared opening policy. Discover installed
Desktop through Launch Services on macOS. Consider only an explicitly selected
host, current direct-caller context, a per-activation callback, or an already
recorded destination; do not scan every installed/running host. A headless
command without supplied presentation context uses the generic route. Preserve
existing caller context where available without building an ancestry resolver
for opening. Missing evidence selects plain-terminal fallback, not a guessed
active window. Explicit unavailable hosts fail with the next usable command;
auto mode proceeds only after a definite route failure.

Each host lists and identifies an existing Work destination before allocating
one. Match structured Work/Session attachment evidence and stable host IDs,
never a human-editable title. Reuse existing attachment/presentation records;
add only the minimal destination receipt needed for reliable reopening where
the existing owner lacks it. Do not collect every host's environment on every
Process or create a parallel workspace database. Revalidate a recorded
destination through its host's public interface when consuming it. Use
one per-Work opening exclusion around lookup/create so concurrent requests and
ambiguous replies do not multiply panes. A timeout with unknown creation effect
requires readback, not immediate creation in another host.

Create terminal destinations with the existing `lf session connect` path and
resolved owner/Work selector, never bare provider invocation, `--fresh`, Flow
start or a fresh agent turn. Quote argv only at shell-only boundaries; do not
paste commands into an existing conversation. Hosts lacking direct argv launch
use a newly allocated shell only. Existing attached destinations are focused
without resending their launch command. Titles project Task identifier, short
Task title and available activity or review stage; preserve human Session names.

The plain-terminal branch uses an existing supported terminal launcher with a
fixed argv command; on macOS Terminal can open a temporary `.command` through
Launch Services without an AppleScript permission dialog. On Linux use a
supported installed terminal executable when a graphical session exists;
otherwise an interactive caller can connect in its own TTY. A headless caller
with no reachable presentation endpoint receives a failure and a usable terminal
command. Never treat stdout as a terminal or block a batch on the first Session.

Queries to the selected host have finite timeouts (initial implementation choice:
2 seconds each, once per batch). Provider startup is asynchronous
under existing Session timeouts. No poll of all repositories on an ordinary
Task click. Batch resolution pages the existing readers once, deduplicates Work
and exact Session targets, opens without focus, then focuses one successful
item. Return partial results with errors; an unavailable peer prevents a claim
that *every available conversation* was opened. No new background inventory service.

### Clickable without Desktop

Package a minimal URL receiver with CLI installation: a macOS application bundle
and a Linux desktop scheme entry call the same `lf open URL` implementation.
They contain no Work policy. Desktop's own URL callback invokes that same
resolution policy and retains its existing UI ownership. Installation/update
maintains the handler for `loopflow:`; it does not add another URL scheme or
require Desktop to be installed. Do not replace an unrelated user-selected
handler silently. Missing/disabled registration is surfaced by link output with
its command fallback.

The same clean Work URL is used for display, activation and copying. The
registered handler delegates to the shared opener, which detects the local OS,
Desktop installation and available host evidence at opening time. This makes the
same address useful on different machines without embedding the producing
Process or host. Explicit CLI host selection still wins; automatic order remains
Desktop when supported/installed, identified current host, then plain terminal.

Use two entry paths to that one dispatcher. A host-native click callback or
`lf open URL` can supply caller/terminal context directly. The OS URL receiver
accepts supplied per-activation context but does not reconstruct a lost origin.
Neither source-app identity nor a list of running terminals proves the exact
originating workspace. Validate context before using it; when absent, use the
plain-terminal route rather than silently choosing a random host/window. Retained
Work destinations may be reused when their identity and presentation ownership
are known. Missing graphics/terminal access yields the executable fallback.
The receiver's launch environment is not per-activation evidence: native probes
below demonstrated stale pane and even stale host identity across later URLs.
Only a direct CLI caller's verified terminal context or context supplied with
the individual activation can select the originating host destination.

Capture host metadata only where required for existing destination lookup/reuse;
there is no persistence migration solely to support an origin URI query. A host
integration may forward bounded context as process arguments/IPC while leaving
the URI unchanged. Generic custom-scheme forwarding from each supported host
requires a probe; HTTP browser interception alone does not establish support.

| Output/client | Activation contract |
| --- | --- |
| Desktop native content | Work link routes directly through the shared resolver and retained registry. |
| CLI output in an OSC 8 terminal | One identifier/title span carries the activatable URL; the installed receiver dispatches it. |
| Agent Markdown renderer | Use the returned Markdown label only where the renderer supports custom schemes; verify real click behavior for supported provider/client combinations. |
| Renderer rejecting custom schemes, or terminal lacking OSC 8/handler | Preserve the readable label and supply `lf open 'canonical-url'` beside the report as the supported executable fallback. No fabricated clickable-success claim. |
| Supported terminal host with Desktop absent | A small optional integration or direct command supplies current context. OS-only activation without that context takes the generic route. Exact native-click support is recorded per host; Rex is deferred. |

No post-processing of native provider escape streams to turn Markdown into OSC
8. The CLI owns its emitted bytes; provider renderers own theirs. Client support
must be recorded from rendered activation, not inferred from the terminal brand.

### Agents open Work where they run

Jack Heart directed on October 18 that agents run `lf open <work-url>` (or
`--available`) themselves when opening is obviously next, letting the shared
dispatcher choose the destination; they print the readable link when opening is
optional or the Work is their own conversation, and report the actual receipt.
Implemented in task/wave/repo operate and session skills and capture-tasks.

On-demand host skills: the shared opener needs only `lf open` guidance, which
every relevant skill carries. Host-specific work beyond opening, such as
arranging cmux panes or Herdr layouts, may justify a host skill that a supported
host makes discoverable when Loopflow detects it is running there. The task
decides whether to load it. Never inject a host's whole skill set into every
conversation. An attached client's host is recorded as its `processes.terminal_id`
placement (`cmux:<ws>`, `herdr:<ws>`, Desktop's `LF_TERMINAL_ID`); no skill
reads it yet. Authoring host-specific skills is follow-up work.

### Prior art and probes for handler integration

October 18 research (behavior only; no cmux source, tests, help, skills or
configuration copied): cmux intercepts HTTP(S) before OS dispatch; Velja and
Finicky show OS-level routers cannot reliably recover the source terminal;
iTerm2 and VS Code links address their own resources. Loopflow's links address
Work. Jack Heart requested parallel native cmux and Herdr probes; Rex is deferred
for lack of access. Reports: `scratch/cmux-link-probe/report.txt`,
`scratch/herdr-link-probe/report.txt`.

#### Installed host results — October 18, 2026

- **Herdr 0.9.3:** an isolated plugin intercepted an actual `loopflow:` OSC 8
  hyperlink through the installed link API and injected Ctrl-click mouse events
  in its running TUI. It received the unchanged URL, server socket, workspace,
  tab and clicked pane. A visible nonfocused split correctly supplied its own
  pane identity. Bare custom-scheme text was not detected; use OSC 8. This
  supports a thin Herdr link handler passing URL and bounded per-activation
  context to the shared opener. It does not prove a physical desktop click.
- **cmux 0.65.0 (108):** both its shell-resolved `open` and `/usr/bin/open`
  delivered a surrogate custom-scheme URL unchanged to a disposable macOS
  receiver. The real `loopflow:` registration was not changed. Physical rendered
  clicks could not be tested because computer use reported active interaction.
  Custom-scheme click interception with context therefore remains unproved;
  successful HTTP interception and shell opening cannot fill that gap.
- **OS dispatch loses current context:** cold launch from a host supplied its
  environment, but later URL events kept that first environment. A receiver
  launched from Herdr pane 2 still reported pane 2 for a URL sent from pane 1,
  then still reported Herdr for a URL sent from cmux. Recorded URL events had
  sender PIDs, with app/path unresolved, and no supplied pane metadata. Do not
  infer current host from the receiver environment or global focus.

These probes proved host transport, before `lf open` existed; the Herdr
handler and the shell-resolved cmux path were then built on them. Keep clean Work URLs. Herdr's plugin can provide
exact context per activation; cmux currently has a proven direct-command path,
with native custom-click forwarding still to establish. OS-only entry retains
Desktop-first routing and the known-destination/plain-terminal fallback when
origin context is unavailable. Rex and Linux activation were not tested.
Detailed matrices, raw-evidence locations and cleanup are in the two reports.

### Availability and review opportunities

Derive available targets in Rust from the existing shared Session reader.
Include interactive conversations available for Jack to work with, including
repository, Wave, standalone Task and secondary conversations. Deduplicate a
Task's primary against its Work target; keep distinct available secondary
Sessions as explicit Session targets. A review, question or next-action owner is
not an inclusion requirement. A Task's CI or dependency condition cannot stand
in for conversation availability. Exclude active headless Runs and unknown
availability; resumable history remains reachable by ordinary opening rather
than being asserted currently available. Empty authoritative input yields
`No available conversations`; incomplete input reports what is unknown.
Do not create a second availability rule in the opener or Swift.

Use `LF_AGENT_CALLER`'s actual Session identity and Work association to compare
with the shared selected destination. Same Work alone is insufficient: a Flow
step and the primary conversation may share a Task. Return
`current_conversation`, `other_conversation` or `unknown` in link/report data.
The first presents the question/demo inline, the second supplies the Work link,
and unknown names the missing evidence with a Task link but no invented Session.
Update task-operate and corresponding Task/Flow/Wave report guidance together.
Never select a different primary just to make a report's equality test pass.

## Alternatives and failure model

- Keep Desktop URLs and teach every agent host commands: rejected; duplicates
  discovery, fails Desktop-absent clicks, and repeats identity guesses.
- Encode a provider command or Session in a host-specific URL: rejected; pins a
  transient conversation and bypasses Work ownership. Generic command URLs also
  turn content into executable shell text.
- Local HTTP link redirector: rejected; another server, lifetime and address
  owner, with no advantage over installed scheme receivers.
- Origin token in every clickable URL: unnecessary for OS/app discovery and
  binds presentation to a producing process. Prefer clean Work URLs with
  activation-time context. OS-only dispatch can lose exact terminal context;
  prove native integration rather than claiming discovery always recovers it.

Wild success is ordinary opening becoming boring: concise names, draft intact,
no searching for IDs and one final focus change for a queue. Wild failure is
three competing stores of conversation identity, silent duplicate launches after
timeouts, or polished links that the renderer cannot click. The plan deliberately
keeps selection and attachment in Rust, makes host observations non-authoritative,
and carries rendered proof separately from fixture results.

Review finding: Task dependency conditions and conversation availability are
independent; next-action ownership must not drive this command. Another finding: pure URL parsing tests cannot
prove dispatch from a terminal; the install/receiver boundary is required scope.

## Delete — do not maintain

- Done: remove `DesktopCommand::Open` and its dispatch/explanation-only wrappers;
  move useful `opening_context` and receipt behavior into the shared opener.
  Retain Desktop list, pane control and text reads.
- Done: replace `opening_url`'s old Task scheme and Swift `TaskLink`'s path/Session
  query grammar. Remove ordinary Task-link gates in `prepareLinkedTask` that
  require Session/diff/internal conversation intent. No old/new parser fallback.
- Done: old URL fixtures/tests and docs replaced, including `global_commands`'
  `desktop open` explain/delivery tests (now `lf open`). Keep encoding,
  ambiguity, stale-result, remote-owner, retained-draft, external-client cases.
- Done: remove the blocking-Session Desktop command recipe from task-operate and all
  ordinary report emitters. Preserve provider/Linear URLs as separate actions.
- Checked: `terminal_title.rs` keeps its own cmux calls. They rename this
  surface (`CMUX_SURFACE_ID`) behind the attachment fence; `lf open` creates and
  focuses workspaces (`CMUX_SOCKET_PATH`). No shared dispatch replaces either.
- Checked: no `home desktop` references remain. Keep `pr open` explicit in docs
  and command-resolution tests.

Forbidden: Swift primary selection, titles as destination identity, Work IDs
inferred from paths, silent takeover, replaying launch argv on reentry, new Work
on link inspection, dual link schemes, a parallel Session/workspace store, Linux
Desktop launch, or a local fallback when the execution owner is unavailable.

## Remaining implementation — one complete delivery

Built in the first pass (October 18):

- `work/link.rs`: the one Work address parser/formatter (URI component
  encoding, validation, OSC 8 and Markdown labels). `session`/`diff` are explicit
  CLI selectors carried to Desktop; ordinary links never emit them.
- Root `lf open` (`lf/commands/open.rs`): folder-name resolution collapsed by
  canonical location and identity, ambiguity listing qualified links, emitted
  qualifier when shared; `--link` (text/OSC 8/Markdown/JSON), `--explain`,
  `--available`, `--session`, `--diff`, `--host auto|desktop|cmux|herdr|rex|terminal`;
  Desktop via Launch Services bundle id, Herdr/cmux from direct caller context
  (Herdr first: innermost), terminal in place for a person, `.command` on macOS,
  `xdg-terminal-exec`/`x-terminal-emulator` on Linux; 2 s host timeouts; unknown
  effects never retried elsewhere; live conversations reported `elsewhere`
  with the `--takeover` command; one focus per batch; `current_conversation` /
  `other_conversation` / `unknown` from the journal's agent caller.
- `lf desktop open`, its explanation DTO/fixture, `opening_url` and the Flow
  `open → desktop` alias are deleted. Desktop pane control reuses
  `open::selected_context`.
- Swift: incoming URLs resolve only through `lf open --link --json`
  (`RegistryQuery.workLink`, `WorkLink` DTO); `TaskLink` is a typed internal
  target (no URL grammar); router/model/views carry it; external Work links open
  the conversation; the choice window is gone (Rust lists qualified links).
- Vocabulary: AGENTS.md, glossary, docs/website, operate/report skills; scoped
  terminology test in `documented_commands.rs`.

Built in the second pass (October 18, after rebasing onto v0.14.7):

- Desktop router carries `LinkedWork` (`.task(TaskLink)` / `.wave(id, address)`);
  external Wave addresses and the palette's cross-repository Wave both select the
  Wave in its repository window. Upstream's `loopflow://open?repo=&wave=` grammar
  is gone (no second scheme).
- `work/link_receiver.rs`: installation (`install promote`, beside native hooks)
  registers a `loopflow:` receiver when Desktop is absent — an `osacompile`
  applet on macOS, an XDG desktop entry + `mimeapps.list` default elsewhere —
  running `/usr/bin/env -u <HOST_CONTEXT…> lf open -- URL`. Another chosen
  handler is kept; installed/just-placed Desktop removes the applet.
- Herdr link handler: installation writes `~/.lf/integrations/herdr` and runs
  `herdr plugin link` when Herdr is on PATH; the click passes the URL as one argv
  with the clicked pane's own context.
- Destination reuse: attached clients record `cmux:<ws>` / `herdr:<ws>` in the
  existing `processes.terminal_id` placement (Desktop's `LF_TERMINAL_ID` wins);
  reopening a live conversation focuses that workspace (`usable`) when the caller
  is in that host, else `elsewhere`. A per-Work lock serializes lookup/create and
  remembers a just-created workspace for 30 s, so repeats/races focus it.
- Vocabulary: `SessionAvailability` / wire `availability`;
  `harness::activity_tracker::SessionActivityTracker`; `RoadmapSection::NotStarted`
  (`not_started`, "Not started"). Swift's Available filter already reads Rust's
  single availability rule (upstream removed the separate projection).
- `lf open` is now observed as a Process, so `LF_AGENT_CALLER` reaches it: links
  report `current_conversation` vs `other_conversation` (a Flow step on the same
  Task is the other one). Before, every relation read `unknown`.
- `WorkOpeningCLITests`: ambiguous folder name
  opens nothing; the qualified address opens that one repository window.

Built in the third pass (October 18):

- Batch focus: a batch's Desktop addresses go in one Launch Services request,
  final item last (`-g` only when the final item went elsewhere). Desktop's
  router opens arrivals serially in order and raises only the latest arrival's
  window; serial opening also keeps a same-repository batch's earlier items.
- Roles: `waiting` split into `warning` (blocked, errors, unavailable) and
  `available` (Session availability, human Workflow node, ready bell):
  `WorkTone`, palette, editor, chooser, catalogue keys (available derived from
  ANSI 5). `~/.lf/themes` files are rewritten `waiting-*` → `warning-*` on
  load; other folders report the retired key without applying it.
- Tests: `WorkOpeningCLITests` covers CLI-registered Task and Wave addresses and
  finds `target/debug/lf` like `WorkCLISubscriptionTests`, so the swift gate
  suite (which builds `lf` first) runs it. `work_open_tests` adds the
  unreadable-owner case: the link prints, opening refuses, nothing local starts.
- Names: cmux/Herdr destinations and attached terminal titles append where the
  Task stands (`Available`, then its Workflow node), via
  `terminal_title::standing`; provider `--name` keeps the Session name.

Compress (fifth): dropped unused `WorkUrl::kind`, narrowed module-only `pub`; `--lib work::link` 11 pass, clippy clean.

Remaining: none planned. "Question" has no shared reader, so titles do not say it.

## Done when and gate checks

Add a composed fixture using the built CLI and isolated Machine stores: two
repositories share a folder name; one has a standalone Task, one Task executes
on a simulated peer, three conversations are available (one with no question
or review), and another has a running tool. Include a Task whose CI is running
while its conversation is available. Seed an existing primary and active
attachment. The real Rust resolver emits a Work URL consumed
by production Swift routing; a host stand-in observes the terminal dispatch.
Repeat and race opening. Assert one primary/worker, one retained Work destination,
unchanged draft, correct execution owner, truthful receipt, no inference from CI
status alone and at most one focus. Qualified links select exactly one repository; ambiguous
links change nothing. Include same-Session, other-Session-on-same-Task and missing
caller cases. Stand-ins isolate host/network effects; assertions concern resulting
Work/Session state, displayed selection and draft, not just mock call sequences.

Gate commands (new test module/suite names are implementation targets):

- `uv run python scripts/test.py --list`, then
  `uv run python scripts/test.py --reuse-passing`: affected suites, formatting,
  Clippy, DTO fixtures, documented command resolution, headless Swift and app
  build pass under the repository's materialized/network-isolated runners.
- Built CLI → Swift integration: `WorkOpeningCLITests` in the swift suite
  (`target/debug/lf` or `LOOPFLOW_TEST_LF`; no installed CLI substitutes).
  Expected: composed scenario above passes, with actual Rust reads and routing.
- `cargo test -p loopflow --test integration work_open_tests:: < /dev/null`
  on Linux and macOS with inherited `LF_*` removed. Expected: all route/no-route,
  ambiguity, availability, race and failure cases pass; Linux auto mode never invokes
  Desktop and an explicit Desktop request names its terminal alternative.
- Receiver installation is covered by Rust unit tests in `work::link_receiver`
  (disposable XDG dirs, applet Info.plist/script, argv-safe Herdr dispatch,
  host-context stripping) rather than a pytest; there is no uninstall command.
  macOS Launch Services registration itself is demo evidence.

Prove the integration boundary with absent and unavailable host adapters: Work
selection and attachment continuity stay unchanged, generic opening remains
usable, and an uncertain host creation never triggers a duplicate fallback.
Include two activations from different hosts into the same running receiver;
its launch environment must not influence the second activation's origin.

Production leaf-view tests inspect names, reason labels, opening/elsewhere/error
states and review presentation headlessly. Display-only click/draft/native-host
checks remain demo/review evidence. The generic opener may report opening before
input readiness; only owner/readiness evidence permits usable. No task completion,
Workflow move or extra Flow follows merely from opening.

Check (2026-10-18, fourth pass, no code change): `work_open_tests::` 10 passed; `--lib work::` 53 and `terminal_title::` 3 passed with `LF_*` unset (earlier terminal_title failure was inherited env); clippy/fmt clean. Gate owns the full matrix.
