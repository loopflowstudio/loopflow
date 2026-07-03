# Loop 01 — Proposal

## Target behavior

**A developer with several active waves opens Concerto and, within seconds and
without clicking into anything, identifies the ONE wave that needs their
attention right now — and opens it.**

Success bar: from cold app launch, eyes-to-answer in under ~5 seconds across a
portfolio of ~20 waves in ~5 repos, and one action to open the wave that needs
them. "Needs them" = a wave that is `failed`, or `waiting` on something a human
must clear (e.g. PR limit reached), before a healthy `running`/`idle` wave.

## Why this behavior now

This is the whole job of the just-shipped reset surface
(`RepoSidebarWindow.swift`), and today it does the job weakly:

- **The row is a flat status word.** `RepoSidebarWaveRow` renders
  `wave.statusText` — literally `"Waiting"` / `"Failed"` — in the status color.
  The model already carries *why* (`WaitingReason.prLimitReached(open, limit)`,
  `iteration`, `diffStat`, `openPRCount`, `trigger`) and the row throws all of
  it away. A wall of same-shaped rows where "Waiting" and "Running" differ only
  by a small colored dot and word does not answer "which one needs me."
- **Attention isn't ranked.** `filteredWaves` preserves registry/insertion
  order. A `failed` wave can sit below three `running` ones. Nothing floats the
  thing that needs a human to the top.
- **The rows don't open.** The slice-1 rows are display-only ("Rows are
  display-only in this slice"). The behavior's final step — *open it* — has no
  affordance yet, so we get to design it rather than retrofit it.
- **Repo-first nav may fight the behavior.** The sidebar defaults to `.all`, but
  its whole visual weight says "pick a repo first." For "find the one wave that
  needs me across everything," repo is the wrong primary axis — attention is.
  The roadmap says "repo is a filter, not a container"; the current layout makes
  it feel like the container. Worth testing whether the shipped hierarchy serves
  this behavior at all.

## Questions to answer

### Discoverability
- Across ~20 waves / ~5 repos, can the developer spot the attention-needing wave
  *without* first choosing a repo? Does the repo sidebar help triage or force a
  repo-at-a-time scan that hides the portfolio view?
- Should "needs you" be a place you look (a view/section) or a property you
  scan for (sort + color)?

### Legibility
- Does a row say *why* a wave needs attention, or just *that* its status is
  non-green? Is `"Waiting"` enough, or does the behavior require
  `"Waiting — 3/3 PRs open"`?
- At a glance, can a healthy portfolio ("nothing needs me") be told apart from
  one with a fire, in under a second — before reading any single row?

### Friction
- From "that's the one" to "I'm in it," how many actions and how much doubt?
  Where does opening a wave *land* the user — and does that honor the
  terminal-first wave screen (harness pane + yazi + terminals + RepoWork strip)
  rather than a rendered detail panel?

## Out of scope this loop

- The wave screen's *internal* layout (harness/yazi/terminal proportions). This
  loop stops at "the list routes attention and opens the right wave"; the wave
  screen is a later loop's target.
- The Wave/RepoWork model split (backend A2). We design against today's
  `wave.repo`, noting where `repos: [RepoWork]` would change a row.
- Creating/quick-starting waves. This loop is triage-and-open, not authoring.
