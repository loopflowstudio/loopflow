# Loopflow — what designing Cadenza taught us

Probe: design a real roadmap (Cadenza), watch where loopflow's model + tooling
strain. Cadenza recut into **Core** (trunk) ⊃ { **Scores** [product], **Feedback**
[research] }, landed live as cadenza **PR #21 (merged)**.

## Structural gaps (the model)

1. **Projects nest; waves are flat.** Core is a trunk with sub-projects. No
   containment relation in the wave model — chord = waves *watching* waves, not
   *containing* them. Needs a mutable parent/child relation; the launchboard
   renders a tree with rolled-up child state. Constraint: cheap to rename/split.

2. **Work-shapes are plural; every flow assumes "ship a PR."** Product track
   (Scores: increment → PR → deploy → release) vs research track (Feedback:
   instrument → collect → experiment → evaluate; output = dataset/findings; "not
   shipping yet"). No research archetype. Needs a **research flow** (done =
   artifact, not merge) and a declared **posture** per project/track that changes
   its flows, its definition of "next," and how progress is shown.
   *Highest leverage — sits directly under the most-invested track.*

3. **Launchboard is single-project + opinionated; Concerto is generic +
   multi-wave.** Open it, see *Cadenza* — its tree, next actions, attention — and
   launch in. Three first-class affordances: **start new** (`lf design --ide`),
   **continue next** (ingest next item), **one-off → full deploy** (fix → PR →
   release in one motion). Targets an arbitrary repo (≠ loopflow itself).
   *The visible prize, but downstream of #1 and #2.*

4. **Roadmaps are behaviors, not cartography.** Items as "X can now do Y,"
   ordered by impact; progress = "can the user do X yet." Lean the item model and
   the design/kickoff steps harder into behavior-framing.

5. **Directions are personas.** Adoption spine (me → son) = project-scoped
   directions (`daily-practice`, `son-onboarding`); view the roadmap *through* a
   lens.

## Live tooling friction (observed while flushing PR #21)

- **`lf op wt create` leaks the `lf()` shell-function body to stdout** before the
  "Created worktree" line. Reads like an error. Suppress the directive-sourcing echo.
- **`lf op land` merges to main with no clear "MERGED ✓ / queue-state" line.**
  Had to `gh pr view` to learn it merged (not queued). A command that merges must
  say so — and this is the engine behind the launchboard's "one-off → full deploy."
- **Worktree renamed under the active cwd on land** (→ `…roadmap.20260629_1429`),
  staling the cwd mid-session. Footgun for any scripted follow-up.
- `lf op land` auto-lowercased the PR title vs the commit subject. Minor.
- No `.lf/` in cadenza → the flat-wave re-cut was pure filesystem. Validates the
  design's "trivially re-cuttable" claim.

## Surfaced design wrinkle (feeds #2)

Feedback ended up holding **both** research (capture/notation) and shipped-product
human-response (video messaging) — a posture conflict *inside one sub-project*.
Suggests **posture attaches to items/tracks, not just whole projects** (or
video-comms belongs in Core). Work-shape pluralism biting on day one.

## Leverage call

Pull **#2 (posture + research flow)** first — the gap under the most-invested
track, and the flush already showed it biting. #1 (nesting) is plumbing; #3
(launchboard) is the prize but downstream. The friction items are a parallel
quick-wins lane.
