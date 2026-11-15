# Homepage, next

`index.html` is a self-contained snapshot of a draft homepage for
loopflow.studio. Open it in a browser. It lives outside `website/`, the build
context for the deployed site, so redeploying never publishes it.

The draft leads with your agent driving `lf`. After that come a full-width `lf`
card, the problem, your process as code, twelve feature cards, your terminal,
your Mac and your phone, and your team. It's 1.5× the height of the current
homepage. Its editable source (`website/home.html`, `website/static/style.css`,
`website/content.yaml`) is on the `homepage-differentiators` branch, served
with `cd website && uv run python dev.py serve`.

Several claims are not true yet. To ship the draft, each needs to be true
first.

## Not built

1. **Notifications when a Task needs you.** Page: "Loopflow for Mac and your
   phone tell you the moment a Task reaches a step that's yours." Reaching an
   `interactive:` step should raise a macOS notification from Desktop, plus a
   push to the phone (see 2).
2. **Loopflow for iPhone.** Page: "Answer a design question, try a demo or say
   ship from Loopflow for iPhone." It needs a list of Tasks waiting on you,
   replies in a Task's conversation, and an approve-ship action.
3. **`lf phone pair`.** It needs a QR pairing flow between a Machine's `lf` and
   the phone app. The transport is still open: the self-hosted Loopflow
   server, or a relay. Either way, `lf` stays out of the byte path between a
   person and their provider.
4. **`lf import conductor`.** Page: "Every worktree becomes a Task, with its
   branch, uncommitted files and setup scripts." Implement it as
   `lf task create --worktree <ws>` for each workspace in
   `~/conductor/workspaces`, plus a conversion of `conductor.json` scripts.
   Check the directory layout on a real install first.

## Built, but the page shows more than `lf` prints

5. **`lf task status` with no ID.** The hero shows a board: each Task, its title,
   step bars, and "design · you" when the step is yours.
6. **The big `lf` card.** It shows `lf usage --wt`, `lf --machine mini monitor`,
   and `lf account` with usage bars. Check each against Clap and real output.
7. **The agent in the hero.** One request becomes three `lf task create … --run`
   calls, then a report of which Tasks want you. Check that the `loopflow`
   skill does this, and record a real run to replace the drawing.
8. **Loopflow for Mac mocks.** They're shown without "Coming soon," so they
   must match Desktop or be replaced by screenshots.

## Before it replaces the live homepage

9. Update `website/tests/e2e/test_homepage.py`, then run
   `uv run python dev.py test` (including the fallback-font touch-target case).
10. GitHub repo page: description, website URL and topics.
11. A `/from-conductor` page, once 4 exists.
