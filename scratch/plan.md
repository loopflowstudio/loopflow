# LOO-1065 Mac New Task

**File to backlog** opens the committed, unstarted Task’s brief in its recorded backlog. **Start with <workflow>** opens the admitted Task’s conversation. Either action leaves the same draft editable on a confirmed refusal. If the outcome is uncertain, the saved submission owns recovery on its original Machine.

The workflow choice belongs to Start. **Use default** follows the selected scope until admission. A named choice survives scope changes, including when its name matches the default. File sends no Graph choice or run intent. A filed Task offers the current default at first Start. The Machine records the Graph when the Task takes it up.

## Backlog handoff

The File walkthrough now holds the planning subscription back until the returned brief mounts and the saved submission retires. The next frame carries an edited brief for the same ID. It takes over the selected row, and removing it on a later frame leaves the row gone. The earlier build failed to find a brief with planning held back.

File uses the Task’s recorded scope. The selected backlog borrows its committed row until current planning includes it. While that brief is open, a repository-qualified Task lookup can settle a move or removal. The reply never adds a Task to the planning inventory.

`scripts/test_desktop.sh --skip-build --filter 'NewTaskTests/(fileRefusalAndRetry|planningBeforeFileReply|exactLookupRemovesFiledBrief|leaveFiledBrief|leaveBeforeFileReply)'` passed all five tests after building Desktop. The mounted backlog kept the brief through a partial lookup and dropped it after a complete lookup found no Task.

`cargo test -p loopflow --test integration documented_commands::` passed all four checks for the updated File instructions.

## Draft lifecycle

The same sheet shows **Filing…** or **Starting…** until the Machine answers. A confirmed refusal leaves the words, destination and workflow editable. A missing Wave shows **Wave unavailable** and requires a destination choice. An admitted Start opens the returned Task and primary Session. An uncertain answer closes the sheet and leaves the saved submission with recovery.

The requested Desktop filter (`scripts/test_desktop.sh --filter 'NewTaskTests|CreationSubmissionTests|BacklogViewTests|TaskGraphProofTests'`) passed with 44 tests reported, including two skipped display diagnostics. A refused named Start used a fresh request ID after correction to **Use default**. A lost reply recovered the original Task with one send.

`scripts/test_desktop.sh --skip-build --filter 'WorkDestinationTests/(newTask|lateCreationReply)'` passed all four tests, including primary Session installation, post-admission opening failure and 12 late-reply cases. `cargo test -p loopflow --test integration documented_commands::` passed all four checks.

## Remaining demo

On an unlocked display, use the Workflow menu, correct a refused Start with **Use default**, and open the admitted conversation. File across a private server restart and check that the brief opens when the Machine answers. Earlier QA saw “Filed” above “Nothing selected” for about 30 seconds. The held-back-planning tests cover the fix headless.

## Prior checks

- That Desktop filter reported 37 tests in six suites, including two skipped display diagnostics. It covered named and inherited Start, refused File, revealing a hidden backlog and retirement after the brief mounts.
- `task_create_tests::task_create_answers_and_its_conversation_starts_the_graph_across_a_server_restart` passed with a lost Start reply against a disposable Machine. The documented command suite passed all four checks.
- [QA](qa.html) drove File in the repository and Wave, inherited Start, a removed Wave and server restart in an isolated Dev bundle. The named-workflow menu, live confirmed refusal after send and live lost reply were untested.

`scripts/test_desktop.sh --filter 'NewTaskTests|BacklogViewTests|CreationSubmissionTests|WorkDestinationTests/(newTask|lateCreationReply)'` built Desktop and passed 39 tests in five suites after compression. The unlocked-display demo remains for QA.
