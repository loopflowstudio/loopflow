# Arrange direction — October 16, 2026

Jack's supplied design conversation with Claude is the accepted product direction;
implementation choices and the questions below remain draft. This condensed
brief is not new approval. The complete original and separate annotated transcript
are preserved at `refs/loopflow/evidence/loo-497-kickoff:arrange-navigation-brief.md`
(tree `69b193fdfd4136530ce911a05df9c05463d0ad84`, blob
`370a2056506056ffec0a54d285b69bf487cc3677`), readable with `git show`.

## Problem and accepted decisions

Desktop is a useful single-terminal experience but buries open Tasks, lacks a
multi-Task view, wastes a header row, and makes switching feel like losing a
Session. Jack wants Work to organize the screen. The win is seeing which of
twelve Tasks needs attention, entering it, and returning without losing position
or live state. Borrow terminal polish; differentiate through knowing the Work.

- **D1:** Selection changes which Work is shown; arrangement changes only
  structure. Every pane has a Work owner and every visible Work belongs to a
  selection. No arbitrary workspace, content insertion, or cwd shell API.
- **D2–D4:** Repo sidebar: open Tasks attention-first, then Waves. Wave sidebar:
  its open Tasks. Task: no sidebar. Clickable repo → Wave → optional Project →
  Task breadcrumbs occupy the native title bar; contextual actions on the right,
  no second header. The title bar follows the focused workspace; other panels
  keep a slim tail-crumb header.
- **D5–D6:** A Wave displays its open Tasks as live tiles. One pane per tile:
  primary Session, overridden only by explicit zoom, never ordinary focus.
  Enter the Task for shells, files, logs and other Sessions.
- **D7:** Containers auto-tile attention-first until structurally touched, then
  preserve arrangement; newcomers enter an inbox and departures collapse.
- **D8:** Each Work owns one layout. Containers store Work/query references;
  tiles project the referenced Task's own layout. Entering a Task reuses its
  surface. No second Task layout.
- **D9:** Layout memory is keyed by Work, with containers additionally keyed by
  query hash, not window. Reopening should restore it, including on another
  machine. Concrete persistence and transport remain implementation work.
- **D10–D11:** One query mechanism covers predicates and enumerated Task sets.
  Repo/Wave defaults use `state:open`. Pinning promotes queries into the sidebar.
  ⌘K searches all Work and queries; remove persistent Find Work. Switching repo
  opens its separate window.
- **D12:** One strongly separated window per repo: native cycling, Mission
  Control and Window menu identity. Literal Dock icons remain unresolved;
  ordinary macOS window separation is the handoff's default.
- **D13:** Click and CLI call the same model; `lf desktop state --json` comes
  first and exposes enough state to drive and verify every operation.

## API boundary (original §1.4a governs)

Historical boundary: October 17's accepted corrections in `design.md` replace
repository shells and shell-per-pane creation with Task-only Shell/Files tabs,
and replace custom default-view binding with independent Dashboards. The original
proposal below remains dated context, not the current implementation contract.

Arrangement: `split --pane <target> --axis v|h`, `move`, `swap`, `resize`,
`zoom`/`unzoom`, `hide`/`restore`, direct/spatial `focus`, and `state --json`.
Targets include `session:<id>`, `task:#N`, `wave:<name>`, `query:<id>` and raw
pane IDs. Hidden members remain in state and restorable. Arrangement never
changes membership or ownership.

Selection: `view <ref|query>` replaces the focused selection; `open <ref|query>`
opens beside it; `query <container> "<expression>"` changes membership;
`shell`/`files`/`log --task <ref>` create scoped companions; a repo shell uses
the repo root. `pin`/`unpin` persist sidebar queries. `rename` writes through
to the Work owner, so all titles follow. `view --plan` emits a derived tree
without touching Desktop, with Work ref, cwd, command, title and attention.

Nested query references expand live. Removing a predicate-matched Task requires
editing the query; hide only changes visibility. The transcript's illustrative
`add` and Task summary's `add` are superseded by §1.4a. No pane-local names,
cross-repo arrangement, ownerless shell, or `new-pane` escape hatch.

Query seeds: `repo:`, `wave:`, `state:open|available|running|blocked`, `pr:open`,
and `task:#412 or task:#418`. Grammar and sorting syntax were not accepted.

## Owners and unsettled details

- LOO-489: navigation, sidebar, title bar/menu audit, grid, queries/pinning UI,
  Work-keyed layout integration and click acceptance; update DESIGN.md to allow
  derived layouts while rejecting arbitrary monitoring walls.
- LOO-497: selection/arrangement API and shared model, state first, headless plan.
- LOO-416: Session retention, app-close survival, reattachment, switching and
  measured surface capacity. No new resident daemon is selected by this brief.
- LOO-498: consume plans in cmux/Rex/Herder; not selected for this launch.

Open product choices: Project crumb versus grouping, inbox appearance, pinned
query UI, query sorting syntax, literal Dock icons, Rex menu/title-bar inventory.
Surface capacity and lifecycle mechanism need evidence from LOO-416. A focused
workspace's crumb is already accepted; no need to reopen that choice.

The comparison favors permanent IDs, explicit ambiguity, JSON, bounded
send/capture/wait, events and OSC 7501 status. Those are directions, not evidence
that every command is absent or cheap. Check existing implementations.
LOO-453 owns readiness/events; LOO-422/415 own status transport; LOO-414 owns
remote launch. “Maestro” in the conversation does not establish an additional
service or planning writer.

The title-bar audit belongs to navigation: keep Open Repo readily reachable;
consider File/Open Repo/New Window, Go/crumbs/back/forward/⌘K, View/sidebar,
zoom/splits, and Window/repo entries. No actual Rex chrome inventory was made.

## Why these choices matter

The sidebar supplies a working set; ⌘K supplies reach; crumbs supply hierarchy.
Tasks participate in queries rather than acquiring duplicate layouts. A primary
Session gives a stable tile face while shells take focus inside the Task. Auto
layout makes entry useful; touched layout protects deliberate arrangement.
Success is the same Work loop through clicks, CLI and later another multiplexer.
Failure is drifting state or duplicate native surfaces for repeated references.
