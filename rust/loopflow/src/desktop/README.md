# Desktop shell execution

New Shell creates execution on the Task's recorded Machine and checkout.
Reconnect reuses that execution; hiding the tab, leaving the workspace and
quitting Desktop detach its view. Close ends the shell and its ordinary jobs.
Undo restores identity and history; it never restarts a command.

`desktop::shells` owns creation receipts, startup resources and Close policy.
The private `desktop shell` destination entry dispatches independently of macOS
UI control and works on Linux without the app. Swift supplies Desktop's bundled
shell integration. Existing Machine routing carries its inputs and the shared
viewer stream across SSH; an unreachable Machine never opens a local shell.
The destination shell runtime carries terminal bytes and Close status; it is
not a GUI action and has no Desktop action-result schema. Exact command inspection
reports a null result schema; public command listings omit the runtime.

Creation accepts a JSON bootstrap with a `script` and relative UTF-8 `files`.
The destination caches resources by payload digest under `shell-resources`.
After literal initial argv finishes, the script sees `LF_SHELL_RESOURCES`,
`SHELL` and terminal identity. Reattachment ignores all creation inputs.

Task cleanup owns completion, absence and Git cleanup policy. It signals verified
owners directly after checking viewing and requested work. Renewed Session viewing
resets checkout cleanup before the next pass. Explicit Close stays on the shell
receipt; its owner polls that request and waits for requested work to settle.
Shell tabs add no checkout hold, and Rex shells never enter Desktop's execution
receipts.

Tests use the production entry through `transport_tests::desktop_shell_` and
`transport_tests::ssh_shells::`, including no-replay, same-execution recovery,
Close and job settlement. `returning_task_viewer_resets_cleanup_without_another_sweep`
covers a returning passive viewer and a brief return/leave before the next sweep.
See [transport](../transport/README.md#task-shells-and-completed-workspaces)
for the viewer and retirement timing contract.
