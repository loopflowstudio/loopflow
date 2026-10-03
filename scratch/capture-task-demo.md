# Capture Tasks demo — 2026-10-02

Jack Heart requested launching the LOO-368 demo. Built and opened Loopflow Dev
from this Task checkout with `uv run python scripts/loopflow-dev.py run`.
Swift build, bundled control CLI build, signing and LaunchServices open succeeded.

The configured launch is blocked before end-to-end capture: installed `lf skill
show capture-tasks` reports `skill not found`. The development app's control
configuration points at the machine-selected CLI gate, and its bundled CLI
forwards to that installed runtime. The design's earlier assumption that a
branch-built app alone supplies the new skill is insufficient.

Jack has not yet supplied visual feedback or accepted a button label. No capture
conversation or cross-repository filing was demonstrated, and no Flow cursor or
review gate was completed. Next: provide a configured runtime containing both
the new CLI launch syntax and capture-tasks, then exercise the walkthrough in
[capture-task.md](capture-task.md). The current UI is open for visual review.

## Screenshot and naming review — 2026-10-02

Jack Heart supplied a Desktop screenshot confirming the Create Task action opens
a shell whose CLI rejects `capture-tasks`, then requested comparison with Codex,
OpenCode, Notion, Linear and Asana before settling the button.

Local demo workaround: linked `~/.lf/skills/capture-tasks.md` to this checkout's
builtin skill. Installed lf 0.12.31 resolves it through `lf help capture-tasks`
and assembles `lf context --skill capture-tasks --json` from the main repository.
This supersedes the missing-skill blocker for this machine, but does not prove a
live Desktop conversation or repair release packaging. Remove the local link
when the builtin ships so it cannot shadow future releases. No runtime or shared
store was replaced. The button still needs a fresh click to verify provider launch.

Naming remains undecided. Agent proposal for Jack's review: a compact compose
entry labeled New conversation, with the initial prompt explaining that ideas
can become Tasks. The current large filled row looks selected at rest and claims
immediate Task creation although capture begins an open-ended conversation.
No label change is accepted or implemented.

## Accepted visual reference — 2026-10-02

After reviewing the comparison board, Jack Heart said, “I think Linear is the
right model here.” Use Linear's dedicated compose row beneath workspace identity
as the reference for the capture entry point: icon, short label and modest filled
treatment. In Loopflow, repository identity occupies that header position.
This supersedes the agent's proposed compact header-icon direction above.

Interpretation: retain a distinct creation row and refine its proportions and
emphasis against the Linear reference. Jack has not selected exact dimensions or
a final label; choosing Linear does not by itself approve “New issue,” “New Task,”
or the existing “Create Task.” The accepted exploratory capture behavior remains.
Reference board: /Users/jack/src/loopflow.product-wave/scratch/creation-board/index.html.
Official visual reference: https://linear.app/docs/custom-views.

## Naming accepted

Jack Heart accepted **New Session** and then explicitly requested keeping the
existing opening prompt. Updated the sidebar and Wave-menu labels, used the
compose icon, and reduced vertical padding from 10 to 6 points with a 5-point
corner radius. Those exact proportions are implementation choices for review.
The capture skill and launch prompt are unchanged.

Check: `scripts/test_desktop.sh --filter TaskCaptureTests` and `git diff --check` — passed; live appearance remains for review.

## Skill-picker prototype accepted

Jack Heart approved the final prototype appearance with “ok works” after
adjusting the skill label and chevron. The accepted reference is
`/Users/jack/src/loopflow.product-wave/scratch/session-picker/index.html`.

Keep the compact Linear-style split row: New Session launches, while the skill
name opens the searchable picker. The final prototype uses matching 13-point,
550-weight labels, a left-aligned skill name with 12-point horizontal padding,
a 7-point gap before a 12-point SVG chevron, and a -0.5-point vertical chevron
offset. Jack requested preserving the existing opening prompt.

This confirms prototype styling; the configurable picker is not implemented in
Desktop yet. The prototype uses illustrative skills, local browser persistence,
and simulated launches. Native skill discovery, selection persistence and real
launch behavior still need implementation and verification.
