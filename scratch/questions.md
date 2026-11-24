# Choices & open threads

The decisions and exact wording for B1 are in [plan.md](plan.md) and
[demo-notes.md](demo-notes.md).

## Open threads (not in B1)

- Sorting by Graph step (`B2` in `explore.html`).
- Smart “heading to the end” sink for `Auto` / `deliver` / settling PRs (`B3` in `explore.html`) — Jack Heart: “i think we need something like moving auto to the bottom, or being able to 'close' certain tasks, but auto has to be smart enough to know like if its moving towards the end, that coutn even if its not explicitly auto”.
- Manual “Close / Park” for individual Working Tasks (`B4` in `explore.html`).
- LOO-937's Project-rank redesign remains separate from Working sidebar order.
- Surfacing `TaskCondition::Blocked` in selection queries.

## Private Machine QA (2026-11-23)

- In [rounds 4 and 5](qa.html), a returning locked-screen launch opened by
  link stayed on saved planning (“Updating…”), highlighted the saved selection,
  and ignored sidebar clicks. Moving the private `desktop-cache/workspace.json`
  aside restored the window. The Machine order survived both launches.
- In [round 5](qa.html), navigating from a Wave to the grid stalled twice for
  17–18 seconds with the Machine under load. The sample was in SwiftUI layout.
  The cause and behavior in an unlocked window remain open.

## Compression choices

- Treat the five sidebar status groups as presentation state in the Mac window.
  Group each row once and keep its incoming order within the group.
- Read lifecycle, rank and Wave membership directly for Task ordering. Resolve
  display identifiers only when building rows for a view.
