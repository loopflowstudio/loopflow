# LOO-717 / LOO-498 coordination

Jack Heart approved LOO-717 implementation and review publication on November 3,
2026, stopping at interactive demo. No shipping, merge or installation approval.
LOO-498’s existing feature Graph was observed running compress on November 3;
no competing run was launched or existing workflow changed.

The accepted contract lives in `questions.md` and `loo-717-shell-lifetimes.md`.
Rex owns its native shells/layouts and adapter; Desktop owns its own. Shared lf
retention treats both hosts equally. No ordinary-shell relay or replicated Rex
layout; LF agent-conversation identity and attachment remain shared.

## Adapter contract shared with LOO-498

Comment `30bf4970-3bb1-481f-8200-83a6734cb26e` refines
`67cd130c-9104-48ef-b019-615e3e41e01e`:

- `task presence TASK --occurrence ID --state STATE --revision N` reports a
  native occurrence as retained, away, closed or unknown, with required
  `--native-settled true|false`. Closed selects explicit-Close policy; it does
  not prove native cleanup. Persist increasing
  revisions; delayed release must not overwrite a return.
- Renew retained leases within 30 seconds. Expiry is unknown, never confirmed
  closure. Another retaining occurrence keeps shared resources alive.
- The destination returns a completion-pinned retirement intent. At `closing`,
  the released adapter settles only that Task's native resources and reports
  native-settled=true with a newer revision, preserving away/closed policy. Away
  native shells are not settled. A Rex Wave session can contain other Tasks;
  do not destroy it wholesale. New retained/unknown observations clear settlement.
- Desktop detaches surfaces on departure, so away+settled can preserve the
  existing fifteen-minute deadline and finish cleanup without relaunch. Quit
  awaits reports within five seconds. Destination Sessions/shell jobs still
  settle through their owners. TaskRetirement response fields are unchanged.
- Host policy owns release. Eligible idle agents end after two minutes;
  completed-Task shells/checkout retire at fifteen total. Explicit last Close
  skips grace, while active requested work, drafts and pending requests still
  protect execution. Eligible cleanup discards uncommitted files, preserving
  existing committed-ref safeguards and Task/conversation history.

Rex native implementation and end-to-end proof remain LOO-498's responsibility.
Its probe found detach/reinsert continuity, but server restart preserved block
IDs while replacing programs with login shells. Invalid cwd fell back to home:
validate recorded checkout before native creation. Supported move APIs did not
establish migration between Rex sessions. These are evidence limits, not new scope.

Earlier coordination: outgoing `aa155162-7bd6-4d4c-b3c0-374f6d89768d`, incoming
`2f7f49fd-5c75-45e0-a688-cb70a3917bcd`. Superseded proposals and discussion are
preserved at `17cf6a71f9:scratch/rex-coordination.md`; accepted decisions above
replace their open timing, retention and post-cleanup restoration questions.

## Confirmed host exit — accepted November 4

Jack Heart selected 1a in the interactive review: positive evidence that the exact
retaining host instance exited releases retention and starts ordinary absence
cleanup without requiring relaunch. Lost contact or lease expiry alone stays
unknown. Rex client exit does not establish retaining-server or native-job
settlement. Active requested work, other retaining hosts and newer-instance
retention remain protected. No implementation or new API shape is claimed.
Shared with LOO-498 in comment `512d1b03-c6de-4fda-878b-d8f5d723f3b5`.
