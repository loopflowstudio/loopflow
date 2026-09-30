# Workspace UX after the data model

LOO-303 · Product · rewritten 2026-09-30 for Jack Heart. The design was approved
in interactive review on 2026-09-26. Merge `01dda6a26` put this branch on the
LOO-298 model (Exec, AgentSession, FlowSession). Earlier pass logs, proof receipts
and the LOO-291 evidence copied in as `from-loo291/` live in git history.

## Goal

Jack wants to reach work from the keyboard or a link, see a Flow's plan before it
runs and its exact progress afterward, and put an orphan conversation under its
Task without losing the terminal. The installed LOO-291 workspace is the base.

## Accepted decisions

- **Light mode only, no teardown.** Jack's comment `41ea97a6`: installed build
  `0.12.22+5838f53a6` "is working well". Build independent pieces first. Dark mode
  waits until light is accepted and is designed from the mock's charcoal, not by
  opacity from light.
- **Orphan sessions.** Jack: "move the sessions to the bottom near the search bar,
  and give it a header like Orphan Sessions … clicking into that header gets you a
  special multiplexer with all the orphan sessions in some sort of control room".
  Header: "Orphan sessions · N", collapsed, above search. The header opens the room
  and a chevron toggles the list. Tile every orphan with no cap. An orphan is a
  conversation whose Task is null, including Wave-only ones, never matched by
  checkout path in Swift. Pre-model orphans in a Task worktree stay orphans with
  that Task preselected. When the room empties after a bind, jump to the bound
  Session under its Task.
- **One universal bind.** Jack: "worth making the design and architecture simple
  and universal if it takes a little extra". It uses one operation and one picker
  from the room, ⌘K and Task-page Session rows. Bind is write-once: no rebind or
  unbind. The picker names the exact target and confirms once. It never changes
  the name, panes or Flow membership. Binding makes the Task Started. Earlier usage
  keeps its recorded owner.
- **Two Flow views.** Jack: "tasks mostly show flows (i.e. minimally unrolled by
  default, but progressively disclosable), but once its running it goes into the
  flow invocation mode which is fully unrolled." A Task with no started Flow shows
  the folded template, even when it already has independent conversations. When a
  Flow starts, the Task shows the captured, fully unrolled FlowSession.
- **The Wave page shows the Project's Flow.** A review participant (name
  unresolved) said "This is waht i meant by wave flows": the current Project's
  default Flow for Tasks is shown folded on the Wave page. There is no Wave-level
  execution, start control or invocation page. Flows outside Tasks stay in overall
  monitoring only: "i dont think we need something like the task page for flow
  invocations outside task".
- **⌘K palette.** It covers Waves, Tasks, Sessions, Flows and actions, with recents
  first and shortcuts shown. The bottom search stays a filter.
- **Deferred.** Wave-page New session and the question of where a Session can
  launch from are deferred "until we like the basics". The waveless first run is
  also deferred.

## Built on this branch (local proof only)

- **⌘K palette** with typed destinations and ≤20 recent destinations per
  window/repository. Search keys never reach a PTY. Escape restores the terminal.
- **`loopflow://task/ISSUE` links** use the exact reader `lf roadmap --task ISSUE
  [--all] --json`. Historical, completed and colliding Tasks are handled, and
  inspection starts nothing. A pending link survives cold launch and is delivered
  to one window. Late results cannot override a later click.
- **Folded templates.** Composition is kept in the shared Rust resolution traversal
  (repeated and empty groups, XOR, both return edges, content revision). Disclosure
  is recursive, works from the keyboard and is shared across Task, Wave and catalog.
  A folded return shows its hidden target's real name.
- **Terminal input isolation.** Hidden retained Ghostty views refuse first responder.

The 2026-09-30 merge onto LOO-298 kept all of this. Template graphs now use the
parent's captured numeric node IDs, and the parent's file browser and active
Session stream are kept. Evidence is focused Rust, CLI and Swift checks plus the
Xcode build-for-testing, using fixture transport and owned PTYs. Nothing here is
installed or live acceptance.

## Remaining work

1. **Invocation view.** Render the Task's captured FlowSession: cursor, per-edge
   return counts, loop passes as lenses, and retained history for finished or
   replaced Flows. Missing capture shows as unavailable. Never redraw history from
   today's template. Before building, check where the parent keeps retries (see
   questions.md).
2. **Exact running line and Session chip.** Replace the "first active Task Run"
   provider and Task-updated elapsed time with the exact current step's
   AgentSession/Exec from the parent's projection. An independent Task conversation
   running at the same time must not change the line.
3. **Orphan section and control room.** Classify by the parent's typed null Task,
   replacing roadmap-unmatched grouping. Add a `.controlRoom` content mode with a
   typed per-repository key in the window's workspace registry. Only the active host
   mounts terminal NSViews: today's hidden checkout host stays mounted at opacity 0,
   so it must be conditionally mounted. A conversation attached to a shell keeps
   that `.shell` terminal. Several records on one shell get one mount plus "Shared
   terminal · Show here". Entering the room launches no providers. Build the room
   and working bind together.
4. **Bind entry points.** The parent already ships `lf session bind SESSION --task X
   [--dry-run] --json` and a Desktop "Bind to Task… → Bind permanently" beside a
   Session name. Reuse that single operation from the room, ⌘K and Task-page rows.
   After success, use the authoritative result plus request generations so a stale
   poll cannot bring back the orphan. Keep the Session, surface and draft. Show the
   parent's reasons when a bind is illegal.
5. **Deletion.** Remove first-orphan header navigation, roadmap-based orphan
   grouping, Task-level running-provider selection, the reverse Session lookup, and
   any duplicate bind UI once their replacements are live. Update `swift/README.md`
   and `docs/lf.md`.
6. **Configured demo.** Walk the demo below on the installed candidate against the
   selected Home. Capture 1440×900 and 1100×800 on live data and get Jack's verdict.
   Name the binary, commit, Home and exact IDs. Label fixture results as fixture
   results.

## Demo

`open 'loopflow://task/LOO-303'` opens that Task's page without starting work.
⌘K reaches any Wave, Task or Session. An unstarted Task expands a folded sub-Flow.
A started Task shows the unrolled FlowSession with exact current-step detail.
"Orphan sessions · N" opens the room. Type a draft, choose Bind to task…, and
confirm the named target once. The Task becomes Started, and the same Session,
terminal and draft now appear under it.

Candidate note (2026-09-27): the first demo attempt stopped before launch. The
installed release predated these changes, and the combined branch could not safely
open the selected dev Home. That Home has since been removed; `~/.lf` is the only
Home. Prepare the candidate through the supported release path after LOO-298 lands.

## Done when

- A mounted native proof binds an orphan from the room. The row moves under its
  Task without relaunching the client, and the draft and child reply survive. Cover
  more than six orphans and two records sharing one shell.
- Races do not redirect or resurrect anything: a competing bind, a stale poll, a
  rejected or uncertain write, completion elsewhere, and a repository switch
  mid-bind.
- A Task shows the folded template before its Flow starts and the captured
  invocation afterward, including after its template source changes.
- ⌘K and cold and warm Task links work on the installed app.
- Captures at both widths on live data, accepted by Jack.

## Constraints

- Swift consumes the shared CLI projections and never infers ancestry, attempts or
  Started from cwd or by counting Runs.
- There is one native mount per terminal. Closing a view is not completing it.
- LOO-298 (#1296) lands first. Sync by merging, not rebasing. On later LOO-298
  merges, keep this branch's deletion of LOO-298's scratch files.
- Perf work belongs to LOO-300. Reuse `hierarchy_interaction_ms` and
  `task_workspace_ready_ms` with palette, link, room and post-bind scenarios.
  Target p95: ≤100 ms for warm navigation, ≤50 ms for palette filtering, with at
  least 20 samples.
- The inherited `wave_chapters` architecture gap is closed: the merge check passed
  33/33 SQLite owners.
