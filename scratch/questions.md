# LOO-823 deferred choices

The demo-step direction in `process-leaks.md` supersedes the input-ledger and
idle-ending alternatives. Earlier questions, proposals and readings are in
`551becf586687262921e0681a169b5b7973fc75b:scratch/questions.md`.

Jack Heart, 2026-11-11: "nothing ends it but i could be open to allowing soem sort of "idle" state for task and wave/repo sessions on the server, if we ahve some cache limit, as long as only cost is slightly slower spin up time next time youj switch to them"

Resident-limit suspension is deferred; Desktop's `warmConversationLimit` bounds
views only and remains unchanged. "6a" selects an agent count per Machine
in config; its default is unnamed. Reading the input line off the server's
emulator is proposed, not chosen ("no opinion on 5"). No replacement input
ledger belongs in this PR. Bare-direct launch moved into this PR with "1b".

2026-11-11 coordination, comment `85810cc1-70e3-41da-a5ad-ee29c7867724`,
Session `9af9d3006a494e3dba19fdd82e01b7a0` for Jack Heart:
"From LOO-824's concept review: '823s rule is correct' (Jack Heart) also governs LOO-824's held terminals; LOO-824 drops its idle grace for them and will sync onto this PR once it lands. The deferred limit suspension that reads the input line off the server's emulator depends on LOO-824's emulator."

LOO-829 (`task_c118380f31f14d07ba28669499efa395`) removes the ignored
`session.end_detached_after` parser after LOO-823 ships; Linear sync was confirmed
2026-11-11. The released `processes.native_input` column remains until staged removal.
`desktop_shells.close_requested` remains the explicit Close authority after "1b 2c";
LOO-840 must not remove it. Removal Task `task_1ddbd3851a8e424e9ce887fa8f39353b` resolved
as LOO-840 in the 2026-11-11 realign status read.

2026-11-11 implementation assumption: a resumed owner that has never had a viewer
uses its existing Process launch time for the fifteen-minute grace; Task completion
is also a lower bound. No new timer record is needed. This matches returning to a
closed Task without making a finished checkout retirement exempt new execution.
For a completed Task lacking a completion date, its existing update time supplies
the lower bound, matching closed-Task ordering; malformed dates cannot authorize
closure.

2026-11-11 implementation choice after "1b 2c": shell owners poll only their
explicit Close receipt and requested work; no per-owner Task poll remains.
Abandoned checkout retries use the existing abandonment checks with `force=false`;
a past one-shot `--force` does not grant future forced deletion.

Proposed infrastructure memory correction (read-only in this Task): replace
`.lf/wave/infrastructure/MEMORY.md`'s "Idle owners end" two-minute rule with
scope-based lifetime. Typed input holds nothing; resident-limit suspension is
deferred. This proposal was carried from the earlier design, not a new Wave read.

2026-11-11 bare-direct implementation assumptions (after "1b", "2a"):
- A Desktop surface (`LF_SERVER_STATUS` or `LF_VIEWER_STREAM` set) still gets an
  owner for a bare Session: it waits for a relay's acknowledgement and would
  otherwise keep input paused. Desktop shells strip both, so `lf` typed in one is bare.
- `lf resume <bare session>` while it runs in another terminal still stops that
  provider (existing "provider client elsewhere" row); server-first says such a
  process "is not one `lf` stops". Unchanged here.
- Bare Codex still starts its observer beside the client; telemetry for bare
  launches is server-first's transcript piece.
- Graph targets (`lf code`, `lf graph …`) keep their owner; only skill targets,
  inline prompts and plain `lf` run direct.
