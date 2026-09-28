# Restore session opening

Jack Heart reported that Sessions would not open from either the CLI or desktop.

## Finish line

The installed `lf session open` command for the reported review reaches Claude
with its review context and can reopen saved native history. The desktop uses
the same `open_argv`; terminal verification alone does not prove desktop rendering.
Listing a Session or preparing a Run does not count as restoration.

## Observations — 2026-09-28

- Installed CLI: `0.12.23+280670217`; data directory:
  `/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea`.
- Reproduced the clipboard command in a PTY. Claude exits 1 with
  `No conversation found with session ID: 9927fb1a-a16a-464b-8800-2c975f5ac3ac`.
- LOO-330, LOO-331 and LOO-329 review Runs each contain a preallocated Claude
  ID, an initial input, and a completed terminal receipt. None has a matching
  native conversation file in its selected account. Their preceding kickoff
  conversation files exist and finish just before review Run creation.
- Launch code writes the allocated Claude ID before spawning; on exit it
  records that ID as observed. Resume trusts that receipt. An initial client
  exit without persisted history therefore strands this review.
- No readiness or completed review feedback exists for these three Sessions.

## Hypotheses

- First launch exited before its initial prompt produced saved history. The
  exact reason is still unknown; there is no captured native terminal output.

## Recovery and live proof

All three review Sessions now open and resume using their original installed
`lf session open '<boundary-id>'` command. Each first launch received its
assembled review context, wrote native user/assistant history, was interrupted
with Escape and exited with `/exit`. A second invocation displayed that saved
history and exited 0. No review feedback was supplied or marked ready.

| Task | Original Run | Restored Run |
| --- | --- | --- |
| LOO-330 | `run_9927fb1aa16a464b88002c975f5ac3ac` | `run_9186b660fa304b208b1dfb946dd870f9` |
| LOO-331 | `run_10afb60a023943dfa0fa45ee15fe6afa` | `run_e44c6e1f2acb4dc3986f0b6c6ffd5a2c` |
| LOO-329 | `run_2e5cd1be2dfa4b9190c999d294cba971` | `run_7b98c4ea500f41ba8b6103111b93a7c3` |

The exact original `task_flow_positions` rows are preserved privately under
`/Users/jack/.lf-dev/installed/local-afee63d734c7482cb94d1071af26d9ea/recovery/session-opening-20260928/`.
Recovery used a SQLite transaction comparing each exact old Run and position
version, clearing only `session_run_id` and incrementing `position_version`.
Installed `lf session open` then prepared and launched the replacement Run.
Final row comparison confirms those are the only two changed columns: invocation,
cursor, review, readiness, failure and claim remain unchanged.

Original Run manifests, inputs, provider receipts and terminal receipts remain
in place. After rebinding, the obsolete Runs appeared as independent Sessions;
`lf session complete <old-run-id>` retired only those orphan surfaces through
the supported command. This did not complete any Task review or advance a Flow.

[Proof receipt](session-opening-proof.json) records native history locations,
exact open commands, message counts and original artifact hashes. A copy lives
beside the private row backups. No binaries, authentication or provider settings
were changed. Desktop rendering was not exercised in this headless environment;
the verified commands are the same `open_argv` emitted for the desktop.

## Review findings and remaining work

- Clearing an unresumable binding without immediately preparing its replacement
  briefly makes the installed Session list fail on an unprepared review. This
  occurred for LOO-329 during recovery and was immediately repaired with
  `lf session open '<boundary-id>' --json`, followed by real launch/resume proof.
  All three are prepared and list normally now. Keep any future recovery atomic
  across replacement preparation and binding publication.
- Native clients received their real review prompts during verification. They
  performed initial reads; the proof interrupted them instead of supplying
  invented review feedback. No proof client remains active.
- The original reason for exiting before native history existed remains unknown.
  Preventing premature publication and recovering this condition automatically
  belong to the following 5 Whys step. No production code changed in restore.
