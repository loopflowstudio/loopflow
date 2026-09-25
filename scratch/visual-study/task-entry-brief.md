# Task row → conversation experiment

Edit only `scratch/visual-study/mockups.html`. Preserve all existing repo scope,
bottom search, palette, zoom, per-Task draft/workspace and B/C interactions.

Human: “Im a little unsure about the Task -> Session thing. I think maybe we
have an in-line indicator or count for open sessions, and wehn you click on a
task with one open session you get the session”. This is an experiment, not
acceptance of every fallback detail.

In A's outline remove nested Session rows and Task disclosure controls. Add a
quiet inline conversation icon + count on each Task with open Sessions. Sample
Task.sessions already contains unresolved/open Sessions, including external ones;
do not count only running or locally attached clients. Zero needs no noisy badge.
Keep title truncation and accessible count text/tooltips.

Task navigation: one local open Session selects that exact Session and foregrounds
Session with context (zoom 1); none opens Task Overview; multiple opens Task
Overview with the existing explicit Conversations picker. Choosing a local Session
from that picker foregrounds it with context. External Session must remain an
explicit opening/move choice, never simulate moving it merely because Task was
clicked. Keep global initial A view as Overview and explicit zoom/back usable.
Remove nested Session rows only for A, including its All repositories mode.
Unbound Sessions remain reachable. Use existing selection/workspace state.

Make the count clickable to inspect the Task Overview/conversations without
immediately foregrounding its one Session, with accessible label describing that
action. This gives a visible way to inspect work without adding a new model.
Keep behavior scoped to actual user Task navigation, not every internal selectTask
call; starting a sample conversation and returning from Wave should stay coherent.

Parent owns design updates, browser proof, captures and presentation. Do not
capture screenshots or open a GUI browser. No commits, native changes, external
APIs or PM actions. Use uv run for any Python. Return changed selectors and checks.
