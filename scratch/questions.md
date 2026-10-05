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

## Verification and integration

October 5 builds supersede the October 4 disk-capacity limitation. Its diagnostics,
the sync onto `8ea0bec9c`, and the Session CLI run (9/10 followed by the repaired
case passing 1/1) remain at
`71a784cdf5135fad8b26a21ddd6ea1ab354c4a93:scratch/questions.md`.
No installed data, unrelated writer or other checkout was changed. Conversion,
populated released-Home preservation and the affected gate remain outstanding.

## October 5 unresolved decision

The [offline recovery decision](finish-removing-the-retired-run.md#offline-recovery-decision--october-5-steer)
owns the external-custody contract, feasibility evidence and opaque-path comparison.
No concrete Mac storage/boot owner is proved. Retaining one opaque `runs/` root
would change acceptance and is not authorized. Conversion remains unimplemented;
no live interruption or permission-helper refinement is selected. Full prior
notes remain at `b6cfe0521:scratch/questions.md`.
