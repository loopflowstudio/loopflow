# LOO-673 · Desktop polish bucket

Status: draft plan, 2026-10-28. Standing Task; Jack Heart closes it.

## Problem

Jack Heart is finding small rough edges as he uses Desktop: spacing, colors, copy,
focus, shortcuts, small visual bugs. Filing a Task for each one costs more than the
fix. He asked for one bucket: drop a comment, and the fix shows up.

## Demo

Jack posts a comment on LOO-673 ("Sidebar Task row title clips at 240pt in dark
mode", plus a screenshot). The next batch PR fixes it, and a view test renders the row
in light and dark themes. The comment gets a reply linking the PR. Relaunching Desktop
from that build shows the row fixed.

## Intake state (2026-10-28)

The only comments so far are the claim and Jack's "start the polish bucket" steer.
**No feedback items have arrived yet.** No fix batch exists until one does. This
kickoff sets up how batches run; it does not invent polish items.

## Batch loop

1. **Read intake.** `lf task comment LOO-673 --json`. A feedback item is any comment
   from Jack Heart that names a Desktop rough edge. Skip claims, steers that only start
   the bucket, and agent progress comments. An item counts as handled once a reply
   cites it (PR, Task link or reason).
2. **Triage each item** into one of three dispositions:
   - *fix:* local to one view or one copy string, no new model/DTO/CLI field, and no
     product choice between alternatives. Goes in the batch.
   - *split:* needs a design choice, crosses into shared Rust/DTO/planning authority,
     or is more than a couple of hours of work. `lf task create --title "<outcome>"`
     (Wave product), linked back. Not absorbed here.
   - *decline:* already fixed on main, intended behavior per `swift/README.md` or
     `swift/DESIGN.md`, or not reproducible. Reply with the reason.
3. **Fix the batch.** Find the leaf view (`swift/LoopflowMac/`, shared views in
   `swift/Loopflow/`). Change only what the item names. Follow `swift/DESIGN.md`
   tokens. Don't add one-off colors or spacing constants. No eyebrow labels.
4. **Prove it.** Each visual fix gets a headless ViewInspector test on the production
   leaf view, rendered under `.environment(\.colorScheme, .light)` and `.dark`, asserting
   the fixed property (text, modifier, token, focus or shortcut binding). Use the
   pattern in `swift/LoopflowTests/AppAppearanceTests.swift`. Leaf views stay internal
   for `@testable` access. No test-only initializers, no mounted windows. Copy and
   shortcut fixes assert on text and command binding. Put batch tests in one
   `DesktopPolishTests.swift` suite unless an existing suite already owns that view.
5. **Publish** each batch as its own PR. The Task has one current PR at a time, so a
   later batch starts after the earlier one lands, continuing in this Task.
6. **Reply** on every item: `lf task comment LOO-673 "<item>: fixed in #NNN — <one line>"`,
   `split → LOO-NNN`, or `declined — <reason>`. If the item is a comment thread,
   reply in it.

## Risk findings

- **Comments are the only intake.** Screenshots arrive as attachments or links in
  the comment body. If one can't be read headless, triage from the text. If the text
  alone is ambiguous, ask in the reply and leave the item open. Don't guess.
- **Mixed suites.** About 29 suites already use ViewInspector, so the pattern is
  supported. Parents that need `@Environment` Observation objects can't be inspected.
  Test the leaf instead (AGENTS.md).
- **Terminal and Ghostty visuals** (cursor, font rendering inside panes) need a
  display. Fix them with a `.requiresDisplay` check deferred to CI, or split them out
  if the fix lives in the Ghostty patch set.
- **Scope creep** is the main way this fails: a "small" item turns into a
  navigation redesign. The split rule in step 2 is the guard.

## Scope

In: Swift view-layer polish in `swift/Loopflow*`, copy strings, keyboard shortcuts,
focus behavior, light and dark appearance.
Out: Rust/CLI behavior, DTO or wire changes, planning semantics, and new features.
Those become split Tasks.

## Acceptance checks (gate, per batch)

- `cd swift && swift build && swift test --filter DesktopPolishTests` (plus any
  existing suite a fix touched): passes, with each fix checked in light and dark.
- `uv run python scripts/loopflow-dev.py install` builds the app (app build check).
- Every intake item in the batch window has a reply (fixed/split/declined), checked
  with `lf task comment LOO-673 --json`.

## Done when

Per batch: every feedback item up to that point has a reply, each fix ships in a PR
with a light/dark view test, and split items have linked Tasks. The Task itself stays
open until Jack Heart closes it.

## Check results

- kickoff: no code change; intake read; 0 feedback items pending.

## Batch 1 intake (2026-10-28, from Jack Heart in the Task conversation)

1. Wave rows count open Tasks, not total. *fix* — WorkNavigator waveRow counts non-terminal.
2. Sidebar drag reorder: green "+" badge instead of an insertion bar; slow and clunky
   (each drop waits on `lf desktop move` + plan refresh). *fix* — move operation, insertion
   line, optimistic local order.
3. Two sidebar sections: current Working Tasks, then Autonomous below (the board's
   Autonomous column = shipping, "no human blocker left"), greyed with a header. *fix* —
   Swift-only from roadmap, same rule as WaveStandup; Rust selection still excludes shipping.
4. "Layout not saved: … CancellationError" banner appears often. *fix* — persistence cancel path.
5. Wave view Session tile hover controls (expand/hide) crowd the rounded corner. *fix* —
   consistent inset. Screenshot: ~/Desktop/Screenshot 2026-10-28 at 1.33.00 PM.png
6. ⌘` should cycle repository windows. *fix* — GhosttyTerminalView.performKeyEquivalent
   returns ⌘`/⌘⇧` to AppKit instead of sending them to the PTY.
7. Codex sessions copy with Ctrl-C, not ⌘C (also in cmux; maybe only cmux-started
   sessions, with hook errors on switch). *split → LOO-677*.

Jack Heart, 2026-10-28: do not launch another dev app; prove later fixes headless.
