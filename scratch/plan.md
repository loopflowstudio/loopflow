# LOO-1065 UI polish

Mac New Task now offers **File to backlog** and **Start with <workflow>**. The Start picker follows the selected scope until a named workflow is chosen. **Use default** restores inheritance. File selects the unstarted Task in its destination backlog.

The Machine's creation receipt distinguishes a confirmed refusal from a lost answer. A refused draft stays open for correction. A submission with an uncertain outcome stays with recovery under its original identity.

## Checks

- `scripts/test_desktop.sh --filter 'NewTaskTests|CreationSubmissionTests|BacklogViewTests|TaskGraphProofTests'` passed. Swift Testing reported 37 tests in six suites, including two skipped display diagnostics. The headless File walkthrough revealed a hidden backlog, mounted its selected brief and observed submission retirement. Start exercised named and inherited workflows.
- `task_create_tests::task_create_answers_and_its_conversation_starts_the_graph_across_a_server_restart` passed against a disposable Machine with a lost reply.
- `cargo test -p loopflow --test integration documented_commands::` passed all four checks.

## Remaining

Demo the sheet in Loopflow Dev and follow the running Graph in its mounted workspace. The display checks belong to demo or QA.
