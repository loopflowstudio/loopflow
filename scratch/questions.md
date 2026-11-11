# LOO-823 deferred choices

The demo-step direction in `process-leaks.md` supersedes the input-ledger and
idle-ending alternatives. Earlier questions, proposals and readings are in
`551becf586687262921e0681a169b5b7973fc75b:scratch/questions.md`.

Jack Heart, 2026-11-11: "nothing ends it but i could be open to allowing soem sort of "idle" state for task and wave/repo sessions on the server, if we ahve some cache limit, as long as only cost is slightly slower spin up time next time youj switch to them"

Resident-limit suspension is deferred. "6a" selects an agent count per Machine
in config; its default is unnamed. Reading the input line off the server's
emulator is proposed, not chosen ("no opinion on 5"). No replacement input
ledger belongs in this PR. Bare-direct launch remains in server-first; existing
bare owners run until quit in this transition.

Open with that limit, not this PR's: whether the sixteen-conversation cap on
background Task starts (`BACKGROUND_TASK_CONVERSATIONS`) becomes the same number
as the per-Machine agent count or stays separate. Nobody has said. It matters
because the cap is now a steady ceiling: past sixteen running Task
conversations a new Task's conversation waits for `lf resume` or Desktop.

2026-11-11 coordination, comment `85810cc1-70e3-41da-a5ad-ee29c7867724`,
Session `9af9d3006a494e3dba19fdd82e01b7a0` for Jack Heart:
"From LOO-824's concept review: '823s rule is correct' (Jack Heart) also governs LOO-824's held terminals; LOO-824 drops its idle grace for them and will sync onto this PR once it lands. The deferred limit suspension that reads the input line off the server's emulator depends on LOO-824's emulator."

LOO-829 (`task_c118380f31f14d07ba28669499efa395`) removes the ignored
`session.end_detached_after` parser after LOO-823 ships; Linear sync was confirmed
2026-11-11. The `processes.native_input` column remains
until a staged contract migration can remove it; runtime no longer uses it.
