# Canvas reset and performance — 2026-09-24

## Latest hierarchy amendment

The human's one-Wave/one-current-Project direction supersedes the exposed Project
tier below. Public navigation is repo → Wave → Task → Session in one outline;
Full presentation does not restore Projects. Chapter Projects remain internal
content/history ownership. Integrate `loopflow.projects`' shared chapter/direct-Task
projection while retaining this branch's outline, pane ownership and exact Run
links. Keep compression and flat Sessions, both performance scenarios, and one
optimization Task per area. See [the reconciliation review](review-wave-reconciliation.md)
for observed source and the next integration boundary.

## Launch decision — LOO-291

Keep one end-to-end core in the existing Product / Desktop Task, checkout and
branch: **one compressible navigation outline, Task Monitor content in the
existing terminal multiplexer, and repeatable measurements of both user paths**.
These settle the shared identity, pane-lifetime and measurement contracts that
the later optimization Tasks must consume. Implement here; do not start a second
worker or split the measurement foundation into a competing UI implementation.

Current `lf task status LOO-291 --json` identifies Task
`task_2aa71a7e36fe416d8a721e2b2f7c54e7`, Project `desktop`, this checkout and
branch `jack-heart/main-view-task`. Its returned state is ready with resume
recommended; its active PR record has no publication. This read establishes no
exclusive checkout ownership. Preserve the existing dirty implementation,
shared-contract corrections and independently supplied hierarchy study.

### Core implementation boundary

1. Replace the competing repository banner, global status instruments, All work
   route and standalone Task-view controls with one outline and one restrained
   typography system. Repository is a root in that outline. Reuse the existing
   shared planning/Session readers and stable typed identities. Keep ordinary
   terminals and scoped conversation creation reachable through contextual
   actions; opening Monitor or selecting a Task never starts a provider.
2. Implement full, compact and Session-leaf presentations through that same
   outline. Implementation defaults: compact first, one presentation menu,
   branch disclosure folds descendants, compression promotes descendants.
   Omit singleton repo/Wave/Project scaffolding only when readable planning
   establishes it; retain access to omitted subjects through the same outline's
   contextual actions/full presentation. Keep Task identity visible in compact
   mode; Session-leaf mode supplies minimal disambiguating ancestry. Preserve
   selection, search, scroll and branch expansion per window/repository.
3. Extend existing pane content with a stable Task-bound Monitor. Reuse the
   checkout's `MultiplexerStore`, outer layout and window surface pool. A Session
   leaf focuses its original terminal; a Task selection restores its retained
   pane choice, initially Monitor if no choice exists. This is an implementation
   default, not a claim of a separate human preference. Sessions/Monitor controls
   reveal retained content; allow both beside a companion shell through the
   existing split/resize/zoom controls. Do not replace the only reference to a
   live terminal to display Monitor. A Task without a checkout can be inspected
   in its repository workspace without creating a worktree.
4. Initially Monitor lists exact currently active Task Runs, with identity,
   readable label, provider and shared activity state where known. Interactive
   waiting is active when its client remains live. Use the Rust Run/ownership
   evidence to establish both Task attribution and liveness; deduplicate multiple
   Exec/process nodes belonging to one Run. Never equate unfinished Run metadata,
   Session presence or cwd coincidence with a live Task Run. Unknown/stale evidence
   stays distinct from a confirmed empty list. One reader owns this evidence;
   opening multiple panes must not create a poller for each one.
5. Establish `hierarchy_interaction_ms` and `task_workspace_ready_ms` during
   implementation, with the endpoints and scenarios below. Supply one repeatable
   native performance command, fixed small/large populations, real retained PTYs,
   per-attempt results and a before/after comparison report. Measure observable
   rendered/usable outcomes and frame hitches. Collect the baseline before any
   targeted optimization. Expose attribution phases in development evidence,
   not new UI widgets. Configure relevant regression runs and measured budgets;
   keep errors/timeouts and unavailable-host outcomes explicit.
6. Preserve Task directive editing, Project/KR inspection and shared human Session
   actions through contextual access; they must not reintroduce another hierarchy.
   Keep repo/Wave/Project conversations and unmatched mandatory boundaries
   reachable. Finish with the real human demo of the simplified composition,
   combined panes, truthful active Runs and retained drafts/input.

### Contract work that cannot be delegated yet

`ActivitySnapshot` currently carries Exec/process IDs, cwd and Wave, but no
explicit Run/Task join. `RunSnapshot` carries subjects but does not itself prove
live ownership; ordinary `lf runs` scans only the last seven days and applies a
fifty-Run budget while retaining every unterminated Run within that window.
Settle the exact live-Run projection inside this core, reusing shared Rust
evidence and mirroring any wire change in Swift/fixtures. Do not ship a Monitor
that silently loses an old active Run or attributes another Task's provider by
checkout. Include these cases in focused behavioral proof. Coordinate with
LOO-293's actual shared contracts if they arrive; do not copy a partial DTO from
another checkout or launch another Watch implementation.

Hierarchy compression and mixed pane rendering are also not implemented yet.
The HTML study is design evidence, not a native baseline. Existing one-off
Session logs and AX count samples do not establish the two new measurements.

### Two named follow-ups — intentionally deferred

No new Tasks are created or launched by this decision. After this PR settles
and provides stable benchmark commands, populations and baselines, create one
Task per experience under Desktop:

- **Optimize hierarchy navigation, first pass.** Use the shipped
  `hierarchy_interaction_ms` scenarios for compact/full/Session-leaf views,
  expansion/filtering and scrolling during refresh. Find the largest measured
  avoidable cost; make one bounded reduction. Deliver comparable before/after
  latency/hitch samples and prove identity, ordering, compression, missingness
  and selection unchanged. No new navigation, caches without measured need,
  background redesign or benchmark-only completion.
- **Optimize Task workspace interaction, first pass.** Use the shipped
  `task_workspace_ready_ms` scenarios for Task changes, existing Session focus,
  Monitor switching and combined-pane resize/zoom. Find the largest measured
  avoidable cost; make one bounded reduction. Deliver comparable before/after
  latency/hitch samples with the same terminal processes, draft, focus, companion
  panes and truthful active-Run population. Never improve timing by dropping
  content, recreating a terminal or claiming readiness early.

Both depend on contracts and baselines this core will establish. Starting them
now, even stacked, would duplicate design/measurement work and optimize a surface
being replaced. Retain these as named design follow-ups until the parent PR
settles; do not file placeholder Tasks or add durable staging state.

### Acceptance and lifecycle

Focused proof covers singleton/multiple/missing ancestry, duplicate labels,
stable selection across compressed presentations, exact Task/Run attribution,
old active Runs, waiting/dead clients, mixed panes, and retained real terminal
input through hide/switch/resize/zoom. Both native and fallback build paths must
compile. The two repeatable performance scenarios must produce honest usable
results, with host gaps distinguished from product failures. Human confirmation
of the new composition remains required; prior technical receipts do not supply it.

The new UI supersedes old layout and one-current-conversation proposals; it does
not waive the original ten human-selected external planning/Session trials,
authorized directive edit, or twenty long-lived-registry planning trials and
published budgets. The Project's fourteen-day Sessions proof and LOO-251's Ask
caller release remain their existing obligations. Do not confuse these with
the repeatable rendering benchmarks or move them into optimization follow-ups.
LOO-284's overlap is an existing shared-contract contribution to reconcile,
not a new child or permission to mark another Task complete. History, throughput,
output feeds and diagrams remain later monitoring work, including existing
LOO-293; do not enlarge this first Monitor increment to implement them.

This invocation runs the launch-plan **skill**, not the complete same-named
flow. The following local step is `implement`. No parent worker is launched,
interrupted or restarted here. A later `pr land -c` must not complete LOO-291
while its retained Task acceptance obligations are unmet; missing proof must
remain explicit at the gate/demo boundary.

## Direction

Strip the desktop back to a blank starting surface, design each addition with
the human, and measure its cost before moving to the next addition.

Human anchors:

> The app is still jsut confusing me overall
>
> I want to go towards more of a blank canvas where we add elements one by one
> and then set up perfomrance testing and one simple first-pass optimization
> child for everything relevant

Clarified: **strip back; design together**, and **one Task per area**. This is
an incremental design process, not a user-configurable canvas/widget system.
Review the previous Product chapter before choosing elements or filing work.

## Accepted navigation direction — one compressible hierarchy

The human chose the first element:

> a simple repo -> wave -> project --> task -> session hierachy
>
> just shows up in only one place. it isnt split across multiple panes or concepts
>
> very good at collapsing to the simplest possible represtnation (e.g. list of
> all sessions) because the extra hierarchy "disappears". or if we only have
> 1 project per wave for example

Build one navigation outline for repository → Wave → Project → Task → Session.
The full hierarchy and a compact Session list are presentations of the same
outline, with the same identities, selection and opening behavior. This replaces
the earlier proposal to begin with an independent Active Task list. Sessions
are explicit leaves; the old prohibition on Session children is superseded.

Proposed compression rules, to validate in the first visual study:

- Omit a structural level when it adds no meaningful distinction: one Project
  within a Wave need not consume another heading and indentation level. Preserve
  access to its identity/details without requiring its permanent heading.
- Distinguish folding a branch from compressing a level. Folding intentionally
  hides descendants; compression promotes them so the useful rows stay visible.
- A Sessions-only presentation promotes Session leaves into one list. Show
  minimal ancestry where needed to distinguish them; do not repeat the full
  breadcrumb on every row. Tasks with no Session remain reachable in the work
  hierarchy, without fabricated Session leaves.
- Keep repo/Wave/Project conversations directly associated with their actual
  subject. A Session need not fabricate a Task or Project to fit the normal
  path. Unknown attribution stays explicit within this same outline.
- Compression changes presentation only. Stable shared IDs, selection, terminal
  grouping and live client ownership do not change when ancestors disappear.
- Keep ordering stable through ordinary polling. Derive singleton structure
  from the available planning scope, not transient provider activity; incomplete
  evidence cannot establish that only one child exists.

Use one typography system and one row/disclosure vocabulary. The outline is the
sole place to navigate this hierarchy: no parallel repository hierarchy in a
banner, alternate All work/Tasks/Sessions navigators, or nested navigation panes.
Selected content can display its own details without becoming another navigator.
Exact compression affordance and whether it is automatic or user-selected remain
design choices; do not preinstall several new filter controls.

First demo: use the same sample population in full structure, a Wave with one
Project, and a flat Session presentation. Select the same Session in each and
retain its terminal/draft. Include multiple repositories, same-named Sessions,
an upcoming Task, a non-Task conversation and unavailable ancestry. The human
should recognize one hierarchy in every presentation.

Placement remains Product / Desktop, existing LOO-291. This amendment supersedes
the assumption that the current complete interface should simply pass another
retention demo. Existing implementation and evidence remain available.

## Task content — Sessions and Monitor

Human direction:

> per task two differnet views: open sessions -- the previous UI with the
> embedded ghosty or mre of a monitoring view showing all the runs that have
> run, curernt throughput, things like that. but to start, just current active
> runs for that task

The human clarified their relationship:

> a switch is ok to start, but we should fundamentally be hooking them into
> the same multiplexer that we had on top of terminals

Sessions and Monitor are content in **one existing multiplexer**, capable of
being shown together. A switch is only an initial presentation affordance,
not two mutually exclusive workspaces with separate layout/focus owners.

Selecting a Task provides these pane contents:

- **Sessions:** its open human Sessions in the existing embedded Ghostty
  workspace, preserving companion terminals, layouts, drafts and explicit
  client transfer. Selecting a Session leaf in the outline focuses its exact
  existing terminal pane. Do not introduce another hierarchy inside this view.
- **Monitor:** initially only the Task's currently active Runs, attributed by
  shared Task identity. Use shared activity/liveness evidence; a matching
  checkout or an unresolved Session alone cannot establish an active Task Run.
  An interactive Run may appear here as well as having a Session: these are
  views of the same work, not mutually exclusive categories.

Switching content preserves Task selection and its terminal workspace. Monitor
is passive: opening it launches, resumes, transfers and resolves nothing.
An empty successful read says no active Runs; unavailable activity stays explicit.
Proposed initial rows identify the Run and its provider/skill where available.

### Multiplexer integration

Extend the existing `PaneContent`/`PaneState`/`LayoutNode` representation with
Task-bound Monitor content. Reuse `MultiplexerStore` for layout, focus, split,
resize and zoom. The existing per-checkout workspace registry and window-local
native surface pool remain their owners; do not create a second Task multiplexer
or a Monitor-specific split tree. Task identity supplies the Monitor's query
subject independently of its checkout placement.

The initial Sessions/Monitor switch reveals or focuses retained content through
that owner. It must not destroy a terminal or overwrite its only pane reference
to show Monitor. Support the design's combined arrangement: Monitor beside an
embedded Session, with an ordinary companion terminal in the same inner layout.
Outer checkout splits retain their existing distinct purpose.

Closing a Monitor pane closes observation only. Hiding it may suspend its reads;
neither operation stops a Run. Monitor panes never acquire native terminal input
focus. Revealing a Session restores its existing surface without duplicating or
transferring it. Shared per-Task readings can feed multiple presentations without
one poller per pane. Exact switch styling and first-open layout remain proposals.

Run history, throughput, output feeds and flow diagrams are later additions,
not prerequisites for this first Monitor view. LOO-293 already owns Watch/output
and flow history; reconcile its eventual presentation with Monitor rather than
introducing a third competing Task view or discarding its existing work. This
narrows the first visible increment, not LOO-293's durable completion criteria.

The demo includes selecting a Task, switching Sessions/Monitor, selecting its
Session leaf, and active Runs appearing/disappearing truthfully. Show Monitor
beside a Session, resize/zoom through the existing multiplexer, and confirm its
draft and companion survive. First-open layout remains undecided.
Performance cases cover active-Run reads/rendering, retained Session switching
and the combined layout under the same Task workspace experience metric.

## Human screenshot — 12:44 PDT

> theres just all these different hierarchys and orginzational widgets and font changes

The supplied screenshot shows a branded repository/status band, an All work /
creation toolbar, a Tasks heading, a centered Task view selector, search, and
Wave/Project headings before the first Task. Serif headings, uppercase tracked
branding, monospace status, and several control treatments compete. This is
direct usability feedback about the accumulated composition, not evidence that
any individual query or terminal operation failed.

For the next composition, propose a plain native window and one consistent text
style. Add only the first human-selected useful element; choose grouping,
navigation and status when that interaction needs them. Do not rebuild the full
current hierarchy with smaller fonts or hide it behind a configurable dashboard.
The subsequent direction above selects one compressible hierarchy as the first
element; its exact visual controls remain to be designed together.

## Chapter findings

The [22 September baseline](../.lf/chapters/20260922-manual-baseline/review-ledger.json)
is the previous review, with incomplete historical coverage. The
[23 September reconciliation](../.lf/chapters/20260923T000959Z-502f011b/desktop-reconciliation.md)
is the accepted current plan, not a completed chapter result.

| Previous Mac Surface UX KR | Recorded verdict | Consequence for this design |
|---|---|---|
| 1: expose every repo, Wave, purpose, planning and activity | Unknown | Already narrowed to selected planning/Session journeys. It does not require all information on the initial screen. |
| 2: visible state agrees with shared APIs | Unknown | Keep exact identity, actions and reasons behind whichever element we add. |
| 3 and 6: launch/navigation and scoped-list performance | Unknown | Publish a repeatable baseline and budgets; current receipts do not establish these KRs. |
| 4: broad control-room operations | Fail | Already narrowed to useful journeys, an authorized edit and Ask completion. Avoid restoring the old control bundle. |
| 5: no second transcript or Swift Work authority | Holds | Preserve the shared foundation through presentation changes. |
| 7: interactive Session readiness | Fail | Measure actual input readiness and truthful failure, not a pane's appearance. |

Auditability additionally failed its reason/provenance claims. Expose the reason
where an action needs it; adding a permanent dashboard is not required by that
outcome. Company Dogfood still measures external product progress, independently
of UI completion or benchmark results.

Current Desktop D1 combines planning/Session usefulness and shared correctness.
D2 combines responsive opening and truthful availability. Its fourteen-day real
use windows and twenty-observation requirements remain recorded obligations;
automated performance tests complement them. Any KR revision must be explicit.
Neither KR directly tests whether a person can identify their next action without
coaching. Today's human feedback establishes that missing usability proof.

## Existing work to reconcile

Read `lf roadmap --wave product --json` at 19:44 UTC on 24 September; this is a
local cached planning read, not a fresh Linear sync. Receipt:
`/tmp/loo291-canvas-roadmap.json`.

- **LOO-291:** current workspace; use it for the incremental presentation reset.
- **LOO-251:** native Session/Ask proof and Session readiness. Preserve its
  remaining behavioral obligations when assigning optimization work.
- **LOO-284:** separately queued, although its shared action/path implementation
  is present on this branch. Reconcile delivery ownership before duplicate work.
- **LOO-293:** Task Watch/output/flow diagram. Treat Watch as an optional later
  element in the design and a distinct performance area; do not cancel its work.
- **LOO-280:** packaging prerequisite only if the configured path requires it.
- **LOO-281/282/283 and LOO-148:** chapter-parked shell blocks, client location,
  simultaneous viewing and recovery. Generic roadmap readiness is not a new
  prioritization decision. Revisit only with the relevant observed need.

## Additive sequence

1. **This slice:** design one compressible hierarchy and define the two experience
   measurements below up front. Implement their measurement path alongside the
   first native UI increment, before expanding the interface. Preserve retained
   terminals while changing presentation.
2. Establish repeatable baseline scenarios for the hierarchy and Task workspace.
   Reuse configured launch/query/native test paths rather than historical
   single-use mutation probes. A blank-surface startup measurement alone cannot
   establish either interaction's performance.
3. Add one agreed element, demonstrate its real action, measure its incremental
   cost, then decide the next element together.
4. One bounded first-pass optimization Task per measured experience, below. Each
   consumes the runner, targets a measured cost, and produces before/after
   evidence. Do not require speculative optimization where no cost is found.

Proposed Tasks remain under Desktop; these are not child Projects or new Waves.
Execution order follows the accepted elements and measured bottlenecks.

## Two experience measurements — agreed scope, proposed endpoints

The human requested identifying approximately two rendering/UI performance
measures up front, with ongoing monitoring and optimization of the high-level
experiences. This narrows the earlier six-area proposal to two optimization
Tasks. Reads, layout, polling and rendering are diagnostic phases within them,
not six independently prioritized performance programs.

| Experience / proposed first-pass optimization Task | Start → finish | Repeatable scenarios |
|---|---|---|
| Navigate the work hierarchy | Accepted expand/fold/compress/filter input → the correct updated rows visibly presented and usable | Full hierarchy and flat Sessions; small and large fixed populations; scrolling while shared data refreshes |
| Open or switch a Task workspace | Accepted Task/Session selection or Sessions–Monitor switch → the correct destination content visibly presented and usable | Monitor's active Runs or confirmed empty state; return to an existing Ghostty terminal; switch Tasks and return with a retained draft and companion |

Use stable metric names `hierarchy_interaction_ms` and `task_workspace_ready_ms`.
Report scenarios separately: a retained terminal switch must not be pooled with
starting/resuming a provider. When launch/resume is measured, shared preparation
and provider readiness remain separately identifiable phases. The initial
Sessions–Monitor switching scenario uses an already-running owned terminal.

The finish is rendered content and usable controls/input, not a model assignment,
completed CLI read, view creation or `.onAppear`. Pair each timed scenario with
an outcome assertion: exact displayed identities, expected active Run population,
and retained terminal focus/input where applicable. Use a harmless owned PTY for
repeatable input proof; keep configured-provider verification separately labeled.
Measure frame hitches during scrolling/switching alongside latency so a quick
first response cannot hide a stuttering interaction. If the runner observes AX
availability rather than actual frame presentation, label that endpoint and
retain the rendering measurement gap instead of calling it paint time.

### Set up once, use throughout development

- Extend the existing native UI test path with the two scripted journeys and
  stable, versioned populations. Include a large hierarchy and multiple retained
  terminals from the start. Avoid a new inventory or provider simulator in the
  production model.
- Add correlated timing intervals at the production interaction boundaries.
  Capture read/decode/projection/layout/render phases for diagnosis using the
  existing logging facilities. A single interaction ID ties phases to a sample;
  instrumentation is not a new product state owner.
- Write machine-readable per-attempt results plus a compact comparison report:
  metric, scenario/population version, build, host, cold/warm state, duration,
  outcome and frame-hitch evidence. Retain failed, interrupted and timed-out
  attempts. No Session transcript or secret belongs in performance records.
- Baseline before optimization; compare the same scenario/population/build mode
  after each relevant UI addition. Report median, p95 with sufficient samples,
  counts and failure rate. Choose and publish regression budgets from that
  baseline before scoring; no numeric target is invented in this design.
- Run the short deterministic scenarios for relevant UI changes, and retain
  configured-host runs separately. Reports belong in development/CI evidence,
  not another dashboard in the simplified app. An unavailable host is an
  unavailable measurement, never a pass.
- Each of the two optimization Tasks takes a measured bottleneck, makes one
  simple first pass, and returns a comparable before/after result with identity,
  truthfulness and terminal-retention checks intact.

These contracts are recorded up front. Instrumentation, the reusable runner and
baselines are still to be implemented with the new native interaction; the
existing one-off logs do not fulfill this setup.

Current `PodiumView` refreshes process activity and Sessions every two seconds,
and planning every fifteen seconds. `SessionsLatencyMetrics` records load/pane
events through OSLog. These are measurement starting points, not proven causes
of slowness. Repository lifecycle/resource budgets are a separate existing system.

## Proof and limits

The human can identify and perform each new element's intended action without a
script, then explicitly confirms that step. A blank window alone is not useful
completion. Existing human boundaries remain reachable as their replacement
entry points are introduced; no client is stopped merely to simplify the view.

Benchmark records identify build, Home, population, provider, cold/warm state,
endpoint and every attempted outcome. Keep small deterministic regression cases
and configured end-to-end samples separate. Report sample counts, p50/p95 where
supported, timeouts and resource use; publish budgets after baseline measurement
and before scoring. No new timing results or numeric thresholds are claimed here.

Open: the hierarchy's compression interaction; final optimization Task boundaries
after that choice; explicit KR revisions if it changes the opening contract.
No PM edits, new Tasks, worker launches or product source changes in this review.
