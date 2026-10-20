# Arrange direction — October 16, 2026

> October 18 accepted correction — Jack Heart: side-by-side Tasks appear only as
> query results, and queries are shown only through Dashboards. The historical
> generic `open <ref|query>` beside-current-selection requirement below is
> superseded. Ordinary navigation selects one destination. Current accepted
> scope and remaining work live in `scratch/design.md`.


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

## API boundary (historical)

The original §1.4a arrangement/selection command proposal (split, move, `open`
beside, repo shells, query seeds) is superseded by `design.md`'s Task-only
Shell/Files tabs, independent Dashboards and October 18 no-outer-split decision.
Text: `d813d4157b:scratch/arrange-navigation-brief.md`. Still true: arrangement
never changes membership or ownership; hidden members remain restorable; nested
query references expand live; no pane-local names or ownerless shell.

## Owners and unsettled details

Owners: LOO-489 navigation UX; LOO-497 Dashboards and the selection/arrangement
model; LOO-416 Session lifetime; LOO-498 external hosts (see arrange-handoff.md).

Open product choices: Project crumb versus grouping, inbox appearance, pinned
query UI, query sorting syntax, literal Dock icons, Rex menu/title-bar inventory.
Surface capacity and lifecycle mechanism need evidence from LOO-416. A focused
workspace's crumb is already accepted; no need to reopen that choice.

Competitor comparisons and the title-bar/Rex inventory notes are dated evidence;
see the preserved text above.

## Why these choices matter

Sidebar: working set; ⌘K: reach; crumbs: hierarchy. Tasks join queries without
duplicate layouts; a primary Session is the stable tile face. Auto layout makes
entry useful; touched layout protects deliberate arrangement. Failure is drifting
state or duplicate native surfaces for repeated references.
