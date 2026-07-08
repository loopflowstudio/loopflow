---
produces: scratch/ux-research/loop-NN/proposal.md
---
Pick the ONE Loopflow behavior this loop will try to enable, and the specific
questions to answer about it — framed from the POV of personas using the app.

## Before you write

Read the accumulating state so this loop starts where the last one ended:

- `scratch/ux-research/questions.md` — open questions + the target-behavior
  backlog. The top unclaimed backlog item is usually this loop's behavior.
- `scratch/ux-research/design-guidelines.md` — what we already believe about
  Loopflow's UX. Don't re-litigate settled beliefs; build on them.
- `scratch/ux-research/personas.md` — who we're designing for.

React to the **actual current UI**, not a blank slate:

- `swift/LoopflowMac/Views/RepoSidebarWindow.swift` — the shipped
  reset surface (repo sidebar filtering a wave list; rows display-only).
- `swift/LoopflowMac/Views/ContentView.swift` — the slice-1 list.
- `swift/LoopflowCore/Models/Wave.swift` + `WaveViewModel.swift` — the data a
  row *could* show (`status`, `statusText`, `iteration`, `diffStat`,
  `openPRCount`, `waitingReason`, `trigger`, `visionTagline`). Note what the
  current row throws away.
- `scratch/waves-one-level-out.md` + `wave/desktop/GOAL.md` — the roadmap and
  guardrails: frame-don't-render, GOAL/MEMORY = singular identity, repo is a
  filter. The intended target is a terminal-first *wave screen* (harness pane +
  yazi over GOAL/MEMORY/scratch + ad-hoc terminals + RepoWork strip).

## Pick the loop number

List `scratch/ux-research/loop-*`. This loop is the next integer. Create
`scratch/ux-research/loop-NN/` and write `proposal.md` there.

## Write the proposal

One behavior, stated concretely as a persona doing a thing with a time/success
bar (e.g. "a developer with several active waves opens Loopflow and within
seconds finds and opens the ONE that needs their attention"). Not a feature, a
behavior.

Then the questions to answer about it — grouped as:

- **Discoverability** — can the persona *find* the thing without knowing where
  to look?
- **Legibility** — once found, does the screen say what they need at a glance?
- **Friction** — how many steps / how much doubt between intent and done?

Ground each question in a specific of the current UI. Name what's at stake.

## Output

`scratch/ux-research/loop-NN/proposal.md`:

```markdown
# Loop NN — Proposal

## Target behavior
<one concrete persona-framed behavior with a success bar>

## Why this behavior now
<what in the current UI / backlog makes this the right next thing>

## Questions to answer
### Discoverability
- ...
### Legibility
- ...
### Friction
- ...

## Out of scope this loop
<what we're deliberately not deciding>
```

Keep it to one screen. The point is a sharp target, not a survey.
