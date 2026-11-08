# Make the aspirational homepage true

The draft homepage on branch `homepage-differentiators` (http://localhost:5001)
claims things `lf` doesn't do yet.

Jack Heart, 2026-11-07:

> ok, lets update the homepage. you can make it 50% longer but no more

> this isnt going to make the live website, so feel free to make it aspirational, along with a task list to make it true

Length, measured with Playwright: the original is 3,978px tall at 1440px and
2160px wide; the draft is 5,949px (1.50×). At 390px it's 7,646px against
10,656px (1.39×).

Each Task below names the claim on the page and what makes it true.

## Not built

1. **Notifications when a Task needs you.** Page: "Loopflow for Mac and your
   phone tell you the moment a Task reaches a step that's yours." Build it so
   that reaching an `interactive:` step raises a macOS notification from
   Desktop, plus a push to the phone (see 2). The Desktop part can ship first.
2. **Loopflow for iPhone.** Page: "Answer a design question, try a demo or say
   ship from Loopflow for iPhone." It needs a list of Tasks waiting on you, the
   ability to reply in a Task's conversation, and an approve-ship action. It
   builds on the iOS app already in progress.
3. **`lf phone pair`.** Page: the phone card's command. It needs a pairing flow
   (QR code) between a Machine's `lf` and the phone app. Open question: the
   transport (the self-hosted Loopflow server, or a relay). The phone must not
   put `lf` in the byte path between a person and their provider.
4. **`lf import conductor`.** Page: "Every worktree becomes a Task, with its
   branch, uncommitted files and setup scripts." Implement it as
   `lf task create --worktree <ws>` for each Conductor workspace, plus a
   conversion of `conductor.json` setup and run scripts into something `lf`
   runs. First check the `~/conductor/workspaces` layout on a real install.
   Timing: Conductor Cloud shuts down 2026-11-20, and the Mac app keeps
   working until early 2027.

## Built, but the page shows more than `lf` prints

5. **`lf task status` with no ID.** The hero shot shows a board: each Task,
   its title, step bars, and "design · you" in wine when the step is yours.
   Make it print that board.
6. **The big `lf` card.** It shows `lf usage --wt`, `lf --machine mini monitor`,
   and `lf account` with usage bars. Check each line against Clap (the
   `documented_commands` check) and against real output. Either fix the card
   or make the commands print what it shows.
7. **The agent in the hero.** The shot shows an agent turning one request into
   three `lf task create … --run` calls and then reporting which Tasks want
   you. Check that the `loopflow` skill does exactly this from that prompt,
   and record a real run to replace the drawing.
8. **Loopflow for Mac mocks.** "Coming soon" is gone. The "Your agents" and
   "Your team" boards must match what Desktop shows, or be replaced by
   screenshots.

## Before this merges

9. Update `website/tests/e2e/test_homepage.py` for the new sections, then run
   `uv run python dev.py test`, including the fallback-font touch-target case.
10. GitHub repo page: description (currently "Arrange LLMs to code in
    harmony."), website URL (empty) and topics (none). This is public, so it
    waits for an explicit go-ahead.
11. A `/from-conductor` page, once 4 exists.
