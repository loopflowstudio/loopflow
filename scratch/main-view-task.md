# A calmer desktop workspace

Accepted design direction, 2026-09-25. Governing design for Product / **LOO-291**
in this checkout. Continue this Task; do not create another planning item.
The human accepted the composition as “good enough” to encode for implementation.
This approves design direction, not native delivery, publication or Task completion.

Build one repository-scoped workspace that drills down **Wave → Task → named
Session**, showing planned work, Flow execution and conversations without losing
terminal context.

This document supersedes conflicting proposals in the
[historical design and evidence](main-view-task-history.md): no public Project
level, one-Session cardinality, separate Session-with-context depth, or competing
Work/Sessions navigation. Prior evidence retains its original scope.

## Reference prototype

Run from the repository root:

```sh
uv run --no-project python -m http.server 8317 --bind 127.0.0.1
```

Use these exact combinations; the older A/B/C alternatives are not the target:

| Surface | Accepted executable reference |
| --- | --- |
| Frame and Wave plan | [A with captured planning](visual-study/mockups.html?v=a&population=current&repo=loopflow&task=none) |
| Unstarted Task | [Flow C](visual-study/mockups.html?v=a&population=current&task=LOO-285&flowstudy=c) |
| Running, paused, blocked Task | [Task 2 scenarios](visual-study/task2-study.html#running) |
| One named Session | [Task 3](visual-study/task3-study.html#one) |
| Multiple Sessions and membership | [Task 3](visual-study/task3-study.html#multiple) |
| Density | [20 started Tasks across Waves, 50 Tasks per Wave](visual-study/mockups.html?v=a&population=large) |

[Prototype source map](visual-study/README.md) identifies the code and data.
[Reference manifest](visual-study/accepted-reference.json) records exact file
hashes. The current-data snapshot preserves captured Loopflow/Etude descriptions
and KRs; Kata has unavailable chapters. Task execution, comments and Sessions
are explicitly simulated. No live provider, PM mutation or native terminal is
connected. Research: [workspace affordances](research-workspace-design-603c72da.md)
and [Flow references](visual-study/flow-research.md).

## Accepted experience

### Repository and Wave

“A calmer desktop workspace” governs spacing and hierarchy. Reuse burgundy,
cream, original Wave typography and the dark terminal surface. Task titles use
the shared sans-serif face; skill names use monospace.

The repo selector heads one connected sidebar. Scope to one repo by default;
put its Waves and started Tasks immediately below it, search at the bottom.
Started means shared evidence of work beginning, independent of an open Session
or currently running provider. Do not make Tasks flicker with process polling.
An unstarted Task appears in its Wave's full plan, not the sidebar. Inspection
alone does not start it. Preserve selected work during partial reads/transfers.

Click a Wave for **objective → Current KRs → full Task plan**. The enduring
objective belongs to Wave; Tasks, KRs and metric targets belong to its current
internal Project. There is no Project/Chapter navigation tier. Keep historical,
unmatched and repository/Wave Sessions reachable through the same inventory.

### Task

Show the full Task title once, with **New session beside the title**. Breadcrumb:
parent Wave / issue ID; the issue ID links to Linear. Remove Open, oversized
repeated titles, Local work evidence, ELSEWHERE badges and internal preview copy.

The main order is Flow, execution state, open Sessions when present, Description,
then collapsed **Comments (count)**. Render actual Markdown. Comments carry dated
updates; Description carries the current problem, outcome, constraints and
material blockers. Never move stored description paragraphs into invented
comments. [Description prompt changes](task-description-prompts.md) guide future
writing; existing Linear descriptions were not rewritten.

| Task situation | Behavior |
| --- | --- |
| No Runs started | Preview recommended/selected Flow; Start; “Not started · No runs yet.” Clicking Flow name becomes searchable typeahead. |
| Execution, no Session | Same diagram with actual current occurrence, iteration and running/paused/blocked summary; recent Runs disclose on demand. |
| One Session | Clicking Task enters that exact Session; count and parent breadcrumb retain access to Task overview. |
| Several Sessions | Task overview lists names, provider, state, Flow membership and useful summary; choose the exact conversation. |

New session opens an independent conversation with Task context in its worktree,
including before Flow execution. Resolve/prepare the checkout through the existing
Task operation; do not start the managed Flow, replace another Session or create a
second Task. Preparation alone must not be presented as an executed Flow step.
The action honors the configured destination and reports failures on the Task.

### Flow

Feature is one continuous path rendered from its actual authored/pinned
definition. Human clarification on 2026-09-25 accepts the continuation branch's
second decision loop after demo. This supersedes the earlier single-loop
prototype topology. The human then requested revisiting the UI: the two-edge
execution shape is accepted, but its visual composition needs another prototype
review before native Flow-diagram implementation. Session work can proceed:
The subsequent instruction is to keep building and have that UI discussion once
the working build is ready. The human confirmed the delivery tail: final
**Advance → queue → land**, while final **Iterate → implement**. Include this
tail in the revised diagram and bind it to the real delivery operations/state;
do not label queue admission as a completed merge. This design direction does
not itself authorize publishing or landing this Task during implementation.

```text
design → implement → compress → review-slice → concept-review → loop-decide → demo → loop-decide → queue → land
             ↑                                                    │                    │
             └────────────────────────────────────────────────────┴────────────────────┘
```

The current sibling definition has `task-design` before `pursue`. In `pursue`,
`decide` and `decide_delivery` are separate `loop-decide` occurrences, both
returning to `implement`; `demo` sits between them. Preserve both edges and
their independent traversal evidence. A return after demo can revisit demo on
the next pass. Design remains outside both loops. Do not hardcode occurrence
identity from the repeated skill name or substitute the prototype's older
single-loop sample for the real definition. “Finally” is a forward section,
not an unconditional cleanup promise.

Use rounded pale-blue regions and return arrows to expose the authored repeated
spans, including the second return after demo. Keep this visibly one Flow. Center
**Loop**, or **Loop · Iteration 3** once executing; arrow and running state are
blue. Completed steps—including human steps—are green; pending human steps
yellow; blocked red; paused neutral. No You/person header or Iterate arrow label.
Display literal lowercase skill names without descriptions; click for detail.
Completion is occurrence-specific, not permanent across loop traversals.

Once started, show the pinned definition. Hover/focus exposes **Stop & restart…**:
search a replacement, then confirm. Cancel/selection alone changes nothing.
Pause/Resume use the real saved boundary; Blocked exposes the reason and help.
Helping a blocked Run is not approval or automatic advancement. Unsupported
controls must remain unavailable with their real reason, never simulate success.

### Session

“Selecting a session should be like drilling down even further from wave > task
> session.” Continue the breadcrumb with its **Session name**. Task and Wave
ancestors navigate upward; the Task ID remains a separate Linear link. Remove
Description and the parallel Overview/Session-with-context/Session mode switch.
The Task ancestor can show its truncated title here because no Task heading is
repeated below. Multiple Sessions can be selected by name at the final breadcrumb.

Initial name: use the **invoked skill** when present (for example,
`review-design`); otherwise reuse the historical **magical musical animal**
generator for raw Sessions. Allow **rename in place**. Keep identity,
history and draft unchanged by renaming. A user name takes precedence over later
automatic suggestions; reject blank edits, retain the old name on failure, and
publish authoritative readback. Track generated versus human-assigned provenance
with the canonical title; do not guess it from whether text resembles a skill or
animal. Reuse/recover the historical generator rather than add another naming
service. Cycle 1 recovered the historical magical-musical pair generator (34 magical
and 26 musical words); history contained no animal list. The original lists
are reused, with stable Run-derived selection so reads do not write.

Add concise guidance to the existing builtin `LOOPFLOW.md` once the shared rename
operation exists: “If this Session still has its generated name and a short,
specific name better describes what you are discussing, update the Session name.
Keep a human-assigned name. Do not rename the Task, worktree or branch.”
The operation must also preserve a concurrent manual rename. Name generation
must not delay opening a Session or require a separate model call.

Distinguish **active Flow name + step + iteration** from **Independent**. Use
explicit shared membership in that exact execution, never matching Task, cwd,
provider or current skill. Historical Flow membership must not be relabeled as
Independent; incomplete evidence must not claim either classification.

Restore the existing native Session surface. Navigation, rename and selection
preserve drafts, viewport, focus, split layout and companion processes. Opening
another conversation must not implicitly transfer/stop a client. Closing a view,
completing a Session, and completing its Task are different actions.

## Implementation shape and owners

This is one coherent native composition change within the existing Task,
implemented in internal slices and delivered together. Do not start parallel
replacement stores or ship the website as the native conversation renderer.

| Existing owner | Responsibility to retain/extend |
| --- | --- |
| Rust planning/Run/Session projection | Typed Work, started-work evidence, Flow definition/occurrence, exact Run membership, title and legal actions |
| RegistryQuery / RegistryQueryLocal | Shared reads and mutations, existing active-Run stream; no per-pane readers |
| PodiumModel / WorkspaceProjection | One published reading and derived hierarchy; preserve unavailable/last-good distinctions |
| WorkspaceNavigation | Repo/window selection and ancestry, expansion/search/scroll; Session ID independent of name |
| SessionsView / SessionsStore | Exact open/preparation/error handling through shared Session actions |
| SessionsWorkspaceRegistry / MultiplexerStore / native surface pool | Retained worktree layouts, native input and processes |

`SessionRecord` already has `id`, `runId`, `title`, `work`, actions and terminal
IDs. Reuse these. Its current wire shape does not expose active Flow membership;
`kind == flow` alone is insufficient. Project the exact execution/step link from
shared ownership evidence, including historical and unavailable outcomes. Extend
Rust/Swift contract fixtures together, without defaults or a Swift-only join.

Proposed operation seams, implemented on existing owners:
`selectTask(id)` inspects or opens its single exact Session;
`selectSession(id)` restores it and publishes ancestry;
`renameSession(id, name)` updates the title through shared Session authority;
`startTask(id, flow)` and `restartTask(id, flow)` dispatch existing Task operations;
`newTaskSession(id)` resolves Task checkout and launches independently.
These are responsibilities, not instructions to add parallel APIs if equivalents
exist. Reuse the shared Flow loader for search/preview and pinned execution for
running diagrams. Read real comments/counts through the planning boundary.

## Internal slices and proof

1. Shared naming contract: implemented and reviewed in
   [cycle 1](implementation-cycle/cycle-01-review.md), including name carry-over
   and agent Run-ID resolution. Flow-specific behavioral proof remains.
2. Named native Sessions: implemented in
   [cycle 2](implementation-cycle/cycle-02-implement.md) — shared exact Flow
   membership recorded at Run capture, native breadcrumb drill-down and inline
   rename/readback. Remote rename remains source-only proof.
3. **This slice:** replace the existing frame, Wave and Task presentation with accepted A/C;
   integrate actual Flow definitions/occurrences, all three Task situations,
   comments and real controls. No per-view runtime inference.
4. Demonstrate the configured app, measure and review the complete experience.

| Done when | Required observation |
| --- | --- |
| Real planning and working set | Loopflow/Etude/Kata reads preserve exact content or explicit gaps; unstarted work remains reachable under Wave; started rows survive process exit. |
| Flow correctness | Definition search, pinned topology including both accepted Feature return edges, iteration/occurrence colors, pause/resume/restart and failures match real shared state. |
| Session identity | One/multiple drill-down, automatic/editable name, explicit Flow/independent membership and historical/unknown cases survive refresh/reopen. |
| Native continuity | Same provider/surface, exact unfinished draft, viewport and responding companion survive Session switching, breadcrumb return, repo return and rename. |
| Description/comments | Source Markdown, actual collapsed count/thread, failed reads and rejected edits remain truthful. |
| One authority | Searches find no duplicate inventory, per-pane reader, lifecycle policy, title store or terminal renderer; Project stays internal. |

The configured demo follows Wave → unstarted Task → Flow preview → Start →
running/paused/blocked work → independent New session → multiple Session choice
→ rename → Task/Wave/repo return. Use proof-owned clients and real shared records;
retain failures and distinguish simulated fallback proof from configured results.

Keep the existing `hierarchy_interaction_ms` and `task_workspace_ready_ms`
measurement obligations with comparable populations and source identity. Include
the accepted 20-started/50-per-Wave density case. Capture/OCR timing does not prove
compositor latency or hitches. Preserve full-Task external trials, authorized
Description edit/readback, published budgets and separately deferred optimization
work; this visual acceptance does not waive them or authorize new PM writes.

## Evidence and remaining decisions

Accepted composition and website interaction proof are recorded in
[Flow research](visual-study/flow-research.md), [Task 2](visual-study/task2-design.md)
and [Task 3](visual-study/task3-design.md). Native stream/retention proofs remain
in [active-runs-stream.md](active-runs-stream.md); integration evidence in
[post1281-integration.md](post1281-integration.md). Their historical source hashes
and limits apply; none proves the newly designed native UI.

New session placement and editable automatic naming are now resolved. Remaining
technical investigations and external acceptance inputs are in
[questions.md](questions.md). Do not reopen settled visual choices as blockers.
