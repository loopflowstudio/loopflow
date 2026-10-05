# LOO-370 decisions — October 5

The updated Task brief accepts Infrastructure's opaque `runs/` root choice under
Jack Heart's delegated autonomous direction. The former conversion/boot-custody
blocker is superseded by an explicit scope change. No further product decision
is pending. Keep one physical root and prove semantic/runtime preservation.

The prior investigation and contrary evidence remain at
`6fcdbe9da0b47ef95f1f92ebdb259401a327cd46:scratch/finish-removing-the-retired-run.md`.
No installed-Home mutation, live interruption, installation or release is authorized.
The current plan is [Finish the Run cutover](finish-removing-the-retired-run.md).

## Local installation-test exclusion

Jack Heart's October 5 steer `bc7a98f5-2796-4786-9372-5c334cb9a046` forbids
these host tests because getpwuid bypasses HOME/LF_HOME isolation:
`global_commands::installation_uses_candidate_authority_from_any_checkout`,
`global_commands::installation_reaches_candidate_verdict_with_an_unreadable_task_registry`,
and `exec_ownership_tests::early_observation_records_preflight_and_screenshot_child_ancestry`.
PR #1444 moves their unchanged assertions to `scripts/test_task_installation.py`
under disposable OS accounts. Gate must reuse that repair after integration or
leave these checks to isolated CI; do not delete or weaken them.
