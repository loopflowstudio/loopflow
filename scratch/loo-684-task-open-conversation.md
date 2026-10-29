# LOO-684 — Task "Open conversation" never does nothing

Status: accepted direction (Jack, 2026-10-29: "fix and ship"); implemented 2026-10-29. Remaining: gate, live check on LOO-675 after merge.

## Problem

Jack opened Tasks such as LOO-675 in Loopflow.app 0.15.0 and saw
**No Task conversation open**; the **Open conversation** button did nothing.
The CLI half works: `lf session connect --task LOO-675 --prepare --json --try`
returns interactive Session `015e85…` (detached) at once. LOO-675 also has
headless implement/compress Runs. Desktop writes no durable log (LOO-683), so
the cause below comes from code reading; both silent paths are real and both get fixed.

## Findings (code, `swift/LoopflowMac/WorkModel.swift`)

The button (`WorkspacePanesView.emptyWorkspace`, also the collapsed-panes offer)
calls `WorkModel.openTaskConversation`. It returns without effect in two ways:

1. **A pane names a Session the store doesn't hold.** `emptyWorkspace` renders
   when a `.session` pane has no `SessionItem` — either no id, *or an id missing
   from `SessionsStore.sessions`*. Saved pane layouts (`loadPaneIntent`) restore
   Session ids; the window's Session inventory excludes headless Runs and is a
   filtered working set. So a restored pane naming a Run or an aged-out Session
   shows the empty state, while `openTaskConversation` computes
   `retainedConversation = allPanes.contains { $0.sessionId != nil }` → true →
   only `openTaskDestination` (Task already selected) → nothing. Best match for
   "shows the empty state on open, button does nothing" with LOO-675's shape.
2. **A stuck `.opening` guard.** `if !retainedConversation, taskOpening?.status == .opening,
   taskLinkExpectedID == id, taskLinkConversation { return }`. `taskOpening`
   stays `.opening` after a successful placement until native mount is observed
   (`updateLinkedOpening`), and forever after the `openRepository` hand-off in
   `prepareLinkedTask` (also `openTaskDestination`'s hand-off). Any later click
   is silently swallowed.
   Generation-mismatch returns are *not* leaks: only `dismissTaskLink` bumps
   `destinationGeneration`, and it settles the old opening as failed first.
   Cancellation is settled by `openTaskLink`'s `defer`.
3. **Errors are hidden.** The empty state says "The conversation could not
   open. Retry the opening above." — nothing above shows the reason;
   `conversationError` is only set when the workspace was already retained
   before `openLinkedTask` started.

## Chosen approach

- **Decide by what the pane shows, not what it names.** `openTaskConversation`
  treats the Task as having a conversation only when some pane's Session id is
  held by that workspace's `SessionsStore` (the same test `SessionPaneView.item`
  uses). Otherwise it always runs `openTaskLink(…, conversation: true)`;
  `prepareTaskSession` returns the Task's conversation and `placeSession`
  reveals it into the pane, replacing the stale id.
- **Delete the silent dedupe guard.** Duplicate clicks during preparation can't
  happen through the button: `workspace.openingConversation` swaps it for the
  "Opening conversation…" spinner. A repeat sidebar click restarts the opening
  through `dismissTaskLink` (settles the old one, bumps the generation);
  `prepareTaskSession` is idempotent.
- **Settle the hand-offs.** When `prepareLinkedTask` / `openTaskDestination`
  hand the Task to another repository window, this window's `taskOpening`
  becomes `nil` (it no longer owns the opening; the receiving window opens it).
  Assumption logged in `scratch/questions.md`.
- **Show the reason in the empty state.** `openLinkedTask` resolves the
  workspace with `registry.workspace(for:…)` (create-or-get, as
  `installConversation` already does) so `conversationRequest` /
  `conversationError` always land on the Task's workspace. `emptyWorkspace`'s
  description becomes `workspace.conversationError ?? "Open this Task’s recorded
  conversation."`; the button stays as Retry.

### Alternatives considered

- *Add the missing Session to the store from the saved layout* (fetch the
  Run/aged-out record): would show a Run's terminal as "the conversation" —
  wrong owner. Rejected; Rust's `prepareTaskSession` owns which Session is the
  Task conversation.
- *Settle `.opening` with a timeout*: a guard on a guard. Deleting the guard
  removes the failure class.

## Demo

Headless: the new WorkDestination test builds LOO-675's shape — Task workspace
restored with a pane naming a headless Run absent from the store, plus a
detached interactive Session that `session connect --task … --prepare` returns —
renders `emptyWorkspace`, calls the button's action, and sees the pane hold the
detached Session. Live, after merge (Jack's): open LOO-675 in Desktop.

## Scope

- In: `WorkModel.openTaskConversation`, `openLinkedTask`, `prepareLinkedTask`,
  `openTaskDestination` hand-off; `WorkspacePanesView.emptyWorkspace` copy;
  tests in `swift/LoopflowTests/WorkDestinationTests.swift`.
- Out: Desktop durable logging (LOO-683); Dashboard tile offer
  (`attachTaskConversation`, already reports via `conversationError`); Rust/CLI;
  `DesktopOpening` wire shape (unchanged — no new status).

## Delete — do not maintain

Done: the `.opening` early return in `openTaskConversation` and the "Retry the
opening above." copy. `taskLinkExpectedID`/`taskLinkConversation` keep readers
(`openLinkedTask`, `prepareLinkedTask`, retry at `failTaskLink`). Nothing left.

## Forbidden outcomes

- A second "is opening" flag beside `taskOpening`/`conversationRequest`.
- Any path where the button's action returns without either placing a held
  Session, showing the spinner, or setting `conversationError`.

## Done when

- Clicking Open conversation on a Task whose pane names a Session not in the
  store opens the Task's conversation (prepared Session in the pane).
- A second click while `taskOpening` is still `.opening` (native not yet
  mounted) runs the opening again instead of returning.
- A failing `session connect --prepare` shows its error text in the
  `emptyWorkspace` description.
- The cross-repository hand-off leaves this window's `taskOpening` nil.
- Gate: `uv run python scripts/test.py --reuse-passing` (runs the `swift` suite: headless app build + view tests)
  passes,
  including existing `taskConversationActiveElsewhere`, `taskEntryOpensConversationThenRetainsWorkspace`
  and the `.opening`-after-prepare expectation at the "Native mount/input is separate" test.

Check: `scripts/test_desktop.sh --filter 'WorkDestinationTests|WorkspacePanesTests|WorkNavigationTests|TaskGraphTests|DesktopPerformanceFixtureTests'` passed (125); the two new WorkDestination tests fail on the old WorkModel. Gate owns the `swift` suite.
