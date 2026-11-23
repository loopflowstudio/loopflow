# LOO-1065 UI polish

Give **File** and **Start** their own clear consequences in New Task on Mac. **File to backlog** saves the request in the selected Wave or repository. **Start** uses the workflow shown next to it. A filed Task follows its current default until somebody starts it, and **Start with…** can pick a different workflow then.

The current sheet submits the selected graph even for File. Rust's creation engine validates that graph and then files without it. A valid choice silently disappears and an invalid choice can prevent filing. The CLI already makes `--graph` a choice for `task create --run`.

## First slice

1. Give the workflow picker a **Workflow for Start** label and show the chosen name on the Start button. Keep the outline and the participation sentence Rust supplies. Add **Use default** to the picker so it can express following the scope or starting a named workflow, including when both resolve to the same name. A change of scope updates the inherited choice and keeps an explicit choice in the draft. Beside **File to backlog**, say “Choose a workflow when you start.” Keep both actions available and preserve the Start keyboard shortcut.
2. Remove the graph parameter from `WorkModel.fileTask`. Submit `task.create` with `run: false` and `graph: null` for File. Start continues to send `run: true` with the explicit name or `null` to follow the current default. Use the existing `WaveGraph`, `WorkflowOutline`, creation receipt, and Task Graph. Filed Tasks gain no workflow preference or Graph row. The Machine resolves a default when starting and captures the Graph at admission or first run. Existing submitted creations retain their original payload and destination for receipt recovery.
3. Demo in a scope whose default is `feature`. Pick a different workflow and file, then open the selected unstarted Task and see `feature`. Open New Task again, pick the alternate workflow, and start. Follow the captured Graph in the Task workspace. Use a fixture with different participation stops so the difference is visible.

## Acceptance

- Extend the headless New Task view and request checks. An explicit choice is visible next to Start, survives a scope change, and returns to inheritance through **Use default** even if the names match. Picking a workflow that disappears before submission still lets File save the brief and select its unstarted row. Assert that File sends no graph override and that Start sends the named choice.
- Cover a refused File and subsequent success. Keep the request, scope, and Start choice in the sheet on refusal. On a lost answer, recover against the original request ID, payload, and Machine through `CreationSubmissions`. Existing tests cover the receipt contract. A setup failure after admission belongs to the existing Task's workspace and recovery.
- Update the New Task usage text in `docs/surfaces.md`. Run `scripts/test_desktop.sh --filter 'NewTaskTests|CreationSubmissionTests|TaskGraphProofTests'` after implementation and record the result.
