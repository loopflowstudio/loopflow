# LOO-550 — collaboration positioning (draft for Jack's review)

Jack Heart (2026-10-21): Loopflow is how you collaborate with other people using
coding agents. Draft is in `website/home.html`, README.md and docs/index.md;
preview with `cd website && uv run python main.py` → http://localhost:5001.

## What changed
- Hero (Jack's sketch, smoothed): "Loopflow is how you [design…study] software in harmony."
- Subline (Jack, 2026-10-21): "Conduct an orchestra that conducts itself, together with your team."
- Jack eliminated the /cli page; the homepage `lf` section links The CLI, Skills and Graphs, Accounts and usage.
- Jack asked to merge the collaboration copy into "Arrange your coding sessions…": two new
  paragraphs after his second, and the comment→steer terminal under the workflows card. Claims
  shipped today: one owner/worktree/PR per Task, shared plan (Linear or Git ref),
  Task comments become steers before the next step, workflow review nodes, Skills/Flows
  as repo files, only planning travels between machines.
- Jack asked for two hero pictures, replacing the workflow mock: "Your agents" (a dashboard of parallel sessions: agent, Flow step, needs-you state) and "Your team" (people, Waves as columns, Tasks with owners).
- Flow→Graph rename filed as LOO-551 (product Wave).
- README/index opening rewritten around collaboration; agent list now includes OpenCode and Pi.
- Jack's four paragraphs, the section heading and "Scale yourself." are untouched.

## Open for Jack
- Kim (no team yet) vs collaboration as the lead: "other people" is meant to include one friend.
- Growth memory says team connection is what Loopflow might sell later; leading free copy
  with collaboration moves toward that line.
- Closing line: "Scale together." (Jack, 2026-10-21)
- LOO-513 (unstarted) covers benefits-led positioning and page roles; overlaps this.
- Checks (gate): `cd website && uv run python dev.py test` → 72 passed; homepage suite rerun after the Linear-comment fix passed.

## Direction, later on 2026-10-21 (Jack)
- The core of `lf` is the split between long-lived Sessions (aligned with however you work with
  your team) and headless background Runs (plans → code, code → verified code, numbers → analysis).
  Sessions hand work to Runs faithfully (your process), visibly (recorded, watchable) and off the
  main thread (no chat context). Get as much as possible off the main thread.
- Tasks/Waves are "ho-hum": concrete and understandable, not the focus, not sold as special.
- Page order: hero, Arrange, `lf` (split leads it), Coming soon pictures, Works with, Scale yourself.
