# LOO-370 execution decisions — October 4

Jack Heart authorized autonomous implementation, verification and landing. The
previous design-review hold is superseded; no additional product approval is
required for reversible source design choices. Jack suggested `sessions/` for
the retired `runs/` layout; implementation must resolve the actual capture owner
without treating an artifact key as a Session ID.

Use the existing Task and checkout. Prove preservation and interrupted conversion
in isolation. Do not migrate the installed Home or interrupt live conversations.
The earlier offline-window proposal remains a deployment design choice to test,
not evidence that a configured maintenance window has been performed or approved.

Current design: [Finish the Run cutover](finish-removing-the-retired-run.md).

## Earlier runtime recovery evidence

The October 4 restart/admission failure is preserved at
`0faa2502a:scratch/questions.md`. This implementation used the supplied checkout;
no competing worker, manual Session completion or database repair was performed.
That earlier observation does not establish the current state of those Execs.

## Earlier verification capacity — October 4

The compression pass retried `uv run python scripts/resource_envelope.py --recover`:
13.3 GiB free against the 32 GiB emergency reserve, no inactive eligible build roots,
and uv cache pruning could not acquire its lock. No active build, installed Home,
live conversation or other checkout was deleted. Build/focused-test/Clippy/Desktop
verification defers to capable gate/CI; compression source edits can proceed.
This does not waive conversion acceptance or establish readiness to land.

October 5: capacity no longer blocks source verification. Rust compilation,
focused Session/authority/history tests and all-target Clippy now pass. The
conversion and populated released-Home matrix remain implementation work;
no installed data or unrelated writer was changed.
