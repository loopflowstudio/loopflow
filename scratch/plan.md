# LOO-1065 Mac New Task

**File to backlog** opens the committed, unstarted Task’s brief in its recorded backlog. **Start with <workflow>** opens the admitted Task’s conversation. Either action leaves the same draft editable on a confirmed refusal. If the outcome is uncertain, the saved submission owns recovery on its original Machine.

The workflow choice belongs to Start. **Use default** follows the selected scope until admission. A named choice survives scope changes, including when its name matches the default. File sends no Graph choice or run intent. A filed Task offers the current default at first Start. The Machine records the Graph when the Task takes it up.

## First slice

1. Hold back the planning subscription in the New Task File walkthrough. QA saw “Filed 8747cc” and “Nothing selected” together for about 30 seconds after a server restart. The fixture currently includes the new row before File, which misses that gap.
2. Present the existing `TaskCreated.row` in the destination backlog as temporary evidence for the selected brief. Use the Task’s recorded scope and the current navigation guards. Reveal the backlog and clear interfering filters as today. Keep the row in this selection, scoped to the original Machine and repository. Leave authoritative inventory alone. Current planning for that ID takes precedence and a current Task read can establish a move or removal. Release the temporary row on either event or when the selection leaves it. Absence from stale or partial planning alone is inconclusive. Render the row once and let its mounted brief retire the saved submission through `CreationPresentation`.
3. Verify the brief opens before planning catches up, the arriving row replaces it without a duplicate or lost selection, and a Task that moved or started is never revived by a delayed reply. Switching destinations before the reply leaves the committed result available to recovery.

## Finish the draft lifecycle

- Keep the same sheet through File and Start, showing which action awaits the Machine. Replace the separate read-only Start failure screen. On a refusal before send, or a definitive refusal with confirmed receipt absence, keep the text, scope and named choice editable. The next deliberate submission can use a fresh identity. If the chosen Wave is known to have disappeared, show it as unavailable and let the person choose another Wave or the repository. A missing Wave lookup cannot represent a repository choice.
- On Start admission, close the sheet and open the returned Task and primary Session. The conversation owns checkout and Graph launch. A failure after admission belongs to the existing Task’s workspace. Keep the existing write and receipt contracts and the exact stored payload for in-flight or recovered submissions. A missing or undecodable answer hands off to recovery. It gives no permission to create under a new ID.
- Extend the New Task checks for a removed Wave and a refused named Start corrected with **Use default**. Keep the lost-reply check on the original receipt. Update the New Task usage in `docs/surfaces.md` and run `scripts/test_desktop.sh --filter 'NewTaskTests|CreationSubmissionTests|BacklogViewTests|TaskGraphProofTests'`. Demo the Workflow menu and handoff on an unlocked display.

## Prior checks

- That Desktop filter reported 37 tests in six suites, including two skipped display diagnostics. It covered named and inherited Start, refused File, revealing a hidden backlog and retirement after the brief mounts.
- `task_create_tests::task_create_answers_and_its_conversation_starts_the_graph_across_a_server_restart` passed with a lost Start reply against a disposable Machine. The documented command suite passed all four checks.
- [QA](qa.html) drove File in the repository and Wave, inherited Start, a removed Wave and server restart in an isolated Dev bundle. The named-workflow menu, live confirmed refusal after send and live lost reply were untested.
