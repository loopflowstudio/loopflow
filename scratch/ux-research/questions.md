# Concerto UX — open questions + target-behavior backlog

The running agenda for the UX research loop. `propose` reads this to pick the
next loop's behavior; `learn` updates it each pass. Distinct from
`scratch/questions.md` (that's the waves-model backlog, not UX).

## Target-behavior backlog (ordered)

Each item is a concrete persona-framed behavior a future loop can target.

1. **[in progress — loop 01] Triage & open the wave that needs you.** From cold
   launch, find the one attention-needing wave across the portfolio and open it.
2. **Resolve the default surface: attention-band (A) vs portfolio-board (C).**
   Loop 01 surfaced this tension and deliberately left it open. Loop 02's likely
   target: same triage behavior, but evaluated at portfolio scale (30 waves) and
   the 1-wave extreme, to see which surface holds up at both ends.
3. **Land inside a wave: the terminal-first wave screen.** Once a wave is opened,
   does the harness pane + yazi (GOAL/MEMORY/scratch) + ad-hoc terminals +
   RepoWork strip actually let the developer do the next thing? First loop that
   targets the wave screen's *internal* layout.
4. **The waiting-nudge.** When a terminal-hosted wave needs you and the app isn't
   focused, is the rollup `waiting` chip enough, or is a native nudge required?
   (Flagged as the sharp one in `wave/desktop/3-wave-surface-ux-exploration.md`.)
5. **Quick-start a wave** from a repo roadmap / Asana item — authoring, not
   triage. Later.

## Open questions

### Resolved by loop 01
- **Is a bare status word enough to route attention?** No. Rows need the
  *reason* (G1). Resolved.
- **Should attention-needing waves be sorted up, or found by scanning?** Sorted
  up, always (G2). Resolved.
- **Does the shipped repo-sidebar-first layout serve the triage behavior?**
  Weakly — it over-weights repo and fights the cross-portfolio glance (G4). Repo
  should be a filter, not the primary axis. Resolved for this behavior; revisit
  for large-portfolio browsing.

### Still open
- **Default visual surface: A vs C?** The loop-01 tension. Which optimizes the
  triage glance without failing the solo (Sol) and power-user (Kai) personas?
  Needs loop 02 evidence at both scale extremes.
- **Can status columns (C) coexist with repo grouping,** or must the board pick
  one spatial axis and lose the other? If repo-as-place matters for Tess at
  scale, this decides whether C is viable.
- **Where exactly does "Open" land, and how do we prove "frame, don't render"
  held?** The wave screen doesn't exist yet; until it does, the open affordance
  in this loop's candidates is routing to a target we can't fully evaluate.
- **`⌘K` palette discoverability.** If the palette (D) is the power path, how
  does a newcomer discover it exists without cluttering the surface for everyone?
- **Attention priority ties.** Two `failed` waves, or `failed` in a repo Tess
  doesn't own vs `waiting` in one she does — what's the tiebreak? Recency, repo
  ownership, blast radius? Undecided.
