# Returning to a chosen Session

Keep each Task's chosen Session in the Task-list navigation state. Back to Tasks and reopening the same Task should return to that conversation as an observer, retaining its remote draft. Choosing a conversation changes no primary Session. A different Task keeps its own choice. This state lives with the connected workspace; cold-app restoration remains separate.

Exercise the actual native path: choose Design review, type an unfinished draft, return to Tasks, reopen, verify the chosen name and draft without a keyboard grant. The test should fail against the current view-owned choice before moving it to the Task list.

November10: native regression failed on unchanged production at the missing chosen-Session label (30.905s). Source5e6d2d627 moves the binding to Task-list state keyed by Work. Final46 terminal/activity tests passed8.001s and native navigation34.963s: chose another Session, kept its draft while visiting another Task, returned as observer, then found the original Session draft. Simulator removed by runner. Sync changed no source; whitespace passed. Logs:reviewroot/session-choice. This proves fixture-backed native navigation, not physical-phone acceptance or cold-app restoration. Signed device build pending.
