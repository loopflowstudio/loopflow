# Live Advance observation — 2026-09-25

Jack explicitly said “Advance” in the control conversation. The development
Session list showed the ready opening gate before the exact approval command.
Approval was recorded for invocation `41729e12-84b5-4ee8-9ca9-4ae836a20b76`,
boundary `bd6c5338-d629-4908-8078-b92b1119ef7b`; no later gate was approved.

Observed through the development CLI Run and Session surfaces:
- First writing Run `run_2285e52a8e11406ba08f269a0e82c59a` completed.
- Decision Run `run_8e7114adcc2e46ac990fb71b6d25580d` recorded Iterate and
  completed. Saved cursor returned to the writing step, iteration 1, carrying
  concrete direction to add Iterate/Blocked and preserve distinct Run evidence.
- Second writing Run `run_08a1e57ad88e47b9b163aa4e700f88ab` completed.
- Decision Run `run_5586692baf53486e96c6176d2f3f7d5d` opened the keyed Ask
  `ask_once_4aa034f190bc7fe22cd1764831999a5f52597de5f3c08d11c052913baa76f95b`.
- Its unblock Run is `run_75f7be6d8a8a4bebae8974a6fd4e723a`. Opened that exact
  Session in Ghostty window `tab-group-abc34e1c0`; Session list reports active.

The diagnostic credential shim had already been removed from PATH resolution.
These workers and the Ask used the rebuilt development executable and real
Codex providers. This remains the staged transport fixture described in the
contract, not whole-Task acceptance. Human answer, Ask Complete, reassessment,
and the final demo gate remain pending.

The first Ghostty open then failed with `No saved session found` for native
thread `01a0d9fb-1123-7a10-b8ec-7946a94f1e46`: Ghostty inherited the engineering
CODEX_HOME while the durable launcher used the default native Home. Retried
the same Ask with CODEX_HOME cleared in the executed command, window
`tab-group-abc1b90e0`. No record or native identity was replaced. This is an
additional handoff limit: an unpinned ambient provider Home can select the wrong
native history even though the Loopflow Session identity is correct.
