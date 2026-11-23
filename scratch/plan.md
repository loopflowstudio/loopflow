# LOO-1065 Mac New Task

**File to backlog** opens the committed, unstarted Task’s brief in its recorded backlog. **Start with <workflow>** opens the admitted Task’s conversation. A refusal confirmed during either action leaves the same sheet editable. If the answer is uncertain, the saved submission owns recovery on its original Machine and offers the draft back if replay confirms a refusal.

The workflow choice belongs to Start. **Use default** follows the selected scope until admission. A named choice survives scope changes, including when its name matches the default. File sends no Graph choice or run intent. A filed Task offers the current default at first Start. The Machine records the Graph when the Task takes it up.

**In** chooses an existing Wave or the repository. A renamed Wave remains the same choice. If it is removed, even between the click and the Machine receiving the request, keep the draft and ask for another destination. A replacement Wave using the same name is a new choice.

## Machine admission

Before this pass, File recreated a retired Wave by name and Start with a missing named Graph returned `outcome_unknown` with an empty receipt lookup. Desktop now sends the selected Wave ID for the workflow preview and in the saved creation request. The nullable `wave` field accepts an existing ID object, a legacy name string or the repository selection. Existing receipts take precedence over changes to Waves and Graphs.

A disposable Machine refused File after removal and after same-name replacement, filed into the renamed Wave, and replayed the original receipt after removal. Missing Graph and Wave refusals left no Task, Graph, claim, primary Session or receipt. The transaction test also retired a Wave after preparation and verified rollback. The commit-time lookup used the Wave primary-key index and took 6.7 ms for 10,000 in-memory reads with 500 Waves and 10,000 Tasks.

Checks on this slice:

- `task_create_honors_the_selected_wave_and_graph_before_admission` passed against the served write, including the workflow preview after replacement and rename and replay by CLI UUID after removal.
- `an_admitted_creation_commits_task_graph_claim_and_conversation_together`, the Rust wire fixture round-trip and the checked-in schema comparison passed.
- `scripts/test_desktop.sh --filter 'NewTaskTests/(removedWave|correctRefusedStart|startChoice|fileRefusalAndRetry)|ServerProtocolTests/taskCreationSelects'` passed all five tests, including stale planning and same-sheet correction.
- `documented_commands::` passed all four checks. `cargo clippy --all-targets -- -D warnings` passed.

## Edit a refusal found during recovery

A refused replay checks the receipt again on the original Machine. A receipt found at that point goes to the existing result or inspector. An unavailable lookup keeps the submission in recovery. Confirmed absence parks the words, destination, workflow choice and refusal in the journal. A name-only Wave from an older build requires a new destination when editing.

**Edit draft** opens the original repository’s New Task sheet. Cancel leaves the saved draft available. Discard removes it. File or Start replaces the same journal file atomically under a fresh request ID. The [concept review](concept-review.html) records the reason for this pass. The unlocked-display walkthrough below remains for QA.

- `cargo test -p loopflow --test integration task_create_tests::` passed all 11 Machine tests.
- `scripts/test_desktop.sh --filter 'NewTaskTests|CreationSubmissionTests'` passed 26 tests. Across a restart, the removed Wave blocked both actions despite stale planning, File kept its unsent workflow choice through discard, and a corrected Start opened the Task under a fresh request ID. A failed replacement save left the parked draft intact.
- `cargo test -p loopflow --test integration documented_commands::` passed all four documentation checks.

## Backlog handoff

The File walkthrough now holds the planning subscription back until the returned brief mounts and the saved submission retires. The next frame carries an edited brief for the same ID. It takes over the selected row, and removing it on a later frame leaves the row gone. The earlier build failed to find a brief with planning held back.

File uses the Task’s recorded scope. The selected backlog borrows its committed row until current planning includes it. While that brief is open, a repository-qualified Task lookup can settle a move or removal. The reply never adds a Task to the planning inventory.

`scripts/test_desktop.sh --skip-build --filter 'NewTaskTests/(fileRefusalAndRetry|planningBeforeFileReply|exactLookupRemovesFiledBrief|leaveFiledBrief|leaveBeforeFileReply)'` passed all five tests after building Desktop. The mounted backlog kept the brief through a partial lookup and dropped it after a complete lookup found no Task.

`cargo test -p loopflow --test integration documented_commands::` passed all four checks for the updated File instructions.

## Draft lifecycle

The same sheet shows **Filing…** or **Starting…** until the Machine answers. A confirmed refusal leaves the words, destination and workflow editable. A missing Wave shows **Wave unavailable** and requires a destination choice. An admitted Start opens the returned Task and primary Session. An uncertain answer closes the sheet and leaves the saved submission with recovery.

The requested Desktop filter (`scripts/test_desktop.sh --filter 'NewTaskTests|CreationSubmissionTests|BacklogViewTests|TaskGraphProofTests'`) passed with 44 tests reported, including two skipped display diagnostics. The fixture's typed refusal for a named Start used a fresh request ID after correction to **Use default**. A lost reply recovered the original Task with one send.

`scripts/test_desktop.sh --skip-build --filter 'WorkDestinationTests/(newTask|lateCreationReply)'` passed all four tests, including primary Session installation, post-admission opening failure and 12 late-reply cases. `cargo test -p loopflow --test integration documented_commands::` passed all four checks.

Pass 2 reran five focused New Task tests against the built Desktop. They passed, including the mocked typed Graph refusal and lost reply.

## Remaining display check

On an unlocked display, use the Workflow menu, correct a refused Start with **Use default**, pick the repository after **Wave unavailable**, and open the admitted conversation. [QA round 2](qa.html) drove File across a server restart live: the brief opened while the Machine was still away. It also fixed Wave links, which name the Wave while Desktop selected them by ID, so New Task opened on **Wave unavailable**.

## Prior checks

- That Desktop filter reported 37 tests in six suites, including two skipped display diagnostics. It covered named and inherited Start, refused File, revealing a hidden backlog and retirement after the brief mounts.
- `task_create_tests::task_create_answers_and_its_conversation_starts_the_graph_across_a_server_restart` passed with a lost Start reply against a disposable Machine. The documented command suite passed all four checks.
- [QA](qa.html) drove File in the repository and Wave, inherited Start, a removed Wave and server restart in an isolated Dev bundle. The named-workflow menu, live confirmed refusal after send and live lost reply were untested.

`scripts/test_desktop.sh --filter 'NewTaskTests|BacklogViewTests|CreationSubmissionTests|WorkDestinationTests/(newTask|lateCreationReply)'` built Desktop and passed 39 tests in five suites after compression. The unlocked-display demo remains for QA.

Compression checks. `scripts/test_desktop.sh --skip-build --filter 'NewTaskTests|CreationSubmissionTests|WorkDestinationTests/(newTask|lateCreationReply)'` passed 30 tests after rebuilding Desktop. Three Rust integration tests passed for Wave selection and provisioning, lost-reply replay, and CLI/served Session adoption.

`cargo clippy --all-targets -- -D warnings` passed after compression.
