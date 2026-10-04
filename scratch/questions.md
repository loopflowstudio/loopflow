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
