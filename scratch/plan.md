# LOO-1065 Mac New Task

**File to backlog** opens the committed, unstarted Task’s brief in its recorded backlog. **Start with <workflow>** opens the admitted Task’s conversation. A refusal confirmed during either action leaves the same sheet editable. If the answer is uncertain, the saved submission owns recovery on its original Machine and offers the draft back if replay confirms a refusal.

The workflow choice belongs to Start. **Use default** follows the selected scope until admission. A named choice survives scope changes, including when its name matches the default. File sends no Graph choice or run intent. A filed Task offers the current default at first Start. The Machine records the Graph when the Task takes it up.

**In** chooses an existing Wave or the repository. A renamed Wave remains the same choice. If it is removed, even between the click and the Machine receiving the request, keep the draft and ask for another destination. A replacement Wave using the same name is a new choice.

## First slice · Make the Machine honor the draft

Use the actual served write in `rust/loopflow/tests/task_create_tests.rs`. Select a Wave ID, keep the Desktop reading stale, then retire the Wave and send File. The Machine must refuse, with no Task, receipt or new Wave. Repeat with the same name reused by another Wave. A rename must still allow File into the original Wave. Send Start with a removed named Graph and require a typed refusal, no receipt and no Task, claim or primary Session. Both current failures were reproduced with Desktop-shaped requests on isolated Machines. The retired Wave's name successfully filed into a newly provisioned Wave. The missing Graph returned `outcome_unknown` even though `creation.get` returned null.

- Carry the chosen Wave's ID from `NewTaskSheet` through `WorkModel.submitTask` and `RegistryQuery.createTask` into the saved request and Rust creation. Use the existing nullable `wave` field as a selector. `null` selects the repository, `{"id":"…"}` selects an existing Wave, and a string keeps the legacy name behavior for `lf task create --wave` and earlier payloads. The object lets an older server refuse the request shape safely. Keep legacy serialization and fingerprints stable and cover both wire shapes in the Rust and Swift fixtures. Read the inherited workflow preview for the same Wave ID.
- Resolve the ID in the requested repository and use the Wave's current address and selected Project at admission. The ID path goes straight to existing-wave resolution. Slug provisioning stays in the name path. Coordinate the live selection and creation write with Wave removal and recheck live Wave and Project membership at commit, using the existing creation transaction. A renamed Wave uses its new address. A removed Wave or a different Wave at that address cannot receive the request. Admission still commits Task, Graph, claim and primary Session together and answers before checkout.
- Return a typed, pre-commit refusal for a missing selected Wave or unavailable Graph definition. Classify known input failures where the Machine can establish they preceded any creation effect. A write or answer whose effect is uncertain stays in receipt recovery. Desktop keeps the existing same-sheet correction, and a typed Wave refusal makes **Wave unavailable** actionable even while planning still shows the old Wave. The text and Start choice stay editable. Test the sheet against the served error contract, including choosing the repository after the Wave refusal and **Use default** after the Graph refusal.

## Finish recovery for a refused submission

Keep the exact command, request ID and original Machine for in-flight and admitted requests. `creation.get` decides whether to recover an existing Task, continue the original command or inspect an uncertain launch. A completed receipt wins over any later Wave or workflow change. If the receipt's Task has since been deleted, recovery reports that outcome with its original identity. A saved Wave name from an earlier build cannot supply an ID retroactively. Keep its existing receipt and protocol-version rules and leave the saved payload intact.

If an ambiguous send hands off to recovery and the exact replay is definitively refused before commit, confirm receipt absence on the original Machine. Retain the saved words, destination and Start choice as a draft awaiting correction, with the Machine's reason. Park that record from automatic sends so a later workflow edit or restart cannot submit it while someone is correcting it. Offer **Edit draft** in the recovery notice for this case. Reopen the same New Task form, require a destination choice for a removed Wave, and use a fresh request ID on deliberate resubmission. Retain the saved draft until its replacement is saved or it is discarded. Test the lost-then-refused path, including a Desktop restart, alongside the existing lost-after-commit test. A missing or unavailable lookup alone still leaves the original submission in recovery.

Update `docs/surfaces.md` with the destination and recovered-draft behavior. Run the focused served creation tests and the New Task and creation journal suites. The [concept review](concept-review.html) records the reason for this pass.

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
